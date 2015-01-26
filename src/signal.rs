use libc::*;
use std::mem;

use constants::*;

pub struct SigVal {
    pub val_int: c_int,
    pub val_ptr: *const c_void
}

// The size_t at the end is a pointer to a ucontext_t
// do I want to implement that type? Lol no
// Protip: we're calling sigaction with SA_SIGINFO
// In part because I didn't realize I didn't have to implement
// all of SigInfo, so I did, and now I want to use that code dammit!
// SigHandler is by definition unsafe, note it as so
pub type SigHandler = unsafe extern fn(c_int, *const SigInfo, *const c_void);
pub type SigSet = [c_ulong; SIGSET_NWORDS];

pub struct SigInfo {
    pub signo: c_int,
    pub errno: c_int,
    pub code: c_int,
    pub trapno: c_int,
    pub pid: pid_t,
    pub uid: uid_t,
    pub status: c_int,
    pub utime: clock_t,
    pub stime: clock_t,
    pub value: SigVal,
    pub int: c_int,
    pub ptr: *const c_void,
    pub overrun: c_int,
    pub timerid: c_int,
    pub addr: *const c_void,
    pub band: c_long,
    pub fd: c_int,
    pub addr_lsb: c_short
}

#[repr(C)]
#[derive(Copy)]
pub struct SigAction {
    pub handler: SigHandler,
    pub mask: SigSet,
    pub flags: c_int,
    // this is a size_t because the manpage say to
    // not provide a function, and setting this to
    // zero is much easier than trying to create a
    // null pointer in rust
    pub restorer: size_t
}

impl SigAction {
    pub fn new() -> SigAction {
        SigAction {
            handler: unsafe {mem::transmute::<usize, SigHandler>(0)},
            mask: full_sigset().unwrap_or([0; SIGSET_NWORDS]),
            flags: SA_RESTART,
            restorer: 0
        }
    }

    pub fn ignore() -> SigAction {
        SigAction {
            handler: unsafe {mem::transmute::<usize, SigHandler>(1)},
            mask: full_sigset().unwrap_or([0; SIGSET_NWORDS]),
            flags: SA_RESTART,
            restorer: 0
        }
    }

    pub fn handler(handler:SigHandler) -> SigAction {
        SigAction {
            handler: handler,
            mask: full_sigset().unwrap_or([0; SIGSET_NWORDS]),
            flags: SA_RESTART | SA_SIGINFO,
            restorer: 0
        }
    }
}

#[link(name="c")]
extern {
    fn sigaction(signum:c_int, act:*const SigAction, oldact:*mut SigAction) -> c_int;
    fn sigfillset(mask:*mut SigSet) -> c_int;
}

pub fn signal_handle(signal:c_int, action:&SigAction) -> bool {
    let (status, _) = signal_handle_inner(signal, action);
    return status;
}

pub fn signal_handle_inner(signal:c_int, action:*const SigAction) -> (bool, *const SigAction) {
    unsafe {
        let old_act:*mut SigAction = &mut SigAction::new();
        let status = sigaction(signal, action, old_act) == 0;
        return (status, old_act);
    }
}

pub fn signal_ignore(signal:c_int) -> bool {
    let (status, _) = signal_ignore_inner(signal);
    return status;
}

pub fn signal_ignore_inner(signal:c_int) -> (bool, *const SigAction) {
    let action = SigAction::ignore();
    signal_handle_inner(signal, &action)
}

pub fn signal_default(signal:c_int) -> bool {
    let (status, _) = signal_default_inner(signal);
    return status;
}

pub fn signal_default_inner(signal:c_int) -> (bool, *const SigAction) {
    let action = SigAction::new();
    signal_handle_inner(signal, &action)
}

pub fn full_sigset() -> Option<SigSet> {
    let mut output:SigSet = [0; SIGSET_NWORDS];
    unsafe {
        match sigfillset(&mut output as *mut SigSet) {
            0 => Some(output),
            _ => None
        }
    }
}
