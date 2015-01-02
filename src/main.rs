extern crate libc;

use libc::{c_uint, c_uchar, c_int};
use std::io;

// used in Termios struct
pub const NCCS:uint = 32;

// stdin, stdout, stderr have standard file descriptors
pub const STDIN:c_int = 0;

// constants for control sequences is useful
pub const EOF:char = '\u{4}';
pub const DEL:char = '\u{7f}';

// select termios constants that we use
pub const ICANON:c_uint   = 2;
pub const ECHO:c_uint     = 8;
pub const TCSANOW:c_int   = 0;
pub const TCSADRAIN:c_int = 1;
pub const TCSAFLUSH:c_int = 2;

// types used in Termios struct
#[allow(non_camel_case_types)]
pub type cc_t = c_uchar;
#[allow(non_camel_case_types)]
pub type speed_t = c_uint;
#[allow(non_camel_case_types)]
pub type tcflag_t = c_uint;


#[repr(C)]
#[deriving(Copy)]
#[deriving(Clone)]
pub struct Termios {
    c_iflag: tcflag_t,
    c_oflag: tcflag_t,
    c_cflag: tcflag_t,
    c_lflag: tcflag_t,
    c_line: cc_t,
    c_cc: [c_uchar, ..NCCS],
    c_ispeed: speed_t,
    c_ospeed: speed_t,
}

impl Termios {
    pub fn new() -> Termios {
        Termios {
            c_cc: [0, ..NCCS],
            c_cflag: 0,
            c_iflag: 0,
            c_ispeed: 0,
            c_lflag: 0,
            c_line: 0,
            c_oflag: 0,
            c_ospeed: 0,
        }
    }

    pub fn get_from(&mut self, fd:c_int) -> bool {
        unsafe {
            return tcgetattr(fd, self) == 0;
        }
    }

    pub fn get(&mut self) -> bool {
        self.get_from(STDIN)
    }

    pub fn set_to(&self, fd:c_int) -> bool {
        unsafe {
            return tcsetattr(fd, TCSANOW, self) == 0;
        }
    }

    pub fn set(&self) -> bool {
        self.set_to(STDIN)
    }

    pub fn lenable(&mut self, flag:c_uint) {
        self.c_lflag |= flag;
    }

    pub fn ldisable(&mut self, flag:c_uint) {
        self.c_lflag &= !flag;
    }
}

#[link(name = "c")]
extern {
    pub fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
    pub fn tcsetattr(fd: c_int, optional_actions: c_int, termios: *const Termios) -> c_int;
}

fn empty_escape(esc:&mut Iterator<char>) -> String {
    let mut out = String::new();
    loop {
        match esc.next() {
            Some(c) => out.push(c),
            None => break
        }
    }
    return out;
}

fn main() {
    let mut tios = Termios::new();
    tios.get();
    let old_tios = tios.clone();
    // turn off canonical mode
    tios.ldisable(ICANON);
    // turn off echo mode
    tios.ldisable(ECHO);
    tios.set();
    let mut stdin = io::stdin();
    loop {
        match stdin.read_char() {
            Ok(EOF) => break,
            Ok(DEL) => {
                print!("{} {}", DEL, DEL);
            },
            Ok(c) => {
                print!("{}", empty_escape(&mut c.escape_default()))
            },
            Err(_) => {
                println!("Error: exiting");
                break;
            }
        }
    }
    // print so we know we've reached this code
    println!("Exiting");
    // restore old term state
    old_tios.set();
}
