#![allow(unstable)]
#![feature(plugin)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;
extern crate unicode;
extern crate regex;
#[plugin] #[no_link]
extern crate regex_macros;

use reader::*;
use constants::*;
use env::*;
use builtins::*;

mod constants;
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

// public so no warnings when we run tests
pub fn main() {
    let mut reader = LineReader::new();
    let mut env = WashEnv::new();
    match load_builtins(&mut env) {
        Err(e) => {
            env.errf(format_args!("Could not load builtings: {}\n", e));
        }
        _ => {}
    }
    env.update_terminal();
    loop {
        env.flush();
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
            Some(line) => {
                env.outc(NL);
                match env.process_line(line) {
                    Err(e) => env.errf(format_args!("{}\n", e)),
                    Ok(v) => {
                        env.outs(v.flatten().as_slice());
                    }
                }
                reader.clear();
            }
        }
    }
    env.outs("\nExiting\n");
    env.flush();
    env.restore_terminal();
}
