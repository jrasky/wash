use std::collections::*;
use std::fmt;

use constants::*;
use types::*;
use env::*;

use types::InputValue::*;
use types::Action::*;
use types::HandlerResult::*;

pub type SectionTable = HashMap<SectionType, LinkedList<Action>>;
pub type HandlerTable = HashMap<String, AstHandler>;

pub type AstHandler = fn(&mut LinkedList<InputValue>, &mut usize, &mut LinkedList<Action>, &mut AST) -> AstResult;

pub struct AST {
    pub env: WashEnv,
    sections: SectionTable,
    handlers: HandlerTable,
    position: SectionType,
    extra_section: usize,
    endline: LinkedList<Action>,
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
            try!(fmt.write_fmt(format_args!("{:?}\n", section)));
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
            endline: LinkedList::new(),
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
            self.sections.insert(self.position, LinkedList::new());
        }
        self.extra_section += 1;
        return out;
    }

    pub fn current_section(&mut self) -> &mut LinkedList<Action> {
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, LinkedList::new());
        }
        return self.sections.get_mut(&self.position).unwrap();
    }

    pub fn move_to(&mut self, section:SectionType) {
        self.position = section;
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, LinkedList::new());
        }
    }

    pub fn get_position(&mut self) -> SectionType {
        self.position
    }

    pub fn add_line(&mut self, line:&mut InputValue) -> Result<(), String> {
        let mut aclist = try!(self.process(line, true));
        aclist.append(&mut self.endline);
        if !self.sections.contains_key(&self.position) {
            self.sections.insert(self.position, LinkedList::new());
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

    pub fn process(&mut self, line:&mut InputValue, run:bool) -> Result<LinkedList<Action>, String> {
        match line {
            &mut Split(_) => Ok(LinkedList::new()),
            &mut Short(ref s) if self.handlers.contains_key(s) => {
                // since this is a function call we can just clone it
                // and it's just cloning a usize, so it's pretty fast
                let callback = self.handlers.get(s).unwrap().clone();
                let mut out = LinkedList::new();
                match try!(callback(&mut LinkedList::new(), &mut 0,
                                    &mut out, self)) {
                    Continue | Stop => Ok(out),
                    More(section) => {
                        self.blocks.push(section);
                        Ok(out)
                    }
                }
            },
            &mut Short(ref s) => {
                let mut out = LinkedList::new();
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
                let mut out = LinkedList::new();
                out.push_back(Set(WashArgs::Flat(s.clone())));
                Ok(out)
            },
            &mut Long(ref mut v) => {
                let mut out = LinkedList::new();
                let mut count = 0;
                let mut items:LinkedList<InputValue> = v.drain().collect();
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
                    out.push_back(Pull);
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
                    aclist = LinkedList::new();
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

    pub fn optimize(&mut self) -> Result<(), String> {
        try!(self.opcombine());
        try!(self.jumpreduce());
        Ok(())
    }

    pub fn jumpreduce(&mut self) -> Result<bool, String> {
        // hashmap of sectiontype to (len, jumps_to, jumped_to_from)
        let mut jumps = HashMap::<SectionType, (usize, HashSet<SectionType>, HashSet<SectionType>)>::new();
        let mut visited = HashSet::new();
        let mut to_visit = vec![];
        let mut position = SectionType::Run;
        let mut graphdone = false;
        let mut changes = false;
        loop {
            while visited.contains(&position) {
                position = match to_visit.pop() {
                    None => {
                        graphdone = true;
                        break;
                    },
                    Some(sec) => sec
                };
            }
            if graphdone {
                // finished visiting everything
                break;
            }
            visited.insert(position);
            let section = match self.sections.get(&position) {
                None => return Err(format!("Section not found: {:?}", position)),
                Some(sec) => sec
            };
            if jumps.contains_key(&position) {
                match jumps.get_mut(&position) {
                    None => panic!("contains_key and get_mut returned differently"),
                    Some(sec) => {
                        sec.0 = section.len();
                    }
                }
            } else {
                jumps.insert(position, (section.len(), HashSet::new(), HashSet::new()));
            }
            for item in section.iter() {
                match item {
                    &Jump(ref n) => {
                        if jumps.contains_key(&SectionType::Number(*n)) {
                            match jumps.get_mut(&SectionType::Number(*n)) {
                                None => panic!("contains_key and get_mut returned differently"),
                                Some(sec) => {
                                    sec.2.insert(position);
                                }
                            }
                        } else {
                            jumps.insert(SectionType::Number(*n),
                                         (0, HashSet::new(), {
                                             let mut t = HashSet::new();
                                             t.insert(position);
                                             t
                                         }));
                        }
                        if jumps.contains_key(&position) {
                            match jumps.get_mut(&position) {
                                None => panic!("contains_key and get_mut returned differently"),
                                Some(sec) => {
                                    sec.1.insert(SectionType::Number(*n));
                                }
                            }
                        } else {
                            jumps.insert(position, (section.len(), {
                                let mut t = HashSet::new();
                                t.insert(SectionType::Number(*n));
                                t
                            }, HashSet::new()));
                        }
                        to_visit.push(SectionType::Number(*n));
                    },
                    &Branch(ref n) | &Root(ref n) => {
                        to_visit.push(SectionType::Number(*n));
                    },
                    _ => {}
                }
            }
        }
        to_visit.clear();
        let mut moved = HashMap::<SectionType, SectionType>::new();
        for sec in jumps.keys() {
            to_visit.push(*sec);
        }
        loop {
            position = match to_visit.pop() {
                None => break,
                Some(sec) => sec
            };
            if moved.contains_key(&position) {
                // skip this section, it's been moved
                continue;
            }
            let info = match jumps.remove(&position) {
                None => continue, // this section has already been dealth with
                Some(v) => v
            };
            if info.0 == 0 {
                let num = match position {
                    SectionType::Number(n) => n,
                    _ => panic!(".run and .load can't be jumped to")
                };
                // this seciton is empty,
                // remove all jumps to it
                for sec in info.2.iter() {
                    let mut t = *sec;
                    while moved.contains_key(&t) {
                        t = *(moved.get(&t).unwrap());
                    }
                    let destsec = match self.sections.get_mut(&t) {
                        None => return Err(format!("Pass 0: Destination {:?} not found", t)),
                        Some(sec) => sec
                    };
                    loop {
                        match destsec.pop_back() {
                            None => return Err(format!("Pass 0: No jump found in section")),
                            Some(Jump(ref n)) if *n == num => {
                                // we've found the jump to our section
                                break;
                            },
                            _ => {/* continue */}
                        }
                    }
                    let destinfo = jumps.get_mut(&t).unwrap();
                    destinfo.1.remove(&position);
                    destinfo.0 = destsec.len();
                    to_visit.push(t);
                }
                self.sections.remove(&position);
                changes = true;
            } else if info.2.len() == 1 {
                // only jumped to once, so move ourselves to that point in that section
                let num = match position {
                    SectionType::Number(n) => n,
                    _ => panic!(".run and .load can't be jumped to")
                };
                let dest = {
                    let mut t = info.2.iter().next().unwrap().clone();
                    while moved.contains_key(&t) {
                        t = *(moved.get(&t).unwrap());
                    }
                    t
                };
                if position == dest {
                    // don't move to ourselves
                    continue;
                }
                let mut orig = match self.sections.remove(&position) {
                    None => return Err(format!("Pass 1: Original {:?} not found", position)),
                    Some(sec) => sec
                };
                moved.insert(position, dest);
                let destsec = match self.sections.get_mut(&dest) {
                    None => return Err(format!("Pass 1: Destination {:?} not found", dest)),
                    Some(sec) => sec
                };
                loop {
                    match destsec.pop_back() {
                        None => return Err(format!("Pass 1: No jump found in section")),
                        Some(Jump(ref n)) if *n == num => {
                            // we've found the jump to our section
                            destsec.append(&mut orig);
                            break;
                        },
                        _ => {/* continue */}
                    }
                }
                let destinfo = jumps.get_mut(&dest).unwrap();
                destinfo.1.remove(&position);
                destinfo.0 = destsec.len();
                to_visit.push(dest);
                changes = true;
            } else {
                jumps.insert(position, info);
            }
        }
        Ok(changes)
    }

    pub fn opcombine(&mut self) -> Result<bool, String> {
        let mut visited = HashSet::new();
        let mut to_visit = vec![];
        let mut position = SectionType::Run;
        let mut section; let mut out;
        let mut cfv_empty = true;
        let mut changes = false;
        loop {
            while visited.contains(&position) {
                position = match to_visit.pop() {
                    None => return Ok(changes), // we're done
                    Some(sec) => sec
                };
            }
            visited.insert(position);
            section = match self.sections.remove(&position) {
                None => return Err(format!("Section not found: {:?}", position)),
                Some(sec) => sec
            };
            out = LinkedList::new();
            loop {
                let item = match section.pop_front() {
                    None => break,
                    Some(item) => item
                };
                match item {
                    Jump(sec) => {
                        to_visit.push(SectionType::Number(sec));
                        out.push_back(Jump(sec));
                        if !section.is_empty() {
                            changes = true;
                        }
                        // any statements after this cannot be run
                        break;
                    },
                    Branch(sec) => {
                        to_visit.push(SectionType::Number(sec));
                        if cfv_empty {
                            changes = true;
                            // this statement can only succeed
                            out.push_back(Jump(sec));
                            // any statements after this are pointless
                            break;
                        } else {
                            out.push_back(Branch(sec));
                        }
                    },
                    Root(sec) => {
                        to_visit.push(SectionType::Number(sec));
                        out.push_back(Root(sec));
                    },
                    Set(v) => match section.front() {
                        Some(&Temp) => {
                            // set followed by a temp can be turned into
                            // just an insert
                            out.push_back(Insert(v));
                            section.pop_front();
                            cfv_empty = true;
                            changes = true;
                        },
                        _ => {
                            if v == WashArgs::Empty {
                                cfv_empty = true;
                            } else {
                                cfv_empty = false;
                            }
                            out.push_back(Set(v));
                        }
                    },
                    Join(n) => match section.front() {
                        Some(&Call(_)) => {
                            // join then call can be turned into proc
                            // then pull
                            let name = match section.pop_front() {
                                Some(Call(n)) => n,
                                _ => panic!("front and pop_front returned differently")
                            };
                            out.push_back(Proc(name, n));
                            // insert pull as a process step
                            section.push_front(Pull);
                            changes = true;
                        },
                        _ => {
                            out.push_back(Join(n));
                            cfv_empty = false;
                        }
                    },
                    Pull => match section.front() {
                        Some(&Temp) => {
                            // inverse statements
                            section.pop_front();
                            changes = true;
                        },
                        Some(&Call(_)) => {
                            let name = match section.pop_front() {
                                Some(Call(n)) => n,
                                _ => panic!("front and pop_front returned differently")
                            };
                            out.push_back(Proc(name, 1));
                            section.push_front(Pull);
                            changes = true;
                        },
                        Some(&Branch(_)) => {
                            let num = match section.pop_front() {
                                Some(Branch(n)) => n,
                                _ => panic!("front and pop_front returned differently")
                            };
                            section.push_front(Root(num));
                            changes = true;
                        },
                        _ => {
                            out.push_back(Pull);
                            cfv_empty = false;
                        }
                    },
                    Load => {
                        if section.front() == Some(&Temp) {
                            match out.back() {
                                Some(&Set(WashArgs::Flat(_))) => {
                                    let var = match out.pop_back() {
                                        Some(Set(WashArgs::Flat(s))) => s,
                                        _ => panic!("bock and pop_back returned differently")
                                    };
                                    let name; let path;
                                    match VAR_PATH_REGEX.captures(var.as_slice()) {
                                        None => match VAR_REGEX.captures(var.as_slice()) {
                                            None => return Err(format!("Load would have failed with {}", var)),
                                            Some(caps) => {
                                                name = caps.at(1).unwrap().to_string();
                                                path = String::new();
                                            }
                                        },
                                        Some(caps) => {
                                            name = caps.at(2).unwrap().to_string();
                                            path = caps.at(1).unwrap().to_string();
                                        }
                                    }
                                    section.pop_front();
                                    out.push_back(Stack(name, path));
                                    changes = true;
                                },
                                _ => {
                                    out.push_back(Load);
                                    cfv_empty = false;
                                }
                            }
                        } else {
                            out.push_back(Load);
                            cfv_empty = false;
                        }
                    },
                    Store | Temp => {
                        out.push_back(item);
                        cfv_empty = true;
                    },
                    Top | Swap => {
                        out.push_back(item);
                        cfv_empty = false;
                    }
                    v => {
                        out.push_back(v);
                    }
                }
            }
            self.sections.insert(position, out);
        }
    }

    pub fn evaluate(&mut self) -> Result<WashArgs, String> {
        if self.in_block() {
            return Err(format!("Tried to evaluate while in block"));
        }
        self.position = SectionType::Run;
        let mut cfv = WashArgs::Empty;
        let mut vs = LinkedList::new();
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
                        Root(n) => {
                            let top = match vs.pop_back() {
                                None => WashArgs::Empty,
                                Some(v) => v
                            };
                            if top.is_empty() {
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
                        Pull => {
                            match vs.pop_back() {
                                None => cfv = WashArgs::Empty,
                                Some(v) => cfv = v
                            }
                        },
                        Swap => {
                            let top = match vs.pop_back() {
                                None => WashArgs::Empty,
                                Some(v) => v
                            };
                            vs.push_back(cfv);
                            cfv = top;
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
