use libc::*;

use std::collections::RingBuf;

use std::os;

use input::*;
use controls::*;
use constants::*;
use util::*;
use signal::*;
use types::*;

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

    pub fn draw_ps1(&mut self) {
        let cwd = os::getcwd().unwrap();
        self.controls.outf(format_args!("{} => run(", condense_path(cwd).display()));
    }

    fn handle_signal(&mut self, set:&SigSet) {
        let sig = match signal_wait_set(set, None) {
            Err(e) => panic!("Didn't get signal: {}", e),
            Ok(s) => s
        };
        if sig.signo != SIGINT {
            panic!("Caught bad signal: {}", sig.signo);
        }
        self.controls.outs("\nInterrupt\n");
        self.clear();
        self.draw_ps1();
    }

    fn read_character(&mut self) {
        match self.controls.read() {
            Err(e) => panic!("\nError: {}\n", e),
            Ok(ch) => match
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
        }
    }

    pub fn read_line(&mut self) -> Option<InputValue> {
        // these panic because if we can't do this we can't run wash at all
        let mut set = tryp!(empty_sigset());
        tryp!(sigset_add(&mut set, SIGINT));
        let sigfd = tryp!(signal_fd(&set));
        // the file descriptors we want to watch
        let read = vec![sigfd, STDIN];
        let emvc = vec![];
        let mut sread;
        let old_set = tryp!(signal_proc_mask(SIG_BLOCK, &set));
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
                        self.controls.clear_line();
                        self.idraw_part();
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
                while self.line.right() {
                    self.controls.cursor_right();
                }
            },
            CTK => {
                self.controls.clear_line();
                self.line.part.clear();
                self.bpart.clear();
            }
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
                // left
                if self.line.left() {
                    self.bpart.clear();
                    self.controls.cursor_left();
                }
                self.escape = false;
            },
            'C' => {
                // right
                if self.line.right() {
                    self.bpart.clear();
                    self.controls.cursor_right();
                }
                self.escape = false;
            },
            'B' => {
                // down
                self.escape = false;
                match self.bhistory.pop() {
                    None if !self.line.is_empty() => {
                        self.history.push_front(self.line.clone());
                        self.controls.cursors_left(self.line.fpart.len());
                        self.controls.clear_line();
                        self.bpart.clear();
                        self.line.clear();
                    },
                    None => return false,
                    Some(line) => {
                        self.history.push_front(self.line.clone());
                        self.controls.cursors_left(self.line.fpart.len());
                        self.controls.outs(build_string(SPC, self.line.fpart.len() +
                                                        self.line.part.len()).as_slice());
                        self.controls.cursors_left(self.line.fpart.len() +
                                                   self.line.part.len());
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
                        self.controls.cursors_left(self.line.fpart.len());
                        self.controls.clear_line();
                        self.line = line;
                        self.controls.outs(self.line.fpart.as_slice());
                        self.bpart.clear();
                        self.idraw_part();
                    }
                }
            },
            _ => {
                self.escape = false;
                return false;
            }
        }
        return true;
    }
}

