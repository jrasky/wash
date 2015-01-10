#![allow(unstable)]
extern crate sodiumoxide;
extern crate libc;
extern crate serialize;

use libc::*;
use sodiumoxide::crypto::hash::sha256;
use serialize::hex::ToHex;

use std::io::process::{Command, StdioContainer, ProcessOutput};
use std::io::fs::PathExtensions;
use std::collections::HashMap;

use std::ffi;
use std::str;
use std::mem;
use std::io;

use reader::*;
use controls::*;
use termios::*;
use signal::*;
use constants::*;
use util::*;
use input::*;

mod constants;
mod util;
mod termios;
mod signal;
mod controls;
mod input;
mod reader;

const RTLD_LOCAL:c_int = 0;
const RTLD_LAZY:c_int = 1;

const ENTRY_SYMBOL:&'static str = "wash_run";
const WO_PATH:&'static str = "/tmp/wash/";

// start off as null pointer
static mut uglobal_reader:*mut LineReader = 0 as *mut LineReader;


#[link(name = "dl")]
extern {
    fn dlopen(filename:*const c_char, flag:c_int) -> *const c_void;
    fn dlsym(handle:*const c_void, symbol:*const c_char) -> *const c_void;
    fn dlclose(handle:*const c_void) -> c_int;
    fn dlerror() -> *const c_char;
}

#[allow(unused_variables)]
unsafe extern fn reader_sigint(signum:c_int, siginfo:*const SigInfo, context:size_t) {
    // This function should only be called when the input line is actually active
    if uglobal_reader.is_null() {
        // More informative than a segfault
        panic!("Line reader location uninitialized");
    }
    // Hopefully no segfault, this *should* be safe code
    let ref mut reader:LineReader = *uglobal_reader;
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

fn run_wash_script(line:&Vec<String>, controls:&mut Controls) {
    let inp = Path::new(&line[0]);
    let inf = match io::File::open(&inp) {
        Ok(f) => f,
        Err(e) => {
            controls.errf(format_args!("File error: {}\n", e));
            return;
        }
    };
    let mut reader = io::BufferedReader::new(inf);
    let contents = reader.read_to_end().unwrap();
    let bytes = contents.as_slice();
    let mut outname = sha256::hash(bytes).0.to_hex();
    outname.push_str(".wo");
    let outp = Path::new(WO_PATH).join(outname);
    
    if !outp.exists() {
        io::fs::mkdir_recursive(&outp.dir_path(), io::USER_RWX).unwrap();
        let mut command = Command::new("rustc");
        command.args(&["-o", outp.as_str().unwrap(), "-"]);
        let mut child = match command.spawn() {
            Err(e) => {
                panic!("Error: {}\n", e);
            },
            Ok(c) => c
        };

        {
            let mut input = child.stdin.as_mut().unwrap();
            input.write(bytes).unwrap();
            input.flush().unwrap();
        }

        match child.wait_with_output() {
            Err(e) => {
                controls.errf(format_args!("Could not compile script: {}\n", e));
            },
            Ok(o) => {
                if !o.status.success() {
                    controls.errf(format_args!("Could not compile script: {}\n",
                                               String::from_utf8(o.error).unwrap()));
                }
            }
        }
    }

    unsafe {
        let handle = dlopen(ffi::CString::from_slice(outp.as_str().unwrap().as_bytes()).as_ptr(),
                            RTLD_LAZY|RTLD_LOCAL);
        if handle.is_null() {
            controls.errf(format_args!("Could not load script object: {}\n",
                                       str::from_utf8(ffi::c_str_to_bytes(&dlerror())).unwrap()));
            return;
        }
        let ptr = dlsym(handle, ffi::CString::from_slice(ENTRY_SYMBOL.as_bytes()).as_ptr());
        if ptr.is_null() {
            controls.errf(format_args!("Could not find entry symbol: {}\n",
                                       str::from_utf8(ffi::c_str_to_bytes(&dlerror())).unwrap()));
            dlclose(handle);
            return;
        }
        let func:extern fn() = mem::transmute(ptr);
        func();
        dlclose(handle);
    }
}


fn process_job(line:&Vec<String>, tios:&Termios, old_tios:&Termios,
               reader:&mut LineReader, controls:&mut Controls) {
    if line.is_empty() {
        return;
    }
    if line[0].as_slice().ends_with(".ws") {
        // run as wash shell script
        update_terminal(old_tios, controls);
        signal_ignore(SIGINT);
        controls.outc(NL);
        controls.flush();
        run_wash_script(line, controls);
        set_reader_sigint(controls);
        update_terminal(tios, controls);
        reader.clear();
    } else {
        update_terminal(old_tios, controls);
        signal_ignore(SIGINT);
        controls.outc(NL);
        controls.flush();
        run_command(line, controls);
        set_reader_sigint(controls);
        update_terminal(tios, controls);
        reader.clear();
    }
}

fn main() {
    let mut controls = &mut Controls::new();
    let mut reader = LineReader::new();
    let mut map = HashMap::<&str, &fn(Vec<String>) -> Vec<String>>::new();
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
                    &mut reader, controls);
        controls.flush();
    }
    controls.outs("Exiting\n");
    update_terminal(&old_tios, controls);
}
