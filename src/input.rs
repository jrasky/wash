use util::*;
use constants::*;

pub struct InputLine {
    pub words: Vec<String>,
    pub front: String,
    pub part: String
}

impl InputLine {
    pub fn new() -> InputLine {
        InputLine {
            words: Vec::<String>::new(),
            front: String::new(),
            part: String::new()
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.front.is_empty() && self.part.is_empty()
    }

    pub fn clear(&mut self) {
        self.words.clear();
        self.front.clear();
        self.part.clear();
    }
    
    pub fn push(&mut self, ch:char) {
        match ch {
            SPC => {
                if is_word(self.front.as_slice()) {
                    self.words.push(self.front.clone());
                    self.front.clear();
                } else {
                    self.front.push(SPC);
                }
            },
            c => {
                self.front.push(c);
            }
        }
    }

    pub fn pop(&mut self) -> Option<char> {
        if self.front.is_empty() {
            self.front = match self.words.pop() {
                Some(s) => s,
                None => return None
            };
            // there are spaces between words
            return Some(SPC);
        } else {
            return self.front.pop();
        }
    }

    pub fn right(&mut self) -> bool {
        match self.part.pop() {
            Some(ch) => { 
                self.push(ch);
                return true;
            },
            None => false
        }
    }

    pub fn left(&mut self) -> bool {
        match self.pop() {
            None => false,
            Some(ch) => {
                self.part.push(ch);
                return true;
            }
        }
    }

    pub fn process_line(line:String) -> Vec<String> {
        let mut inp = InputLine::new();
        let mut cline = line.clone();
        loop {
            match cline.pop() {
                Some(c) => inp.part.push(c),
                None => break
            }
        }
        return inp.process();
    }

    pub fn process(&self) -> Vec<String> {
        let mut part = self.part.clone();
        let mut front = self.front.clone();
        let mut words = self.words.clone();
        loop {
            match part.pop() {
                Some(SPC) => {
                    if is_word(front.as_slice()) {
                        words.push(front.clone());
                        front.clear();
                    } else {
                        front.push(SPC);
                    }
                },
                Some(ch) => {
                    front.push(ch);
                },
                None => break
            }
        }
        if !front.is_empty() {
            words.push(front);
        }
        return words;
    }
}
