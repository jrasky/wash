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
use builtins::*;
use types::*;
use ast::*;
use handlers::*;

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
mod ioctl;
mod ast;
mod handlers;

// public so no warnings when we run tests
pub fn main() {
    let mut reader = LineReader::new();
    let mut ast = AST::new();
    let mut cleaned_jobs;
    match load_builtins(&mut ast.env) {
        Err(e) => ast.env.errf(format_args!("Could not load builtings: {}\n", e)),
        _ => {}
    }
    load_handlers(&mut ast);
    ast.env.update_terminal();
    loop {
        ast.env.flush();
        cleaned_jobs = ast.env.clean_jobs();
        match cleaned_jobs {
            WashArgs::Long(v) => {
                for status in v.iter() {
                    ast.env.outf(format_args!("{}\n", status.flatten()));
                }
            },
            _ => {/* nothing */}
        }
        if ast.in_block() {
            match ast.env.runf(&format!("subprompt"), &WashArgs::Empty) {
                Err(_) => reader.controls.outs("prompt failed => run("),
                Ok(v) => reader.controls.outs(v.flatten().as_slice())
            }
        } else {
            match ast.env.runf(&format!("prompt"), &WashArgs::Empty) {
                Err(_) => reader.controls.outs("prompt failed => run("),
                Ok(v) => reader.controls.outs(v.flatten().as_slice())
            }
        }
        match reader.read_line() {
            None => {
                if reader.eof {
                    break;
                } else if !reader.line.is_empty() {
                    ast.env.outc(BEL);
                    reader.restart();
                } else {
                    ast.env.outc(NL);
                    reader.clear();
                }
            },
            Some(mut line) => {
                ast.env.outc(NL);
                match ast.add_line(&mut line) {
                    Err(ref e) if *e == STOP => {
                        // the silent error
                        ast.clear();
                    },
                    Err(e) => {
                        println!("Error: {}", e);
                        ast.clear();
                    },
                    Ok(_) => {
                        if !ast.in_block() {
                            match ast.evaluate() {
                                Err(ref e) if *e == STOP => {
                                    // the silent error
                                }
                                Err(e) => {
                                    println!("Error: {}", e);
                                },
                                Ok(WashArgs::Empty) => {
                                    // print nothing
                                },
                                Ok(v) => {
                                    println!("{}", v.flatten());
                                }
                            };
                            ast.clear();
                        }
                    }
                }
                reader.clear();
            }
        }
    }
    ast.env.outs("\nExiting\n");
    ast.env.flush();
}
