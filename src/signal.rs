use libc::*;
use std::mem;

use constants::*;

// This could also be a pointer,
// but it probably isn't
pub type SigVal = c_int;

// The size_t at the end is a pointer to a ucontext_t
// do I want to implement that type? Lol no
// Protip: we're calling sigaction with SA_SIGINFO
// In part because I didn't realize I didn't have to implement
// all of SigInfo, so I did, and now I want to use that code dammit!
// SigHandler is by definition unsafe, note it as so
pub type SigHandler = unsafe extern fn(c_int, *const SigInfo, *const c_void);
pub type SigSet = [c_ulong; SIGSET_NWORDS];

// _pad is needed to make all these types the same size
// this is the case because rust doesn't have an equivalent to C unions
// it's really a bitch
// at least let me transmute between different sized types I mean jesus christ

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
    _pad: [c_int; (SI_PAD_SIZE - 3)]
}

#[derive(Copy)]
pub struct PSignalFields {
    pub pid: pid_t,
    pub uid: uid_t,
    pub sigval: SigVal,
    _pad: [c_int; (SI_PAD_SIZE - 3)]
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

// If only Rust had a C union equivalent.
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
    signo: c_int,
    errno: c_int,
    code: c_int,

    // Rust doesn't currently support unions for FFI's
    // the best option is to have a generic pad and use
    // transmute to change between types
    // No, you can't just use enums, that would be too easy
    pad: [c_int; SI_PAD_SIZE]
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
            flags: SA_RESTART | SA_SIGINFO,
            restorer: 0
        }
    }

    pub fn ignore() -> SigAction {
        SigAction {
            handler: unsafe {mem::transmute::<usize, SigHandler>(1)},
            mask: full_sigset().unwrap_or([0; SIGSET_NWORDS]),
            flags: SA_RESTART | SA_SIGINFO,
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
    // included for completeness, not currently used
    #[allow(dead_code)]
    pub fn determine_sigfields(&self) -> SigFields {
        unsafe {
            match self.signo {
                SIGKILL => SigFields::Kill(mem::transmute(self.pad)),
                SIGALRM | SIGVTALRM | SIGPROF => SigFields::PTimer(mem::transmute(self.pad)),
                SIGCHLD => SigFields::SigChld(mem::transmute(self.pad)),
                SIGILL | SIGFPE | SIGSEGV | SIGBUS => SigFields::SigFault(mem::transmute(self.pad)),
                SIGPOLL => SigFields::SigPoll(mem::transmute(self.pad)),
                SIGSYS => SigFields::SigSys(mem::transmute(self.pad)),
                _ => SigFields::PSignal(mem::transmute(self.pad))
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

pub fn full_sigset() -> Option<SigSet> {
    let mut output:SigSet = [0; SIGSET_NWORDS];
    unsafe {
        match sigfillset(&mut output as *mut SigSet) {
            0 => Some(output),
            _ => None
        }
    }
}
