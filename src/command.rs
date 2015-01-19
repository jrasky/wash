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
    
    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdout(StdioContainer::InheritFd(STDOUT));
        process.stdin(StdioContainer::InheritFd(STDIN));
        process.stderr(StdioContainer::InheritFd(STDERR));
        // set terminal settings for process
        self.restore_terminal();
        let mut child = match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        let out = match child.wait() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't wait for child to exit: {}", e));
            },
            Ok(v) => v
        };
        // restore settings for Wash
        self.update_terminal();
        return Ok(out);
    }

    pub fn run_command_directed(&mut self, name:&String,
                                args:&Vec<String>) -> Result<ProcessOutput, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        match process.output() {
            Err(e) => {
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => Ok(v)
        }
    }
}

