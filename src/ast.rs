use std::collections::*;
use std::fmt;

use constants::*;
use types::*;
use env::*;

use types::InputValue::*;
use types::Action::*;
use types::HandlerResult::*;

pub type SectionTable = HashMap<SectionType, DList<Action>>;
pub type HandlerTable = HashMap<String, AstHandler>;

pub type AstHandler = fn(&mut DList<InputValue>, &mut usize, &mut DList<Action>, &mut AST) -> AstResult;

pub struct AST {
    pub env: WashEnv,
    sections: SectionTable,
    handlers: HandlerTable,
    position: SectionType,
    extra_section: usize,
    endline: DList<Action>,
    blocks: Vec<SectionType>,
    pub elif: Option<SectionType>,
    pub sec_loop: bool
}

impl fmt::Debug for AST {
    fn fmt(&self, fmt:&mut fmt::Formatter) -> fmt::Result {
        try!(fmt.write_fmt(format_args!("\nPosition: {:?}\n", self.position)));
        try!(fmt.write_fmt(format_args!("Extra section number: {}\n", self.extra_section)));
        for block in self.blocks.iter() {
            try!(fmt.write_fmt(format_args!("Block jumped to from {:?}\n", block)));
        }
        try!(fmt.write_str("\n"));
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
            env: WashEnv::new(),
            sections: HashMap::new(),
            handlers: HashMap::new(),
            position: SectionType::Run,
            extra_section: 0,
            endline: DList::new(),
            blocks: vec![],
            elif: None,
            sec_loop: false
        }
    }

    pub fn clear(&mut self) {
        self.sections.clear();
        self.position = SectionType::Run;
        self.extra_section = 0;
        self.endline.clear();
        self.blocks.clear();
        self.elif = None;
        self.sec_loop = false;
    }

    pub fn in_block(&self) -> bool {
        !self.blocks.is_empty()
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

    pub fn get_position(&mut self) -> SectionType {
        self.position
    }

    pub fn add_line(&mut self, line:&mut InputValue) -> Result<(), String> {
        let mut aclist = try!(self.process(line, true));
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

    pub fn end_block(&mut self) -> Result<(), String> {
        match self.blocks.pop() {
            None => Err(format!("No block to end")),
            Some(SectionType::Number(n)) => {
                if self.sec_loop {
                    match self.position {
                        SectionType::Number(n) => {
                            self.current_section().push_back(Jump(n));
                            self.sec_loop = false;
                        }, _ => panic!("Cannot loop .run")
                    }
                } else {
                    self.current_section().push_back(Jump(n));
                }
                self.move_to(SectionType::Number(n));
                Ok(())
            },
            Some(section) => {
                self.move_to(section);
                Ok(())
            }
        }
    }

    pub fn process(&mut self, line:&mut InputValue, run:bool) -> Result<DList<Action>, String> {
        match line {
            &mut Split(_) => Ok(DList::new()),
            &mut Short(ref s) if self.handlers.contains_key(s) => {
                // since this is a function call we can just clone it
                // and it's just cloning a usize, so it's pretty fast
                let callback = self.handlers.get(s).unwrap().clone();
                let mut out = DList::new();
                match try!(callback(&mut DList::new(), &mut 0,
                                    &mut out, self)) {
                    Continue | Stop => Ok(out),
                    More(section) => {
                        self.blocks.push(section);
                        Ok(out)
                    }
                }
            },
            &mut Short(ref s) => {
                let mut out = DList::new();
                match VAR_PATH_REGEX.captures(s.as_slice()) {
                    None => if VAR_REGEX.is_match(s.as_slice()) {
                        out.push_back(Set(WashArgs::Flat(s.clone())));
                        out.push_back(Load);
                    } else {
                        out.push_back(Set(WashArgs::Flat(s.clone())));
                        if run {
                            out.push_back(Call(format!("run")));
                            out.push_back(Call(format!("describe_process_output")));
                        }
                    },
                    Some(caps) => {
                        if caps.at(2).unwrap().is_empty() {
                            let path = caps.at(1).unwrap();
                            if path.is_empty() {
                                out.push_back(Set(WashArgs::Empty));
                            } else {
                                out.push_back(Set(WashArgs::Flat(path.to_string())));
                            }
                            out.push_back(Call(format!("getall")));
                            if run {
                                out.push_back(Call(format!("flatten_eqlist")));
                            }
                        } else {
                            out.push_back(Set(WashArgs::Flat(s.clone())));
                            out.push_back(Load);
                        }
                    }
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
                                More(section) => {
                                    self.blocks.push(section);
                                    continue
                                }
                            }
                        },
                        Some(mut item) => {
                            let mut aclist = try!(self.process(&mut item, false));
                            let was_empty = aclist.is_empty();
                            out.append(&mut aclist);
                            if !was_empty {
                                out.push_back(Temp);
                                count += 1;
                            }
                        }
                    }
                }
                // this code is duplicated in handle_endblock
                if count == 1 {
                    out.push_back(Get);
                    if run {
                        out.push_back(Call(format!("run")));
                        out.push_back(Call(format!("describe_process_output")));
                    }
                } else if count > 1 {
                    out.push_back(Join(count));
                    if run {
                        out.push_back(Call(format!("run")));
                        out.push_back(Call(format!("describe_process_output")));
                    }
                }
                Ok(out)
            },
            &mut Function(ref n, ref mut v) => {
                let mut aclist;
                let old_blocks = self.blocks.clone();
                if v.is_empty() {
                    aclist = DList::new();
                } else if v.len() == 1 {
                    aclist = try!(self.process(&mut v[0], false));
                } else {
                    aclist = try!(self.process(&mut Long(v.clone()), false));
                }
                if self.blocks == old_blocks {
                    // unbalanced block delimiters cancel
                    // function calls on a line
                    aclist.push_back(Call(n.clone()));
                }
                Ok(aclist)
            }
        }
    }

    pub fn evaluate(&mut self) -> Result<WashArgs, String> {
        if self.in_block() {
            return Err(format!("Tried to evaluate while in block"));
        }
        self.position = SectionType::Run;
        let mut cfv = WashArgs::Empty;
        let mut vs = DList::new();
        loop {
            let section = match self.sections.get(&self.position) {
                None => return Err(format!("Reached unknown section")),
                Some(sec) => sec.clone()
            };
            let mut iter = section.into_iter();
            loop {
                match iter.next() {
                    None => return Ok(cfv),
                    Some(action) => match action {
                        Jump(n) => {
                            self.position = SectionType::Number(n);
                            break;
                        },
                        Branch(n) => {
                            if cfv.is_empty() {
                                self.position = SectionType::Number(n);
                                break;
                            }
                        },
                        Set(v) => {
                            cfv = v;
                        },
                        Insert(v) => {
                            vs.push_back(v);
                        },
                        ReInsert => {
                            match vs.pop_back() {
                                None => {},
                                Some(v) => {
                                    vs.push_back(v.clone());
                                    vs.push_back(v);
                                }
                            }
                        },
                        Temp => {
                            vs.push_back(cfv);
                            cfv = WashArgs::Empty;
                        },
                        Top => {
                            let top = match vs.back() {
                                None => WashArgs::Empty,
                                Some(v) => v.clone()
                            };
                            cfv = top;
                        },
                        Swap => {
                            let top = match vs.pop_back() {
                                None => WashArgs::Empty,
                                Some(v) => v
                            };
                            vs.push_back(cfv);
                            cfv = top;
                        },
                        Get => {
                            match vs.pop_back() {
                                None | Some(WashArgs::Empty) => {
                                    cfv = WashArgs::Empty;
                                },
                                Some(WashArgs::Long(mut v)) => {
                                    match cfv {
                                        WashArgs::Long(ref mut cv) => {
                                            cv.append(&mut v);
                                        },
                                        WashArgs::Flat(s) => {
                                            v.insert(0, WashArgs::Flat(s));
                                            cfv = WashArgs::Long(v);
                                        },
                                        WashArgs::Empty => {
                                            cfv = WashArgs::Long(v);
                                        }
                                    }
                                },
                                Some(WashArgs::Flat(s)) => {
                                    match cfv {
                                        WashArgs::Long(ref mut cv) => {
                                            cv.push(WashArgs::Flat(s));
                                        },
                                        WashArgs::Flat(cs) => {
                                            let v = vec![WashArgs::Flat(cs),
                                                         WashArgs::Flat(s)];
                                            cfv = WashArgs::Long(v);
                                        },
                                        WashArgs::Empty => {
                                            cfv = WashArgs::Flat(s);
                                        }
                                    }
                                }
                            }
                        },
                        Join(n) => {
                            let index = {
                                if vs.len() > n {
                                    vs.len() - n
                                } else {
                                    0
                                }
                            };
                            cfv = WashArgs::Long(vs.split_off(index).into_iter().collect());
                        },
                        Call(n) => {
                            cfv = try!(self.env.runf(&n, &cfv));
                        },
                        Proc(n, c) => {
                            let index = {
                                if vs.len() > c {
                                    vs.len() - c
                                } else {
                                    0
                                }
                            };
                            let mut vargs:Vec<WashArgs> = vs.split_off(index).into_iter().collect();
                            let args = {
                                if vargs.is_empty() {
                                    WashArgs::Empty
                                } else if vargs.len() == 1 {
                                    vargs.pop().unwrap()
                                } else {
                                    WashArgs::Long(vargs)
                                }
                            };
                            vs.push_back(try!(self.env.runf(&n, &args)));
                        },
                        Fail(m) => {
                            return Err(m);
                        },
                        DStore(n, p) => {
                            if p.is_empty() {
                                try!(self.env.insv(n, cfv));
                                cfv = WashArgs::Empty;
                            } else {
                                try!(self.env.insvp(n, p, cfv));
                                cfv = WashArgs::Empty;
                            }
                        },
                        UnStack(n, p) => {
                            let top = match vs.pop_back() {
                                None => WashArgs::Empty,
                                Some(v) => v
                            };
                            if p.is_empty() {
                                try!(self.env.insv(n, top));
                            } else {
                                try!(self.env.insvp(n, p, top));
                            }
                        },
                        Stack(n, p) => {
                            if p.is_empty() {
                                vs.push_back(try!(self.env.getv(&n)));
                            } else {
                                vs.push_back(try!(self.env.getvp(&n, &p)));
                            }
                        },
                        Store => {
                            let com_name = match vs.pop_back() {
                                None => return Err(format!("No variable name found")),
                                Some(WashArgs::Flat(s)) => s,
                                Some(_) => return Err(format!("Variable names must be flat"))
                            };
                            match VAR_PATH_REGEX.captures(com_name.as_slice()) {
                                None => match VAR_REGEX.captures(com_name.as_slice()) {
                                    None => return Err(format!("Variable name {} could not be resolved into $path:name",
                                                               com_name)),
                                    Some(caps) => {
                                        let name = caps.at(1).unwrap();
                                        try!(self.env.insv(name.to_string(), cfv));
                                        cfv = WashArgs::Empty;
                                    }
                                },
                                Some(caps) => {
                                    let path = caps.at(1).unwrap();
                                    let name = caps.at(2).unwrap();
                                    try!(self.env.insvp(name.to_string(), path.to_string(), cfv));
                                    cfv = WashArgs::Empty;
                                }
                            }
                        },
                        Load => {
                            let com_name = match cfv {
                                WashArgs::Flat(s) => s,
                                _ => return Err(format!("Variable names must be flat"))
                            };
                            match VAR_PATH_REGEX.captures(com_name.as_slice()) {
                                None => match VAR_REGEX.captures(com_name.as_slice()) {
                                    None => return Err(format!("Variable name {} could not be resolved into $path:name",
                                                               com_name)),
                                    Some(caps) => {
                                        let name = caps.at(1).unwrap();
                                        cfv = try!(self.env.getv(&name.to_string()));
                                    }
                                },
                                Some(caps) => {
                                    let path = caps.at(1).unwrap();
                                    let name = caps.at(2).unwrap();
                                    cfv = try!(self.env.getvp(&name.to_string(), &path.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
