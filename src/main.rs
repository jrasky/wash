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

fn process_line(line:InputValue, term:&mut TermState, env:&mut WashEnv) -> WashArgs {
    match line {
        Function(n, a) => {
            if env.hasf(&n) {
                let func = WashEnv::getf(env, &n).unwrap();
                let mut args = vec![];
                for item in a.iter() {
                    args.push(process_line(item.clone(), term, env));
                }
                return func(&WashArgs::Long(args), env, term);
            } else {
                return WashArgs::Empty;
            }
        },
        Short(s) => return WashArgs::Flat(s),
        Literal(s) => return WashArgs::Flat(s),
        _ => return WashArgs::Empty
    }
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
                match process_line(line, &mut term, &mut env) {
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
