#![feature(globs)]
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
    bpart: String
}

impl InputState {
    fn new() -> InputState {
        InputState {
            line: Vec::<String>::new(),
            word: String::new(),
            part: String::new(),
            bpart: String::new()
        }
    }

    fn clear(&mut self) {
        self.word.clear();
        self.line.clear();
        self.part.clear();
        self.bpart.clear();
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

fn update_terminal(tios:Termios, stderr:&mut Stdw) -> bool {
    if !tios.set() {
        stderr.write_str("Warning: Could not set terminal mode\n").unwrap();
        return false;
    }
    return true;
}

fn handle_escape(stdin:&mut Stdr, stdout:&mut Stdw,
                 line:&mut Vec<String>, word:&mut String,
                 part:&mut String, bpart:&mut String) {
    // Handle an ANSI escape sequence
    if stdin.read_char() != Ok(ANSI) {
        return;
    }
    match stdin.read_char() {
        Err(_) => return,
        Ok('D') => {
            // left
            match word.pop() {
                Some(c) => {
                    if !bpart.is_empty() {
                        bpart.clear();
                    }
                    part.push(c);
                    cursor_left(stdout);
                },
                None => match line.pop() {
                    Some(s) => {
                        part.push(' ');
                        word.clear();
                        word.push_str(s.as_slice());
                        cursor_left(stdout);
                    }
                    None => return
                }
            }
        },
        Ok('C') => {
            // right
            match part.pop() {
                Some(' ') => {
                    if !bpart.is_empty() {
                        bpart.clear();
                    }
                    line.push(word.clone());
                    word.clear();
                    cursor_right(stdout);
                },
                Some(c) => {
                    if !bpart.is_empty() {
                        bpart.clear();
                    }
                    word.push(c);
                    cursor_right(stdout);
                },
                None => return
            }
        },
        Ok(_) => return
    }
}

fn build_string(ch:char, count:uint) -> String {
    let mut s = String::new();
    let mut i = 0u;
    loop {
        if i == count {
            return s;
        }
        i += 1;
        s.push(ch);
    }
}

fn cursor_left(stdout:&mut Stdw) {
    stdout.write_char(DEL).unwrap();
}

fn cursor_right(stdout:&mut Stdw) {
    stdout.write(&[ESC as u8, ANSI as u8, 'C' as u8]).unwrap();
}

fn draw_part(part:&String, bpart:&mut String) {
    // quick out if part is empty
    if part.is_empty() {
        return;
    }
    if bpart.is_empty() {
        // only calculate bpart when it needs to be recalculated
        let mut cpart = part.clone();
        loop {
            match cpart.pop() {
                Some(c) => bpart.push(c),
                None => break
            }
        }
    }
    print!("{}", bpart);
}

fn cursors_left(by:uint) {
    // move back by a given number of characters
    print!("{}", build_string(DEL, by));
}

fn idraw_part(part:&String, bpart:&mut String) {
    // in-place draw of the line part
    draw_part(part, bpart);
    cursors_left(part.len());
}

fn prepare_signals(stderr:&mut Stdw) {
    let mut sa = SigAction {
        handler: handle_sigint,
        mask: [0; SIGSET_NWORDS],
        flags: SA_RESTART | SA_SIGINFO
    };
    unsafe {
        if sigfillset(&mut sa.mask) != 0 {
            stderr.write_str("Warning: could not fill mask set for SIGINT handler\n").unwrap();
        }
        if sigaction(SIGINT, &sa, ptr::null_mut::<SigAction>()) != 0 {
            stderr.write_str("Warning: could not set handler for SIGINT\n").unwrap();
        }
    }
}

fn run_command(line:&Vec<String>) {
    let mut process = Command::new(&line[0]);
    process.args(line.slice_from(1));
    process.stdout(StdioContainer::InheritFd(STDOUT));
    process.stdin(StdioContainer::InheritFd(STDIN));
    process.stderr(StdioContainer::InheritFd(STDERR));
    let mut child = match process.spawn() {
        Err(e) => {
            io::stderr().write_fmt(format_args!("Couldn't spawn {}: {}\n", &line[0], e)).unwrap();
            return;
        },
        Ok(child) => child
    };
    match child.wait() {
        Err(e) => {
            io::stderr().write_fmt(format_args!("Couldn't wait for child to exit: {}\n", e.desc)).unwrap();
        },
        Ok(_) => {
            // nothing
        }
    };
}

fn main() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    prepare_signals(&mut stderr);
    let mut tios = Termios::new();
    tios.get();
    let old_tios = tios.clone();
    prepare_terminal(&mut tios);
    update_terminal(tios, &mut stderr);
    let mut state = InputState::new();
    set_instate_location(&mut state);
    loop {
        // Note: in non-canonical mode
        match stdin.read_char() {
            Ok(EOF) => {
                if state.line.is_empty() && state.word.is_empty() {
                    break;
                }
            },
            Ok(NL) => {
                // start command output on next line
                stdout.write_char(NL).unwrap();
                // push any remaining word onto the line
                if !state.word.is_empty() {
                    state.line.push(state.word.clone());
                }
                // run command if one was specified
                if !state.line.is_empty() {
                    // run command
                    update_terminal(old_tios, &mut stderr);
                    run_command(&state.line);
                    update_terminal(tios, &mut stderr);
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
                    cursor_left(&mut stdout);
                } else {
                    state.word.pop();
                    cursor_left(&mut stdout);
                    draw_part(&state.part, &mut state.bpart);
                    stdout.write_char(NL).unwrap();
                    cursors_left(state.part.len() + 1);
                }
            },
            Ok(ESC) => handle_escape(&mut stdin, &mut stdout,
                                     &mut state.line, &mut state.word,
                                     &mut state.part, &mut state.bpart),
            Ok(SPC) => {
                state.line.push(state.word.clone());
                state.word.clear();
                stdout.write_char(SPC).unwrap();
                idraw_part(&state.part, &mut state.bpart);
            },
            Ok(c) => {
                state.word.push(c);
                stdout.write_char(c).unwrap();
                idraw_part(&state.part, &mut state.bpart);
            },
            Err(e) => {
                stdout.write_fmt(format_args!("Error: {}\n", e)).unwrap();
                break;
            }
        }
        // flush output
        stdout.flush().unwrap();
    }
    stdout.write_str("Exiting\n").unwrap();
    // restore old term state
    update_terminal(old_tios, &mut stderr);
}
