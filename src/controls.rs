use std::io;
use std::fmt;

use constants::*;
use util::*;

type Stdr = io::stdio::StdinReader;
type Stdw = io::stdio::StdWriter;

pub struct Controls {
    stdin: Stdr,
    stdout: Stdw,
    stderr: Stdw,
}

impl Controls {
    pub fn new() -> Controls {
        Controls {
            stdin: io::stdin(),
            stdout: io::stdio::stdout_raw(),
            stderr: io::stdio::stderr_raw()
        }
    }

    pub fn outc(&mut self, ch:char) {
        self.stdout.write_char(ch).unwrap()
    }

    pub fn outs(&mut self, s:&str) {
        self.stdout.write_str(s).unwrap()
    }

    pub fn err(&mut self, s:&str) {
        self.stderr.write_str(s).unwrap();
    }

    pub fn errf(&mut self, args:fmt::Arguments) {
        self.stderr.write_fmt(args).unwrap();
    }

    pub fn read(&mut self) -> io::IoResult<char> {
        self.stdin.read_char()
    }
    
    pub fn cursor_left(&mut self) {
        self.outc(DEL);
    }

    pub fn cursor_right(&mut self) {
        self.outs(CRSR_RIGHT);
    }
    
    pub fn cursors_left(&mut self, by:usize) {
        // move back by a given number of characters
        self.outs(build_string(DEL, by).as_slice());
    }

    pub fn flush(&mut self) {
        self.stdout.flush().unwrap();
        self.stderr.flush().unwrap();
    }
}
