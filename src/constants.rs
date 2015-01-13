// ignore unused constants
#![allow(dead_code)]

use libc::*;

// standard file descriptiors
pub const STDIN:c_int  = 0;
pub const STDOUT:c_int = 1;
pub const STDERR:c_int = 2;

// constants for control sequences is useful
pub const CEOF:char = '\u{4}';
pub const DEL:char = '\u{7f}';
pub const NL:char  = '\n';
pub const ESC:char = '\u{1b}';
pub const ANSI:char = '\u{5b}';
pub const BS:char = '\u{8}';
pub const SPC:char = ' ';
pub const BEL:char = '\u{7}';
// we specifically need a constant for "cursor right"
pub const CRSR_RIGHT:&'static str = "\u{1b}\u{5b}C";

// select termios constants that we use
pub const ICANON:c_uint   = 2;
pub const ECHO:c_uint     = 8;
pub const TCSANOW:c_int   = 0;
pub const TCSADRAIN:c_int = 1;
pub const TCSAFLUSH:c_int = 2;

// signal values
pub const SIGHUP:c_int    = 1;       /* Hangup (POSIX).  */
//pub const SIGINT:c_int    = 2;       /* Interrupt (ANSI).  */
pub const SIGQUIT:c_int   = 3;       /* Quit (POSIX).  */
pub const SIGILL:c_int    = 4;       /* Illegal instruction (ANSI).  */
pub const SIGTRAP:c_int   = 5;       /* Trace trap (POSIX).  */
pub const SIGABRT:c_int   = 6;       /* Abort (ANSI).  */
pub const SIGIOT:c_int    = 6;       /* IOT trap (4.2 BSD).  */
pub const SIGBUS:c_int    = 7;       /* BUS error (4.2 BSD).  */
pub const SIGFPE:c_int    = 8;       /* Floating-point exception (ANSI).  */
//pub const SIGKILL:c_int   = 9;       /* Kill, unblockable (POSIX).  */
pub const SIGUSR1:c_int   = 10;      /* User-defined signal 1 (POSIX).  */
pub const SIGSEGV:c_int   = 11;      /* Segmentation violation (ANSI).  */
pub const SIGUSR2:c_int   = 12;      /* User-defined signal 2 (POSIX).  */
//pub const SIGPIPE:c_int   = 13;      /* Broken pipe (POSIX).  */
pub const SIGALRM:c_int   = 14;      /* Alarm clock (POSIX).  */
//pub const SIGTERM:c_int   = 15;      /* Termination (ANSI).  */
pub const SIGSTKFLT:c_int = 16;      /* Stack fault.  */
pub const SIGCLD:c_int    = SIGCHLD; /* Same as SIGCHLD (System V).  */
pub const SIGCHLD:c_int   = 17;      /* Child status has changed (POSIX).  */
pub const SIGCONT:c_int   = 18;      /* Continue (POSIX).  */
pub const SIGSTOP:c_int   = 19;      /* Stop, unblockable (POSIX).  */
pub const SIGTSTP:c_int   = 20;      /* Keyboard stop (POSIX).  */
pub const SIGTTIN:c_int   = 21;      /* Background read from tty (POSIX).  */
pub const SIGTTOU:c_int   = 22;      /* Background write to tty (POSIX).  */
pub const SIGURG:c_int    = 23;      /* Urgent condition on socket (4.2 BSD).  */
pub const SIGXCPU:c_int   = 24;      /* CPU limit exceeded (4.2 BSD).  */
pub const SIGXFSZ:c_int   = 25;      /* File size limit exceeded (4.2 BSD).  */
pub const SIGVTALRM:c_int = 26;      /* Virtual alarm clock (4.2 BSD).  */
pub const SIGPROF:c_int   = 27;      /* Profiling alarm clock (4.2 BSD).  */
pub const SIGWINCH:c_int  = 28;      /* Window size change (4.3 BSD, Sun).  */
pub const SIGPOLL:c_int   = SIGIO;   /* Pollable event occurred (System V).  */
pub const SIGIO:c_int     = 29;      /* I/O now possible (4.2 BSD).  */
pub const SIGPWR:c_int    = 30;      /* Power failure restart (System V).  */
pub const SIGSYS:c_int    = 31;      /* Bad system call.  */
pub const SIGUNUSED:c_int = 31;

// sizeof(int) = 4
// sizeof(unsigned long int) = 8
pub const SI_MAX_SIZE:usize = 128;
pub const SI_PAD_SIZE:usize = (SI_MAX_SIZE / 4)  - 4;
pub const SIGSET_NWORDS:usize = (1024 / (8 * 8));

pub const SA_NOCLDSTOP:c_int = 1;
pub const SA_NOCLDWAIT:c_int = 2;
pub const SA_SIGINFO:c_int   = 4;
pub const SA_RESTART:c_int   = 0x10000000;

pub const RTLD_LOCAL:c_int = 0;
pub const RTLD_LAZY:c_int = 1;

pub const WASH_RUN_SYMBOL:&'static str = "wash_run";
pub const WASH_LOAD_SYMBOL:&'static str = "wash_load";
pub const WO_PATH:&'static str = "/tmp/wash/";

pub const NCCS:usize = 32;
