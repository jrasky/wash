extern crate libc;

use libc::{c_int, size_t};
use std::io;
use std::ptr;
use std::mem;
use std::fmt;
use std::sync::Arc;
use std::io::process::{Command, StdioContainer};

use termios::*;
use signal::*;
use constants::*;

mod termios;
mod signal;
mod constants;

type Stdr = io::stdio::StdinReader;
type Stdw = io::stdio::StdWriter;

// start off as null pointer
static mut reader_location:size_t = 0;

struct InputLine {
    words: Vec<String>,
    front: String,
    part: String
}

impl InputLine {
    fn new() -> InputLine {
        InputLine {
            words: Vec::<String>::new(),
            front: String::new(),
            part: String::new()
        }
    }
    fn is_empty(&self) -> bool {
        self.words.is_empty() && self.front.is_empty() && self.part.is_empty()
    }

    fn clear(&mut self) {
        self.words.clear();
        self.front.clear();
        self.part.clear();
    }
    
    fn push(&mut self, ch:char) {
        match ch {
            SPC => {
                if is_word(&self.front) {
                    self.words.push(self.front.clone());
                    self.front.clear();
                }
            },
            c => {
                self.front.push(c);
            }
        }
    }

    fn pop(&mut self) -> Option<char> {
        if self.front.is_empty() {
            self.front = match self.words.pop() {
                Some(s) => s,
                None => return None
            };
            // there are spaces between words
            return Some(SPC);
        } else {
            return self.front.pop();
        }
    }

    fn right(&mut self) -> bool {
        let part = self.part.clone();
        match self.part.pop() {
            Some(ch) => { 
                self.push(ch);
                return true;
            },
            None => false
        }
    }

    fn left(&mut self) -> bool {
        match self.pop() {
            None => false,
            Some(ch) => {
                self.part.push(ch);
                return true;
            }
        }
    }

    fn process(&self) -> Vec<String> {
        let mut part = self.part.clone();
        let mut front = self.front.clone();
        let mut words = self.words.clone();
        loop {
            match part.pop() {
                Some(SPC) => {
                    if is_word(&front) {
                        words.push(front.clone());
                        front.clear();
                    } else {
                        front.push(SPC);
                    }
                },
                Some(ch) => {
                    front.push(ch);
                },
                None => break
            }
        }
        if !front.is_empty() {
            words.push(front);
        }
        return words;
    }
}

struct LineReader {
    line: InputLine,
    controls: Controls,
    bpart: String,
    escape: bool,
    escape_chars: String,
    finished: bool,
    eof: bool
}

impl LineReader {
    fn new(controls:Controls) -> LineReader {
        LineReader {
            line: InputLine::new(),
            controls: controls,
            bpart: String::new(),
            escape: false,
            escape_chars: String::new(),
            finished: false,
            eof: false
        }
    }

    fn clear(&mut self) {
        self.line.clear();
        self.bpart.clear();
        self.escape = false;
        self.escape_chars.clear();
        self.finished = false;
        self.eof = false;
    }

    fn read_line(&mut self) -> Option<Vec<String>> {
        while !self.finished && !self.eof {
            match self.controls.read() {
                Ok(ch) => {
                    if self.escape {
                        self.handle_escape(ch);
                    } else if ch.is_control() {
                        self.handle_control(ch);
                    } else {
                        self.handle_ch(ch);
                    }
                },
                Err(e) => {
                    self.controls.errf(format_args!("\nError: {}\n", e));
                    break;
                }
            }
        }
        if self.eof {
            return None;
        } else {
            return Some(self.line.process());
        }
    }
    
    fn draw_part(&mut self) {
        // quick out if part is empty
        if self.line.part.is_empty() {
            return;
        }
        if self.bpart.is_empty() {
            // only calculate bpart when it needs to be recalculated
            let mut cpart = self.line.part.clone();
            loop {
                match cpart.pop() {
                    Some(c) => self.bpart.push(c),
                    None => break
                }
            }
        }
        self.controls.outs(self.bpart.as_slice());
    }

    fn idraw_part(&mut self) {
        // in-place draw of the line part
        self.draw_part();
        self.controls.cursors_left(self.line.part.len());
    }

    fn handle_ch(&mut self, ch:char) {
        match ch {
            SPC => {
                if is_word(&self.line.front) {
                    self.line.words.push(self.line.front.clone());
                    self.line.front.clear();
                    self.controls.outc(SPC);
                    self.idraw_part();
                } else {
                    self.handle_default(ch);
                }
            },
            _ => self.handle_default(ch)
        }
    }

    fn handle_default(&mut self, ch:char) {
        self.line.push(ch);
        self.controls.outc(ch);
        self.idraw_part();
    }

    fn handle_control(&mut self, ch:char) {
        match ch {
            EOF => {
                if self.line.is_empty() {
                    self.finished = true;
                    self.eof = true;
                }
            },
            NL => {
                self.finished = true;
            },
            ESC => {
                self.escape = true;
                self.escape_chars = String::new();
            },
            DEL => {
                match self.line.pop() {
                    None => return,
                    Some(ch) => {
                        self.controls.cursor_left();
                        self.draw_part();
                        self.controls.outc(SPC);
                        self.controls.cursors_left(self.line.part.len() + 1);
                    }
                }
            },
            _ => return
        }
    }

