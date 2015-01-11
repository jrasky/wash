#![allow(unstable)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;
extern crate unicode;

use libc::*;

use std::io::process::{Command, StdioContainer, ProcessOutput};
use std::collections::HashMap;

use reader::*;
use controls::*;
use termios::*;
use signal::*;
use constants::*;
use util::*;
use input::*;
use script::*;

mod constants;
mod util;
mod termios;
mod signal;
mod controls;
mod input;
mod reader;
mod script;

type ScriptTable<'a> = HashMap<String, WashScript>;

// start off as null pointer
static mut uglobal_reader:*mut LineReader = 0 as *mut LineReader;

#[allow(unused_variables)]
unsafe extern fn reader_sigint(signum:c_int, siginfo:*const SigInfo, context:size_t) {
    // Hopefully no segfault, this *should* be safe code
    let reader:&mut LineReader = match uglobal_reader.as_mut() {
        Some(v) => v,
        None => {
            panic!("Line reader location uninitalized");
        }
    };
    if reader.line.is_empty() {
        reader.controls.outs("Interrupt\n");
    } else {
        reader.controls.outs("\nInterrupt\n");
    }
    // reset line
    reader.clear();
}

fn set_reader_location(reader:&mut LineReader) {
    unsafe {
        if !uglobal_reader.is_null() {
            panic!("Tried to set reader location twice");
        }
        uglobal_reader = reader as *mut LineReader;
    }
}

fn terminal_settings(controls:&mut Controls) -> (Termios, Termios) {
    let mut tios = match Termios::get() {
        Some(t) => t,
        None => {
            controls.err("Warning: Could not get terminal mode\n");
            Termios::new()
        }
    };
    let ctios = tios.clone();
    tios.fdisable(0, 0, ICANON|ECHO, 0);
    return (tios, ctios);
}

fn update_terminal(tios:&Termios, controls:&mut Controls) {
    if !Termios::set(tios) {
        controls.err("Warning: Could not set terminal mode\n");
    }
}

fn set_reader_sigint(controls:&mut Controls) {
    let mut sa = SigAction {
        handler: reader_sigint,
        mask: [0; SIGSET_NWORDS],
        flags: SA_RESTART | SA_SIGINFO,
        restorer: 0 // null pointer
    };
    let mask = full_sigset().expect("Could not get a full sigset");
    sa.mask = mask;
    if !signal_handle(SIGINT, &sa) {
        controls.err("Warning: could not set handler for SIGINT\n");
    }
}

fn run_command(line:&Vec<String>, controls:&mut Controls) {
    let mut process = Command::new(&line[0]);
    process.args(line.slice_from(1));
    process.stdout(StdioContainer::InheritFd(STDOUT));
    process.stdin(StdioContainer::InheritFd(STDIN));
    process.stderr(StdioContainer::InheritFd(STDERR));
    let mut child = match process.spawn() {
        Err(e) => {
            controls.errf(format_args!("Couldn't spawn {}: {}\n", &line[0], e));
            return;
        },
        Ok(child) => child
    };
    match child.wait() {
        Err(e) => {
            controls.errf(format_args!("Couldn't wait for child to exit: {}\n", e.desc));
        },
        Ok(_) => {
            // nothing
        }
    };
}

fn run_command_directed(line:&Vec<String>, controls:&mut Controls) -> Option<ProcessOutput> {
    let mut process = Command::new(&line[0]);
    process.args(line.slice_from(1));
    match process.output() {
        Err(e) => {
            controls.errf(format_args!("Couldn't spawn {}: {}\n", &line[0], e));
            return None;
        },
        Ok(out) => Some(out)
    }
}

fn process_line(line:Vec<String>, controls:&mut Controls) -> Option<Vec<String>> {
    let mut out = strip_words(line);
    out = match process_sublines(out, controls) {
        None => return None,
        Some(l) => l
    };
    return Some(out);
}

fn process_sublines(line:Vec<String>, controls:&mut Controls) -> Option<Vec<String>> {
    let mut out = Vec::<String>::new();
    for word in line.iter() {
        if word.as_slice().starts_with("$(") &&
            word.as_slice().ends_with(")") {
                let mut subline = InputLine::process_line(String::from_str(word.slice_chars(2, word.len() - 1)));
                subline = match process_line(subline, controls) {
                    None => return None,
                    Some(l) => l
                };
                match run_command_directed(&subline, controls) {
                    None => return None,
                    Some(ProcessOutput {status, error, output}) => {
                        if status.success() {
                            let mut cout = String::from_utf8_lossy(output.as_slice()).into_owned();
                            if cout.as_slice().ends_with("\n") {
                                // remove traling newlines, they aren't useful
                                cout.pop();
                            }
                            out.push(cout);
                        } else {
                            controls.errf(format_args!("{} failed: {}\n", &subline[0],
                                                       String::from_utf8_lossy(error.as_slice())));
                            return None;
                        }
                    }
                }
            } else {
                out.push(word.clone());
            }
    }
    return Some(out);
}

#[allow(unused_variables)]
fn update(line:&Vec<String>) {
    // nothing yet
}

fn process_job<'a>(line:&'a Vec<String>, tios:&Termios, old_tios:&Termios,
                   reader:&mut LineReader, functions:&mut FuncTable,
                   scripts:&mut ScriptTable<'a>, controls:&mut Controls) {
    if line.is_empty() {
        return;
    }
    if functions.contains_key(line[0].as_slice()) {
        let func = functions.get(line[0].as_slice()).unwrap();
        let mut args = line.clone();
        args.pop();
        controls.flush();
        func(&args, controls);
    } else if line[0].as_slice().ends_with(".ws") {
        // run as wash shell script
        if !scripts.contains_key(&line.clone()[0]) {
            scripts.insert::<'a>(line[0].clone(),
                                 WashScript::new(Path::new(&line[0])));
        }
        let script = scripts.get_mut(line[0].as_slice()).unwrap();
        if !script.is_compiled() && !script.compile() {
            controls.err("Failed to compile script\n");
            return;
        }
        let mut args = line.clone();
        args.pop();
        controls.flush();
        if script.is_runnable() {
            script.run(&args, functions);
        } else if script.is_loadable() {
            script.load(&args, functions);
        } else {
            controls.err("Cannot load or run script\n");
        }
    } else {
        update_terminal(old_tios, controls);
        signal_ignore(SIGINT);
        controls.flush();
        run_command(line, controls);
        set_reader_sigint(controls);
        update_terminal(tios, controls);
    }
}

fn main() {
    let mut controls = &mut Controls::new();
    let mut reader = LineReader::new();
    let mut functions:FuncTable = HashMap::new();
    let mut scripts:ScriptTable = HashMap::new();
    let (tios, old_tios) = terminal_settings(controls);
    update_terminal(&tios, controls);
    set_reader_location(&mut reader);
    set_reader_sigint(controls);
    let mut line:Vec<String>;
    loop {
        line = match reader.read_line() {
            None => break,
            Some(l) => l
        };
        line = match process_line(line, controls) {
            None => {
                controls.err("Command failed\n");
                continue;
            },
            Some(l) => l
        };
        update(&line);
        process_job(&line, &tios, &old_tios,
                    &mut reader, &mut functions,
                    &mut scripts, controls);
        reader.clear();
        controls.flush();
    }
    controls.outs("Exiting\n");
    update_terminal(&old_tios, controls);
}
