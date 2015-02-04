use libc::*;

use std::os::unix::prelude::*;
use std::old_io::*;

use constants::*;

#[derive(Copy)]
#[repr(C)]
pub struct WinSize {
    pub row: c_ushort,
    pub col: c_ushort,
    pub xpixel: c_ushort,
    pub ypixel: c_ushort
}

impl WinSize {
    pub fn new() -> WinSize {
        WinSize {
            row: 1,
            col: 1,
            xpixel: 1,
            ypixel: 1
        }
    }
}

#[link(name="c")]
extern {
    fn ioctl(d:Fd, request:c_ulong, ...) -> c_int;
}

pub fn term_winsize() -> IoResult<WinSize> {
    let mut size = WinSize::new();
    match unsafe {ioctl(STDIN, TIOCGWINSZ, &mut size)} {
        0 => Ok(size),
        _ => Err(IoError::last_error())
    }
}
