#![allow(unstable)]
use libc::*;

use std::os;

use input::*;
use controls::*;
use constants::*;
use util::*;
use signal::*;
use types::*;

// start off as null pointer
static mut uglobal_reader:*mut LineReader = 0 as *mut LineReader;

unsafe extern fn reader_sigint(_:c_int, _:*const SigInfo,
                               _:*const c_void) {
    // Hopefully no segfault, this *should* be safe code
    let reader:&mut LineReader = match uglobal_reader.as_mut() {
        Some(v) => v,
        None => {
            // this handler should never be called when the reader
            // isn't active
            panic!("Reader signal interrupt called when reader not active");
        }
    };
    reader.controls.outs("\nInterrupt\n");
    // reset line
    reader.clear();
    // re-print PS1
    let cwd = os::getcwd().unwrap();
    reader.controls.outf(format_args!("{}$ ", condense_path(cwd).display()));
}

pub struct LineReader {
    pub line: InputLine,
    pub controls: Controls,
    pub bpart: String,
    pub escape: bool,
    pub escape_chars: String,
    pub finished: bool,
    pub eof: bool,
    pub restarted: bool
}

impl LineReader {
    pub fn new() -> LineReader {
        LineReader {
            line: InputLine::new(),
            controls: Controls::new(),
            bpart: String::new(),
            escape: false,
            escape_chars: String::new(),
            finished: false,
            eof: false,
            restarted: false
        }
    }

    fn set_pointer(&mut self) {
        unsafe {
            if !uglobal_reader.is_null() {
                panic!("Tried to set reader location twice");
            }
            uglobal_reader = self as *mut LineReader;
        }
    }

    fn unset_pointer(&self) {
        unsafe {
            if uglobal_reader.is_null() {
                panic!("Tried to unset reader location twice");
            }
            uglobal_reader = 0 as *mut LineReader;
        }
    }

    fn handle_sigint(&mut self) {
        self.set_pointer();
        let mut sa = SigAction {
            handler: reader_sigint,
            mask: [0; SIGSET_NWORDS],
            flags: SA_RESTART | SA_SIGINFO,
            restorer: 0 // null pointer
        };
        let mask = full_sigset().unwrap();
        sa.mask = mask;
        match signal_handle(SIGINT, &sa) {
            Err(e) => self.controls.errf(format_args!("Failed to set SIGINT handler: {}", e)),
            _ => {/* ok */}
        }
    }

    fn unhandle_sigint(&mut self) {
        self.unset_pointer();
        match signal_ignore(SIGINT) {
            Err(e) => self.controls.errf(format_args!("Failed to unset SIGINT handler: {}\n", e)),
            _ => {}
        }
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.bpart.clear();
        self.escape = false;
        self.escape_chars.clear();
        self.finished = false;
        self.eof = false;
        self.restarted = false;
    }

    pub fn restart(&mut self) {
        self.finished = false;
        self.eof = false;
        self.restarted = true;
    }

    pub fn draw_ps1(&mut self) {
        let cwd = os::getcwd().unwrap();
        self.controls.outf(format_args!("{}$ ", condense_path(cwd).display()));
    }

    pub fn read_line(&mut self) -> Option<InputValue> {
        // handle sigint
        self.handle_sigint();
        while !self.finished && !self.eof {
            match self.controls.read() {
                Ok(ch) => {
                    match
                        if self.escape {
                            self.handle_escape(ch)
                        } else if ch.is_control() {
                            self.handle_control(ch)
                        } else {
                            self.handle_ch(ch)
                        } {
                            false => self.controls.outc(BEL),
                            _ => {}
                        }
                },
                Err(e) => {
                    self.controls.errf(format_args!("\nError: {}\n", e));
                    break;
                }
            }
            self.controls.flush();
        }
        // unhandle sigint
        self.unhandle_sigint();
        if self.eof {
            return None;
        } else {
            return self.line.process();
        }        
    }
    
    pub fn draw_part(&mut self) {
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

    pub fn idraw_part(&mut self) {
        // in-place draw of the line part
        self.draw_part();
        self.controls.cursors_left(self.line.part.len());
    }

    pub fn handle_ch(&mut self, ch:char) -> bool {
        if self.line.push(ch) {
            self.controls.outc(ch);
            self.idraw_part();
            return true;
        } else {
            return false;
        }
    }

    pub fn handle_control(&mut self, ch:char) -> bool {
        match ch {
            CEOF => {
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
                    None => return false,
                    Some(_) => {
                        self.controls.cursor_left();
                        self.draw_part();
                        self.controls.outc(SPC);
                        self.controls.cursors_left(self.line.part.len() + 1);
                    }
                }
            },
            _ => return false
        }
        return true;
    }

    pub fn handle_escape(&mut self, ch:char) -> bool {
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
                    self.bpart.clear();
                    self.controls.cursor_left();
                }
                self.escape = false;
            },
            'C' => {
                if self.line.right() {
                    self.bpart.clear();
                    self.controls.cursor_right();
                }
                self.escape = false;
            },
            _ => {
                self.escape = false;
                return false;
            }
        }
        return true;
    }
}

