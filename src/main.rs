#![allow(unstable)]
#![feature(plugin)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;
extern crate unicode;
extern crate regex;
#[plugin] #[no_link]
extern crate regex_macros;

use std::os;

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
        Short(ref s) if VAR_PATH_REGEX.is_match(s.as_slice()) => {
            let caps = VAR_PATH_REGEX.captures(s.as_slice()).unwrap();
            let path = caps.at(1).unwrap().to_string();
            if path == "env".to_string() {
                match caps.at(2).unwrap() {
                    name if name == "".to_string() => {
                        let envs = os::env();
                        let mut out = vec![]; let mut cenv;
                        for env in envs.iter() {
                            cenv = env.clone();
                            out.push(WashArgs::Long(vec![WashArgs::Flat(cenv.0),
                                                         WashArgs::Flat(cenv.1)]));
                        }
                        return Ok(WashArgs::Long(out));
                    },
                    name => match os::getenv(name.as_slice()) {
                        None => return Err(format!("Environment variable not found: {}", name)),
                        Some(v) => return Ok(WashArgs::Flat(v))
                    }
                }
            } else if path == "".to_string() {
                match caps.at(2).unwrap().to_string() {
                    ref name if *name == "".to_string() => {
                        return Ok(WashEnv::getall(env));
                    },
                    name => {
                        if !env.hasv(&name) {
                            return Err(format!("Variable not found: {}", name));
                        } else {
                            return Ok(WashEnv::getv(env, &name));
                        }
                    }
                }
            } else {
                if !env.hasp(&path) {
                    return Err(format!("Path not found: {}", path));
                }
                match caps.at(2).unwrap().to_string() {
                    ref name if *name == "".to_string() => {
                        return Ok(WashEnv::getallp(env, &path));
                    },
                    name => {
                        if !env.hasvp(&name, &path) {
                            return Err(format!("Variable not found: {}:{}", path, name));
                        } else {
                            return Ok(env.getvp(&name, &path));
                        }
                    }
                }
            }
        },
        Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
            let caps = VAR_REGEX.captures(s.as_slice()).unwrap();
            let name = caps.at(1).unwrap().to_string();
            if env.hasv(&name) {
                return Ok(env.getv(&name));
            } else {
                return Err(format!("Variable not found: {}", name));
            }
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
    load_builtins(&mut env);
    env.term.update_terminal();
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
                match run_line(line, &mut env) {
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
    env.term.restore_terminal();
}
