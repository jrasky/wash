use libc::*;
use std::mem;

use constants::*;

#[derive(Copy)]
pub struct SigVal {
    // _data is either a c_int (4 bytes), or a *const c_void (8 bytes)
    _data: [c_int; SI_VAL_SIZE]
}

// The size_t at the end is a pointer to a ucontext_t
// do I want to implement that type? Lol no
// Protip: we're calling sigaction with SA_SIGINFO
// In part because I didn't realize I didn't have to implement
// all of SigInfo, so I did, and now I want to use that code dammit!
// SigHandler is by definition unsafe, note it as so
pub type SigHandler = unsafe extern fn(c_int, *const SigInfo, *const c_void);
pub type SigSet = [c_ulong; SIGSET_NWORDS];

// "What is all this?"
// This is an implementation of C unions in Rust

#[derive(Copy)]
pub struct KillFields {
    pub pid: pid_t,
    pub uid: uid_t,
    _pad: [c_int; (SI_PAD_SIZE - 2)]
}

#[derive(Copy)]
pub struct PTimerFields {
    pub tid: c_int,
    pub overrun: c_int,
    pub sigval: SigVal,
    _pad: [c_int; (SI_PAD_SIZE - 4)]
}

#[derive(Copy)]
pub struct PSignalFields {
    pub pid: pid_t,
    pub uid: uid_t,
    pub sigval: SigVal,
    _pad: [c_int; (SI_PAD_SIZE - 4)]
}

#[derive(Copy)]
pub struct SigChldFields {
    pub pid: pid_t,
    pub uid: uid_t,
    pub status: c_int,
    pub utime: clock_t,
    pub stime: clock_t,
    _pad: [c_int; (SI_PAD_SIZE - 8)]
}

#[derive(Copy)]
pub struct SigFaultFields {
    pub addr: size_t, // pointer
    pub addr_lsb: c_short,
    _pad: [c_int; (SI_PAD_SIZE - 3)]
}

#[derive(Copy)]
pub struct SigPollFields {
    pub band: c_long,
    pub fd: c_int,
    _pad: [c_int; (SI_PAD_SIZE - 3)]
}

#[derive(Copy)]
pub struct SigSysFields {
    _call_addr: size_t, // pointer
    _syscall: c_int,
    _arch: c_uint,
    _pad: [c_int; (SI_PAD_SIZE - 4)]
}

#[derive(Copy)]
pub enum SigFields {
    Kill(KillFields),
    PTimer(PTimerFields),
    PSignal(PSignalFields),
    SigChld(SigChldFields),
    SigFault(SigFaultFields),
    SigPoll(SigPollFields),
    SigSys(SigSysFields)
}

#[repr(C)]
#[derive(Copy)]
pub struct SigInfo {
    pub signo: c_int,
    pub errno: c_int,
    pub code: c_int,

    // Rust has no struct alignment, so manual padding is necessary
    _align: c_int, // four bytes of padding

    // Generic memory pad for union contents
    _data: [c_int; SI_PAD_SIZE]
}

#[repr(C)]
#[derive(Copy)]
pub struct SigAction {
    pub handler: SigHandler,
    pub mask: SigSet,
    pub flags: c_int,
    // This is technically a pointer, but the man pages say to keep it null,
    // and that's easier if we just use a size_t
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

impl SigInfo {
    pub fn determine_sigfields(&self) -> SigFields {
        unsafe {
            match self.signo {
                SIGKILL => SigFields::Kill(mem::transmute(self._data)),
                SIGALRM | SIGVTALRM | SIGPROF => SigFields::PTimer(mem::transmute(self._data)),
                SIGCHLD => SigFields::SigChld(mem::transmute(self._data)),
                SIGILL | SIGFPE | SIGSEGV | SIGBUS => SigFields::SigFault(mem::transmute(self._data)),
                SIGPOLL => SigFields::SigPoll(mem::transmute(self._data)),
                SIGSYS => SigFields::SigSys(mem::transmute(self._data)),
                _ => SigFields::PSignal(mem::transmute(self._data))
            }
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
