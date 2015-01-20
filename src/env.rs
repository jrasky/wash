use std::io::process::{ProcessOutput, ProcessExit, Process};
use std::io::process::ProcessExit::*;
use std::collections::HashMap;
use std::os::unix::prelude::*;

use std::mem;
use std::os;
use std::fmt;

use types::WashArgs::*;

use constants::*;
use command::*;
use types::*;
use util::*;
use script::*;

use self::HandlerResult::*;

// !!!
// Wash function calling convention
pub type WashFunc = fn(&WashArgs, &mut WashEnv) -> Result<WashArgs, String>;

// Note with handlers: Err means Stop, not necessarily Fail
// return semi-redundant result type because try! is so damn useful
pub type WashHandler = fn(&mut Vec<WashArgs>, &mut Vec<InputValue>, &mut WashEnv) -> Result<HandlerResult, String>;

// >Dat pointer indirection
// Sorry bro, Rust doesn't have DSTs yet
// Once it does they'll turn into a more compact structure
pub type VarTable = HashMap<String, WashArgs>;
pub type FuncTable = HashMap<String, WashFunc>;
pub type ScriptTable = HashMap<Path, WashScript>;
pub type PathTable = HashMap<String, VarTable>;
pub type HandlerTable = HashMap<String, WashHandler>;

// WashLoad returns two lists, the first of initialized functions,
// the second the same of variables
type WashLoad = extern fn(*const WashArgs, *mut WashEnv) -> Result<WashArgs, String>;
type WashRun = extern fn(*const WashArgs, *mut WashEnv) -> Result<WashArgs, String>;

pub enum HandlerResult {
    Continue,
    More(String)
}

pub struct WashEnv {
    pub paths: PathTable,
    pub variables: String,
    pub functions: FuncTable,
    pub scripts: ScriptTable,
    pub handlers: HandlerTable,
    term: TermState
}

impl WashEnv {
    pub fn new() -> WashEnv {
        WashEnv {
            paths: HashMap::new(),
            variables: String::new(),
            functions: HashMap::new(),
            scripts: HashMap::new(),
            handlers: HashMap::new(),
            term: TermState::new()
        }
    }

    pub fn update_terminal(&mut self) {
        self.term.update_terminal();
    }

    pub fn restore_terminal(&mut self) {
        self.term.restore_terminal();
    }

    pub fn outc(&mut self, ch:char) {
        self.term.controls.outc(ch);
    }

    pub fn outs(&mut self, s:&str) {
        self.term.controls.outs(s);
    }

    pub fn outf(&mut self, args:fmt::Arguments) {
        self.term.controls.outf(args);
    }

    pub fn err(&mut self, s:&str) {
        self.term.controls.err(s);
    }

    pub fn errf(&mut self, args:fmt::Arguments) {
        self.term.controls.errf(args);
    }

    pub fn flush(&mut self) {
        self.term.controls.flush();
    }

    pub fn run_job(&mut self, name:&String, args:&Vec<String>) -> Result<(usize, String), String> {
        self.term.run_job(name, args)
    }

    pub fn run_job_directed(&mut self, name:&String, args:&Vec<String>) -> Result<(usize, String), String> {
        self.term.run_job_directed(name, args)
    }

