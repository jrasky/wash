use std::collections::*;
use std::fmt;

use constants::*;
use types::{WashArgs, InputValue};

use types::InputValue::*;

use self::Action::*;
use self::HandlerResult::*;

macro_rules! handler {
    ($name:ident, $contents:pat, $count:pat,
     $out:pat, $ast:pat, $func:block) => {
        fn $name($contents:&mut DList<InputValue>, $count:&mut usize,
                 $out:&mut DList<Action>, $ast:&mut AST) -> AstResult
            $func
    }
}

pub type SectionTable = HashMap<SectionType, DList<Action>>;
pub type HandlerTable = HashMap<String, AstHandler>;

pub type AstResult = Result<HandlerResult, String>;
pub type AstHandler = fn(&mut DList<InputValue>, &mut usize, &mut DList<Action>, &mut AST) -> AstResult;

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
    // fail with the given message
    Fail(String),
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
    // clone the value on top of VS to CFV
    Top,
    // swap CFV and top of VS
    Swap,
    // pop top of VS, append to CFV if CFV is Long
    // and top of VS is not long, join to CFV if
    // VS is long, join with CFV in new Long if CFV
    // is not Empty and top of VS is not Long,
    // prepend CFV to top of VS and replace CFV with
    // that if top of VS is Long, replace CFV if it
    // is Empty
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
    Stack(String, String),
    // copy and insert the top of VS
    ReInsert
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
            &Fail(ref m) => match other {
                &Fail(ref om) if *m == *om => true,
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
            &Top => match other {
                &Top => true,
                _ => false
            },
            &Swap => match other {
                &Swap => true,
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
            },
            &ReInsert => match other {
                &ReInsert => true,
                _ => false
            },
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
            &Fail(ref m) => {
                try!(fmt.write_fmt(format_args!("Fail({})", m)));
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
            &Top => {
                try!(fmt.write_str("Top"));
            },
            &Swap => {
                try!(fmt.write_str("Swap"));
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
            },
            &ReInsert => {
                try!(fmt.write_str("ReInsert"));
            },
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
    position: SectionType,
    extra_section: usize,
    endline: DList<Action>
}

impl fmt::Debug for AST {
    fn fmt(&self, fmt:&mut fmt::Formatter) -> fmt::Result {
        for handler in self.handlers.keys() {
            try!(fmt.write_fmt(format_args!("Handler for {}\n", handler)));
        }
        try!(fmt.write_fmt(format_args!("\nPosition: {:?}\n", self.position)));
        try!(fmt.write_fmt(format_args!("Extra section number: {}\n\n", self.extra_section)));
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
            sections: HashMap::new(),
            handlers: HashMap::new(),
            position: SectionType::Run,
            extra_section: 0,
            endline: DList::new()
        }
    }

    pub fn clear(&mut self) {
        self.sections.clear();
        self.position = SectionType::Run;
        self.extra_section = 0;
        self.endline.clear();
    }

    pub fn add_handler(&mut self, word:&str, callback:AstHandler) {
        self.handlers.insert(word.to_string(), callback);
    }

    pub fn add_endline(&mut self, action:Action) {
        self.endline.push_back(action);
    }

    pub fn new_section(&mut self) -> SectionType {
        let out = self.position;
        self.position = SectionType::Number(self.extra_section);
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, DList::new());
        }
        self.extra_section += 1;
        return out;
    }

    pub fn current_section(&mut self) -> &mut DList<Action> {
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, DList::new());
        }
        return self.sections.get_mut(&self.position).unwrap();
    }

    pub fn move_to(&mut self, section:SectionType) {
        self.position = section;
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, DList::new());
        }
    }

    pub fn move_current_to(&mut self, section:SectionType) {
        let content = match self.sections.remove(&self.position) {
            None => DList::new(),
            Some(l) => l
        };
        self.position = section;
        self.sections.insert(self.position, content);
    }

    pub fn get_position(&mut self) -> SectionType {
        self.position
    }

    pub fn add_line(&mut self, line:&mut InputValue) -> Result<(), String> {
        let mut aclist = try!(self.process(line));
        aclist.append(&mut self.endline);
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, DList::new());
        }
        match self.sections.get_mut(&self.position) {
            None => Err(format!("Position not found in section table")),
            Some(mut section) => {
                section.append(&mut aclist);
                Ok(())
            }
        }
    }

    pub fn process(&mut self, line:&mut InputValue) -> Result<DList<Action>, String> {
        match line {
            &mut Split(_) => Ok(DList::new()),
            &mut Short(ref s) => {
                let mut out = DList::new();
                if VAR_PATH_REGEX.is_match(s.as_slice()) ||
                    VAR_REGEX.is_match(s.as_slice()) {
                        out.push_back(Set(WashArgs::Flat(s.clone())));
                        out.push_back(Load);
                    } else {
                        out.push_back(Set(WashArgs::Flat(s.clone())));
                    }
                Ok(out)
            },
            &mut Literal(ref s) => {
                let mut out = DList::new();
                out.push_back(Set(WashArgs::Flat(s.clone())));
                Ok(out)
            },
            &mut Long(ref mut v) => {
                let mut out = DList::new();
                let mut count = 0;
                let mut items:DList<InputValue> = v.drain().collect();
                loop {
                    match items.pop_front() {
                        None => break,
                        Some(Short(ref s)) if self.handlers.contains_key(s) => {
                            // since this is a function call we can just clone it
                            // and it's just cloning a usize, so it's pretty fast
                            let callback = self.handlers.get(s).unwrap().clone();
                            match try!(callback(&mut items, &mut count,
                                                &mut out, self)) {
                                Continue => continue,
                                Stop => return Ok(out),
                                More => panic!("Not implemented")
                            }
                        },
                        Some(mut item) => {
                            let mut aclist = try!(self.process(&mut item));
                            let was_empty = aclist.is_empty();
                            out.append(&mut aclist);
                            if !was_empty {
                                out.push_back(Temp);
                                count += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    out.push_back(Join(count));
                }
                Ok(out)
            },
            &mut Function(ref n, ref mut v) => {
                let mut aclist;
                if v.is_empty() {
                    aclist = DList::new();
                } else if v.len() == 1 {
                    aclist = try!(self.process(&mut v[0]));
                } else {
                    aclist = try!(self.process(&mut Long(v.clone())));
                }
                aclist.push_back(Call(n.clone()));
                Ok(aclist)
            }
        }
    }
}

handler!(handle_equal, contents, count, out, ast, {
    // since this is a Long, the name/path combo
    // should already be on VS at this point
    // if the variable is named directly, it could be that
    // two instructions up is a Load
    // remove this so you don't have to "$name" every time
    let back1 = out.pop_back();
    let back2 = out.pop_back();
    if back1 == Some(Temp) && back2 == Some(Load) {
        out.push_back(Temp);
    } else {
        match back2 {
            Some(v) => out.push_back(v),
            _ => {}
        }
        match back1 {
            Some(v) => out.push_back(v),
            _ => {}
        }
    }
    // now evaluate the value
    let mut newacs = DList::new();
    if contents.is_empty() {
        newacs.push_back(Set(WashArgs::Empty));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value));
        if aclist.is_empty() {
            newacs.push_back(Set(WashArgs::Empty));
        } else {
            newacs.append(&mut aclist);
        }
    }
    // now the value is on CFV
    // name is hopefully on the top of VS
    newacs.push_back(Store);
    while match contents.front() {
        Some(&Split(_)) => true,
        _ => false
    } {
        contents.pop_front();
    }
    if !contents.is_empty() {
        // there are other things on the line, so this variable
        // should be unset at the end.
        out.push_back(ReInsert);
        out.push_back(Top);
        out.push_back(Load);
        out.push_back(Swap);
        out.push_back(Temp);
        ast.add_endline(Set(WashArgs::Empty));
        ast.add_endline(Get);
        ast.add_endline(Store);
        
    } else {
        out.append(&mut newacs);
    }
    // in either case the end result is one item
    // is consumed from the original, given VS
    *count -= 1;
    return Ok(Continue);
});

