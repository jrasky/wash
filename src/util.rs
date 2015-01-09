
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
pub fn build_string(ch:char, count:uint) -> String {
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
