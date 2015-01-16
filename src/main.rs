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
use controls::*;
use constants::*;
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

fn run_line(line:InputValue, term:&mut TermState, env:&mut WashEnv) -> Result<WashArgs, String> {
    match line {
        Long(a) => {
            // run as command
            let out = try!(process_line(Function("run".to_string(), a), term, env));
            let outv = out.flatten_vec();
            if out.is_empty() {
                return Err("Command failed".to_string());
            } else if outv.len() < 2 {
                return Err(format!("Command failed: {}", out.flatten()));
            } else if outv != vec!["status", "0"] {
                return Err(format!("Command failed with {} {}", outv[0], outv[1]));
            } else {
                return Ok(WashArgs::Empty);
            }
        },
        Short(s) | Literal(s) => {
            // run command without args
            let out = try!(process_line(Function("run".to_string(), vec![Short(s)]), term, env));
            let outv = out.flatten_vec();
            if out.is_empty() {
                return Err("Command Failed".to_string());
            } else if outv.len() < 2 {
                return Err(format!("Command failed: {}", out.flatten()));
            } else if outv != vec!["status", "0"] {
                return Err(format!("Command failed with {} {}", outv[0], outv[1]));
            } else {
                return Ok(WashArgs::Empty);
            }
        },
        v => {
            return process_line(v, term, env);
        }
    }
}

fn process_line(line:InputValue, term:&mut TermState, env:&mut WashEnv) -> Result<WashArgs, String> {
    match line {
        Function(n, a) => {
            if env.hasf(&n) {
                let func = WashEnv::getf(env, &n).unwrap();
                let mut args = vec![];
                for item in a.iter() {
                    match process_line(item.clone(), term, env) {
                        Ok(WashArgs::Empty) => {/* do nothing */},
                        Ok(v) => args.push(v),
                        Err(e) => return Err(e)
                    }
                }
                return Ok(func(&WashArgs::Long(args), env, term));
            } else {
                return Err("Function not found".to_string())
            }
        },
        Long(a) => {
            let mut args = vec![];
            for item in a.iter() {
                match process_line(item.clone(), term, env) {
                    Ok(v) => args.push(v),
                    Err(e) => return Err(e)
                }
            }
            return Ok(WashArgs::Long(args));
        },
        Short(s) => return Ok(WashArgs::Flat(s)),
        Literal(s) => return Ok(WashArgs::Flat(s)),
        Split(_) => return Ok(WashArgs::Empty)
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
                match run_line(line, &mut term, &mut env) {
                    Err(e) => controls.errf(format_args!("{}\n", e)),
                    Ok(v) => {
                        controls.outs(v.flatten().as_slice());
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