handler!(equalequal_inner, contents, count, out, ast, {
    // LHS is already partially evaluated into VS
    if *count > 1 {
        // more than one element means we need to join
        // them and re-push them back
        out.push_back(Join(*count));
        out.push_back(Temp);
    }
    if contents.is_empty() {
        out.push_back(Insert(WashArgs::Empty));
    } else {
        while match contents.front() {
            Some(&Split(_)) => true,
            _ => false
        } {
            contents.pop_front();
        }
        let mut aclist = {
            if contents.len() > 1 {
                let mut v = vec![];
                loop {
                    match contents.pop_front() {
                        None => break,
                        Some(val) => v.push(val)
                    }
                }
                try!(ast.process(&mut Long(v)))
            } else {
                try!(ast.process(contents.front_mut().unwrap()))
            }
        };
        out.append(&mut aclist);
        out.push_back(Temp);
    }
    // now the two arguments we're interested in are at the top
    // of the VS
    out.push_back(Join(2));
    // VS has been emptied as a result
    *count = 0;
    return Ok(Continue);
});

handler!(handle_equalequal, contents, count, out, ast, {
    try!(equalequal_inner(contents, count, out, ast));
    out.push_back(Call(format!("equal?")));
    return Ok(Stop);
});

handler!(handle_tildaequal, contents, count, out, ast, {
    try!(equalequal_inner(contents, count, out, ast));
    out.push_back(Call(format!("re_equal?")));
    return Ok(Stop);
});

