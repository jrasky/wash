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

#[derive(Copy)]
#[repr(C)]
pub struct TM {
    pub sec: c_int,
    pub min: c_int,
    pub hour: c_int,
    pub mday: c_int,
    pub mon: c_int,
    pub year: c_int,
    pub wday: c_int,
    pub yday: c_int,
    pub isdst: c_int,
    _gmtoff: c_long,
    _tmzone: *const c_char
}

#[link(name="c")]
extern {
    fn ioctl(d:Fd, request:c_ulong, ...) -> c_int;
    fn gethostname(name:*mut c_char, len:size_t) -> c_int;
    fn getlogin() -> *const c_char;
    fn strftime(s:*mut c_char, max:size_t, format:*const c_char,
                tm:*const TM) -> size_t;
    fn time(t:*mut time_t) -> time_t;
    fn localtime(timep:*const time_t) -> *const TM;
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
        0 => Ok(String::from_utf8_lossy(unsafe {
            ffi::c_str_to_bytes(&name.as_ptr())}).into_owned()),
        _ => Err(IoError::last_error())
    }
}

pub fn get_login() -> IoResult<String> {
    let name = unsafe {getlogin()};
    if name.is_null() {
        return Err(IoError::last_error())
    } else {
        return Ok(String::from_utf8_lossy(unsafe {
            ffi::c_str_to_bytes(&name)}).into_owned());
    }
}

pub fn get_time() -> Option<TM> {
    let t = unsafe {time(0 as *mut time_t)};
    let tm = unsafe {localtime(&t)};
    match unsafe {tm.as_ref()} {
        None => return None,
        Some(v) => return Some(*v)
    }
}

pub fn strf_time(format:&String, time:&TM) -> String {
    let mut out = [1; STRF_BUF_SIZE];
    let format_cstr = ffi::CString::from_slice(format.as_bytes());
    match unsafe {strftime(out.as_mut_ptr(), STRF_BUF_SIZE as u64,
                           format_cstr.as_ptr(), time)} {
        0 => return String::new(), // contents of out may be undefined
        _ => return String::from_utf8_lossy(unsafe {
            ffi::c_str_to_bytes(&out.as_ptr())}).into_owned()
    }
    
}