    fn handle_escape(&mut self, ch:char) {
        match ch {
            ESC => {
                self.escape = false;
            },
            ANSI => {
                if self.escape_chars.is_empty() {
                    self.escape_chars.push(ANSI);
                } else {
                    self.escape = false;
                }
            },
            'D' => {
                if self.line.left() {
                    self.controls.cursor_left();
                }
                self.escape = false;
            },
            'C' => {
                if self.line.right() {
                    self.controls.cursor_right();
                }
                self.escape = false;
            },
            _ => {
                self.escape = false;
            }
        }
    }
}

struct Controls {
    stdin: Stdr,
    stdout: Stdw,
    stderr: Stdw,
}

impl Controls {
    fn new() -> Controls {
        Controls {
            stdin: io::stdin(),
            stdout: io::stdio::stdout_raw(),
            stderr: io::stdio::stderr_raw(),
        }
    }
    
    fn outc(&self, ch:char) {
        self.stdout.write_char(ch).unwrap()
    }

    fn outs(&self, s:&str) {
        self.stdout.write_str(s).unwrap()
    }

    fn err(&self, s:&str) {
        self.stderr.write_str(s).unwrap();
    }

    fn errf(&self, args:fmt::Arguments) {
        self.stderr.write_fmt(args).unwrap();
    }

    fn read(&self) -> io::IoResult<char> {
        self.stdin.read_char()
    }
    
    fn cursor_left(&self) {
        self.outc(DEL);
    }

    fn cursor_right(&self) {
        self.outs(CRSR_RIGHT);
    }
    
    fn cursors_left(&self, by:uint) {
        // move back by a given number of characters
        self.outs(build_string(DEL, by).as_slice());
    }

    fn flush(&mut self) {
        self.stdout.flush();
        self.stderr.flush();
    }
}

#[allow(unused_variables)]
unsafe extern fn reader_sigint(signum:c_int, siginfo:*const SigInfo, context:size_t) {
    // This function should only be called when the input line is actually active
    if reader_location == 0 {
        panic!("Line reader location uninitialized");
    }
    // Below: completely disregarding everything Rust stands for
    let reader:&mut LineReader = mem::transmute(reader_location);
    if reader.line.is_empty() {
        reader.controls.outs("Interrupt\n");
    } else {
        reader.controls.outs("\nInterrupt\n");
    }
    // reset line
    reader.clear();
}

fn set_reader_location(reader:&LineReader) {
    unsafe {
        if (reader_location != 0) {
            panic!("Tried to set reader location twice");
        }
        reader_location = mem::transmute(reader);
    }
}

fn is_word(word:&String) -> bool {
    !word.as_slice().starts_with("\"") ||
        (word.len() > 1 &&
         word.as_slice().starts_with("\"") &&
         word.as_slice().ends_with("\""))
}

// work around lack of DST
fn build_string(ch:char, count:uint) -> String {
    let mut s = String::new();
    let mut i = 0u;
    loop {
        if i == count {
            return s;
        }
        s.push(ch);
        i += 1;
    }
}

fn strip_words(line:Vec<String>) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for word in line.iter() {
        out.push(strip_word(word.clone()));
    }
    return out;
}

fn strip_word(mut word:String) -> String {
    if word.as_slice().starts_with("\"") &&
        word.as_slice().ends_with("\"") {
            word.remove(0);
            let len = word.len();
            word.remove(len - 1);
        }
    return word;
}

fn terminal_settings(controls:Controls) -> (Termios, Termios) {
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

fn update_terminal(tios:Termios, controls:Controls) {
    if Termios::set(&tios) {
        controls.err("Warning: Could not set terminal mode\n");
    }
}

fn set_reader_sigint(controls:Controls) {
    let mut sa = SigAction {
        handler: reader_sigint,
        mask: [0; SIGSET_NWORDS],
        flags: SA_RESTART | SA_SIGINFO,
        restorer: 0 // null pointer
    };
    unsafe {
        let mask = full_sigset().expect("Could not get a full sigset");
        if !signal_handle(SIGINT, &sa) {
            controls.err("Warning: could not set handler for SIGINT\n");
        }
    }
}

fn run_command(line:Vec<String>, controls:Controls) {
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
    let controls = Controls::new();
    let mut reader = LineReader::new(controls);
    let (tios, old_tios) = terminal_settings(controls);
    update_terminal(tios, controls);
    set_reader_location(&reader);
    set_reader_sigint(controls);
    let mut line:Vec<String>;
    loop {
        line = match reader.read_line() {
            None => break,
            Some(l) => l
        };
        if !line.is_empty() {
            update_terminal(old_tios, controls);
            signal_ignore(SIGINT);
            controls.flush();
            run_command(line, controls);
            set_reader_sigint(controls);
            update_terminal(tios, controls);
            reader.clear();
        }
    }
}