handler!(handle_dot, contents, count, out, ast, {
    if contents.is_empty() {
        out.push_back(Insert(WashArgs::Empty));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value));
        if aclist.is_empty() {
            out.push_back(Insert(WashArgs::Empty));
        } else {
            out.append(&mut aclist);
            out.push_back(Temp);
        }
    }
    out.push_back(Join(2));
    out.push_back(Call(format!("dot")));
    if !contents.is_empty() || *count > 1 {
        // Only temp if there's something else on the line
        out.push_back(Temp);
    } else {
        *count -= 1;
    }
    return Ok(Continue);
});

handler!(handle_semiamper, _, count, out, _, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 0;
    }
    out.push_back(Call(format!("run")));
    return Ok(Continue);
});

handler!(handle_amper, _, count, out, _, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 0;
    }
    out.push_back(Call(format!("job")));
    return Ok(Continue);
});

handler!(handle_amperamper, contents, count, out, ast, {
    // amperamper is an extension of semiamper
    try!(handle_semiamper(contents, count, out, ast));
    out.push_back(Call(format!("run_failed?")));
    let old_section = ast.new_section();
    let new_num = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't a numbered one")
    };
    ast.current_section().push_back(Fail(STOP.to_string()));
    ast.move_to(old_section);
    out.push_back(Branch(new_num));
    return Ok(Continue);
});

handler!(handle_bar, contents, count, out, ast, {
    // extension of amper
    try!(handle_amper(contents, count, out, ast));
    out.push_back(Insert(WashArgs::Flat(format!("$pipe:"))));
    out.push_back(Temp);
    out.push_back(Join(2));
    out.push_back(Call(format!("dot")));
    out.push_back(Load);
    out.push_back(Temp);
    *count += 1;
    return Ok(Continue);
});

handler!(handle_geq, contents, count, out, ast, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 0;
    }
    out.push_back(Temp);
    if contents.is_empty() {
        return Err(format!("No file name given"));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value));
        if aclist.is_empty() {
            return Err(format!("No file name given"));
        }
        out.append(&mut aclist);
        // the following demonstrates the beauty of Get
        out.push_back(Call(format!("open_output")));
        out.push_back(Temp);
        out.push_back(Set(WashArgs::Flat(format!("@out:"))));
        out.push_back(Get);
        out.push_back(Call(format!("dot")));
        out.push_back(Get);
        return Ok(Continue);
    }
});

handler!(handle_leq, contents, count, out, ast, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 0;
    }
    out.push_back(Temp);
    if contents.is_empty() {
        return Err(format!("No file name given"));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value));
        if aclist.is_empty() {
            return Err(format!("No file name given"));
        }
        out.append(&mut aclist);
        // the following demonstrates the beauty of Get
        out.push_back(Call(format!("open_input")));
        out.push_back(Temp);
        out.push_back(Set(WashArgs::Flat(format!("@:"))));
        out.push_back(Get);
        out.push_back(Call(format!("dot")));
        out.push_back(Get);
        return Ok(Continue);
    }
});

pub fn load_handlers(ast:&mut AST) {
    ast.add_handler("=", handle_equal);
    ast.add_handler("==", handle_equalequal);
    ast.add_handler("~=", handle_tildaequal);
    ast.add_handler(".", handle_dot);
    ast.add_handler("&;", handle_semiamper);
    ast.add_handler("&", handle_amper);
    ast.add_handler("&&", handle_amperamper);
    ast.add_handler("|", handle_bar);
    ast.add_handler(">", handle_geq);
    ast.add_handler("<", handle_leq);
}
