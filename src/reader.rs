use libc::*;

use std::collections::RingBuf;

use std::num::*;

use input::*;
use controls::*;
use constants::*;
use signal::*;
use types::*;
use ioctl::*;

pub struct LineReader {
    pub line: InputLine,
    pub controls: Controls,
    pub bpart: String,
    pub escape: bool,
    pub escape_chars: String,
    pub finished: bool,
    pub eof: bool,
    pub restarted: bool,
    history: RingBuf<InputLine>,
    bhistory: Vec<InputLine>
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
            restarted: false,
            history: RingBuf::new(),
            bhistory: vec![]
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

    fn handle_signal(&mut self, set:&SigSet) {
        let sig = match signal_wait_set(set, None) {
            Err(e) => panic!("Didn't get signal: {}", e),
            Ok(s) => s
        };
        match sig.signo {
            SIGINT => {
                self.controls.outs("\nInterrupt");
                self.clear();
                self.finished = true;
            },
            SIGWINCH => {
                match term_winsize() {
                    Err(e) => self.controls.errf(format_args!("\nCouldn't get terminal size: {}\n", e)),
                    Ok(size) => self.controls.update_size(size)
                }
                self.controls.query_cursor();
            },
            s => panic!("Caught bad signal: {}", s)
        }
    }

    fn read_character(&mut self) {
        match self.controls.read() {
            Err(e) => panic!("Error: {}", e),
            Ok(ch) => match
                if self.escape {
                    self.handle_escape(ch)
                } else if ch.is_control() {
                    self.handle_control(ch)
                } else {
                    self.handle_ch(ch)
                } {
                    false => self.controls.bell(),
                    _ => {}
                }
        }
    }

    pub fn read_line(&mut self) -> Option<InputValue> {
        // these panic because if we can't do this we can't run wash at all
        let mut set = tryp!(empty_sigset());
        tryp!(sigset_add(&mut set, SIGINT));
        tryp!(sigset_add(&mut set, SIGWINCH));
        let sigfd = tryp!(signal_fd(&set));
        // the file descriptors we want to watch
        let read = vec![sigfd, STDIN];
        let emvc = vec![];
        let mut sread;
        let old_set = tryp!(signal_proc_mask(SIG_BLOCK, &set));
        match term_winsize() {
            Err(e) => self.controls.errf(format_args!("\nCouldn't get terminal size: {}\n", e)),
            Ok(size) => self.controls.update_size(size)
        }
        // update cursor position before anything
        self.controls.clear_rows();
        self.controls.query_cursor();
        while !self.finished && !self.eof {
            sread = match select(&read, &emvc, &emvc,
                                 None, &set) {
                Err(_) => continue, // try again
                Ok(v) => v
            };
            if sread.len() == 2 {
                // prefer SIGINT
                self.handle_signal(&set);
            } else {
                match sread.pop() {
                    None => self.handle_signal(&set),
                    Some(ref fd) if *fd == sigfd =>
                        self.handle_signal(&set),
                    Some(ref fd) if *fd == STDIN =>
                        self.read_character(),
                    _ => panic!("select returned unknown file descriptor")
                }
            }
            self.controls.flush();
        }
        tryp!(signal_proc_mask(SIG_SETMASK, &old_set));
        if self.eof {
            return None;
        } else {
            // push back history onto history
            if !self.bhistory.is_empty() {
                self.history.push_front(self.line.clone());
            }
            let mut popped;
            while !self.bhistory.is_empty() {
                popped = self.bhistory.pop().unwrap();
                if !popped.is_empty() {
                    self.history.push_front(popped);
                }
            }
            self.history.push_front(self.line.clone());
            while self.history.len() > HISTORY_SIZE {
                self.history.pop_back();
            }
            return self.line.process();
        }        
    }
    
