use regex::NoExpand;

use std::cmp::*;
use std::os;

pub fn is_word(word:&str) -> bool {
    // try easy outs
    if regex!("^[^ \t\r\n\"()]*(\"[^\"]*\"|\\([^()]*\\))$").is_match(word) {
        // no nested delimiters means we can check with one regex call
        return true;
    } else if !regex!("^[^ \t\r\n\"()]*(\\(.*\\)|\".*\")*$").is_match(word) {
        // basic delimiter check, if this doesn't match then this is definitely not a word
        return false;
    }

    // ok, it's going to be more difficult
    let re = regex!("\"[^\"]*\"|[^ \t\r\n\"()]+\\([^()]*\\)");
    let check_re = regex!("^[^\"()]*$");
    let mut val = re.replace_all(word, NoExpand(""));
    let mut nval = re.replace_all(val.as_slice(), NoExpand(""));
    loop {
        if check_re.is_match(nval.as_slice()) {
            // all delimiters are balanced
            return true;
        } else if val == nval {
            // there are unbalanced delimiters
            return false;
        }
        // keep trying
        // Do two at a time to speed things up
        val = re.replace_all(nval.as_slice(), NoExpand(""));
        nval = re.replace_all(val.as_slice(), NoExpand(""));
    }
}

pub fn split_at(word:String, at:Vec<usize>) -> Vec<String> {
    let mut last = 0; let mut pclone;
    let mut out = Vec::<String>::new();
    for pos in at.iter() {
        pclone = pos.clone();
        out.push(word[last..pclone].to_string());
        last = pclone;
    }
    out.push(word[last..word.len()].to_string());
    return out;
}

pub fn get_index<T>(mut vec:&mut Vec<T>, index:usize) -> Option<&mut T> {
    if index >= vec.len() {
        return None;
    } else {
        return Some(&mut vec[index]);
    }
}

pub fn comma_intersect(commas:Vec<(usize, usize)>, words:Vec<(usize, usize)>) -> Vec<usize> {
    let mut out = Vec::<usize>::new();
    let mut iter;
    for &(pos, _) in commas.iter() {
        iter = words.iter();
        loop {
            match iter.next() {
                None => {
                    out.push(pos);
                    break;
                },
                Some(&(start, end)) => {
                    if start < pos || pos < end {
                        break;
                    }
                }
            }
        }
    }
    return out;
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

pub fn expand_path(path:Path) -> Path {
    if Path::new("~").is_ancestor_of(&path) {
        return match os::homedir() {
            None => Path::new("/"),
            Some(val) => Path::new(val)
        }.join(Path::new(path.as_vec().slice_from(min(path.as_vec().len(), 2))));
    } else {
        return path;
    }
}

pub fn condense_path(path:Path) -> Path {
    let homep = Path::new(match os::homedir() {
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

#[test]
fn build_string_test() {
    assert!(build_string('a', 5) == String::from_str("aaaaa"));
}

#[test]
fn is_word_test() {
    assert!(is_word(""));
    assert!(is_word("hello"));
    assert!(is_word("\"hello world\""));
    assert!(is_word("TEST=\"hello world\""));
    assert!(is_word("func(test function)"));
    assert!(is_word("this(is a \"complex series\" with nested (delimiters))"));

    assert!(!is_word("TEST=\"hello"));
    assert!(!is_word("func(test fun"));
    assert!(!is_word("this(is a \"complex series\" with (unbalanced parens)"));
    assert!(!is_word("(invalid func call)"));
}

#[test]
fn strip_word_test() {
    assert!(strip_word(&String::from_str("\"hello world\"")) == String::from_str("hello world"));
    assert!(strip_word(&String::from_str("hello")) == String::from_str("hello"));
}

#[test]
fn strip_words_test() {
    assert!(strip_words(vec![String::from_str("\"hel\"lo world\""), String::from_str("hello")])
            == vec![String::from_str("hel\"lo world"), String::from_str("hello")]);
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

