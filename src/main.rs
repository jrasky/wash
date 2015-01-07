#![feature(globs)]
extern crate libc;

use libc::{c_int, size_t};
use std::io;
use std::ptr;
use std::io::process::{Command, StdioContainer};

use termios::*;
use signal::*;
use constants::*;

mod termios;
mod signal;
mod constants;

#[allow(unused_variables)]
extern fn handle_sigint(signum:c_int, siginfo:*const SigInfo, context:size_t) {
    print!("^C");
    io::stdio::flush();
}

fn prepare_terminal(tios:&mut Termios) {
    tios.ldisable(ICANON);
    tios.ldisable(ECHO);
}

fn update_terminal(tios:Termios) -> bool {
    if !tios.set() {
        io::stderr().write_line("Warning: Could not set terminal mode").unwrap();
        return false;
    }
    return true;
}

fn handle_escape(stdin:&mut io::stdio::StdinReader,
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
                    cursor_left();
                },
                None => match line.pop() {
                    Some(s) => {
                        part.push(' ');
                        word.clear();
                        word.push_str(s.as_slice());
                        cursor_left();
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
                    cursor_right();
                },
                Some(c) => {
                    if !bpart.is_empty() {
                        bpart.clear();
                    }
                    word.push(c);
                    cursor_right();
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

fn cursor_left() {
    print!("{}", DEL);
}

fn cursor_right() {
    print!("{}{}C", ESC, ANSI);
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

fn prepare_signals() {
    let mut sa = SigAction {
        handler: handle_sigint,
        mask: [0; SIGSET_NWORDS],
        flags: SA_RESTART | SA_SIGINFO
    };
    unsafe {
        if sigfillset(&mut sa.mask) != 0 {
            io::stderr().write_line("Warning: could not fill mask set for SIGINT handler").unwrap();
        }
        if sigaction(SIGINT, &sa, ptr::null_mut::<SigAction>()) != 0 {
            io::stderr().write_line("Warning: could not set handler for SIGINT").unwrap();
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
            io::stderr().write_line(format!("Couldn't spawn {}: {}", &line[0], e).as_slice()).unwrap();
            return;
        },
        Ok(child) => child
    };
    match child.wait() {
        Err(e) => {
            io::stderr().write_line(format!("Couldn't wait for child to exit: {}", e.desc).as_slice()).unwrap();
        },
        Ok(_) => {
            // nothing
        }
    };
}

fn main() {
    prepare_signals();
    let mut tios = Termios::new();
    tios.get();
    let old_tios = tios.clone();
    prepare_terminal(&mut tios);
    update_terminal(tios);
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut line = Vec::<String>::new();
    let mut word = String::new();
    let mut part = String::new();
    // store bpart so we don't need to recalulate it every time
    let mut bpart = String::new();
    loop {
        // Note: in non-canonical mode
        match stdin.read_char() {
            Ok(EOF) => {
                if line.is_empty() && word.is_empty() {
                    break;
                }
            },
            Ok(NL) => {
                stdout.write_char(NL).unwrap();
                if !word.is_empty() {
                    line.push(word.clone());
                }
                if !line.is_empty() {
                    // run command
                    update_terminal(old_tios);
                    run_command(&line);
                    update_terminal(tios);
                }
                word.clear();
                line.clear();
                part.clear();
                bpart.clear();
            },
            Ok(DEL) => {
                if word.is_empty() {
                    word = match line.pop() {
                        Some(s) => s,
                        None => continue
                    };
                    cursor_left();
                } else {
                    word.pop();
                    cursor_left();
                    draw_part(&part, &mut bpart);
                    stdout.write_char(NL).unwrap();
                    cursors_left(part.len() + 1);
                }
            },
            Ok(ESC) => handle_escape(&mut stdin, &mut line,
                                     &mut word, &mut part,
                                     &mut bpart),
            Ok(SPC) => {
                line.push(word.clone());
                word.clear();
                stdout.write_char(SPC).unwrap();
                idraw_part(&part, &mut bpart);
            },
            Ok(c) => {
                word.push(c);
                stdout.write_char(c).unwrap();
                idraw_part(&part, &mut bpart);
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
    update_terminal(old_tios);
}
