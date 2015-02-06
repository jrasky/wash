use std::cmp::*;
use std::env;

use types::*;

#[macro_export]
macro_rules! tryp {
    ($e:expr) => ({
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{}", e),
        }
    })
}

#[macro_export]
macro_rules! tryf {
    ($e:expr, $($arg:tt)*) => ({
        match $e {
            Ok(e) => e,
            Err(e) => return Err(format!($($arg)*, err=e))
        }
    })
}

pub fn get_nm_index<T>(vec:&Vec<T>, index:usize) -> Option<&T> {
    if index >= vec.len() {
        return None;
    } else {
        return Some(&vec[index]);
    }
}

// work around lack of DST
pub fn build_string(ch:char, count:usize) -> String {
    let mut s = String::new();
    let mut i = 0us;
    loop {
        if i == count {
            return s;
        }
        s.push(ch);
        i += 1;
    }
}

pub fn reverse<T:Clone>(vec:Vec<T>) -> Vec<T> {
    let mut vec = vec.clone();
    let mut out = vec![];
    loop {
        match vec.pop() {
            None => break,
            Some(v) => out.push(v)
        }
    }
    return out;
}

pub fn expand_path(path:Path) -> Path {
    if Path::new("~").is_ancestor_of(&path) {
        return match env::home_dir() {
            None => Path::new("/"),
            Some(val) => Path::new(val)
        }.join(Path::new(&path.as_vec()[min(path.as_vec().len(), 2)..]));
    } else {
        return path;
    }
}

pub fn condense_path(path:Path) -> Path {
    let homep = Path::new(match env::home_dir() {
            None => return path,
            Some(val) => val
    });
    if homep.is_ancestor_of(&path) {
        match path.path_relative_from(&homep) {
            None => path,
            Some(path) => Path::new("~").join(path)
        }
    } else {
        return path;
    }
}


pub fn create_content(next:&mut Vec<InputValue>) -> Result<Vec<InputValue>, String> {
    let mut one_line = false;
    let mut line = vec![];
    loop {
        match next.pop() {
            Some(InputValue::Short(ref s)) if *s == "{".to_string() => break,
            Some(InputValue::Short(ref s)) if *s == "}".to_string() && !one_line => {
                // one-line block
                one_line = true;
            },
            Some(InputValue::Split(_)) if !one_line => continue,
            Some(v) => {
                if one_line {
                    line.insert(0, v);
                } else {
                    return Err("Malformed block".to_string())
                }
            },
            _ => return Err(format!("Malformed block"))
        }
    }
    if line.is_empty() {
        return Ok(vec![]);
    } else {
        return Ok(vec![InputValue::Long(line)]);
    }
}

#[test]
fn build_string_test() {
    assert!(build_string('a', 5) == String::from_str("aaaaa"));
}

#[test]
fn expand_path_test() {
    // tests require the HOME env set
    let homep = Path::new(os::homedir().unwrap());
    assert!(expand_path(Path::new("~/Documents/scripts/")) == homep.join("Documents/scripts/"));
    assert!(expand_path(Path::new("/etc/wash/")) == Path::new("/etc/wash/"));
}

#[test]
fn condense_path_test() {
    // tests require the HOME env set
    let homep = Path::new(os::homedir().unwrap());
    assert!(condense_path(homep.join("Documents/scripts/")) ==
            Path::new("~/Documents/scripts/"));
    assert!(condense_path(Path::new("/home/")) == Path::new("/home/"));
    assert!(condense_path(Path::new("/etc/wash/")) == Path::new("/etc/wash/"));
}

#[test]
fn str_to_usize_test() {
    assert_eq!(str_to_usize("123"), Some(123));
}