    pub fn draw_part(&mut self) -> usize {
        // quick out if part is empty
        if self.line.part.is_empty() {
            return 0;
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
        let splits:Vec<&str> = NL_REGEX.split(self.bpart.as_slice()).collect();
        if self.controls.grow_check(splits[0].len()) {
            let old = self.controls.get_pos();
            for part in splits.iter() {
                if self.controls.grow_check(part.len()) {
                    let crow = self.controls.get_row();
                    let total = crow + part.len();
                    for row in range(crow + 1,
                                     crow + total/self.controls.width() + 1) {
                        self.controls.clear_line();
                        self.controls.move_to(Position {
                            col: 1,
                            row: row
                        });
                    }
                    self.controls.clear_line();
                    self.controls.next_start();
                } else {
                    self.controls.clear_line();
                    self.controls.next_start();
                }
            }
            self.controls.move_to(old);
            self.controls.outs(self.bpart.as_slice());
            return self.bpart.len();
        } else {
            // change doesn't affect anything other than this line
            self.controls.outs(splits[0]);
            return splits[0].len();
        }
    }

    pub fn idraw_part(&mut self) {
        // in-place draw of the line part
        let count = self.draw_part();
        self.controls.cursors_left(count);
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
                if !self.line.push(NL) {
                    self.finished = true;
                } else {
                    self.controls.outc(NL);
                }
            },
            ESC => {
                self.escape = true;
                self.escape_chars = String::new();
            },
            DEL => {
                match self.line.pop() {
                    None => return false,
                    Some(_) => {
                        self.controls.del();
                        let count = self.draw_part();
                        self.controls.outc(SPC);
                        self.controls.cursors_left(count + 1);
                    }
                }
            },
            CTA => {
                // C-a
                self.controls.cursors_left(self.line.fpart.len());
                loop {
                    match self.line.fpart.pop() {
                        Some(ch) => self.line.part.push(ch),
                        None => break
                    }
                }
                self.line.fpart.clear();
                self.line.back.clear();
                self.line.front.clear();
                self.line.back.push(InputValue::Long(vec![]));
                self.bpart.clear();
            },
            CTE => {
                // C-e
                self.controls.cursors_right(self.line.part.len());
                while self.line.right() {}
            },
            CTK => self.clear_line(),
            _ => return false
        }
        return true;
    }

    fn clear_line(&mut self) {
        self.controls.clear_line_to(self.line.part.len());
        self.line.part.clear();
        self.bpart.clear();
    }

    fn clear_entire_line(&mut self) {
        self.controls.cursors_left(self.line.fpart.len());
        self.controls.clear_line_to(self.line.fpart.len() + self.line.part.len());
        self.bpart.clear();
        self.line.clear();
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
                // left
                self.escape = false;
                if self.line.left() {
                    self.bpart.clear();
                    self.controls.cursor_left();
                } else {
                    return false;
                }
            },
            'C' => {
                // right
                self.escape = false;
                if self.line.right() {
                    self.bpart.clear();
                    self.controls.cursor_right();
                } else {
                    return false;
                }
            },
            'B' => {
                // down
                self.escape = false;
                match self.bhistory.pop() {
                    None if !self.line.is_empty() => {
                        self.history.push_front(self.line.clone());
                        self.clear_entire_line();
                    },
                    None => return false,
                    Some(line) => {
                        self.history.push_front(self.line.clone());
                        self.clear_entire_line();
                        self.line = line;
                        self.controls.outs(self.line.fpart.as_slice());
                        self.bpart.clear();
                        self.idraw_part();
                    }
                }
            },
            'A' => {
                // up
                self.escape = false;
                match self.history.pop_front() {
                    None => return false,
                    Some(line) => {
                        self.bhistory.push(self.line.clone());
                        self.clear_entire_line();
                        self.line = line;
                        self.controls.outs(self.line.fpart.as_slice());
                        self.bpart.clear();
                        self.idraw_part();
                    }
                }
            },
            'R' => {
                // cursor position
                self.escape = false;
                if !PPOS_REGEX.is_match(self.escape_chars.as_slice()) {
                    return false;
                } else {
                    let caps = PPOS_REGEX.captures(self.escape_chars.as_slice()).unwrap();
                    let row = caps.at(1).unwrap();
                    let col = caps.at(2).unwrap();
                    let pointer = Position {
                        row: match from_str_radix(row, 10) {
                            Err(_) => return false,
                            Ok(v) => v
                        },
                        col: match from_str_radix(col, 10) {
                            Err(_) => return false,
                            Ok(v) => v
                        }
                    };
                    self.controls.update_cursor(pointer);
                }
            },
            ch => {
                self.escape_chars.push(ch);
                if self.escape_chars.len() > MAX_ESCAPE {
                    self.escape = false;
                    return false;
                }
            }
        }
        return true;
    }
}

