use std::io::process::{Command, StdioContainer,
                       ProcessOutput, ProcessExit};

use controls::*;
use constants::*;
use termios::*;

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios
}

impl TermState {
    pub fn new() -> TermState {
        let mut controls = Controls::new();
        let mut tios = match Termios::get() {
            Some(t) => t,
            None => {
                controls.err("Warning: Could not get terminal mode\n");
                Termios::new()
            }
        };
        let old_tios = tios.clone();
        tios.fdisable(0, 0, ICANON|ECHO, 0);
        
        return TermState {
            controls: controls,
            tios: tios,
            old_tios: old_tios
        };
    }

    pub fn update_terminal(&mut self) {
        if !Termios::set(&self.tios) {
            self.controls.err("Warning: Could not set terminal mode\n");
        }
    }

    pub fn restore_terminal(&mut self) {
        if !Termios::set(&self.old_tios) {
            self.controls.err("Warning: Could not set terminal mode\n");
        }
    }
    
    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Option<ProcessExit> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdout(StdioContainer::InheritFd(STDOUT));
        process.stdin(StdioContainer::InheritFd(STDIN));
        process.stderr(StdioContainer::InheritFd(STDERR));
        // set terminal settings for process
        self.restore_terminal();
        let mut child = match process.spawn() {
            Err(e) => {
                self.controls.errf(format_args!("Couldn't spawn {}: {}\n", name, e));
                self.update_terminal();
                return None;
            },
            Ok(child) => child
        };
        let out = match child.wait() {
            Err(e) => {
                self.controls.errf(format_args!("Couldn't wait for child to exit: {}\n", e.desc));
                self.update_terminal();
                return None;
            },
            Ok(status) => status
        };
        // restore settings for Wash
        self.update_terminal();
        return Some(out);
    }

    pub fn run_command_directed(&mut self, name:&String,
                                args:&Vec<String>) -> Option<ProcessOutput> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        match process.output() {
            Err(e) => {
                self.controls.errf(format_args!("Couldn't spawn {}: {}\n", name, e));
                return None;
            },
            Ok(out) => Some(out)
        }
    }
}

