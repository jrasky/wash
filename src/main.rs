extern crate libc;

use libc::{c_int, size_t};
use std::io::process::{Command, StdioContainer};

use reader::*;
use controls::*;
use termios::*;
use signal::*;
use constants::*;
use util::*;

mod constants;
mod util;
mod termios;
mod signal;
mod controls;
mod input;
mod reader;

// start off as null pointer
static mut uglobal_reader:*mut LineReader = 0 as *mut LineReader;

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

fn run_command(line:Vec<String>, controls:&mut Controls) {
    let mut process = Command::new(&line[0]);
    process.args(line.slice_from(1));
    process.stdout(StdioContainer::InheritFd(STDOUT));
    process.stdin(StdioContainer::InheritFd(STDIN));
    process.stderr(StdioContainer::InheritFd(STDERR));
    let mut child = match process.spawn() {
        Err(e) => {
            controls.err(format!("Couldn't spawn {}: {}\n", &line[0], e).as_slice());
            return;
        },
        Ok(child) => child
    };
    match child.wait() {
        Err(e) => {
            controls.err(format!("Couldn't wait for child to exit: {}\n", e.desc).as_slice());
        },
        Ok(_) => {
            // nothing
        }
    };
}

fn main() {
    let mut controls = &mut Controls::new();
    let mut reader = LineReader::new();
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
        if !line.is_empty() {
            update_terminal(&old_tios, controls);
            signal_ignore(SIGINT);
            controls.outc(NL);
            controls.flush();
            run_command(strip_words(line), controls);
            set_reader_sigint(controls);
            update_terminal(&tios, controls);
            reader.clear();
        }
    }
    controls.outs("Exiting\n");
    update_terminal(&old_tios, controls);
}
