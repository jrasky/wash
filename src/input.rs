// "thousand_lines_of_madness.rs"
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
    // There is but one blemish on this otherwise
    // beautiful, perfect data type:
    // Empty lists are Long(Short(""))
    // There is not really any way around this
    // which wouldn't have a special case in the
    // data type, and the only way to get a list
    // with an empty short is to type an empty list
    // so this solution isn't terrible
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

    #[cfg(test)]
    pub fn print(&self) {
        match self {
            &Short(ref v) => {
                print!("Short({})\n", v);
            },
            &Literal(ref v) => {
                print!("Literal({})\n", v);
            },
            &Split(ref v) => {
                print!("Split({})\n", v);
            },
            &Long(ref v) => {
                print!("Long(");
                for item in v.clone().iter() {
                    item.print();
                }
                print!(")\n");
            },
            &Function(ref n, ref v) => {
                print!("Function({}, (", n);
                for item in v.clone().iter() {
                    item.print();
                }
                print!("))\n");
            }
        }
    }
}

impl PartialEq for InputValue {
    fn eq(&self, other:&InputValue) -> bool {
        match self {
            &Long(ref v) => match other {
                &Long(ref ov) => return v == ov,
                _ => return false
            },
            &Function(ref n, ref v) => match other {
                &Function(ref on, ref ov) => return n == on && v == ov,
                _ => return false
            },
            &Short(ref s) => match other {
                &Short(ref os) => return s == os,
                _ => return false
            },
            &Literal(ref s) => match other {
                &Literal(ref os) => return s == os,
                _ => return false
            },
            &Split(ref s) => match other {
                &Split(ref os) => return s == os,
                _ => return false
            }
        }
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
                Split(ref s) if ch == CMA && !s.is_empty() => {
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
                Split(ref mut s) => {
                    s.push(ch);
                    Split(s.clone())
                },
                Literal(ref mut s) => {
                    s.push(ch);
                    Literal(s.clone())
                },
                Short(ref s) if *s == "".to_string()  => {
                    if ch == CMA {
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
                        t.push(ch);
                        Split(t)
                    } else {
                        return false;
                    }
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
                        },
                        _ => after_borrow = true
                    }
                    if after_borrow {
                        self.back.push(Split(s.clone()));
                    }
                    self.back.push(Long(vec![]));
                    Short(String::new())
                },
                Short(ref s) => {
                    // function
                    self.back.push(Function(s.clone(), vec![]));
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
                    let mut pop_push_back;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            v.push(s.clone());
                            pop_push_back = true;
                        },
                        _ => {
                            // invalid input
                            return false;
                        }
                    }
                    if pop_push_back {
                        // other cases have been handled already
                        let func = self.back.pop().unwrap();
                        let mut after_borrow = false;
                        match get_index(&mut self.back, len - 1) {
                            Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                                v.push(func.clone());
                            }
                            _ => {
                                after_borrow = true;
                            }
                        }
                        if after_borrow {
                            self.back.push(func);
                        }
                    }
                    // we've covered the None case above
                    self.back.pop().unwrap()
                },
                Literal(ref mut v) => {
                    v.push(ch);
                    Literal(v.clone())
                },
                Split(_) => {
                    let len = self.back.len();
                    let mut end_args;
                    match get_index(&mut self.back, len - 1) {
                        Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                            let len = v.len();
                            match get_index(v, len - 1) {
                                Some(&mut Literal(_)) => {
                                    end_args = true;
                                }, _ => return false
                            }
                        }, _ => return false
                    }
                    if end_args {
                        // special case, it's ok to type a CPR here
                        let func = self.back.pop().unwrap();
                        let mut after_borrow = false;
                        match get_index(&mut self.back, len - 1) {
                            Some(&mut Function(_, ref mut v)) | Some(&mut Long(ref mut v)) => {
                                v.push(func.clone());
                            }
                            _ => {
                                after_borrow = true;
                            }
                        }
                        if after_borrow {
                            self.back.push(func);
                        }
                        self.back.pop().unwrap()
                    } else {
                        return false;
                    }
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
                    Split(String::new())
                },
                Long(_) | Function(_, _) => {
                    // invalid input
                    return false;
                }
            },
            ch => match self.front.clone() {
                Split(ref mut s) if s.is_empty() => {
                    // invalid input
                    return false;
                },
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
            Split(ref mut s) if s.len() == 1 => {
                let mut t = self.back.pop();
                let out = s.pop().unwrap();
                let mut push_back = false;
                match t {
                    Some(Short(ref v)) => {
                        self.front = Short(v.clone());
                    },
                    Some(Literal(_)) => {
                        self.front = Split(String::new());
                        push_back = true;
                    }
                    Some(Function(_, ref mut v)) | Some(Long(ref mut v))
                        if !v.is_empty() => {
                            let mut inner_push = false;
                            let t = v.pop().unwrap();
                            self.front = match t {
                                Literal(_) => {
                                    inner_push = true;
                                    Split(String::new())
                                },
                                ref v => v.clone()
                            };
                            if inner_push {
                                v.push(t);
                            }
                            push_back = true;
                        },
                    None => {
                        self.front = Short(String::new());
                    },
                    _ => {
                        self.front = Short(String::new());
                        push_back = true;
                    }
                }
                if push_back {
                    self.back.push(t.unwrap());
                }
                return Some(out);
            },
            Short(ref mut s) if s.len() == 1 => {
                let mut t = self.back.pop();
                let out = s.pop().unwrap();
                let mut push_back = false;
                match t {
                    Some(Split(ref s)) => {
                        self.front = Split(s.clone());
                    },
                    Some(Function(_, ref mut v)) | Some(Long(ref mut v))
                        if !v.is_empty() => {
                            self.front = v.pop().unwrap();
                            push_back = true;
                        },
                    None => {
                        self.front = Short(String::new());
                    },
                    _ => {
                        self.front = Short(String::new());
                        push_back = true;
                    }
                }
                if push_back {
                    self.back.push(t.unwrap());
                }
                return Some(out);
            },
            Short(ref mut s) | Split(ref mut s) => {
                match s.pop() {
                    Some(v) => Some(v),
                    None => {
                        let mut t = self.back.pop();
                        match t {
                            None => return None,
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
                            Some(Long(ref mut v)) if *v != vec![] => {
                                self.front = v.pop().unwrap();
                                self.back.push(Long((*v).clone()));
                                match self.front {
                                    Literal(_) => return Some(QUT),
                                    _ => return self.pop()
                                }
                            },
                            Some(Function(ref mut n, ref mut v)) if *v != vec![] => {
                                self.front = v.pop().unwrap();
                                self.back.push(Function((*n).clone(), (*v).clone()));
                                match self.front {
                                    Literal(_) => return Some(QUT),
                                    _ => return self.pop()
                                }
                            },
                            Some(Function(ref mut n, _)) => {
                                // in this case the function's args are empty
                                self.front = Short((*n).clone());
                                return Some(OPR);
                            },
                            Some(v) => {
                                self.front = v;
                                return self.pop();
                            }
                        }
                    }
                }
            },
            Literal(ref mut s) => {
                match s.pop() {
                    Some(v) => Some(v),
                    None => {
                        let mut t = self.back.pop();
                        match t {
                            None => {
                                self.front = Short(String::new());
                                return Some(QUT);
                            },
                            Some(ref v) if match v {
                                &Split(_) | &Short(_) | &Literal(_) => true,
                                _ => false
                            } => {
                                self.front = v.clone();
                                return Some(QUT);
                            },
                            Some(Long(ref mut v)) if *v != vec![] => {
                                self.front = v.pop().unwrap();
                                self.back.push(Long((*v).clone()));
                                return Some(QUT);
                            },
                            Some(Function(ref mut n, ref mut v)) if *v != vec![] => {
                                self.front = v.pop().unwrap();
                                self.back.push(Function((*n).clone(), (*v).clone()));
                                return Some(QUT);
                            },
                            Some(v) => {
                                // Function with empty args and empty long
                                self.back.push(v);
                                self.front = Short(String::new());
                                return Some(QUT);
                            }
                        }
                    }
                }
            },
            Long(ref mut v) => {
                match v {
                    ref v if **v == vec![] => {
                        let mut t = self.back.pop();
                        let mut push_back = false;
                        match t {
                            Some(Split(ref s)) => {
                                self.front = Split(s.clone());
                            },
                            Some(Function(_, ref mut v)) | Some(Long(ref mut v))
                                if !v.is_empty() => {
                                    self.front = v.pop().unwrap();
                                    push_back = true;
                                },
                            None => {
                                self.front = Short(String::new());
                            },
                            _ => {
                                self.front = Short(String::new());
                                push_back = true;
                            }
                        }
                        if push_back {
                            self.back.push(t.unwrap());
                        }
                        return Some(OPR);
                    },
                    ref v => {
                        let mut nv = (**v).clone();
                        let s = nv.pop().unwrap();
                        let out = match s {
                            Split(_) => None,
                            _ => Some(CPR)
                        };
                        self.front = match s {
                            Literal(_) => {
                                nv.push(s.clone());
                                Split(String::new())
                            },
                            t => t
                        };
                        self.back.push(Long(nv));
                        match out {
                            Some(v) => return Some(v),
                            None => return self.pop()
                        }
                    },
                }
            },
            Function(ref mut n, ref mut v) => {
                match v {
                    ref v if **v == vec![] => {
                        let mut t = self.back.pop();
                        let mut push_back = false;
                        match t {
                            Some(Split(ref s)) => {
                                self.front = Split(s.clone());
                            },
                            Some(Function(_, ref mut v)) | Some(Long(ref mut v))
                                if !v.is_empty() => {
                                    self.front = v.pop().unwrap();
                                    push_back = true;
                                },
                            None => {
                                self.front = Short((*n).clone());
                            },
                            _ => {
                                self.front = Short((*n).clone());
                                push_back = true;
                            }
                        }
                        if push_back {
                            self.back.push(t.unwrap());
                        }
                        return Some(OPR);
                    },
                    ref v => {
                        let mut t = (**v).clone();
                        let popped = t.pop().unwrap();
                        let out = match popped {
                            Split(_) => None,
                            _ => Some(CPR)
                        };
                        self.front = match popped {
                            Literal(_) => {
                                t.push(popped.clone());
                                Split(String::new())
                            },
                            t => t
                        };
                        self.back.push(Function(n.clone(), t));
                        match out {
                            Some(v) => return Some(v),
                            None => return self.pop()
                        }
                    },
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

    pub fn process(&self) -> Option<InputValue> {
        let mut cself = self.clone();
        loop {
            match cself.part.pop() {
                Some(ch) => {
                    cself.push(ch);
                },
                None => break
            }
        }
        if !cself.front.is_empty() {
            match cself.front.clone() {
                Short(_) | Function(_, _) | Long(_) => {cself.push(SPC);},
                Literal(_) => {cself.push(QUT);},
                Split(_) => {}
            }
        }
        if cself.back.len() < 2 {
            return cself.back.pop();
        } else {
            return Some(Long(cself.back.clone()));
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
                    Long(ooinput.back).print();
                    println!("--------");
                    ooinput.front.print();
                    return false;
                }
                let oinput = input.clone();
                let popped = input.pop();
                if popped != Some(ch) {
                    println!("\nDidn't pop out the same character: pushed \"{}\" got \"{}\"", ch, popped.unwrap());
                    Long(ooinput.back).print();
                    println!("--------");
                    Long(input.back).print();
                    println!("--------");
                    ooinput.front.print();
                    println!("--------");
                    input.front.print();
                    return false;
                }
                if input.back != ooinput.back ||
                    input.front != ooinput.front ||
                    input.part != ooinput.part {
                        println!("\nPopping didn't return to state before pushing: \"{}\"", ch);
                        Long(ooinput.back).print();
                        println!("--------");
                        Long(oinput.back).print();
                        println!("--------");
                        Long(input.back).print();
                        println!("--------");
                        ooinput.front.print();
                        println!("--------");
                        oinput.front.print();
                        println!("--------");
                        input.front.print();
                        return false;
                    }
                let binput = input.clone();
                input.push(ch);
                if input.back != oinput.back ||
                    input.front != oinput.front ||
                    input.part != oinput.part {
                        println!("\nPushing didn't return to state before popping: \"{}\"", ch);
                        Long(ooinput.back).print();
                        println!("--------");
                        Long(oinput.back).print();
                        println!("--------");
                        Long(binput.back).print();
                        println!("--------");
                        Long(input.back).print();
                        println!("--------");
                        ooinput.front.print();
                        println!("--------");
                        oinput.front.print();
                        println!("--------");
                        binput.front.print();
                        println!("--------");
                        input.front.print();
                        return false;
                    }
            }
        }
    }
    println!("");

    match input.process() {
        Some(ref v) if v.clone() == against => {},
        Some(ref v) => {
            v.print();
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
fn test_input() {
    // test short
    assert!(test_input_against("hello_world".to_string(), Short("hello_world".to_string())));

    // test literal
    assert!(test_input_against("\"hello world\"".to_string(), Literal("hello world".to_string())));
    
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
    assert!(test_input_against("((((()))))".to_string(), Long(vec![
        Long(vec![
            Long(vec![
                Long(vec![
                    Long(vec![
                        Short(String::new()) // there is no way around an empty short in here
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
