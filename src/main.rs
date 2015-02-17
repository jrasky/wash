#![feature(box_syntax)]
#![feature(unsafe_destructor)]
#![feature(collections)]
#![feature(core)]
#![feature(path)]
#![feature(env)]
#![feature(io)]
#![feature(libc)]
#![feature(std_misc)]
#![feature(unicode)]
#![feature(rustc_private)]
#![feature(plugin)]
#![feature(hash)]
#![plugin(regex_macros)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;
extern crate unicode;
extern crate regex;
extern crate core;
#[no_link]
extern crate regex_macros;

use reader::*;
use constants::*;
use state::*;
use builtins::*;
use handlers::*;
use types::*;

use ast::AST;
use types::InputValue::*;

mod constants;
#[macro_use]
mod util;
mod termios;
mod signal;
mod controls;
mod input;
mod reader;
mod script;
mod builtins;
mod command;
mod types;
mod env;
mod state;
mod handlers;
mod ioctl;
mod ast;

// public so no warnings when we run tests
pub fn main() {
    let mut reader = LineReader::new();
    let mut state = ShellState::new();
    let mut ast = AST::new();
    let mut cleaned_jobs;
    match load_builtins(&mut state.env) {
        Err(e) => state.env.errf(format_args!("Could not load builtings: {}\n", e)),
        _ => {}
    }
    match load_handlers(&mut state) {
        Err(e) => state.env.errf(format_args!("Could not load handlers: {}\n", e)),
        _ => {}
    }
    ast::load_handlers(&mut ast);
    state.env.update_terminal();
    loop {
        state.env.flush();
        cleaned_jobs = state.env.clean_jobs();
        match cleaned_jobs {
            WashArgs::Long(v) => {
                for status in v.iter() {
                    state.env.outf(format_args!("{}\n", status.flatten()));
                }
            },
            _ => {/* nothing */}
        }
        if state.in_block() {
            match state.env.runf(&format!("subprompt"), &WashArgs::Empty) {
                Err(_) => reader.controls.outs("prompt failed => run("),
                Ok(v) => reader.controls.outs(v.flatten().as_slice())
            }
        } else {
            match state.env.runf(&format!("prompt"), &WashArgs::Empty) {
                Err(_) => reader.controls.outs("prompt failed => run("),
                Ok(v) => reader.controls.outs(v.flatten().as_slice())
            }
        }
        match reader.read_line() {
            None => {
                if reader.eof {
                    break;
                } else if !reader.line.is_empty() {
                    state.env.outc(BEL);
                    reader.restart();
                } else {
                    state.env.outc(NL);
                    reader.clear();
                }
            },
            Some(line) => {
                state.env.outc(NL);
                /*
                match state.process_line(line) {
                    Err(e) => {
                        if e == STOP.to_string() {
                            // Stop, not Fail
                        } else {
                            state.env.errf(format_args!("{}\n", e));
                        }
                    },
                    Ok(v) => {
                        if !v.is_empty() {
                            state.env.outs(v.flatten().as_slice());
                            // add extra newline
                            state.env.outc(NL);
                        }
                    }
                }*/
                let args = match line {
                    InputValue::Long(v) => v,
                    v => vec![v]
                };
                let mut exec_func = Function(format!("describe_function_output"),
                                             vec![Function(format!("run"), args)]);
                match ast.add_line(&mut exec_func) {
                    Err(e) => println!("Error: {}", e),
                    Ok(_) => {
                        println!("AST:\n{:?}", ast);
                    }
                }
                ast.clear();
                reader.clear();
            }
        }
    }
    state.env.outs("\nExiting\n");
    state.env.flush();
}
