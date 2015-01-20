// Simple types that rely mainly on themselves
// Useful in many places
use std::cmp::*;

use self::WashArgs::*;

#[derive(Clone)]
pub enum WashArgs {
    Flat(String),
    Long(Vec<WashArgs>),
    Empty
}

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
        use self::InputValue::*;
        match self {
            &Long(ref v) => v.is_empty(),
            &Function(ref n, ref v) => n.is_empty() && v.is_empty(),
            &Literal(ref v) | &Split(ref v) | &Short(ref v) => v.is_empty(),
        }
    }

    pub fn clear(&mut self) {
        *self = InputValue::Short(String::new());
    }

    #[cfg(test)]
    pub fn print(&self) {
        use self::InputValue::*;
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
        use self::InputValue::*;
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


impl WashArgs {
    pub fn flatten_vec(&self) -> Vec<String> {
        match self {
            &Flat(ref s) => vec![s.clone()],
            &Long(ref v) => {
                let mut out:Vec<String> = vec![];
                for item in v.iter() {
                    out = vec![out, item.flatten_vec()].concat();
                }
                return out;
            },
            &Empty => vec![]
        }
    }
    
    pub fn flatten_with(&self, with:&str) -> String {
        match self {
            &Flat(ref s) => s.clone(),
            &Long(ref v) => {
                let mut out = String::new();
                for item in v.iter() {
                    out.push_str(item.flatten_with(with).as_slice());
                    out.push_str(with);
                }
                // remove last NL
                out.pop();
                return out;
            },
            &Empty => {
                return String::new();
            }
        }
    }

    pub fn flatten_with_inner(&self, outer:&str, inner:&str) -> String {
        match self {
            &Flat(ref s) => s.clone(),
            &Long(ref v) => {
                let mut out = String::new();
                for item in v.iter() {
                    out.push_str(item.flatten_with(inner).as_slice());
                    out.push_str(outer);
                }
                // remove last NL
                out.pop();
                return out;
            },
            &Empty => {
                return String::new();
            }
        }
    }

    pub fn flatten(&self) -> String {
        return self.flatten_with("\n");
    }

    pub fn len(&self) -> usize {
        match self {
            &Flat(_) => 1,
            &Long(ref v) => v.len(),
            &Empty => 0
        }
    }
    
    pub fn is_empty(&self) -> bool {
        match self {
            &Flat(_) | &Long(_) => false,
            &Empty => true
        }
    }

    pub fn is_flat(&self) -> bool {
        match self {
            &Flat(_) => true,
            _ => false
        }
    }

    pub fn get(&self, index:usize) -> WashArgs {
        if index >= self.len() {
            return Empty;
        }
        match self {
            &Flat(ref v) => Flat(v.clone()),
            &Long(ref v) => v[index].clone(),
            &Empty => Empty
        }
    }

    pub fn get_flat(&self, index:usize) -> String {
        match self.get(index) {
            Flat(ref v) => v.clone(),
            Long(_) | Empty => "".to_string()
        }
    }

    pub fn slice(&self, u_from:isize, u_to:isize) -> WashArgs {
        let from = min(max(0, u_from) as usize, self.len()) as usize;
        let to = {
            match u_to {
                v if v < 0 => self.len(),
                _ => min(from, self.len())
            }
        };
        if to <= from {
            return Long(vec![]);
        }
        match self {
            &Flat(_) => Empty,
            &Empty => Empty,
            &Long(ref v) => Long(v[from..to].to_vec())
        }
    }
}
