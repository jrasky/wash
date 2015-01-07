extern crate libc;

use self::libc::{c_uint, c_uchar, c_int};

use constants::*;

// used in Termios struct
const NCCS:uint = 32;

// types used in Termios struct
type CCType = c_uchar;
type SpeedType = c_uint;
type TCFlag = c_uint;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Termios {
    iflag: TCFlag,
    oflag: TCFlag,
    cflag: TCFlag,
    lflag: TCFlag,
    line: CCType,
    cc: [c_uchar; NCCS],
    ispeed: SpeedType,
    ospeed: SpeedType,
}

impl Termios {
    pub fn new() -> Termios {
        Termios {
            cc: [0; NCCS],
            cflag: 0,
            iflag: 0,
            ispeed: 0,
            lflag: 0,
            line: 0,
            oflag: 0,
            ospeed: 0,
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

    #[allow(dead_code)]
    pub fn lenable(&mut self, flag:c_uint) {
        self.lflag |= flag;
    }

    pub fn ldisable(&mut self, flag:c_uint) {
        self.lflag &= !flag;
    }
}

#[link(name = "c")]
extern {
    fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
    fn tcsetattr(fd: c_int, optional_actions: c_int, termios: *const Termios) -> c_int;
}
