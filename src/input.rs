// "thousand_lines_of_madness.rs"
use constants::*;
use types::*;
use types::InputValue::*;

#[derive(Clone)]
pub struct InputLine {
    pub back: Vec<InputValue>,
    pub front: InputValue,
    pub part: String,
    pub fpart: String
}

impl InputLine {
    pub fn new() -> InputLine {
        InputLine {
            back: vec![Long(vec![])],
            front: Short(String::new()),
            part: String::new(),
            fpart: String::new()
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.back == vec![Long(vec![])] && self.front.is_empty() && self.part.is_empty() && self.fpart.is_empty()
    }

    pub fn clear(&mut self) {
        self.back.clear();
        self.back.push(Long(vec![]));
        self.front.clear();
        self.part.clear();
        self.fpart.clear();
    }

    pub fn push(&mut self, ch:char) -> bool {
        let pushed = match ch.clone() {
            SPC => self.push_spc(),
            CMA => self.push_cma(),
            OPR => self.push_opr(),
            CPR => self.push_cpr(),
            QUT => self.push_qut(),
            NL  => self.push_nl(),
            c => self.push_simple(c)
        };
        if pushed {
            self.fpart.push(ch);
        }
        return pushed;
    }

    fn push_spc(&mut self) -> bool {
        match self.front {
            Split(ref mut s) => {
                s.push(SPC);
                return true;
            },
            Literal(ref mut s) => {
                s.push(SPC);
                return true;
            },
            Short(_) => {
                let inner_empty = match self.front {
                    Short(ref s) => s.is_empty(),
                    _ => panic!("") // should never happen
                };
                if !inner_empty {
                    if !self.push_back() {
                        return false; // invalid input
                    } else {
                        self.back.push(self.front.clone());
                    }
                }
                self.front = Split(SPC.to_string());
                return true;
            },
            Long(_) | Function(_, _) => {
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    self.front = Split(SPC.to_string());
                    return true;
                }
            }
        }
    }

    fn pop_spc(&mut self) -> bool {
        match self.front {
            Literal(ref mut s) => {
                let popped = s.pop();
                if !(popped == Some(SPC)) {
                    if popped.is_some() {
                        s.push(popped.unwrap());
                    }
                    return false;
                } else {
                    return true;
                }
            },
            Split(_) => {
                let empty;
                match self.front {
                    Split(ref mut s) => {
                        let popped = s.pop();
                        if popped != Some(SPC) {
                            if popped.is_some() {
                                s.push(popped.unwrap());
                            }
                            return false;
                        }
                        empty = s.is_empty();
                    },
                    _ => panic!("") // should never happen
                }
                if empty {
                    let popped = self.back.pop();
                    match popped {
                        Some(Long(_)) | Some(Function(_, _)) => {
                            self.front = popped.clone().unwrap();
                            if !self.pop_back() {
                                self.back.push(popped.unwrap());
                                self.front = Short(String::new());
                                return true;
                            } else {
                                match self.front {
                                    Literal(_) => {
                                        // the pop_back was incorrect, in this case
                                        // front was an empty split
                                        if !self.push_back() {
                                            panic!("Pop/push back aren't inverse");
                                        }
                                        self.back.push(self.front.clone());
                                        self.front = Split(String::new());
                                    },
                                    _ => {}
                                }
                                return true;
                            }
                        },
                        Some(v) => {
                            // in this case front was an empty short
                            self.back.push(v);
                            self.front = Short(String::new());
                            return true;
                        },
                        _ => {
                            self.front = Short(String::new());
                            return true;
                        }
                    }
                } else {
                    // front isn't empty yet
                    return true;
                }
            },
            _ => return false // invalid pop
        }
    }