    pub fn get_job(&self, id:&usize) -> Option<(String, &Process)> {
        return self.term.get_job(id);
    }

    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        self.term.run_command(name, args)
    }

    pub fn run_command_directed(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessOutput, String> {
        self.term.run_command_directed(name, args)
    }

    pub fn run_command_fd(&mut self, stdin:Fd, name:&String,
                          args:&Vec<String>) -> Result<ProcessExit, String> {
        self.term.run_command_fd(stdin, name, args)
    }

    pub fn run_job_fd(&mut self, stdin:Fd, name:&String,
                      args:&Vec<String>) -> Result<(usize, String), String> {
        self.term.run_job_fd(stdin, name, args)
    }

    pub fn has_handler(&self, word:&String) -> bool {
        return self.handlers.contains_key(word);
    }

    pub fn run_handler(&mut self, word:&String, pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>) -> Result<HandlerResult, String> {
        let func = match self.handlers.get(word) {
            None => return Err("Handler not found".to_string()),
            Some(func) => func.clone()
        };
        return func(pre, next, self);
    }

    pub fn insert_handler(&mut self, word:&str, func:WashHandler) -> Result<WashArgs, String> {
        self.handlers.insert(word.to_string(), func);
        return Ok(Empty);
    }

    pub fn hasv(&self, name:&String) -> bool {
        self.hasvp(name, &self.variables)
    }

    pub fn hasvp(&self, name:&String, path:&String) -> bool {
        match self.paths.get(path) {
            None => false,
            Some(table) => return table.contains_key(name)
        }
    }

    pub fn hasf(&self, name:&String) -> bool {
        self.functions.contains_key(name)
    }

    pub fn hasp(&self, path:&String) -> bool {
        self.paths.contains_key(path)
    }

    pub fn insv(&mut self, name:String, val:WashArgs) -> Result<WashArgs, String> {
        let path = self.variables.clone();
        if !self.hasp(&path) {
            try!(self.insp(path.clone()));
        }
        return self.insvp(name, path, val);
    }

    pub fn insvp(&mut self, name:String, path:String, val:WashArgs) -> Result<WashArgs, String> {
        if val.is_empty() {
            // unset
            if path == "env" {
                os::unsetenv(name.as_slice());
                return Ok(Empty);
            } else {
                if !self.hasp(&path) {
                    // effectively unset
                    return Ok(Empty);
                } else {
                    self.paths.get_mut(path.as_slice()).unwrap().remove(&name);
                    return Ok(val);
                }
            }
        } else {
            if path == "env" {
                if !val.is_flat() {
                    return Err("Environment variables can only be flat".to_string());
                }
                os::setenv(name.as_slice(), val.flatten().as_slice());
                return Ok(val);
            } else {
                if !self.hasp(&path) {
                    try!(self.insp(path.clone()));
                }
                self.paths.get_mut(path.as_slice()).unwrap().insert(name, val.clone());
                return Ok(val);
            }
        }
    }

    pub fn insp(&mut self, path:String) -> Result<WashArgs, String> {
        self.paths.insert(path, HashMap::new());
        return Ok(Empty);
    }

    pub fn insf(&mut self, name:&str, func:WashFunc) -> Result<WashArgs, String> {
        self.functions.insert(name.to_string(), func);
        return Ok(Empty);
    }


    pub fn getv(&self, name:&String) -> Result<WashArgs, String> {
        return match self.getvp(name, &self.variables) {
            Err(_) => return self.getvp(name, &"".to_string()),
            v => v
        };
    }

    pub fn getall(&self) -> Result<WashArgs, String> {
        let mut out = match self.getallp(&self.variables) {
            Ok(Long(v)) => v,
            _ => vec![]
        };
        if !self.variables.is_empty() {
            for item in match self.getallp(&"".to_string()) {
                Ok(Long(v)) => v,
                _ => return Ok(Long(out))
            }.iter() {
                if !self.hasv(&item.get(0).flatten()) {
                    out.push(item.clone());
                }
            }
        }
        return Ok(Long(out));
    }
    
    pub fn getallp(&self, path:&String) -> Result<WashArgs, String> {
        if *path == "env".to_string() {
            let mut out = vec![];
            let envs = os::env();
            for &(ref name, ref value) in envs.iter() {
                out.push(Long(vec![Flat(name.clone()), Flat(value.clone())]));
            }
            return Ok(Long(out));
        } else if *path == "pipe".to_string() {
            // list of non-background jobs (which can be piped)
            let mut out = vec![];
            for &(ref id, ref name, ref job) in self.term.get_jobs().iter() {
                match job.stdout {
                    None => {/* don't include this job */},
                    Some(_) => {
                        // include this job
                        out.push(Long(vec![Flat(format!("{}", id)),
                                           Flat(name.clone())]));
                    }
                }
            }
            return Ok(Long(out));
        } else if self.hasp(path) {
            let mut out = vec![];
            let vars = self.paths.get(path).unwrap();
            let mut names = vars.keys();
            let mut values = vars.values();
            loop {
                match (names.next(), values.next()) {
                    (Some(name), Some(value)) =>
                        out.push(Long(vec![Flat(name.clone()), value.clone()])),
                    _ => break
                }
            }
            return Ok(Long(out));
        } else {
            return Err("Path not found".to_string());
        }
    }

    pub fn getvp(&self, name:&String, path:&String) -> Result<WashArgs, String> {
        if *path == "env".to_string() {
            // environment variables
            return match os::getenv(name.as_slice()) {
                None => Err("Environment variable not found".to_string()),
                Some(v) => Ok(Flat(v))
            };
        } else if *path == "pipe".to_string() {
            // pipe Fd's
            let (_, from) = match self.get_job(&match str_to_usize(name.as_slice()) {
                None => return Err("Did not give job number".to_string()),
                Some(v) => v
            }) {
                None => return Err("Job not found".to_string()),
                Some(p) => p
            };
            match from.stdout {
                None => return Err("Job has no output handles".to_string()),
                Some(ref p) => Ok(Flat(format!("{}", p.as_raw_fd())))
            }
        } else {
            return match self.paths.get(path) {
                None => Err("Path not found".to_string()),
                Some(table) => match table.get(name) {
                    None => Err("Variable not found".to_string()),
                    Some(val) => Ok(val.clone())
                }
            };
        }
    }

    pub fn runf(&mut self, name:&String, args:&WashArgs) -> Result<WashArgs, String> {
        let func = match self.functions.get(name) {
            None => return Err("Function not found".to_string()),
            Some(func) => func.clone()
        };
        return func(args, self);
    }

    pub fn runfs(&mut self, name:&str, args:&WashArgs) -> Result<WashArgs, String> {
        return self.runf(&name.to_string(), args);
    }

    pub fn load_script(&mut self, path:Path, args:&WashArgs) -> Result<WashArgs, String> {
        let mut script = match self.scripts.remove(&path) {
            Some(script) => script,
            None => WashScript::new(path.clone())
        };
        if !script.is_compiled() && !try!(script.compile()) {
            return Err("Failed to compile script".to_string());
        }
        self.term.controls.flush();
        if script.is_runnable() {
            let out = self.run_script(args, &mut script);
            self.scripts.insert(path.clone(), script);
            return out;
        } else if script.is_loadable() {
            let out = self.load_script_inner(args, &mut script);
            self.scripts.insert(path.clone(), script);
            return out;
        } else {
            return Err("Cannot load or run script".to_string());
        }
    }

    fn describe_process_output(&mut self, out:&WashArgs) -> Result<WashArgs, String> {
        return self.runfs("describe_process_output", out);
    }
    
    fn run_script(&mut self, args:&WashArgs, script:&mut WashScript) -> Result<WashArgs, String> {
        if !script.is_compiled() {
            return Err("String is not compiled".to_string());
        }
        
        let run_func:WashRun = unsafe {mem::transmute(try!(script.get_run()))};

        if !script.loaded && script.is_loadable() {
            try!(self.load_script_inner(args, script));
        }

        return run_func(args, self);
    }

    fn load_script_inner(&mut self, args:&WashArgs, script:&mut WashScript) -> Result<WashArgs, String> {
        if !script.is_compiled() {
            return Err("Script is not compiled".to_string());
        }

        let load_func:WashLoad = unsafe {mem::transmute(try!(script.get_load()))};

        if script.loaded {
            return Err("Script is already loaded".to_string());
        }

        let out = load_func(args, self);
        script.loaded = true;
        return out;
    }

    pub fn get_jobs(&mut self) -> WashArgs {
        let mut out = vec![];
        for &(ref id, ref name, ref job) in self.term.get_jobs().iter() {
            match job.stdout {
                None => {
                    // background job
                    out.push(Flat(format!("{}: background job {}", name, id)));
                },
                Some(_) => {
                    // foreground job
                    out.push(Flat(format!("{}: job {}", name, id)));
                }
            }
        }
        return Long(out);
    }

    pub fn clean_jobs(&mut self) -> WashArgs {
        let mut out = vec![];
        let result = self.term.clean_jobs();
        for &(ref id, ref name, ref job) in result.iter() {
            match job {
                &Err(ref e) => out.push(Flat(format!("Job {} ({}) failed: {}", id, name, e))),
                &Ok(v) => {
                    if v.success() {
                        out.push(Flat(format!("Job {} ({}) finished", id, name)));
                    } else {
                        match v {
                            ExitSignal(sig) => {
                                out.push(Flat(format!("Job {} ({}) failed: signal {}", id, name, sig)));
                            },
                            ExitStatus(status) => {
                                out.push(Flat(format!("Job {} ({}) failed: status {}", id, name, status)));
                            }
                        }
                    }
                }
            }
        }
        return Long(out);
    }
    
    pub fn process_command(&mut self, args:Vec<WashArgs>) -> Result<WashArgs, String> {
        if args.is_empty() {
            // this happens when a handler ends a line and passes nothing on
            return Ok(Empty);
        }
        let out = try!(self.process_function("run".to_string(), args));
        return self.describe_process_output(&out);
    }

    pub fn process_function(&mut self, name:String, args:Vec<WashArgs>) -> Result<WashArgs, String> {
        let out = try!(self.runf(&name, &WashArgs::Long(args)));
        return Ok(out);
    }

    pub fn process_line(&mut self, line:InputValue) -> Result<WashArgs, String> {
        match line {
            InputValue::Function(n, a) => {
                let vec = try!(self.input_to_vec(a));
                return self.process_function(n, vec);
            },
            InputValue::Long(a) => {
                // run as command
                let vec = try!(self.input_to_vec(a));
                return self.process_command(vec);
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
                    match try!(self.run_handler(name, &mut out, &mut scope)) {
                        More(_) => return Err("More not yet implemented".to_string()),
                        Continue => {/* continue */}
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
                        return self.getall();
                    } else {
                        return self.getallp(&path);
                    }
                } else {
                    return self.getvp(&name, &path);
                }
            },
            InputValue::Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
                let caps = VAR_REGEX.captures(s.as_slice()).unwrap();
                let name = caps.at(1).unwrap().to_string();
                return self.getv(&name);
            },
            InputValue::Short(s) | InputValue::Literal(s) => return Ok(Flat(s)),
            InputValue::Split(_) => return Ok(Empty)
        }
    }

}
