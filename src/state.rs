use std::collections::HashMap;

use std::cmp::*;

use types::WashArgs::*;

use constants::*;
use types::*;
use util::*;
use env::*;

use self::HandlerResult::*;

// Note with handlers: Err means Stop, not necessarily Fail
// return semi-redundant result type because try! is so damn useful
pub type WashHandler = fn(&mut Vec<WashArgs>, &mut Vec<InputValue>, &mut ShellState) -> Result<HandlerResult, String>;

pub type HandlerTable = HashMap<String, WashHandler>;

pub enum HandlerResult {
    Continue,
    Stop,
    More(WashBlock)
}

#[derive(Clone)]
pub struct WashBlock {
    pub start: String,
    pub next: Vec<InputValue>,
    pub close: Vec<InputValue>,
    pub content: Vec<InputValue>
}

pub struct ShellState {
    pub env: WashEnv,
    blocks: Vec<WashBlock>,
    handlers: HandlerTable,
    last: Option<Result<WashArgs, String>>
}

impl ShellState {
    pub fn new() -> ShellState {
        ShellState {
            handlers: HashMap::new(),
            env: WashEnv::new(),
            blocks: vec![],
            last: None
        }
    }

    pub fn in_block(&self) -> bool {
        return !self.blocks.is_empty();
    }
    
    pub fn has_handler(&self, word:&String) -> bool {
        return self.handlers.contains_key(word);
    }

    pub fn insert_handler(&mut self, word:&str, func:WashHandler) -> Result<WashArgs, String> {
        self.handlers.insert(word.to_string(), func);
        return Ok(Empty);
    }

    pub fn run_handler(&mut self, word:&String, pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>) -> Result<HandlerResult, String> {
        let func = match self.handlers.get(word) {
            None => return Err("Handler not found".to_string()),
            Some(func) => func.clone()
        };
        return func(pre, next, self);
    }
        
    pub fn process_command(&mut self, args:Vec<WashArgs>) -> Result<WashArgs, String> {
        if args.is_empty() {
            // this happens when a handler ends a line and passes nothing on
            return Ok(Empty);
        } else if self.env.hasf(&args[0].flatten()) {
            // run as a function instead
            return self.env.runf(&args[0].flatten(), &Long(args[min(1, args.len())..].to_vec()));
        } else {
            let out = try!(self.process_function("run".to_string(), args));
            return self.env.describe_process_output(&out);
        }
    }

    pub fn process_function(&mut self, name:String, args:Vec<WashArgs>) -> Result<WashArgs, String> {
        let out = try!(self.env.runf(&name, &WashArgs::Long(args)));
        return Ok(out);
    }

    pub fn process_lines<'a, T:Iterator<Item=&'a InputValue>>(&mut self, mut lines:T) -> Result<WashArgs, String> {
        let mut out = Flat(String::new());
        // handle sigint
        self.env.handle_sigint();
        // prevent env from unsetting that handler
        self.env.catch_sigint = false;
        for line in lines {
            // check for stop
            try!(self.env.func_stop());
            out = match self.process_line(line.clone()) {
                Err(ref e) if *e == STOP => Empty,
                Err(e) => {
                    self.env.catch_sigint = true;
                    self.env.unhandle_sigint();
                    return Err(e);
                },
                Ok(v) => v
            }
        }
        self.env.catch_sigint = true;
        self.env.unhandle_sigint();
        return Ok(out);
    }

    pub fn process_block(&mut self) -> Result<WashArgs, String> {
        if self.blocks.is_empty() {
            return Err("No block defined".to_string());
        }
        let block = self.blocks.pop().unwrap();
        if block.start == "act" {
            return self.process_lines(block.content.iter());
        } else if block.start == "if" || block.start == "else" {
            let mut cond = self.last.clone().unwrap_or(Err("No last value".to_string()));
            let next_empty = block.next.is_empty();
            if block.start == "else" && cond.is_ok() {
                // return early in the else case
                return Err(STOP.to_string());
            }
            if !next_empty {
                cond = self.process_line(InputValue::Long(block.next));
            }
            if cond.is_ok() || (block.start == "else" && next_empty) {
                return self.process_lines(block.content.iter());
            } else {
                return Err(STOP.to_string());
            }
        } else {
            return Err(format!("Don't know how to handle block: {}", block.start));
        }
    }

    pub fn process_line(&mut self, line:InputValue) -> Result<WashArgs, String> {
        let out = self.process_line_inner(line);
        self.last = Some(out.clone());
        return out;
    }

