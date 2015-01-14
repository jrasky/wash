use util::*;
use constants::*;

use self::InputValue::*;

#[derive(Clone)]
pub enum InputValue {
    Long(Vec<InputValue>),
    Function(String, Vec<InputValue>),
    Short(String),
    Literal(String),
    Split(String),
}

impl InputValue {
    pub fn is_empty(&self) -> bool {
        match self {
            &Long(ref v) => v.is_empty(),
            &Function(ref n, ref v) => n.is_empty() && v.is_empty(),
            &Literal(ref v) | &Split(ref v) | &Short(ref v) => v.is_empty(),
        }
    }

    pub fn clear(&mut self) {
        *self = Short(String::new());
    }
}

#[derive(Clone)]
pub struct InputLine {
    pub back: Vec<InputValue>,
    pub front: InputValue,
    pub part: String
}

impl InputLine {
    pub fn new() -> InputLine {
        InputLine {
            back: vec![],
            front: Short(String::new()),
            part: String::new()
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.back.is_empty() && self.front.is_empty() && self.part.is_empty()
    }

    pub fn clear(&mut self) {
        self.back.clear();
        self.front.clear();
        self.part.clear();
    }
    
    pub fn push(&mut self, ch:char) -> bool {
        let new = match ch {
            SPC | CMA => match self.front.clone() {
                Split(ref s) if ch == CMA => {
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(Split(s.clone()));
                            v.push(Short(String::new()));
                        },
                        _ => after_borrow = true
                    };
                    if after_borrow {
                        self.back.push(Split(s.clone()));
                        self.back.push(Short(String::new()));
                    }
                    let mut t = String::new();
                    t.push(CMA);
                    Split(t)
                },
                Split(ref mut s) if ch == SPC => {
                    s.push(ch);
                    Split(s.clone())
                },
                Literal(ref mut s) => {
                    s.push(ch);
                    Literal(s.clone())
                },
                ref s if match s {
                    &Short(_) | &Long(_) | &Function(_, _) => true,
                    &Split(_) | &Literal(_) => false
                } => {
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(s.clone());
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(s.clone());
                    }
                    let mut t = String::new();
                    t.push(ch);
                    Split(t)
                },
                _ => {
                    // invalid input
                    return false;
                }
            },
            OPR => match self.front.clone() {
                Short(ref s) if *s == "".to_string() => {
                    // arg list
                    self.back.push(Long(vec![]));
                    Short(String::new())
                },
                Split(ref s) if *s == "".to_string() => {
                    // invalid input
                    return false;
                },
                Split(ref s) => {
                    // arg list
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(Split(s.clone()));
                            v.push(Long(vec![]));
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(Split(s.clone()));
                        self.back.push(Long(vec![]));
                    }
                    Short(String::new())
                },
                Short(ref s) => {
                    // arg list
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(Function(s.clone(), vec![]));
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(Function(s.clone(), vec![]));
                    }
                    Short(String::new())
                },
                Literal(ref mut v) => {
                    v.push(ch);
                    Literal(v.clone())
                },
                Long(_) | Function(_, _) => {
                    // invalid input
                    return false;
                },
            },
            CPR => match self.front.clone() {
                ref s if match s {
                    &Short(_) | &Long(_) | &Function(_, _) => true,
                    &Split(_) | &Literal(_) => false
                } => {
                    // end of argument list
                    let len = self.back.len();
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(s.clone());
                        },
                        _ => {
                            // invalid input
                            return false;
                        }
                    }
                    // we've covered the None case above
                    self.back.pop().unwrap()
                },
                Literal(ref mut v) => {
                    v.push(ch);
                    Literal(v.clone())
                },
                _ => {
                    // invalid input
                    return false;
                }
            },
            QUT => match self.front.clone() {
                Split(ref mut s) => {
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(Split(s.clone()));
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(Split(s.clone()));
                    }
                    Literal(String::new())
                },
                Short(ref mut s) => {
                    let mut t = String::new();
                    t.push_str(s.as_slice());
                    Literal(t)
                },
                Literal(ref mut s) => {
                    // end of literal
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(Literal(s.clone()));
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(Literal(s.clone()));
                    }
                    Short(String::new())
                },
                Long(_) | Function(_, _) => {
                    // invalid input
                    return false;
                }
            },
            ch => match self.front.clone() {
                Split(ref mut s) => {
                    let len = self.back.len();
                    let mut after_borrow = false;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(Split(s.clone()));
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(Split(s.clone()));
                    }
                    let mut t = String::new();
                    t.push(ch);
                    Short(t)
                },
                Short(ref mut s) => {
                    s.push(ch);
                    Short(s.clone())
                },
                Literal(ref mut s) => {
                    s.push(ch);
                    Literal(s.clone())
                },
                _ => {
                    // invalid input
                    return false;
                }
            }
        };
        self.front = new;
        // default
        return true;
    }

    pub fn pop(&mut self) -> Option<char> {
        let mut cfront = self.front.clone();
        let out = match cfront {
            Short(ref mut s) | Split(ref mut s) => {
                match s.pop() {
                    Some(v) => Some(v),
                    None => match self.back.pop() {
                        None => None,
                        Some(ref v) if match v {
                            &Split(_) | &Short(_) => true,
                            _ => false
                        } => {
                            self.front = v.clone();
                            return self.pop();
                        },
                        Some(Literal(v)) => {
                            self.front = Literal(v);
                            return Some(QUT);
                        },
                        Some(v) => {
                            // Long and Function
                            self.front = v;
                            return Some(CPR);
                        }
                    }
                }
            },
            Literal(ref mut s) => {
                match s.pop() {
                    Some(v) => Some(v),
                    None => match self.back.pop() {
                        None => {
                            self.front = Short(String::new());
                            return Some(QUT);
                        },
                        Some(v) => {
                            self.front = v;
                            return Some(QUT);
                        }
                    }
                }
            },
            Long(ref mut s) => {
                match s.pop() {
                    None => match self.back.pop() {
                        None => {
                            self.front = Short(String::new());
                            return Some(OPR);
                        },
                        Some(v) => {
                            self.front = v;
                            return Some(OPR);
                        }
                    },
                    Some(v) => {
                        self.front = v;
                        return self.pop();
                    }
                }
            },
            Function(ref mut n, ref mut v) => {
                match v.pop() {
                    Some(v) => {
                        self.front = v;
                        return self.pop();
                    },
                    None => {
                        self.front = Short(n.clone());
                        return Some(OPR);
                    }
                }
            }
        };
        self.front = cfront;
        return out;
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

    pub fn process_line(line:String) -> Option<InputValue> {
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


    pub fn process(&mut self) -> Option<InputValue> {
        loop {
            match self.part.pop() {
                Some(ch) => {
                    self.push(ch);
                },
                None => break
            }
        }
        if !self.front.is_empty() {
            match self.front.clone() {
                Short(_) | Function(_, _) | Long(_) => {self.push(SPC);},
                Literal(_) => {self.push(QUT);},
                Split(_) => {}
            }
        }
        if self.back.len() < 2 {
            return self.back.pop();
        } else {
            return Some(Long(self.back.clone()));
        }
    }
}

