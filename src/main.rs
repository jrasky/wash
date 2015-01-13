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

use script::WashArgs::*;

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

fn run_line(line:Vec<String>, term:&mut TermState, env:&mut WashEnv) -> WashArgs {
    match line.len() {
        0 => Empty,
        1 => process_line(line, term, env),
        _ => match run_func(&WashArgs::from_vec(line), env, term) {
            Empty | Flat(_) => Empty, // run_func doesn't return Flat
            Long(v) =>
                if v[0].flatten() == "status".to_string() &&
                v[1].flatten() == "0".to_string() {
                    return Long(vec![]);
                } else {
                    return Flat(format!("Command failed: {} {}", v[0].flatten(), v[1].flatten()));
                }
        }
    }
}

fn process_line(line:Vec<String>, term:&mut TermState, env:&mut WashEnv) -> WashArgs {
    // oh boy! (Assume that the line is correctly formed)
    let sliteral_re = regex!("^([^ \t\r\n\"()]*)(\"([^\"]*)\")*$");
    //let string_re = regex!("([^ \t\r\n\"()]*)\"([^\"]*)\"");
    //let string_re_check = regex!("^[^\"]*$");
    let func_re = regex!("([^ \t\r\n\"()]+)\\(([^()]*)\\)");
    //let func_re_check = regex!("^[^()]*$");
    let args_word_re = regex!("\".*\"|[^ \t\r\n\"()]+\\(.*\\)");
    let c_re = regex!(","); let spc_regex = regex!("(\\S.*\\S)");
    let mut out = Vec::<WashArgs>::new();
    let mut caps; let mut argsl; let mut name;
    let mut fargs; let mut func;
    for arg in line.iter() {
        argsl = arg.as_slice();
        // if argument is a simple string literal, no more processing is needed
        if sliteral_re.is_match(argsl) {
            out.push(Flat(sliteral_re.replace_all(argsl, "$1$3")));
            continue;
        } else if func_re.is_match(argsl) {
            // *cries* ok, work harder
            // this shouldn't panic because we checked that it matched
            caps = func_re.captures(argsl).unwrap();
            name = caps.at(1).unwrap().to_string();
            // check to make sure function exists before we do any processing
            if !env.hasf(&name) {
                env.controls.errf(format_args!("{}: function not found", name));
                return Empty;
            }
            func = WashEnv::getf(env, &name).unwrap();
            // free memory once we're done with this operation
            fargs = {
                let fargs = {
                    let fargs = caps.at(2).unwrap();
                    let awmatches = args_word_re.find_iter(fargs).collect::<Vec<(usize, usize)>>();
                    let cmatches = c_re.find_iter(fargs).collect::<Vec<(usize, usize)>>();
                    let asplits = comma_intersect(cmatches, awmatches);
                    let ifargs = split_at(fargs.to_string(), asplits);
                    ifargs.iter().map(|s| {spc_regex.replace_all(s.as_slice(), NoExpand(""))}).collect::<Vec<String>>()
                };
                // at this point sifargs should contain a list of arguments, with the whitespace
                // around them removed
                // guess what?
                match process_line(fargs, term, env) {
                    Empty => return Empty, // that call already provided feedback
                    v => v
                }
            };
            // run the function!
            match func(&fargs, env, term) {
                Empty => {
                    env.controls.errf(format_args!("Command failed: {}", arg));
                    return Empty;
                },
                v => out.push(v.clone())
            }
        } else {
            // this case shouldn't happen
            panic!("process_line given invalid line");
        }
    }
    // return everything
    match out.len() {
        0 => Empty,
        1 => out[0].clone(),
        _ => Long(out)
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
        match reader.read_line() {
            None => {
                if reader.line.is_empty() {
                    break;
                } else {
                    controls.outc(BEL);
                }
            },
            Some(line) => {
                controls.outc(NL);
                match run_line(line, &mut term, &mut env) {
                    Empty => controls.err("Command failed\n"),
                    v => {
                        controls.outs(v.flatten().as_slice());
                        controls.outc(NL);
                    }
                }
                reader.clear();
                controls.flush();
            }
        }
    }
    controls.outs("\nExiting\n");
    controls.flush();
    term.restore_terminal();
}
