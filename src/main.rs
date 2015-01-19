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
use script::*;
use builtins::*;
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

fn run_command(args:Vec<WashArgs>, env:&mut WashEnv) -> Result<WashArgs, String> {
    let out = try!(run_function("run".to_string(), args, env));
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
}

fn run_function(name:String, args:Vec<WashArgs>, env:&mut WashEnv) -> Result<WashArgs, String> {
    if env.hasf(&name) {
        return Ok(env.runf(&name, &WashArgs::Long(args)));
    } else {
        return Err("Function not found".to_string());
    }
}

fn run_line(line:InputValue, env:&mut WashEnv) -> Result<WashArgs, String> {
    match line {
        Function(n, a) => {
            return run_function(n, try!(input_to_vec(a, env)), env);
        },
        Long(a) => {
            // run as command
            return run_command(try!(input_to_vec(a, env)), env);
        },
        Short(ref s) if VAR_PATH_REGEX.is_match(s.as_slice()) => {
            let out = try!(input_to_args(Short(s.clone()), env));
            return Ok(WashArgs::Flat(format!("{}\n", out.flatten_with_inner("\n", "="))));
        },
        Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
            let out = try!(input_to_args(Short(s.clone()), env));
            return Ok(WashArgs::Flat(format!("{}\n", out.flatten())));
        },
        Short(s) | Literal(s) => {
            // run command without args
            return run_command(vec![WashArgs::Flat(s)], env);
        },
        _ => {
            // do nothing
            return Ok(WashArgs::Empty);
        }
    }
}

fn input_to_vec(input:Vec<InputValue>, env:&mut WashEnv) -> Result<Vec<WashArgs>, String> {
    let mut args = vec![];
    for item in input.iter() {
        match try!(input_to_args(item.clone(), env)) {
            WashArgs::Empty => {/* do nothing */},
            v => args.push(v)
        }
    }
    return Ok(args);
}

fn input_to_args(input:InputValue, env:&mut WashEnv) -> Result<WashArgs, String> {
    match input {
        Function(n, a) => {
            return run_function(n, try!(input_to_vec(a, env)),
                                env);
        },
        Long(a) => {
            let mut args = vec![];
            for item in a.iter() {
                match try!(input_to_args(item.clone(), env)) {
                    WashArgs::Empty => {/* do nothing */},
                    v => args.push(v)
                }
            }
            return Ok(WashArgs::Long(args));
        },
        // the special cases with regex make for more informative errors
        Short(ref s) if VAR_PATH_REGEX.is_match(s.as_slice()) => {
            let caps = VAR_PATH_REGEX.captures(s.as_slice()).unwrap();
            let path = caps.at(1).unwrap().to_string();
            let name = caps.at(2).unwrap().to_string();
            if name.is_empty() {
                if path.is_empty() {
                    return match env.getall() {
                        WashArgs::Empty => Err(format!("Path not found: {}", path)),
                        v => Ok(v)
                    }
                } else {
                    return match env.getallp(&path) {
                        WashArgs::Empty => Err(format!("Path not found: {}", path)),
                        v => Ok(v)
                    }
                }
            } else {
                return match env.getvp(&name, &path) {
                    WashArgs::Empty => Err(format!("Variable not found: {}:{}", path, name)),
                    v => Ok(v)
                }
            }
        },
        Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
            let caps = VAR_REGEX.captures(s.as_slice()).unwrap();
            let name = caps.at(1).unwrap().to_string();
            return match env.getv(&name) {
                WashArgs::Empty => Err(format!("Variable not found: {}", name)),
                v => Ok(v)
            }
        },
        Short(s) | Literal(s) => return Ok(WashArgs::Flat(s)),
        Split(_) => return Ok(WashArgs::Empty)
    }
}

// public so no warnings when we run tests
pub fn main() {
    let mut reader = LineReader::new();
    let mut env = WashEnv::new();
    load_builtins(&mut env);
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
                match run_line(line, &mut env) {
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
