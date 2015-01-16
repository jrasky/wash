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

fn run_command(args:Vec<WashArgs>, term:&mut TermState, env:&mut WashEnv) -> Result<WashArgs, String> {
    let out = try!(run_function("run".to_string(), args, term, env));
    let outv = out.flatten_vec();
    if out.is_empty() {
        return Err("Command failed".to_string());
    } else if out.len() < 2 {
        return Err(format!("Command failed: {}", out.flatten()));
    } else if outv != vec!["status", "0"] {
        return Err(format!("Command failed with {} {}", outv[0], outv[1]));
    } else {
        return Ok(WashArgs::Empty);
    }
}

fn run_function(name:String, args:Vec<WashArgs>, term:&mut TermState, env:&mut WashEnv) -> Result<WashArgs, String> {
    if env.hasf(&name) {
        let func = WashEnv::getf(env, &name).unwrap();
        return Ok(func(&WashArgs::Long(args), env, term));
    } else {
        return Err("Function not found".to_string());
    }
}

fn run_line(line:InputValue, term:&mut TermState, env:&mut WashEnv) -> Result<WashArgs, String> {
    match line {
        Function(n, a) => {
            return run_function(n, try!(input_to_vec(a, term, env)), term, env);
        },
        Long(a) => {
            // run as command
            return run_command(try!(input_to_vec(a, term, env)), term, env);
        },
        Short(s) | Literal(s) => {
            // run command without args
            return run_command(vec![WashArgs::Flat(s)], term, env);
        },
        _ => {
            // do nothing
            return Ok(WashArgs::Empty);
        }
    }
}

fn input_to_vec(input:Vec<InputValue>, term:&mut TermState, env:&mut WashEnv) -> Result<Vec<WashArgs>, String> {
    let mut args = vec![];
    for item in input.iter() {
        match try!(input_to_args(item.clone(), term, env)) {
            WashArgs::Empty => {/* do nothing */},
            v => args.push(v)
        }
    }
    return Ok(args);
}

fn input_to_args(input:InputValue, term:&mut TermState, env:&mut WashEnv) -> Result<WashArgs, String> {
    match input {
        Function(n, a) => {
            return run_function(n, try!(input_to_vec(a, term, env)),
                                term, env);
        },
        Long(a) => {
            let mut args = vec![];
            for item in a.iter() {
                match try!(input_to_args(item.clone(), term, env)) {
                    WashArgs::Empty => {/* do nothing */},
                    v => args.push(v)
                }
            }
            return Ok(WashArgs::Long(args));
        },
        Short(s) | Literal(s) => return Ok(WashArgs::Flat(s)),
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
