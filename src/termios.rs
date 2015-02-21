use libc::{c_uint, c_uchar, c_int};

use std::io;

use constants::*;

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

    pub fn get() -> io::Result<Termios> {
        let mut tios = Termios::new();
        match unsafe {tcgetattr(STDIN, &mut tios)} {
            0 => Ok(tios),
            _ => Err(io::Error::last_os_error())
        }

    }

    pub fn set(tios:&Termios) -> io::Result<()> {
        match unsafe {tcsetattr(STDIN, TCSANOW, tios)} {
            0 => Ok(()),
            _ => Err(io::Error::last_os_error())
        }
    }

    // Include for completeness
    // isn't used currently, may be in the future
    #[allow(dead_code)]
    pub fn fenable(&mut self, cflag:TCFlag, iflag:TCFlag,
                   lflag:TCFlag, oflag:TCFlag) {
        self.cflag |= cflag;
        self.iflag |= iflag;
        self.lflag |= lflag;
        self.oflag |= oflag;
    }

    pub fn fdisable(&mut self, cflag:TCFlag, iflag:TCFlag,
                    lflag:TCFlag, oflag:TCFlag) {
        self.cflag &= !cflag;
        self.iflag &= !iflag;
        self.lflag &= !lflag;
        self.oflag &= !oflag;
    }
}

#[link(name = "c")]
extern {
    fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
    fn tcsetattr(fd: c_int, optional_actions: c_int, termios: *const Termios) -> c_int;
}