    fn push_cma(&mut self) -> bool {
        match self.front {
            Split(ref mut s) if s.is_empty() => {
                s.push(CMA);
                return true;
            },
            Split(_) => {
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    self.front = Split(CMA.to_string());
                    return true;
                }
            }
            Literal(ref mut s) => {
                s.push(CMA);
                return true;
            },
            Short(_) => {
                let inner_empty = match self.front {
                    Short(ref s) => s.is_empty(),
                    _ => panic!("") // should never happen
                };
                if !inner_empty {
                    if !self.push_back() {
                        return false; // invalid input
                    } else {
                        self.back.push(self.front.clone());
                    }
                }
                self.front = Split(CMA.to_string());
                return true;
            },
            Long(_) | Function(_, _) => {
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    self.front = Split(CMA.to_string());
                    return true;
                }
            }
        }
    }

    fn pop_cma(&mut self) -> bool {
        match self.front {
            Literal(ref mut s) => {
                let popped = s.pop();
                if !(popped == Some(CMA)) {
                    if popped.is_some() {
                        s.push(popped.unwrap());
                    }
                    return false;
                } else {
                    return true;
                }
            },
            Split(_) => {
                let empty;
                match self.front {
                    Split(ref mut s) => {
                        let popped = s.pop();
                        if popped != Some(CMA) {
                            if popped.is_some() {
                                s.push(popped.unwrap());
                            }
                            return false;
                        }
                        empty = s.is_empty();
                    },
                    _ => panic!("") // should never happen
                }
                if empty {
                    let popped = self.back.pop();
                    match popped {
                        Some(Long(_)) | Some(Function(_, _)) => {
                            self.front = popped.clone().unwrap();
                            if !self.pop_back() {
                                self.back.push(popped.unwrap());
                                self.front = Split(CMA.to_string());
                                return false;
                            } else {
                                match self.front {
                                    Literal(_) => {
                                        // the pop_back was incorrect, in this case
                                        // front was an empty split
                                        if !self.push_back() {
                                            panic!("Pop/push back aren't inverse");
                                        }
                                        self.back.push(self.front.clone());
                                        self.front = Split(String::new());
                                    },
                                    _ => {}
                                }
                                return true;
                            }
                        },
                        Some(v) => {
                            // in this case front was an empty short
                            self.back.push(v);
                            self.front = Short(String::new());
                            return true;
                        },
                        _ => {
                            self.front = Short(String::new());
                            return true;
                        }
                    }
                } else {
                    // front isn't empty yet
                    return true;
                }
            },
            _ => return false // invalid pop
        }
    }

    fn push_opr(&mut self) -> bool {
        match self.front {
            Split(_) => {
                // arg list
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    self.back.push(Long(vec![]));
                    self.front = Short(String::new());
                    return true;
                }
            },
            Short(_) => {
                let name = match self.front {
                    Short(ref s) => s.clone(),
                    _ => panic!("") // should never happen
                };
                if name.is_empty() {
                    // arg list
                    self.back.push(Long(vec![]));
                } else {
                    // function
                    self.back.push(Function(name, vec![]));
                }
                self.front = Short(String::new());
                return true;
            },
            Literal(ref mut s) => {
                s.push(OPR);
                return true;
            },
            _ => return false // invalid input
        }
    }

    fn pop_opr(&mut self) -> bool {
        match self.front {
            Literal(ref mut s) => {
                let popped = s.pop();
                if !(popped == Some(OPR)) {
                    if popped.is_some() {
                        s.push(popped.unwrap());
                    }
                    return false;
                } else {
                    return true;
                }
            },
            Short(_) => {
                match self.front {
                    Short(ref s) if s.is_empty() => {},
                    _ => return false // invalid pop
                }
                match self.back.pop() {
                    Some(Long(v)) => {
                        if !v.is_empty() {
                            self.back.push(Long(v));
                            return false; // invalid pop
                        } else {
                            let popped = self.back.pop();
                            match popped {
                                Some(Function(_, _)) | Some(Long(_)) => {
                                    self.front = popped.clone().unwrap();
                                    if !self.pop_back() {
                                        self.back.push(popped.unwrap());
                                        self.front = Short(String::new());
                                        // popped back as far as we can
                                        return true;
                                    } else {
                                        // front is in the correct place
                                        return true;
                                    }
                                },
                                _ => {
                                    self.back.push(popped.unwrap());
                                    // front before/after this pop was an empty Short
                                    return true;
                                }
                            }
                        }
                    },
                    Some(Function(name, v)) => {
                        if !v.is_empty() {
                            self.back.push(Function(name, v));
                            return false;
                        } else {
                            self.front = Short(name);
                            return true;
                        }
                    },
                    _ => return false // invalid pop
                }
            },
            _ => return false // invalid pop
        }
    }

    fn push_cpr(&mut self) -> bool {
        match self.front {
            Split(_) | Short(_) | Long(_) | Function(_, _) => {
                // end argument list
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    // front should now be correct
                    return true;
                }
            },
            Literal(ref mut s) => {
                s.push(CPR);
                return true;
            }
        }
    }

    fn pop_cpr(&mut self) -> bool {
        match self.front {
            Literal(ref mut s) => {
                let popped = s.pop();
                if !(popped == Some(CPR)) {
                    if popped.is_some() {
                        s.push(popped.unwrap());
                    }
                    return false;
                } else {
                    return true;
                }
            },
            Function(_, _) | Long(_) => {
                if !self.pop_back() {
                    let popped = self.back.pop();
                    match popped {
                        Some(v) => {
                            if v.is_empty() {
                                // special case
                                self.back.push(self.front.clone());
                                self.back.push(v);
                                self.front = Short(String::new());
                                return true;
                            } else {
                                self.back.push(v);
                                return false;
                            }
                        },
                        None => {
                            // special case
                            self.back.push(self.front.clone());
                            self.front = Short(String::new());
                            return true;
                        }
                    }
                } else {
                    match self.front {
                        Literal(_) => {
                            // the pop_back was incorrect, in this case
                            // front was an empty split
                            if !self.push_back() {
                                panic!("Pop/push back aren't inverse");
                            }
                            self.back.push(self.front.clone());
                            self.front = Split(String::new());
                        },
                        _ => {}
                    }
                    return true;
                }
            },
            _ => return false // invalid pop
        }
    }

    fn push_qut(&mut self) -> bool {
        match self.front {
            Split(_) => {
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    self.front = Literal(String::new());
                    return true;
                }
            },
            Short(_) => {
                let contents = match self.front {
                    Short(ref s) => s.clone(),
                    _ => panic!("")
                };
                self.front = Literal(contents);
                return true;
            },
            Literal(_) => {
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    self.front = Split(String::new());
                    return true;
                }
            },
            _ => return false // invalid input
        }
    }

    fn pop_qut(&mut self) -> bool {
        match self.front {
            Literal(_) => {
                let contents = match self.front {
                    Literal(ref s) => s.clone(),
                    _ => panic!("")
                };
                if !contents.is_empty() {
                    self.front = Short(contents);
                    return true;
                } else {
                    match self.back.pop() {
                        Some(v) => {
                            self.front = v.clone();
                            if !self.pop_back() {
                                self.back.push(v);
                                self.front = Short(String::new());
                                // popped back as far as we can go
                                return true;
                            } else {
                                // front is now in the right place
                                return true;
                            }
                        },
                        _ => return false // invalid pop
                    }
                }
            },
            Split(_) => {
                match self.front {
                    Split(ref s) => {
                        if !s.is_empty() {
                            return false; // invalid pop
                        }
                    },
                    _ => panic!("")
                }
                match self.back.pop() {
                    Some(v) => {
                        self.front = v.clone();
                        if !self.pop_back() {
                            self.back.push(v);
                            self.front = Split(String::new());
                            return false;
                        } else {
                            // front is now in the right place
                            return true;
                        }
                    },
                    _ => return false // invalid pop
                }
            },
            _ => return false // invalid pop
        }
    }

    fn push_nl(&mut self) -> bool {
        match self.front {
            Literal(ref mut s) => {
                s.push(NL);
                return true;
            },
            _ => return false
        }
    }

    fn pop_nl(&mut self) -> bool {
        match self.front {
            Literal(ref mut s) => {
                let popped = s.pop();
                if !(popped == Some(NL)) {
                    s.push(NL);
                    return false;
                } else {
                    return true;
                }
            },
            _ => return false
        }
    }

    fn push_simple(&mut self, ch:char) -> bool {
        match self.front {
            Split(_) => {
                if !self.push_back() {
                    return false; // invalid input
                } else {
                    self.back.push(self.front.clone());
                    let mut contents = String::new();
                    contents.push(ch);
                    self.front = Short(contents);
                    return true;
                }
            },
            Short(ref mut s) | Literal(ref mut s) => {
                s.push(ch);
                return true;
            },
            _ => return false // invalid input
        }
    }

    fn pop_simple(&mut self, ch:char) -> bool {
        match self.front {
            Literal(ref mut s) => {
                let popped = s.pop();
                if !(popped == Some(ch)) {
                    if popped.is_some() {
                        s.push(popped.unwrap());
                    }
                    return false;
                } else {
                    return true;
                }
            },
            Short(_) => {
                let empty;
                match self.front {
                    Short(ref mut s) => {
                        let popped = s.pop();
                        match popped {
                            Some(c) if c == ch => {
                                empty = s.is_empty();
                            },
                            Some(c) => {
                                s.push(c);
                                return false; // invalid pop
                            },
                            _ => {
                                return false;
                            }
                        }
                    },
                    _ => panic!("")
                }
                if empty {
                    let popped = self.back.pop();
                    match popped {
                        Some(Long(_)) | Some(Function(_, _)) => {
                            let old = self.front.clone();
                            self.front = popped.clone().unwrap();
                            if !self.pop_back() {
                                self.back.push(popped.unwrap());
                                self.front = old;
                                // pushed back as far as we can go
                                return true;
                            } else {
                                // front is in the right place
                                return true;
                            }
                        },
                        _ => {
                            self.back.push(popped.unwrap());
                            // front is already in the right place
                            return true;
                        }
                    }
                } else {
                    // front isn't empty yet
                    return true;
                }
            },
            _ => return false // invalid pop
        }
    }

    fn push_back(&mut self) -> bool {
        // The way the back vector works is that the front
        // is always the inner-most level of the currently
        // typed structure
        // This means that the back vector will contain a
        // series of objects corresponding to progressively
        // more outer levels of the line
        // This means that to move up one level, we take
        // the current front and push it into the contents
        // of the first thing on the back vector
        let mut last = match self.back.pop() {
            None => return false, // Trying to push back too far
            Some(v) => v
        };
        match last {
            Function(_, ref mut v) | Long(ref mut v) =>
                match self.front {
                    Long(_) =>
                        v.push(self.front.clone()),
                    _ if !self.front.is_empty() =>
                        v.push(self.front.clone()),
                    _ => {}
                },
            _ => {
                self.back.push(last);
                return false;
            }
        }
        self.front = last;
        return true;
    }

    fn pop_back(&mut self) -> bool {
        // inverse of push_back
        match self.front {
            Long(_) | Function(_, _) => {
                let next = match self.front {
                    Long(ref mut v) | Function(_, ref mut v)
                        => match v.pop() {
                            None => return false, // can't pop back here
                            Some(v) => v
                        },
                    _ => panic!("") // should never happen
                };
                self.back.push(self.front.clone());
                self.front = next;
                return true;
            },
            _ => return false // can't pop back here
        }
    }

    pub fn pop(&mut self) -> Option<char> {
        // easy part: get the character
        let out = match self.fpart.pop() {
            None => return None, // nothing else to pop
            Some(v) => v
        };
        // hard part: unwind the data structure
        // to match
        let ok = match out {
            SPC => self.pop_spc(),
            CMA => self.pop_cma(),
            OPR => self.pop_opr(),
            CPR => self.pop_cpr(),
            QUT => self.pop_qut(),
            NL  => self.pop_nl(),
            c => self.pop_simple(c)
        };
        if !ok {
            return None;
        } else {
            return Some(out);
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

    pub fn process(&self) -> Option<InputValue> {
        let mut output = self.clone();
        loop {
            match output.part.pop() {
                Some(ch) => {
                    output.push(ch);
                },
                None => break
            }
        }
        while output.push_back() && !output.back.is_empty() {}
        if !output.back.is_empty() {
            return None;
        } else {
            return match output.front {
                Long(mut v) => {
                    if v.len() == 1 {
                        return Some(v.pop().unwrap());
                    } else {
                        return Some(Long(v));
                    }
                },
                v => Some(v)
            }
        }
    }
}

#[cfg(test)]
fn test_input_against(line:String, against:InputValue) -> bool {
    // test winding
    let mut input = InputLine::new();
    let mut st = line.clone();
    let mut bst = String::new();
    let mut out = String::new();
    let mut bout = String::new();
    loop {
        match st.pop() {
            Some(ch) => {bst.push(ch);},
            None => break
        }
    }
    loop {
        match bst.pop() {
            None => break,
            Some(ch) => {
                print!("{}", ch);
                let ooinput = input.clone();
                if !input.push(ch) {
                    println!("\nRefused to push character: \"{}\"", ch);
                    println!("{:?}", ooinput.back);
                    println!("--------");
                    println!("{:?}", ooinput.front);
                    return false;
                }
                let oinput = input.clone();
                let popped = input.pop();
                if popped != Some(ch) {
                    println!("\nDidn't pop out the same character: pushed \"{}\" got \"{}\"", ch, match popped {
                        Some(v) => v.to_string(),
                        None => format!("None")
                    });
                    println!("{:?}", ooinput.back);
                    println!("--------");
                    println!("{:?}", input.back);
                    println!("--------");
                    println!("{:?}", ooinput.front);
                    println!("--------");
                    println!("{:?}", input.front);
                    return false;
                }
                if input.back != ooinput.back ||
                    input.front != ooinput.front ||
                    input.part != ooinput.part {
                        println!("\nPopping didn't return to state before pushing: \"{}\"", ch);
                        println!("{:?}", ooinput.back);
                        println!("--------");
                        println!("{:?}", oinput.back);
                        println!("--------");
                        println!("{:?}", input.back);
                        println!("--------");
                        println!("{:?}", ooinput.front);
                        println!("--------");
                        println!("{:?}", oinput.front);
                        println!("--------");
                        println!("{:?}", input.front);
                        return false;
                    }
                let binput = input.clone();
                input.push(ch);
                if input.back != oinput.back ||
                    input.front != oinput.front ||
                    input.part != oinput.part {
                        println!("\nPushing didn't return to state before popping: \"{}\"", ch);
                        println!("{:?}", ooinput.back);
                        println!("--------");
                        println!("{:?}", oinput.back);
                        println!("--------");
                        println!("{:?}", binput.back);
                        println!("--------");
                        println!("{:?}", input.back);
                        println!("--------");
                        println!("{:?}", ooinput.front);
                        println!("--------");
                        println!("{:?}", oinput.front);
                        println!("--------");
                        println!("{:?}", binput.front);
                        println!("--------");
                        println!("{:?}", input.front);
                        return false;
                    }
            }
        }
    }
    println!("");

    match input.process() {
        Some(ref v) if v.clone() == against => {},
        Some(ref v) => {
            println!("{:?}", v);
            return false;
        }
        _ => return false
    };

    loop {
        match input.pop() {
            Some(ch) => {
                out.push(ch);
            },
            None => break
        }
    }
    loop {
        match out.pop() {
            Some(ch) => {bout.push(ch);},
            None => break
        }
    }

    if bout != line {
        println!("Out: {}", bout);
        return false;
    }

    return true;
}

#[test]
fn test_input_short() {
    // test short
    assert!(test_input_against("hello_world".to_string(), Short("hello_world".to_string())));
}

#[test]
fn test_input_literal() {
    // test literal
    assert!(test_input_against("\"hello world\"".to_string(), Literal("hello world".to_string())));
}

#[test]
fn test_input_arg_list() {
    // test arg list
    assert!(test_input_against("hello_world, \"hello world\", another arg".to_string(), Long(vec![
        Short("hello_world".to_string()),
        Split(", ".to_string()),
        Literal("hello world".to_string()),
        Split(", ".to_string()),
        Short("another".to_string()),
        Split(" ".to_string()),
        Short("arg".to_string())
            ])));
}

#[test]
fn test_input_function() {
    // test function
    assert!(test_input_against("test_func(hello_world, \"hello world\", \"another arg\")".to_string(),
                               Function("test_func".to_string(), vec![
                                   Short("hello_world".to_string()),
                                   Split(", ".to_string()),
                                   Literal("hello world".to_string()),
                                   Split(", ".to_string()),
                                   Literal("another arg".to_string()),
                                   ])));

    // test function
    assert!(test_input_against("test_func(\"hello world\")".to_string(),
                               Function("test_func".to_string(), vec![
                                   Literal("hello world".to_string()),
                                   ])));
}

#[test]
fn test_input_nested_lists() {
    // test nested lists
    assert!(test_input_against("list (within (lists (within lists)))".to_string(), Long(vec![
        Short("list".to_string()),
        Split(" ".to_string()),
        Long(vec![
            Short("within".to_string()),
            Split(" ".to_string()),
            Long(vec![
                Short("lists".to_string()),
                Split(" ".to_string()),
                Long(vec![
                    Short("within".to_string()),
                    Split(" ".to_string()),
                    Short("lists".to_string())
                        ])
                    ])
                ])
            ])));

    // more complex list testing
    assert!(test_input_against("(((((())))))".to_string(), Long(vec![
        Long(vec![
            Long(vec![
                Long(vec![
                    Long(vec![
                        Long(vec![])
                            ])
                        ])
                    ])
                ])
            ])));
    
    // more complex list testing
    assert!(test_input_against("(((((\"test arg\")))))".to_string(), Long(vec![
            Long(vec![
                Long(vec![
                    Long(vec![
                        Long(vec![
                            Literal("test arg".to_string())
                                ])
                            ])
                        ])
                    ])
            ])));
}

#[test]
fn test_input_nested_functions() {
    // test nested functions
    assert!(test_input_against("functions(calling functions(calling functions(with args)))".to_string(),
                               Function("functions".to_string(), vec![
                                   Short("calling".to_string()),
                                   Split(" ".to_string()),
                                   Function("functions".to_string(), vec![
                                       Short("calling".to_string()),
                                       Split(" ".to_string()),
                                       Function("functions".to_string(), vec![
                                           Short("with".to_string()),
                                           Split(" ".to_string()),
                                           Short("args".to_string())
                                               ])
                                           ])
                                       ])));
}

#[test]
fn test_input_function_list_args() {
    // harder test
    assert!(test_input_against("function(with, (more \"args\"))".to_string(),
                               Function("function".to_string(), vec![
                                   Short("with".to_string()),
                                   Split(", ".to_string()),
                                   Long(vec![
                                       Short("more".to_string()),
                                       Split(" ".to_string()),
                                       Literal("args".to_string())
                                           ])
                                       ])
                               ));
}

#[test]
fn test_input_multiline_literal() {
    // multiline literal test
    assert!(test_input_against("\"literal with\nmultiple lines\"".to_string(),
                               Literal("literal with\nmultiple lines".to_string())));
}

#[test]
fn test_input_long_space() {
    // test spaces at the beginning of longs
    assert!(test_input_against(format!(" ( ( ( test args)))"),
                               Long(vec![Split(format!(" ")),
                                         Long(vec![Split(format!(" ")),
                                                   Long(vec![Split(format!(" ")),
                                                             Long(vec![Split(format!(" ")),
                                                                       Short(format!("test")),
                                                                       Split(format!(" ")),
                                                                       Short(format!("args"))
                                                                       ])])])])));
}
