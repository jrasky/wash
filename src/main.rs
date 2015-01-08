extern crate libc;

use libc::{c_int, size_t};
use std::io;
use std::ptr;
use std::mem;
use std::io::process::{Command, StdioContainer};

use termios::*;
use signal::*;
use constants::*;

mod termios;
mod signal;
mod constants;

type Stdr = io::stdio::StdinReader;
type Stdw = io::LineBufferedWriter<io::stdio::StdWriter>;

// start off as null pointer
static mut instate_location:size_t = 0;

struct InputState {
    line: Vec<String>,
    word: String,
    part: String,
    bpart: String,
    stdin: Stdr,
    stdout: Stdw,
    stderr: Stdw
}

impl InputState {
    fn new() -> InputState {
        InputState {
            line: Vec::<String>::new(),
            word: String::new(),
            part: String::new(),
            bpart: String::new(),
            stdin: io::stdin(),
            stdout: io::stdout(),
            stderr: io::stderr()
        }
    }

    fn clear(&mut self) {
        self.word.clear();
        self.line.clear();
        self.part.clear();
        self.bpart.clear();
    }

    fn flush(&mut self) {
        self.stdout.flush().unwrap();
    }

    fn outw(&mut self, msg:&str) {
        self.stdout.write_str(msg).unwrap();
    }

    fn outc(&mut self, ch:char) {
        self.stdout.write_char(ch).unwrap();
    }

    fn err(&mut self, msg:&str) {
        self.stderr.write_str(msg).unwrap();
    }
}

#[allow(unused_variables)]
unsafe extern fn handle_sigint(signum:c_int, siginfo:*const SigInfo, context:size_t) {
    if instate_location == 0 {
        panic!("Shell state location uninitialized");
    }
    print!("^C\n");
    // Below: completely disregarding everything Rust stands for
    let state:&mut InputState = mem::transmute(instate_location);
    state.clear();
    io::stdio::flush();
}

fn set_instate_location(state:&mut InputState) {
    unsafe {
        if instate_location != 0 {
            panic!("Tried to set shell state location twice");
        }
        instate_location = mem::transmute::<&mut InputState, size_t>(state);
    }
}

fn prepare_terminal(tios:&mut Termios) {
    tios.ldisable(ICANON);
    tios.ldisable(ECHO);
}

fn update_terminal(tios:Termios, state:&mut InputState) -> bool {
    if !tios.set() {
        state.err("Warning: Could not set terminal mode\n");
        return false;
    }
    return true;
}

fn handle_escape(state:&mut InputState) {
    // Handle an ANSI escape sequence
    if state.stdin.read_char() != Ok(ANSI) {
        return;
    }
    match state.stdin.read_char() {
        Err(_) => return,
        Ok('D') => {
            // left
            match state.word.pop() {
                Some(c) => {
                    state.bpart.clear();
                    state.part.push(c);
                    cursor_left(state);
                },
                None => match state.line.pop() {
                    Some(s) => {
                        state.bpart.clear();
                        state.part.push(' ');
                        state.word.clear();
                        state.word.push_str(s.as_slice());
                        cursor_left(state);
                    }
                    None => return
                }
            }
        },
        Ok('C') => {
            // right
            match state.part.pop() {
                Some(' ') => {
                    state.bpart.clear();
                    state.line.push(state.word.clone());
                    state.word.clear();
                    cursor_right(state);
                },
                Some(c) => {
                    state.bpart.clear();
                    state.word.push(c);
                    cursor_right(state);
                },
                None => return
            }
        },
        Ok(_) => return
    }
}

fn cursor_left(state:&mut InputState) {
    state.outc(DEL);
}

fn cursor_right(state:&mut InputState) {
    state.outw(CRSR_RIGHT);
}

fn draw_part(state:&mut InputState) {
    // quick out if part is empty
    if state.part.is_empty() {
        return;
    }
    if state.bpart.is_empty() {
        // only calculate bpart when it needs to be recalculated
        let mut cpart = state.part.clone();
        loop {
            match cpart.pop() {
                Some(c) => state.bpart.push(c),
                None => break
            }
        }
    }
    let c = state.bpart.clone();
    state.outw(c.as_slice());
}

