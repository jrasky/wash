#![allow(unstable)]
#![feature(plugin)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;
extern crate unicode;
extern crate regex;
extern crate core;
#[plugin] #[no_link]
extern crate regex_macros;

use reader::*;
use constants::*;
use env::*;
use builtins::*;
use types::*;

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
    let mut cleaned_jobs;
    match load_builtins(&mut env) {
        Err(e) => {
            env.errf(format_args!("Could not load builtings: {}\n", e));
        }
        _ => {}
    }
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
                        if v.is_flat() {
                            // extra newline
                            env.outc(NL);
                        }
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
