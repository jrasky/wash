#![feature(old_io)]
use std::old_io;

fn empty_escape(esc:&mut Iterator<Item=char>) -> String {
    let mut out = String::new();
    loop {
        match esc.next() {
            Some(c) => out.push(c),
            None => break
        }
    }
    return out;
}

pub fn main() {
    let mut stdin = old_io::stdin();

    print!("Type a key: ");
    let c = stdin.read_char().unwrap();
    println!("In escaped form: {}", empty_escape(&mut c.escape_default()));
}