// work around lack of DST
fn build_string(ch:char, count:uint) -> String {
    let mut s = String::new();
    let mut i = 0u;
    loop {
        if i == count {
            return s;
        }
        s.push(ch);
        i += 1;
    }
}

fn cursors_left(by:uint, state:&mut InputState) {
    // move back by a given number of characters
    state.outw(build_string(DEL, by).as_slice());
}

fn idraw_part(state:&mut InputState) {
    // in-place draw of the line part
    draw_part(state);
    cursors_left(state.part.len(), state);
}

fn prepare_signals(state:&mut InputState) {
    let mut sa = SigAction {
        handler: handle_sigint,
        mask: [0; SIGSET_NWORDS],
        flags: SA_RESTART | SA_SIGINFO
    };
    unsafe {
        if sigfillset(&mut sa.mask) != 0 {
            state.err("Warning: could not fill mask set for SIGINT handler\n");
        }
        if sigaction(SIGINT, &sa, ptr::null_mut::<SigAction>()) != 0 {
            state.err("Warning: could not set handler for SIGINT\n");
        }
    }
}

fn run_command(state:&mut InputState) {
    let mut process = Command::new(&state.line[0]);
    process.args(state.line.slice_from(1));
    process.stdout(StdioContainer::InheritFd(STDOUT));
    process.stdin(StdioContainer::InheritFd(STDIN));
    process.stderr(StdioContainer::InheritFd(STDERR));
    let mut child = match process.spawn() {
        Err(e) => {
            let s = state.line[0].clone();
            state.err(format!("Couldn't spawn {}: {}\n", s, e).as_slice());
            return;
        },
        Ok(child) => child
    };
    match child.wait() {
        Err(e) => {
            state.err(format!("Couldn't wait for child to exit: {}\n", e.desc).as_slice());
        },
        Ok(_) => {
            // nothing
        }
    };
}

fn main() {
    let mut state = &mut InputState::new();
    prepare_signals(state);
    let mut tios = Termios::new();
    tios.get();
    let old_tios = tios.clone();
    prepare_terminal(&mut tios);
    update_terminal(tios, state);

    set_instate_location(state);
    loop {
        // Note: in non-canonical mode
        match state.stdin.read_char() {
            Ok(EOF) => {
                if state.line.is_empty() && state.word.is_empty() {
                    break;
                }
            },
            Ok(NL) => {
                // start command output on next line
                state.outc(NL);
                // push any remaining word onto the line
                if !state.word.is_empty() {
                    state.line.push(state.word.clone());
                }
                // debug info
                let numargs = state.line.len();
                state.outw(format!("Number of arguments: {}\n", numargs).as_slice());
                // run command if one was specified
                if !state.line.is_empty() {
                    // run command
                    update_terminal(old_tios, state);
                    run_command(state);
                    update_terminal(tios, state);
                }
                // clear the state
                state.clear();
            },
            Ok(DEL) => {
                if state.word.is_empty() {
                    state.word = match state.line.pop() {
                        Some(s) => s,
                        None => continue
                    };
                } else {
                    state.word.pop();
                }
                cursor_left(state);
                draw_part(state);
                state.outc(SPC);
                cursors_left(state.part.len() + 1, state);
            },
            Ok(ESC) => handle_escape(state),
            Ok(SPC) => {
                state.line.push(state.word.clone());
                state.word.clear();
                state.outc(SPC);
                idraw_part(state);
            },
            Ok(c) => {
                state.word.push(c);
                state.outc(c);
                idraw_part(state);
            },
            Err(e) => {
                state.err(format!("Error: {}\n", e).as_slice());
                break;
            }
        }
        // flush output
        state.flush();
    }
    state.outw("Exiting\n");
    // restore old term state
    update_terminal(old_tios, state);
}
