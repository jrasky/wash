use std::cmp::*;
use std::os;

pub fn is_word(word:&String) -> bool {
    (!word.as_slice().starts_with("\"") ||
     (word.len() > 1 &&
      word.as_slice().starts_with("\"") &&
      word.as_slice().ends_with("\""))) &&
        (!word.as_slice().starts_with("$(") ||
         (word.len() > 2 &&
          word.as_slice().starts_with("$(") &&
          word.as_slice().ends_with(")")))
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

pub fn strip_words(line:Vec<String>) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for word in line.iter() {
        out.push(strip_word(word));
    }
    return out;
}

pub fn strip_word(word:&String) -> String {
    if word.as_slice().starts_with("\"") &&
        word.as_slice().ends_with("\"") {
            return String::from_str(word.slice_chars(1, word.len() - 1));
        } else {
            return word.clone();
        }
}

pub fn expand_path(path:Path) -> Path {
    if Path::new("~").is_ancestor_of(&path) {
        return match os::getenv("HOME") {
            None => Path::new("/"),
            Some(val) => Path::new(val)
        }.join(Path::new(path.as_vec().slice_from(min(path.as_vec().len(), 2))));
    } else {
        return path;
    }
}

pub fn condense_path(path:Path) -> Path {
    let homep = Path::new(match os::getenv("HOME") {
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
    assert!(is_word(&String::from_str("hello")));
    assert!(is_word(&String::from_str("\"hello world\"")));
    assert!(is_word(&String::from_str("$(test command)")));

    assert!(!is_word(&String::from_str("\"hello wor")));
    assert!(!is_word(&String::from_str("$(test com")));
}

#[test]
fn strip_word_test() {
    assert!(strip_word(&String::from_str("\"hello world\"")) == String::from_str("hello world"));
    assert!(strip_word(&String::from_str("hello")) == String::from_str("hello"));
}

#[test]
fn strip_words_test() {
    assert!(strip_words(vec![String::from_str("\"hello world\""), String::from_str("hello")])
            == vec![String::from_str("hello world"), String::from_str("hello")]);
}

#[test]
fn expand_path_test() {
    // tests require the HOME env set
    let homep = Path::new(os::getenv("HOME").unwrap());
    assert!(expand_path(Path::new("~/Documents/scripts/")) == homep.join("Documents/scripts/"));
    assert!(expand_path(Path::new("/etc/wash/")) == Path::new("/etc/wash/"));
}

#[test]
fn condense_path_test() {
    // tests require the HOME env set
    let homep = Path::new(os::getenv("HOME").unwrap());
    assert!(condense_path(homep.join("Documents/scripts/")) ==
            Path::new("~/Documents/scripts/"));
    assert!(condense_path(Path::new("/home/")) == Path::new("/home/"));
    assert!(condense_path(Path::new("/etc/wash/")) == Path::new("/etc/wash/"));
}
