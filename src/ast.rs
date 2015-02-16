use std::collections::*;
use std::fmt;

use constants::*;
use types::{WashArgs, InputValue};

use types::InputValue::*;

use self::Action::*;
use self::HandlerResult::*;

pub type SectionTable = HashMap<SectionType, DList<Action>>;
pub type HandlerTable = HashMap<String, AstHandler>;

pub type AstResult = Result<HandlerResult, String>;
// Fun fact: AstHandler actually takes four usizes
pub type AstHandler = fn(&Vec<InputValue>, usize, &mut usize, &mut DList<Action>, &mut AST) -> AstResult;

pub enum HandlerResult {
    Continue, Stop, More
}

// Acronyms
// VS: Variable Stack, a stack containing runtime variables
// GS: General State, the general environmental state
// CFV: Current Front Variable, a separate variable temporary
//      "storage" for function calls and the such.

#[derive(Clone)]
pub enum Action {
    // * Basic actions
    // set CFV to given
    Set(WashArgs),
    // call function given name
    // arguments are on the CFV
    // store value in CFV
    Call(String),
    // take name, path stored on the top of VS (like $path:name)
    // store CFV in that name, path
    Store,
    // load variable into CFV given name, path on CFV
    Load,
    // branch to given section number if the CFV
    // is not empty
    // branches do not touch stack
    Branch(usize),
    // Unconditional branch
    Jump(usize),
    // * VS Specific actions
    // push CFV onto VS
    Temp,
    // join CFV and top of VS into Long, unless
    // CFV is Empty, in which case just put top of
    // VS into new Long
    Get,
    // pop last given elements of VS into new
    // long, put into CFV
    Join(usize),
    // * Useful variations of basic actions
    // given name, path, store CFV
    DStore(String, String),
    // push given onto VS
    Insert(WashArgs),
    // Same as Call, push result onto VS
    // given name and # of arguments to pull from VS
    Proc(String, usize),
    // pop off VS, store into name, path
    UnStack(String, String),
    // load given variable onto VS
    Stack(String, String)
}

impl PartialEq for Action {
    fn eq(&self, other:&Action) -> bool {
        match self {
            &Set(ref v) => match other {
                &Set(ref ov) if *v == *ov => true,
                _ => false
            },
            &Call(ref n) => match other {
                &Call(ref on) if *n == *on => true,
                _ => false
            },
            &Store => match other {
                &Store => true,
                _ => false
            },
            &DStore(ref n, ref p) => match other {
                &DStore(ref on, ref op) if *n == *on &&
                    *p == *op => true,
                _ => false
            },
            &Load => match other {
                &Load => true,
                _ => false
            },
            &Branch(ref d) => match other {
                &Branch(ref od) if *d == *od => true,
                _ => false
            },
            &Jump(ref d) => match other {
                &Jump(ref od) if *d == *od => true,
                _ => false
            },
            &Temp => match other {
                &Temp => true,
                _ => false
            },
            &Get => match other {
                &Get => true,
                _ => false
            },
            &Join(ref n) => match other {
                &Join(ref on) if *n == *on => true,
                _ => false
            },
            &Insert(ref v) => match other {
                &Insert(ref ov) if *v == *ov => true,
                _ => false
            },
            &Proc(ref n, ref a) => match other {
                &Proc(ref on, ref oa) if *n == *on &&
                    *a == *oa => true,
                _ => false
            },
            &UnStack(ref n, ref p) => match other {
                &UnStack(ref on, ref op) if *n == *on &&
                    *p == *op => true,
                _ => false
            },
            &Stack(ref n, ref p) => match other {
                &Stack(ref on, ref op) if *n == *on &&
                    *p == *op => true,
                _ => false
            }
        }
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, fmt:&mut fmt::Formatter) -> fmt::Result {
        match self {
            &Set(ref a) => {
                try!(fmt.write_fmt(format_args!("Set({:?})", a)));
            },
            &Call(ref n) => {
                try!(fmt.write_fmt(format_args!("Call({})", n)));
            },
            &Store => {
                try!(fmt.write_str("Store"));
            },
            &DStore(ref n, ref p) => {
                try!(fmt.write_fmt(format_args!("DStore({}, {})", n, p)));
            },
            &Load => {
                try!(fmt.write_str("Load"));
            },
            &Branch(ref d) => {
                try!(fmt.write_fmt(format_args!("Branch({})", d)));
            },
            &Jump(ref d) => {
                try!(fmt.write_fmt(format_args!("Jump({})", d)));
            },
            &Temp => {
                try!(fmt.write_str("Temp"));
            },
            &Get => {
                try!(fmt.write_str("Get"));
            },
            &Join(ref n) => {
                try!(fmt.write_fmt(format_args!("Join({})", n)));
            },
            &Insert(ref a) => {
                try!(fmt.write_fmt(format_args!("Insert({:?})", a)));
            },
            &Proc(ref n, ref c) => {
                try!(fmt.write_fmt(format_args!("Proc({}, {})", n, c)));
            },
            &UnStack(ref n, ref p) => {
                try!(fmt.write_fmt(format_args!("UnStack({}, {})", n, p)));
            },
            &Stack(ref n, ref p) => {
                try!(fmt.write_fmt(format_args!("Stack({}, {})", n, p)));
            }
        }
        Ok(())
    }
}

