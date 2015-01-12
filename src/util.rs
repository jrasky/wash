use std::cmp::*;
use std::os;

pub fn is_word(word:&str) -> bool {
    // defines the following "container" sequences: (.*) and ".*"
    // words cannot end when either are unclosed
    regex!("^([^ \t\n\r\"()]*|\".*\"|\\(.*\\))*$").is_match(word)
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
    let first_removed:String = regex!("\"").splitn(word.as_slice(), 2).collect::<Vec<&str>>().as_slice().concat();
    let second_removed:String = regex!("\"$").splitn(first_removed.as_slice(), 2).collect::<Vec<&str>>().as_slice().concat();
    return second_removed.to_string();
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
    assert!(is_word("this(is)a\"complex\"series"));

    assert!(!is_word("TEST=\"hello"));
    assert!(!is_word("func(test fun"));
    assert!(!is_word("this(is)a\"complex\"series(with no end"));
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
