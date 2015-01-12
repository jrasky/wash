#![allow(unstable)]
use input::*;
use controls::*;
use constants::*;
use util::*;

use std::os;

pub struct LineReader {
    pub line: InputLine,
    pub controls: Controls,
    pub bpart: String,
    pub escape: bool,
    pub escape_chars: String,
    pub finished: bool,
    pub eof: bool
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
            eof: false
        }
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.bpart.clear();
        self.escape = false;
        self.escape_chars.clear();
        self.finished = false;
        self.eof = false;
    }

    pub fn read_line(&mut self) -> Option<Vec<String>> {
        let cwd = os::getcwd().unwrap();
        self.controls.outf(format_args!("{}$ ", condense_path(cwd).display()));
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
            self.controls.flush();
        }
        if self.eof {
            return None;
        } else {
            return Some(self.line.process());
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

    pub fn handle_ch(&mut self, ch:char) {
        self.line.push(ch);
        self.controls.outc(ch);
        self.idraw_part();
    }

    pub fn handle_control(&mut self, ch:char) {
        match ch {
            CEOF => {
                if self.line.is_empty() {
                    self.finished = true;
                    self.eof = true;
                }
            },
            NL => {
                self.controls.outc(NL);
                self.finished = true;
            },
            ESC => {
                self.escape = true;
                self.escape_chars = String::new();
            },
            DEL => {
                match self.line.pop() {
                    None => return,
                    Some(_) => {
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

    pub fn handle_escape(&mut self, ch:char) {
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
            }
        }
    }
}