#[derive(Copy, Eq, Hash)]
pub enum SectionType {
    // Special section types
    Load, Run,
    // Other sections are numbered
    Number(usize)
}

impl PartialEq for SectionType {
    fn eq(&self, other:&SectionType) -> bool {
        use self::SectionType::*;
        match self {
            &Load => match other {
                &Load => true,
                _ => false
            },
            &Run => match other {
                &Run => true,
                _ => false
            },
            &Number(ref n) => match other {
                &Number(ref on) if *n == *on => true,
                _ => false
            }
        }
    }
}

impl fmt::Debug for SectionType {
    fn fmt(&self, fmt:&mut fmt::Formatter) -> fmt::Result {
        use self::SectionType::*;
        match self {
            &Load => {
                try!(fmt.write_str("load"));
            },
            &Run => {
                try!(fmt.write_str("run"));
            },
            &Number(ref n) => {
                try!(fmt.write_fmt(format_args!("{}", n)));
            }
        }
        Ok(())
    }
}

pub struct AST {
    sections: SectionTable,
    handlers: HandlerTable,
    position: SectionType
}

impl fmt::Debug for AST {
    fn fmt(&self, fmt:&mut fmt::Formatter) -> fmt::Result {
        for handler in self.handlers.keys() {
            try!(fmt.write_fmt(format_args!("Handler for {}\n", handler)));
        }
        try!(fmt.write_fmt(format_args!("\nPosition: {:?}\n\n", self.position)));
        for section in self.sections.keys() {
            try!(fmt.write_fmt(format_args!(".{:?}\n", section)));
            for item in self.sections.get(section).unwrap().iter() {
                try!(fmt.write_fmt(format_args!("{:?}\n", item)));
            }
            try!(fmt.write_str("\n"));
        }
        Ok(())
    }
}

impl AST {
    pub fn new() -> AST {
        AST {
            sections: {
                let mut map = HashMap::new();
                map.insert(SectionType::Run, DList::new());
                map
            },
            handlers: HashMap::new(),
            position: SectionType::Run
        }
    }

    pub fn add_handler(&mut self, word:&str, callback:AstHandler) {
        self.handlers.insert(word.to_string(), callback);
    }

    pub fn add_line(&mut self, line:InputValue) -> Result<(), String> {
        let mut aclist = try!(self.process(line));
        match self.sections.get_mut(&self.position) {
            None => Err(format!("Position not found in section table")),
            Some(mut section) => {
                section.append(&mut aclist);
                Ok(())
            }
        }
    }

    pub fn process(&mut self, line:InputValue) -> Result<DList<Action>, String> {
        match line {
            Split(_) => Ok(DList::new()),
            Short(s) | Literal(s) => {
                let mut out = DList::new();
                out.push_back(Set(WashArgs::Flat(s)));
                Ok(out)
            },
            Long(v) => {
                let mut out = DList::new();
                let mut count = 0;
                let mut index = 0;
                let mut iter = v.iter();
                loop {
                    match iter.next() {
                        None => break,
                        Some(&Short(ref s)) if self.handlers.contains_key(s) => {
                            // since this is a function call we can just clone it
                            // and it's just cloning a usize, so it's pretty fast
                            let callback = self.handlers.get(s).unwrap().clone();
                            match try!(callback(&v, index, &mut count,
                                                &mut out, self)) {
                                Continue => continue,
                                Stop => return Ok(out),
                                More => panic!("Not implemented")
                            }
                        },
                        Some(item) => {
                            let aclist = try!(self.process(item.clone()));
                            for acitem in aclist.iter() {
                                out.push_back(acitem.clone());
                            }
                            if !aclist.is_empty() {
                                out.push_back(Temp);
                                count += 1;
                            }
                            index += 1;
                        }
                    }
                }
                out.push_back(Join(count));
                Ok(out)
            },
            Function(n, v) => {
                let mut aclist;
                if v.is_empty() {
                    aclist = DList::new();
                } else if v.len() == 1 {
                    aclist = try!(self.process(v[0].clone()));
                } else {
                    aclist = try!(self.process(Long(v)));
                }
                aclist.push_back(Call(n));
                Ok(aclist)
            }
        }
    }
}

fn handle_equal(contents:&Vec<InputValue>, index:usize,
                _:&mut usize, out:&mut DList<Action>,
                ast:&mut AST) -> AstResult {
    // both the name and value are evaluated
    // out already contains the evaluation of
    // the name, temp it
    // since this is a Long, the name/path combo
    // should already be on VS at this point
    // now evaluate the value
    if contents.len() - 1 == index {
        out.push_back(Set(WashArgs::Empty));
    } else {
        let aclist;
        let mut value = contents[index + 1].clone();
        match value {
            Split(_) if contents.len() -2 > index => {
                value = contents[index + 2].clone();
            },
            _ => {}
        }
        aclist = try!(ast.process(value));
        if aclist.is_empty() {
            out.push_back(Set(WashArgs::Empty));
        } else {
            for item in aclist.iter() {
                out.push_back(item.clone())
            }
        }
    }
    // now the value is on CFV
    // name is hopefully on the top of VS
    out.push_back(Store);
    return Ok(Stop);
}

pub fn load_handlers(ast:&mut AST) {
    ast.add_handler("=", handle_equal);
}
