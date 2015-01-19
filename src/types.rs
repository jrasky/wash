// Simple types that rely mainly on themselves
// Useful in many places

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
