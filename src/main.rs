#![allow(unstable)]
#![feature(plugin)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;
extern crate unicode;
extern crate regex;
#[plugin] #[no_link]
extern crate regex_macros;

use regex::NoExpand;

use reader::*;
use controls::*;
use constants::*;
use util::*;
use script::*;
use builtins::*;
use command::*;
use input::*;

use input::InputValue::*;

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

fn run_line(line:InputValue, term:&mut TermState, env:&mut WashEnv) -> WashArgs {
    // this needs to be re-written, it will be soon
    match line {
        Short(v) | Literal(v) => {
            print!("{}\n", v);
        },
        Long(v) => {
            print!("(");
            for item in v.iter() {
                run_line(item.clone(), term, env);
            }
            print!(")\n");
        },
        Function(n, v) => {
            print!("{}(", n);
            for item in v.iter() {
                run_line(item.clone(), term, env);
            }
            print!(")\n");
        },
        _ => {}
    }
    return WashArgs::Long(vec![]);
}

fn process_line(line:InputValue, term:&mut TermState, env:&mut WashEnv) -> WashArgs {
    // this needs to be re-written, it will be soon
    return WashArgs::Empty;
}

// public so no warnings when we run tests
pub fn main() {
    let mut controls = &mut Controls::new();
    let mut reader = LineReader::new();
    let mut env = WashEnv::new();
    let mut term = TermState::new();
    load_builtins(&mut env);
    term.update_terminal();
    loop {
        controls.flush();
        match reader.read_line() {
            None => {
                if reader.eof {
                    break;
                } else if !reader.line.is_empty() {
                    controls.outc(BEL);
                    reader.restart();
                } else {
                    controls.outc(NL);
                    reader.clear();
                }
            },
            Some(line) => {
                controls.outc(NL);
                match run_line(line, &mut term, &mut env) {
                    WashArgs::Empty => controls.err("Command failed\n"),
                    v => {
                        controls.outs(v.flatten().as_slice());
                        controls.outc(NL);
                    }
                }
                reader.clear();
            }
        }
    }
    controls.outs("\nExiting\n");
    controls.flush();
    term.restore_terminal();
}