    pub fn process_line_inner(&mut self, line:InputValue) -> Result<WashArgs, String> {
        if self.blocks.is_empty() {
            match line {
                InputValue::Function(n, a) => {
                    let vec = try!(self.input_to_vec(a));
                    return self.process_function(n, vec);
                },
                InputValue::Long(a) => {
                    // run as command
                    let vec = try!(self.input_to_vec(a));
                    if vec.is_empty() {
                        if self.blocks.is_empty() {
                            return Ok(Empty);
                        } else if !self.blocks[0].close.is_empty() {
                            return Ok(Empty);
                        } else {
                            return self.process_block();
                        }
                    } else {
                        return self.process_command(vec);
                    }
                },
                InputValue::Short(ref s) if VAR_PATH_REGEX.is_match(s.as_slice()) => {
                    let out = try!(self.input_to_args(InputValue::Short(s.clone())));
                    return Ok(Flat(format!("{}\n", out.flatten_with_inner("\n", "="))));
                },
                InputValue::Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
                    let out = try!(self.input_to_args(InputValue::Short(s.clone())));
                    return Ok(Flat(format!("{}\n", out.flatten())));
                },
                InputValue::Short(s) | InputValue::Literal(s) => {
                    // run command without args
                    return self.process_command(vec![Flat(s)]);
                },
                _ => {
                    // do nothing
                    return Ok(Flat(String::new()));
                }
            }
        } else {
            if self.blocks[0].close.is_empty() {
                return self.process_block();
            } else if self.blocks[0].close[0] == line.clone() {
                self.blocks[0].close.pop();
                if self.blocks[0].close.is_empty() {
                    return self.process_block();
                } else {
                    self.blocks[0].content.push(line);
                    return Ok(Empty);
                }
            } else {
                match line {
                    InputValue::Long(ref v) =>
                        if create_content(&mut v.clone()) == Ok(vec![]) {
                            self.blocks[0].close.push(InputValue::Short("}".to_string()));
                        },
                    _ => {}
                }
                self.blocks[0].content.push(line);
                // continue block
                return Ok(Empty);
            }
        }
    }

    pub fn input_to_vec(&mut self, input:Vec<InputValue>) -> Result<Vec<WashArgs>, String> {
        let mut out = vec![];
        // avoid O(n^2) situation
        let mut iter = reverse(input);
        let mut scope = vec![];
        loop {
            match iter.pop() {
                None => break,
                Some(InputValue::Short(ref name)) if self.has_handler(name) => {
                    while match get_nm_index(&iter, iter.len() - 1) {
                        Some(&InputValue::Split(_)) => true,
                        _ => false
                    } {
                        // skip any splits after the handle sequence
                        iter.pop();
                    }
                    // produce a correct scope for the handler
                    scope.clear();
                    while match get_nm_index(&iter, iter.len() - 1) {
                        None => false,
                        Some(&InputValue::Split(ref ns)) if self.has_handler(ns) => false,
                        Some(_) => true
                    } {
                        // doing this means scope will be in the same order as input
                        scope.push(iter.pop().unwrap());
                    }
                    // this can change out and scope, be careful
                    match self.run_handler(name, &mut out, &mut scope) {
                        Ok(Stop) => return Err(STOP.to_string()),
                        Ok(More(block)) => {
                            // start of a block
                            self.blocks.push(block);
                            return Ok(vec![]);
                        },
                        Ok(Continue) => {/* continue */},
                        Err(e) => return Err(e) // this is an error
                    }
                    // push remaining scope back onto iter
                    loop {
                        match scope.pop() {
                            None => break,
                            Some(v) => iter.push(v)
                        }
                    }
                },
                Some(v) => {
                    match try!(self.input_to_args(v.clone())) {
                        Empty => {},
                        new => out.push(new)
                    }
                }
            };
        }
        return Ok(out);
    }

    pub fn input_to_args(&mut self, input:InputValue) -> Result<WashArgs, String> {
        match input {
            InputValue::Function(n, a) => {
                let vec = try!(self.input_to_vec(a));
                return self.process_function(n, vec);
            },
            InputValue::Long(a) => {
                return Ok(Long(try!(self.input_to_vec(a))));
            },
            // the special cases with regex make for more informative errors
            InputValue::Short(ref s) if VAR_PATH_REGEX.is_match(s.as_slice()) => {
                let caps = VAR_PATH_REGEX.captures(s.as_slice()).unwrap();
                let path = caps.at(1).unwrap().to_string();
                let name = caps.at(2).unwrap().to_string();
                if name.is_empty() {
                    if path.is_empty() {
                        return self.env.getall();
                    } else {
                        return self.env.getallp(&path);
                    }
                } else {
                    return self.env.getvp(&name, &path);
                }
            },
            InputValue::Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
                let caps = VAR_REGEX.captures(s.as_slice()).unwrap();
                let name = caps.at(1).unwrap().to_string();
                return self.env.getv(&name);
            },
            InputValue::Short(s) | InputValue::Literal(s) => return Ok(Flat(s)),
            InputValue::Split(_) => return Ok(Empty)
        }
    }
}
