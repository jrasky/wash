#![feature(globs)]
extern crate libc;

use libc::{c_int, size_t};
use std::io;
use std::ptr;

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

fn prepare_terminal() -> Termios {
    let mut tios = Termios::new();
    tios.get();
    let tios_clone = tios.clone();
    tios.ldisable(ICANON);
    tios.ldisable(ECHO);
    update_terminal(tios);
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
                 line:&mut String, part:&mut String,
                 bpart:&mut String) {
    // Handle an ANSI escape sequence
    if stdin.read_char() != Ok(ANSI) {
        return;
    }
    match stdin.read_char() {
        Err(_) => return,
        Ok('D') => {
            match line.pop() {
                Some(c) => {
                    if !bpart.is_empty() {
                        bpart.clear();
                    }
                    part.push(c);
                    cursor_left();
                },
                None => return
            }
        },
        Ok('C') => {
            match part.pop() {
                Some(c) => {
                    if !bpart.is_empty() {
                        bpart.clear();
                    }
                    line.push(c);
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

fn main() {
    prepare_signals();
    let old_tios = prepare_terminal();
    let mut stdin = io::stdin();
    let mut line = String::new();
    let mut part = String::new();
    // store bpart so we don't need to recalulate it every time
    let mut bpart = String::new();
    loop {
        // Note: in non-canonical mode
        match stdin.read_char() {
            Ok(EOF) => break,
            Ok(NL) => {
                line.clear();
                part.clear();
                bpart.clear();
                print!("\n");
            },
            Ok(DEL) => {
                if line.is_empty() {
                    continue;
                }
                line.pop();
                cursor_left();
                draw_part(&part, &mut bpart);
                print!(" ");
                cursors_left(part.len() + 1);
            },
            Ok(ESC) => handle_escape(&mut stdin, &mut line,
                                     &mut part, &mut bpart),
            Ok(c) => {
                line.push(c);
                print!("{}", c);
                idraw_part(&part, &mut bpart);
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
