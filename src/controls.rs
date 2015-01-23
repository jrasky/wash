use std::io::IoErrorKind::*;
use unicode::str::*;

use std::str;
use std::io;
use std::fmt;

use constants::*;
use util::*;

type Stdr = io::stdio::StdReader;
type Stdw = io::stdio::StdWriter;

pub struct Controls {
    stdin: Stdr,
    stdout: Stdw,
    stderr: Stdw,
}

impl Controls {
    pub fn new() -> Controls {
        Controls {
            stdin: io::stdio::stdin_raw(),
            stdout: io::stdio::stdout_raw(),
            stderr: io::stdio::stderr_raw()
        }
    }

    pub fn outc(&mut self, ch:char) {
        self.stdout.write_char(ch).unwrap();
    }

    pub fn outs(&mut self, s:&str) {
        self.stdout.write_str(s).unwrap();
    }

    pub fn outf(&mut self, args:fmt::Arguments) {
        self.stdout.write_fmt(args).unwrap();
    }

    pub fn err(&mut self, s:&str) {
        self.stderr.write_str(s).unwrap();
    }

    pub fn errf(&mut self, args:fmt::Arguments) {
        self.stderr.write_fmt(args).unwrap();
    }

    pub fn read(&mut self) -> io::IoResult<char> {
        // Below lifted almost verbatim from rust's read_char.
        // In compliance with MIT, nothing to worry about
        let first_byte = try!(self.stdin.read_byte());
        let width = utf8_char_width(first_byte);
        if width == 1 { return Ok(first_byte as char) }
        if width == 0 { return Err(io::standard_error(InvalidInput)) } // not utf8
        let mut buf = [first_byte, 0, 0, 0];
        {
            let mut start = 1;
            while start < width {
                match try!(self.stdin.read(&mut buf[start..width])) {
                    n if n == width - start => break,
                    n if n < width - start => { start += n; }
                    _ => return Err(io::standard_error(InvalidInput)),
                }
            }
        }
        match str::from_utf8(&buf[..width]).ok() {
            Some(s) => Ok(s.char_at(0)),
            None => Err(io::standard_error(InvalidInput))
        }
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

