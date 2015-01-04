// This needs to be here
#![allow(dead_code)]

extern crate libc;
extern crate term;

use libc::{c_uint, c_uchar, c_int, c_short, c_long, c_ulong};
use libc::{clock_t, pid_t, uid_t, size_t};
use std::io;
use std::mem;
use std::ptr;

// used in Termios struct
const NCCS:uint = 32;

// stdin, stdout, stderr have standard file descriptors
const STDIN:c_int = 0;

// constants for control sequences is useful
const EOF:char = '\u{4}';
const DEL:char = '\u{7f}';
const NL:char  = '\n';
const ESC:char = '\u{1b}';
const ANSI:char = '\u{5b}';
const BS:char = '\u{8}';

// select termios constants that we use
const ICANON:c_uint   = 2;
const ECHO:c_uint     = 8;
const TCSANOW:c_int   = 0;
const TCSADRAIN:c_int = 1;
const TCSAFLUSH:c_int = 2;

// signal values
const SIGHUP:c_int    = 1;       /* Hangup (POSIX).  */
const SIGINT:c_int    = 2;       /* Interrupt (ANSI).  */
const SIGQUIT:c_int   = 3;       /* Quit (POSIX).  */
const SIGILL:c_int    = 4;       /* Illegal instruction (ANSI).  */
const SIGTRAP:c_int   = 5;       /* Trace trap (POSIX).  */
const SIGABRT:c_int   = 6;       /* Abort (ANSI).  */
const SIGIOT:c_int    = 6;       /* IOT trap (4.2 BSD).  */
const SIGBUS:c_int    = 7;       /* BUS error (4.2 BSD).  */
const SIGFPE:c_int    = 8;       /* Floating-point exception (ANSI).  */
const SIGKILL:c_int   = 9;       /* Kill, unblockable (POSIX).  */
const SIGUSR1:c_int   = 10;      /* User-defined signal 1 (POSIX).  */
const SIGSEGV:c_int   = 11;      /* Segmentation violation (ANSI).  */
const SIGUSR2:c_int   = 12;      /* User-defined signal 2 (POSIX).  */
const SIGPIPE:c_int   = 13;      /* Broken pipe (POSIX).  */
const SIGALRM:c_int   = 14;      /* Alarm clock (POSIX).  */
const SIGTERM:c_int   = 15;      /* Termination (ANSI).  */
const SIGSTKFLT:c_int = 16;      /* Stack fault.  */
const SIGCLD:c_int    = SIGCHLD; /* Same as SIGCHLD (System V).  */
const SIGCHLD:c_int   = 17;      /* Child status has changed (POSIX).  */
const SIGCONT:c_int   = 18;      /* Continue (POSIX).  */
const SIGSTOP:c_int   = 19;      /* Stop, unblockable (POSIX).  */
const SIGTSTP:c_int   = 20;      /* Keyboard stop (POSIX).  */
const SIGTTIN:c_int   = 21;      /* Background read from tty (POSIX).  */
const SIGTTOU:c_int   = 22;      /* Background write to tty (POSIX).  */
const SIGURG:c_int    = 23;      /* Urgent condition on socket (4.2 BSD).  */
const SIGXCPU:c_int   = 24;      /* CPU limit exceeded (4.2 BSD).  */
const SIGXFSZ:c_int   = 25;      /* File size limit exceeded (4.2 BSD).  */
const SIGVTALRM:c_int = 26;      /* Virtual alarm clock (4.2 BSD).  */
const SIGPROF:c_int   = 27;      /* Profiling alarm clock (4.2 BSD).  */
const SIGWINCH:c_int  = 28;      /* Window size change (4.3 BSD, Sun).  */
const SIGPOLL:c_int   = SIGIO;   /* Pollable event occurred (System V).  */
const SIGIO:c_int     = 29;      /* I/O now possible (4.2 BSD).  */
const SIGPWR:c_int    = 30;      /* Power failure restart (System V).  */
const SIGSYS:c_int    = 31;      /* Bad system call.  */
const SIGUNUSED:c_int = 31;

// sizeof(int) = 4
// sizeof(unsigned long int) = 8
const SI_MAX_SIZE:uint = 128;
const SI_PAD_SIZE:uint = (SI_MAX_SIZE / 4)  - 4;
const SIGSET_NWORDS:uint = (1024 / (8 * 8));

const SA_NOCLDSTOP:c_int = 1;
const SA_NOCLDWAIT:c_int = 2;
const SA_SIGINFO:c_int   = 4;
const SA_RESTART:c_int   = 0x10000000;

