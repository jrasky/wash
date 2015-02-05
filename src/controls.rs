use std::old_io::IoErrorKind::*;
use unicode::str::*;

use std::str;
use std::old_io;
use std::fmt;

use constants::*;
use util::*;
use types::*;
use ioctl::*;

type Stdr = old_io::stdio::StdReader;
type Stdw = old_io::stdio::StdWriter;

pub struct Controls {
    stdin: Stdr,
    stdout: Stdw,
    stderr: Stdw,
    cursor: Position,
    tsize: WinSize
}

impl Controls {
    pub fn new() -> Controls {
        Controls {
            stdin: old_io::stdio::stdin_raw(),
            stdout: old_io::stdio::stdout_raw(),
            stderr: old_io::stdio::stderr_raw(),
            cursor: Position::new(),
            tsize: WinSize::new()
        }
    }

    pub fn update_cursor(&mut self, pos:Position) {
        self.cursor = pos;
    }

    pub fn update_size(&mut self, size:WinSize) {
        self.tsize = size;
    }

    fn move_right(&mut self, by:usize) {
        self.cursor.col += by;
        if self.cursor.col > self.tsize.col as usize {
            self.cursor.row += self.cursor.col / self.tsize.col as usize;
            self.cursor.col = self.cursor.col % self.tsize.col as usize;
            if self.cursor.row > self.tsize.row as usize {
                self.cursor.row = self.tsize.row as usize;
            }
        }
    }

    fn move_left(&mut self, by:usize) {
        if by >= self.cursor.col {
            let diff = by - self.cursor.col;
            self.cursor.col = self.tsize.col as usize -
                (diff % self.tsize.col as usize);
            let rdiff = (diff / self.tsize.col as usize) + 1;
            if rdiff > self.cursor.row {
                self.cursor.row = 0;
            } else {
                self.cursor.row -= rdiff;
            }
        } else {
            self.cursor.col -= by;
        }
    }

    pub fn outc(&mut self, ch:char) {
        self.stdout.write_char(ch).unwrap();
        self.move_right(1);
    }

    pub fn outs(&mut self, s:&str) {
        self.stdout.write_str(s).unwrap();
        self.move_right(s.len());
    }

    pub fn outf(&mut self, args:fmt::Arguments) {
        self.outs(fmt::format(args).as_slice());
    }

    pub fn err(&mut self, s:&str) {
        self.stderr.write_str(s).unwrap();
        self.move_right(s.len());
    }

    pub fn errf(&mut self, args:fmt::Arguments) {
        self.err(fmt::format(args).as_slice());
    }

    pub fn read(&mut self) -> old_io::IoResult<char> {
        // Below lifted almost verbatim from rust's read_char.
        // In compliance with MIT, nothing to worry about
        let first_byte = try!(self.stdin.read_byte());
        let width = utf8_char_width(first_byte);
        if width == 1 { return Ok(first_byte as char) }
        if width == 0 { return Err(old_io::standard_error(InvalidInput)) } // not utf8
        let mut buf = [first_byte, 0, 0, 0];
        {
            let mut start = 1;
            while start < width {
                match try!(self.stdin.read(&mut buf[start..width])) {
                    n if n == width - start => break,
                    n if n < width - start => { start += n; }
                    _ => return Err(old_io::standard_error(InvalidInput)),
                }
            }
        }
        match str::from_utf8(&buf[..width]).ok() {
            Some(s) => Ok(s.char_at(0)),
            None => Err(old_io::standard_error(InvalidInput))
        }
    }
    
    pub fn cursor_left(&mut self) {
        if self.cursor.col > 1 {
            self.stdout.write_char(DEL).unwrap();
            self.move_left(1);
        } else {
            self.move_left(1);
            self.move_to_pointer();
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor.col < self.tsize.col as usize {
            self.stdout.write_str(CRSR_RIGHT).unwrap();
            self.move_right(1);
        } else {
            self.move_right(1);
            self.move_to_pointer();
        }
    }
    
    pub fn cursors_left(&mut self, by:usize) {
        if by == 0 {
            return;
        } else if by <= 3 && by + self.cursor.col < self.tsize.col as usize {
            self.stdout.write_str(build_string(DEL, by).as_slice()).unwrap();
            self.move_left(by);
        } else if by < self.cursor.col {
            self.stdout.write_fmt(format_args!("{}{}D", ANSI_BEGIN, by)).unwrap();
            self.move_left(by);
        } else {
            self.move_left(by);
            self.move_to_pointer();
        }
    }

    pub fn cursors_right(&mut self, by:usize) {
        if by == 0 {
            return;
        } else if by + self.cursor.col < self.tsize.col as usize {
            self.stdout.write_fmt(format_args!("{}{}C", ANSI_BEGIN, by)).unwrap();
            self.move_right(by);
        } else {
            self.move_right(by);
            self.move_to_pointer();
        }
    }

    pub fn clear_line(&mut self) {
        self.stdout.write_str(ANSI_BEGIN).unwrap();
        self.stdout.write_char('K').unwrap();
    }

    pub fn clear_line_to(&mut self, len:usize) {
        self.clear_line();
        let total = len + self.cursor.col;
        if total > self.tsize.col as usize {
            let old = self.cursor;
            for row in range(self.cursor.row + 1,
                             self.cursor.row + total/self.tsize.col as usize + 1) {
                self.move_to(Position {
                    col: 1,
                    row: row
                });
                self.clear_line();
            }
            self.move_to(old)
        }
    }

    pub fn flush(&mut self) {
        self.stdout.flush().unwrap();
        self.stderr.flush().unwrap();
    }

    pub fn query_cursor(&mut self) {
        self.stdout.write_str(CRSR_POS).unwrap();
    }

    fn move_to_pointer(&mut self) {
        let (row, col) = (self.cursor.row, self.cursor.col);
        self.stdout.write_fmt(format_args!("{}{};{}f", ANSI_BEGIN, row, col)).unwrap();
    }

    pub fn move_to(&mut self, pos:Position) {
        self.cursor = pos;
        self.move_to_pointer();
    }

    pub fn bell(&mut self) {
        self.stdout.write_char(BEL).unwrap();
    }
}

