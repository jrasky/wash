extern crate libc;

use self::libc::{c_uint, c_int, c_short, c_long, c_ulong};
use self::libc::{clock_t, pid_t, uid_t, size_t};
use std::mem;

use constants::*;

// This could also be a pointer,
// but it probably isn't
pub type SigVal = c_int;

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

// The size_t at the end is a pointer to a ucontext_t
// do I want to implement that type? Lol no
// Protip: we're calling sigaction with SA_SIGINFO
// In part because I didn't realize I didn't have to implement
// all of SigInfo, so I did, and now I want to use that code dammit!
pub type SigHandler = extern fn(c_int, *const SigInfo, size_t);
pub type SigSet = [c_ulong; SIGSET_NWORDS];

#[repr(C)]
#[derive(Copy)]
pub struct SigAction {
    pub handler: SigHandler,
    pub mask: SigSet,
    pub flags: c_int
}

impl SigInfo {
    #[allow(dead_code)]
    pub fn determine_sigfields(&self) -> SigFields {
        unsafe {
            match self.signo {
                SIGKILL => return SigFields::Kill(mem::transmute(self.pad)),
                SIGALRM | SIGVTALRM | SIGPROF => return SigFields::PTimer(mem::transmute(self.pad)),
                SIGCHLD => return SigFields::SigChld(mem::transmute(self.pad)),
                SIGILL | SIGFPE | SIGSEGV | SIGBUS => return SigFields::SigFault(mem::transmute(self.pad)),
                SIGPOLL => return SigFields::SigPoll(mem::transmute(self.pad)),
                SIGSYS => return SigFields::SigSys(mem::transmute(self.pad)),
                _ => return SigFields::PSignal(mem::transmute(self.pad))
            }
        }
    }
}

#[link(name="c")]
extern {
    pub fn sigaction(signum:c_int, act:&SigAction, oldact:*mut SigAction) -> c_int;
    pub fn sigfillset(mask:*mut SigSet) -> c_int;
}
