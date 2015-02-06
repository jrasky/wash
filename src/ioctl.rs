use libc::*;

use std::os::unix::prelude::*;
use std::old_io::*;
use std::ffi;

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
    fn gethostname(name:*mut c_char, len:size_t) -> c_int;
    fn getlogin() -> *const c_char;
}

pub fn term_winsize() -> IoResult<WinSize> {
    let mut size = WinSize::new();
    match unsafe {ioctl(STDIN, TIOCGWINSZ, &mut size)} {
        0 => Ok(size),
        _ => Err(IoError::last_error())
    }
}

pub fn get_hostname() -> IoResult<String> {
    let mut name = [1; HOST_NAME_MAX];
    match unsafe {gethostname(name.as_mut_ptr(), HOST_NAME_MAX as u64)} {
        0 => Ok(String::from_utf8_lossy(unsafe {ffi::c_str_to_bytes(&name.as_ptr())}).into_owned()),
        _ => Err(IoError::last_error())
    }
}

pub fn get_login() -> IoResult<String> {
    let name = unsafe {getlogin()};
    if name.is_null() {
        return Err(IoError::last_error())
    } else {
        return Ok(String::from_utf8_lossy(unsafe {ffi::c_str_to_bytes(&name)}).into_owned());
    }
}