// types used in Termios struct
type CCType = c_uchar;
type SpeedType = c_uint;
type TCFlag = c_uint;

#[repr(C)]
#[deriving(Copy)]
#[deriving(Clone)]
struct Termios {
    iflag: TCFlag,
    oflag: TCFlag,
    cflag: TCFlag,
    lflag: TCFlag,
    line: CCType,
    cc: [c_uchar, ..NCCS],
    ispeed: SpeedType,
    ospeed: SpeedType,
}

impl Termios {
    fn new() -> Termios {
        Termios {
            cc: [0, ..NCCS],
            cflag: 0,
            iflag: 0,
            ispeed: 0,
            lflag: 0,
            line: 0,
            oflag: 0,
            ospeed: 0,
        }
    }

    fn get_from(&mut self, fd:c_int) -> bool {
        unsafe {
            return tcgetattr(fd, self) == 0;
        }
    }

    fn get(&mut self) -> bool {
        self.get_from(STDIN)
    }

    fn set_to(&self, fd:c_int) -> bool {
        unsafe {
            return tcsetattr(fd, TCSANOW, self) == 0;
        }
    }

    fn set(&self) -> bool {
        self.set_to(STDIN)
    }

    fn lenable(&mut self, flag:c_uint) {
        self.lflag |= flag;
    }

    fn ldisable(&mut self, flag:c_uint) {
        self.lflag &= !flag;
    }
}

#[link(name = "c")]
extern {
    fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
    fn tcsetattr(fd: c_int, optional_actions: c_int, termios: *const Termios) -> c_int;
}

// This could also be a pointer,
// but it probably isn't
type SigVal = c_int;

// _pad is needed to make all these types the same size
// this is the case because rust doesn't have an equivalent to C unions
// it's really a bitch
// at least let me transmute between different sized types I mean jesus christ

struct KillFields {
    pid: pid_t,
    uid: uid_t,
    _pad: [c_int, ..(SI_PAD_SIZE - 2)]
}

struct PTimerFields {
    tid: c_int,
    overrun: c_int,
    sigval: SigVal,
    _pad: [c_int, ..(SI_PAD_SIZE - 3)]
}

struct PSignalFields {
    pid: pid_t,
    uid: uid_t,
    sigval: SigVal,
    _pad: [c_int, ..(SI_PAD_SIZE - 3)]
}

struct SigChldFields {
    pid: pid_t,
    uid: uid_t,
    status: c_int,
    utime: clock_t,
    stime: clock_t,
    _pad: [c_int, ..(SI_PAD_SIZE - 8)]
}

struct SigFaultFields {
    addr: size_t, // pointer
    addr_lsb: c_short,
    _pad: [c_int, ..(SI_PAD_SIZE - 3)]
}

struct SigPollFields {
    band: c_long,
    fd: c_int,
    _pad: [c_int, ..(SI_PAD_SIZE - 3)]
}

struct SigSysFields {
    _call_addr: size_t, // pointer
    _syscall: c_int,
    _arch: c_uint,
    _pad: [c_int, ..(SI_PAD_SIZE - 4)]
}

// If only Rust had a C union equivalent...
enum SigFields {
    Kill(KillFields),
    PTimer(PTimerFields),
    PSignal(PSignalFields),
    SigChld(SigChldFields),
    SigFault(SigFaultFields),
    SigPoll(SigPollFields),
    SigSys(SigSysFields)
}

#[repr(C)]
struct SigInfo {
    signo: c_int,
    errno: c_int,
    code: c_int,

    // Rust doesn't currently support unions for FFI's
    // the best option is to have a generic pad and use
    // transmute to change between types
    // No, you can't just use enums, that would be too easy
    pad: [c_int, ..SI_PAD_SIZE]
}

// The size_t at the end is a pointer to a ucontext_t
// do I want to implement that type? Lol no
// Protip: we're calling sigaction with SA_SIGINFO
// In part because I didn't realize I didn't have to implement
// all of SigInfo, so I did, and now I want to use that code dammit!
type SigHandler = extern fn(c_int, *const SigInfo, size_t);
type SigSet = [c_ulong, ..SIGSET_NWORDS];

#[repr(C)]
struct SigAction {
    handler: SigHandler,
    mask: SigSet,
    flags: c_int
}

