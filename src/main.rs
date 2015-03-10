#![feature(box_syntax)]
// we have to use the old process module, because the new one doesn't support
// inheritfd, which is needed for process piping
#![allow(deprecated)]
#![feature(unsafe_destructor)]
#![feature(unboxed_closures)]
#![feature(collections)]
#![feature(core)]
#![feature(path_ext)]
#![feature(path)]
#![feature(io)]
#![feature(old_io)]
#![feature(old_path)]
#![feature(libc)]
#![feature(std_misc)]
#![feature(unicode)]
#![feature(rustc_private)]
#![feature(plugin)]
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
use env::*;
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
    let mut env = WashEnv::new();
    let mut ast = AST::new();
    let mut cleaned_jobs;
    match load_builtins(&mut env) {
        Err(e) => env.errf(format_args!("Could not load builtings: {}\n", e)),
        _ => {}
    }
    load_handlers(&mut ast);
    env.update_terminal();
    loop {
        env.flush();
        cleaned_jobs = env.clean_jobs();
        match cleaned_jobs {
            WashArgs::Long(v) => {
                for status in v.iter() {
                    env.outf(format_args!("{}\n", status.flatten()));
                }
            },
            _ => {/* nothing */}
        }
        if ast.in_block() {
            match env.runf(&format!("subprompt"), &WashArgs::Empty) {
                Err(_) => reader.controls.outs("prompt failed => run("),
                Ok(v) => reader.controls.outs(v.flatten().as_slice())
            }
        } else {
            match env.runf(&format!("prompt"), &WashArgs::Empty) {
                Err(_) => reader.controls.outs("prompt failed => run("),
                Ok(v) => reader.controls.outs(v.flatten().as_slice())
            }
        }
        match reader.read_line() {
            None => {
                if reader.eof {
                    break;
                } else if !reader.line.is_empty() {
                    env.outc(BEL);
                    reader.restart();
                } else {
                    env.outc(NL);
                    reader.clear();
                }
            },
            Some(mut line) => {
                env.outc(NL);
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
                            //println!("{:?}", ast);
                            match ast.optimize() {
                                Err(e) => {
                                    println!("Optimization error: {}", e);
                                },
                                Ok(_) => {
                                    //println!("{:?}", ast);
                                }
                            }
                            match ast.into_runner().evaluate(&WashArgs::Empty, &mut env) {
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
                        }
                    }
                }
                reader.clear();
            }
        }
    }
    env.outs("\nExiting\n");
    env.flush();
}