impl SigInfo {
    fn determine_sigfields(&self) -> SigFields {
        use SigFields::{Kill, PTimer, PSignal, SigChld, SigFault, SigPoll, SigSys};
        unsafe {
            match self.signo {
                SIGKILL => return Kill(mem::transmute(self.pad)),
                SIGALRM | SIGVTALRM | SIGPROF => return PTimer(mem::transmute(self.pad)),
                SIGCHLD => return SigChld(mem::transmute(self.pad)),
                SIGILL | SIGFPE | SIGSEGV | SIGBUS => return SigFault(mem::transmute(self.pad)),
                SIGPOLL => return SigPoll(mem::transmute(self.pad)),
                SIGSYS => return SigSys(mem::transmute(self.pad)),
                _ => return PSignal(mem::transmute(self.pad))
            }
        }
    }
}

#[link(name="c")]
extern {
    fn sigaction(signum:c_int, act:&SigAction, oldact:*mut SigAction) -> c_int;
    fn sigfillset(mask:*mut SigSet);
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

#[allow(unused_variables)]
extern fn handle_sigint(signum:c_int, siginfo:*const SigInfo, context:size_t) {
    println!("Caught SIGINT");
}

fn prepare_terminal() -> Termios {
    // new terminal mode info
    let mut tios = Termios::new();
    // populate terminal info
    tios.get();
    let tios_clone = tios.clone();
    // turn off canonical mode
    tios.ldisable(ICANON);
    // turn off echo mode
    tios.ldisable(ECHO);
    // set the terminal mode
    update_terminal(tios);
    // return the old terminal mode
    return tios_clone;
}

fn update_terminal(tios:Termios) -> bool {
    if !tios.set() {
        io::stderr().write_line("Warning: Could not set terminal mode").unwrap();
        return false;
    }
    return true;
}

fn handle_escape(stdin:&mut io::stdio::StdinReader,
                 line:&mut String, part:&mut String) {
    // Handle an ANSI escape sequence
    if stdin.read_char() != Ok(ANSI) {
        return;
    }
    match stdin.read_char() {
        Err(_) => return,
        Ok('D') => {
            match line.pop() {
                Some(c) => {
                    part.push(c);
                    cursor_left();
                },
                None => return
            }
        },
        Ok('C') => {
            match part.pop() {
                Some(c) => {
                    line.push(c);
                    cursor_right();
                },
                None => return
            }
        },
        Ok(_) => return
    }
}

fn redraw_line(line:&String, pad_to:uint) {
    // TODO: make this more optimized
    print!("\r{}", line);
    if pad_to > line.len() {
        let pad_amount = pad_to - line.len();
        print!("{}", String::from_char(pad_amount, ' '));
    }

}

fn cursor_left() {
    print!("{}", DEL);
}

fn cursor_right() {
    print!("{}{}C", ESC, ANSI);
}

fn draw_part(part:&String) {
    // quick out if part is empty
    if part.is_empty() {
        return;
    }
    let mut cpart = part.clone();
    let mut rpart = String::new();
    loop {
        match cpart.pop() {
            Some(c) => rpart.push(c),
            None => break
        }
    }
    print!("{}", rpart);
}

fn cursors_left(by:uint) {
    // move back by a given number of characters
    print!("{}", String::from_char(by, DEL));
}

fn idraw_part(part:&String) {
    // in-place draw of the line part
    draw_part(part);
    cursors_left(part.len());
}

fn main() {
    let mut sa = SigAction {
        handler: handle_sigint,
        mask: [0, ..SIGSET_NWORDS],
        flags: SA_RESTART | SA_SIGINFO
    };
    unsafe {
        sigfillset(&mut sa.mask);
        assert!(sigaction(SIGINT, &sa, ptr::null_mut::<SigAction>()) == 0);
    }
    let old_tios = prepare_terminal();
    let mut stdin = io::stdin();
    let mut line = String::new();
    let mut part = String::new();
    loop {
        // Note: in non-canonical mode
        match stdin.read_char() {
            Ok(EOF) => break,
            Ok(NL) => {
                line.clear();
                part.clear();
                print!("\n");
            },
            Ok(DEL) => {
                if line.is_empty() {
                    continue;
                }
                line.pop();
                cursor_left();
                draw_part(&part);
                print!(" ");
                cursors_left(part.len() + 1);
            },
            Ok(ESC) => handle_escape(&mut stdin, &mut line, &mut part),
            Ok(c) => {
                line.push(c);
                print!("{}", c);
                idraw_part(&part);
            },
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }
    // print so we know we've reached this code
    println!("Exiting");
    // restore old term state
    update_terminal(old_tios);
}
