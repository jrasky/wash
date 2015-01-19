use libc::*;

use sodiumoxide::crypto::hash::sha256;

use serialize::hex::ToHex;

use std::io::process::{Command, ProcessOutput, ProcessExit};
use std::io::fs::PathExtensions;
use std::collections::HashMap;

use std::io;
use std::ffi;
use std::str;
use std::mem;
use std::cmp::*;
use std::os;
use std::fmt;

use controls::*;
use constants::*;
use command::*;
use input::*;

use self::WashArgs::*;

// !!!
// Wash function calling convention
pub type WashFunc = fn(&WashArgs, &mut WashEnv) -> WashArgs;

// >Dat pointer indirection
// Sorry bro, Rust doesn't have DSTs yet
// Once it does they'll turn into a more compact structure
pub type VarTable = HashMap<String, WashArgs>;
pub type FuncTable = HashMap<String, WashFunc>;
pub type ScriptTable = HashMap<Path, WashScript>;
pub type PathTable = HashMap<String, VarTable>;

// WashLoad returns two lists, the first of initialized functions,
// the second the same of variables
type WashLoad = extern fn(*const WashArgs, *mut WashEnv) -> WashArgs;
type WashRun = extern fn(*const WashArgs, *mut WashEnv) -> WashArgs;

#[derive(Clone)]
pub enum WashArgs {
    Flat(String),
    Long(Vec<WashArgs>),
    Empty
}

pub struct WashEnv {
    pub paths: PathTable,
    pub variables: String,
    pub functions: FuncTable,
    pub scripts: ScriptTable,
    term: TermState
}

pub struct WashScript {
    pub path: Path,
    pub hash: String,
    controls: Controls,
    handle: *const c_void,
    run_ptr: *const c_void,
    load_ptr: *const c_void,
    pub loaded: bool
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

impl WashEnv {
    pub fn new() -> WashEnv {
        WashEnv {
            paths: HashMap::new(),
            variables: String::new(),
            functions: HashMap::new(),
            scripts: HashMap::new(),
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

    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Option<ProcessExit> {
        self.term.run_command(name, args)
    }

    pub fn run_command_directed(&mut self, name:&String, args:&Vec<String>) -> Option<ProcessOutput> {
        self.term.run_command_directed(name, args)
    }

    pub fn hasv(&self, name:&String) -> bool {
        match self.paths.get(&self.variables) {
            None => false,
            Some(table) => return table.contains_key(name)
        }
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

    pub fn insv(&mut self, name:String, val:WashArgs) {
        let path = self.variables.clone();
        if !self.hasp(&path) {
            self.insp(path.clone())
        }
        self.insvp(name, path, val);
    }

    pub fn insvp(&mut self, name:String, path:String, val:WashArgs) {
        if path == "env" {
            if !val.is_flat() {
                self.term.controls.err("Environment variables can only be flat");
                return;
            }
            os::setenv(name.as_slice(), val.flatten().as_slice());
        } else {
            if !self.hasp(&path) {
                self.insp(path.clone())
            }
            self.paths.get_mut(path.as_slice()).unwrap().insert(name, val);
        }
    }

    pub fn insp(&mut self, path:String) {
        self.paths.insert(path, HashMap::new());
    }

    pub fn insf(&mut self, name:&str, func:WashFunc) {
        self.functions.insert(name.to_string(), func);
    }

    pub fn getv(&self, name:&String) -> WashArgs {
        return match self.getvp(name, &self.variables) {
            Empty => return self.getvp(name, &"".to_string()),
            v => return v
        };
    }

    pub fn getall(&self) -> WashArgs {
        let mut out = match self.getallp(&self.variables) {
            Long(v) => v,
            _ => vec![]
        };
        if !self.variables.is_empty() {
            for item in match self.getallp(&"".to_string()) {
                Long(v) => v,
                _ => return Long(out)
            }.iter() {
                out.push(item.clone());
            }
        }
        return Long(out);
    }
    
    pub fn getallp(&self, path:&String) -> WashArgs {
        if *path == "env".to_string() {
            let mut out = vec![];
            let envs = os::env();
            for &(ref name, ref value) in envs.iter() {
                out.push(Long(vec![Flat(name.clone()), Flat(value.clone())]));
            }
            return Long(out);
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
            return Long(out);
        } else {
            return Empty;
        }
    }

    pub fn getvp(&self, name:&String, path:&String) -> WashArgs {
        if *path == "env".to_string() {
            return match os::getenv(name.as_slice()) {
                None => Empty,
                Some(v) => Flat(v)
            };
        } else {
            return match self.paths.get(path) {
                None => Empty,
                Some(table) => match table.get(name) {
                    None => Empty,
                    Some(val) => val.clone()
                }
            };
        }
    }

    pub fn runf(&mut self, name:&String, args:&WashArgs) -> WashArgs {
        let func = match self.functions.get(name) {
            None => return Empty,
            Some(func) => func.clone()
        };
        return func(args, self);
    }

    pub fn load_script(&mut self, path:Path, args:&WashArgs) -> WashArgs {
        let mut script = match self.scripts.remove(&path) {
            Some(script) => script,
            None => WashScript::new(path.clone())
        };
        if !script.is_compiled() && !script.compile() {
            self.term.controls.err("Failed to compile script\n");
            return Empty;
        }
        self.term.controls.flush();
        if script.is_runnable() {
            let out = script.run(args, self);
            self.scripts.insert(path.clone(), script);
            return out;
        } else if script.is_loadable() {
            let out = script.load(args, self);
            self.scripts.insert(path.clone(), script);
            return out;
        } else {
            self.term.controls.err("Cannot load or run script\n");
            return Empty;
        }
    }
    
    pub fn process_command(&mut self, args:Vec<WashArgs>) -> Result<WashArgs, String> {
        let out = try!(self.process_function("run".to_string(), args));
        let outv = out.flatten_vec();
        if out.is_empty() {
            return Err("Command failed".to_string());
        } else if outv.len() < 2 {
            return Err(format!("Command failed: {}", out.flatten()));
        } else if outv != vec!["status", "0"] {
            return Err(format!("Command failed with {} {}", outv[0], outv[1]));
        } else {
            return Ok(WashArgs::Empty);
        }
    }

    pub fn process_function(&mut self, name:String, args:Vec<WashArgs>) -> Result<WashArgs, String> {
        if self.hasf(&name) {
            return Ok(self.runf(&name, &WashArgs::Long(args)));
        } else {
            return Err("Function not found".to_string());
        }
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
        let mut args = vec![];
        for item in input.iter() {
            match try!(self.input_to_args(item.clone())) {
                Empty => {/* do nothing */},
                v => args.push(v)
            }
        }
        return Ok(args);
    }

    pub fn input_to_args(&mut self, input:InputValue) -> Result<WashArgs, String> {
        match input {
            InputValue::Function(n, a) => {
                let vec = try!(self.input_to_vec(a));
                return self.process_function(n, vec);
            },
            InputValue::Long(a) => {
                let mut args = vec![];
                for item in a.iter() {
                    match try!(self.input_to_args(item.clone())) {
                        Empty => {/* do nothing */},
                        v => args.push(v)
                    }
                }
                return Ok(Long(args));
            },
            // the special cases with regex make for more informative errors
            InputValue::Short(ref s) if VAR_PATH_REGEX.is_match(s.as_slice()) => {
                let caps = VAR_PATH_REGEX.captures(s.as_slice()).unwrap();
                let path = caps.at(1).unwrap().to_string();
                let name = caps.at(2).unwrap().to_string();
                if name.is_empty() {
                    if path.is_empty() {
                        return match self.getall() {
                            Empty => Err(format!("Path not found: {}", path)),
                            v => Ok(v)
                        }
                    } else {
                        return match self.getallp(&path) {
                            Empty => Err(format!("Path not found: {}", path)),
                            v => Ok(v)
                        }
                    }
                } else {
                    return match self.getvp(&name, &path) {
                        Empty => Err(format!("Variable not found: {}:{}", path, name)),
                        v => Ok(v)
                    }
                }
            },
            InputValue::Short(ref s) if VAR_REGEX.is_match(s.as_slice()) => {
                let caps = VAR_REGEX.captures(s.as_slice()).unwrap();
                let name = caps.at(1).unwrap().to_string();
                return match self.getv(&name) {
                    Empty => Err(format!("Variable not found: {}", name)),
                    v => Ok(v)
                }
            },
            InputValue::Short(s) | InputValue::Literal(s) => return Ok(Flat(s)),
            InputValue::Split(_) => return Ok(Empty)
        }
    }

}

impl Drop for WashScript {
    fn drop(&mut self) {
        self.close();
    }
}

impl WashScript {
    pub fn new(path:Path) -> WashScript {
        WashScript {
            path: path,
            hash: String::new(),
            controls: Controls::new(),
            handle: 0 as *const c_void,
            run_ptr: 0 as *const c_void,
            load_ptr: 0 as *const c_void,
            loaded: false
        }
    }

    pub fn is_runnable(&self) -> bool {
        !self.run_ptr.is_null()
    }

    pub fn is_loadable(&self) -> bool {
        !self.load_ptr.is_null()
    }

    pub fn is_compiled(&self) -> bool {
        !self.handle.is_null()
    }

    pub fn run(&mut self, args:&WashArgs, env:&mut WashEnv) -> WashArgs {
        if !self.is_compiled() {
            self.controls.err("Script not compiled\n");
            return Empty;
        }
        
        let run_func:WashRun = unsafe {
            match self.run_ptr.as_ref() {
                Some(f) => mem::transmute(f),
                None => {
                    self.controls.err("Script cannot be run directly\n");
                    return Empty;
                }
            }
        };

        if !self.loaded && self.is_loadable() {
            self.load(args, env);
        }

        return run_func(args, env);
    }

    pub fn load(&mut self, args:&WashArgs, env:&mut WashEnv) -> WashArgs {
        if !self.is_compiled() {
            self.controls.err("Script is not compiled\n");
            return Empty;
        }
        
        let load_func:WashLoad = unsafe {
            match self.load_ptr.as_ref() {
                Some(f) => mem::transmute(f),
                None => {
                    self.controls.err("Script has no load actions\n");
                    return Empty;
                }
            }
        };

        if self.loaded {
            self.controls.err("Script already loaded\n");
        }

        let out = load_func(args, env);
        self.loaded = true;
        return out;
    }

    pub fn close(&mut self) {
        if self.is_compiled() {
            // prevent memory leaks
            unsafe {
                match dlclose(self.handle) {
                    0 => {
                        // nothing
                    },
                    _ => {
                        let c = dlerror();
                        let e = str::from_utf8(ffi::c_str_to_bytes(&c)).unwrap();
                        self.controls.errf(format_args!("Couldn't unload wash script: {}\n", e));
                    }
                }
            }
        }
        self.handle = 0 as *const c_void;
        self.run_ptr = 0 as *const c_void;
        self.load_ptr = 0 as *const c_void;
    }

    pub fn compile(&mut self) -> bool {
        if self.is_compiled() {
            // script is already compiled
            return true;
        }
        if !self.path.exists() {
            self.controls.errf(format_args!("Could not find {}\n", self.path.display()));
            return false;
        }
        let inf = match io::File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                self.controls.errf(format_args!("File error: {}\n", e));
                return false;
            }
        };
        let mut reader = io::BufferedReader::new(inf);
        let content_s = reader.read_to_end().unwrap();
        let contents = content_s.as_slice();
        self.hash = sha256::hash(contents).0.to_hex();
        let outp = {
            let mut outname = self.hash.clone();
            outname.push_str(".wo");
            Path::new(WO_PATH).join(outname)
        };
        if !outp.exists() {
            // scripts needs to be compiled
            match io::fs::mkdir_recursive(&outp.dir_path(), io::USER_RWX) {
                Ok(_) => {
                    // nothing
                },
                Err(e) => {
                    self.controls.errf(format_args!("Couldn't create wash script cache directory: {}\n", e));
                    return false;
                }
            }
            let mut command = Command::new("rustc");
            command.args(&["-o", outp.as_str().unwrap(), "-"]);
            let mut child = match command.spawn() {
                Err(e) => {
                    self.controls.errf(format_args!("Couldn't start compiler: {}\n", e));
                    return false;
                },
                Ok(c) => c
            };
            {
                // TODO: maybe write match statements instead of
                // just calling unwrap
                let mut input = child.stdin.as_mut().unwrap();
                input.write(contents).unwrap();
                input.flush().unwrap();
            }

            match child.wait_with_output() {
                Err(e) => {
                    self.controls.errf(format_args!("Compiler failed to run: {}\n", e));
                    return false;
                },
                Ok(o) => {
                    if !o.status.success() {
                        self.controls.errf(format_args!("Couldn't compile script: {}\n",
                                                        String::from_utf8(o.error).unwrap()));
                        return false;
                    }
                }
            }
        }

        let path_cstr = ffi::CString::from_slice(outp.as_str().unwrap().as_bytes());
        let run_cstr = ffi::CString::from_slice(WASH_RUN_SYMBOL.as_bytes());
        let load_cstr = ffi::CString::from_slice(WASH_LOAD_SYMBOL.as_bytes());
        unsafe {
            self.handle = dlopen(path_cstr.as_ptr(), RTLD_LAZY|RTLD_LOCAL);
            if self.handle.is_null() {
                let c = dlerror();
                let e = str::from_utf8(ffi::c_str_to_bytes(&c)).unwrap();
                self.controls.errf(format_args!("Could not load script object: {}\n", e));
                return false;
            }
            
            self.run_ptr = dlsym(self.handle, run_cstr.as_ptr());
            if self.run_ptr.is_null() {
                // this script isn't run directly
                // clear error message
                dlerror();
            }
            
            self.load_ptr = dlsym(self.handle, load_cstr.as_ptr());
            if self.load_ptr.is_null() {
                // this script is only run directly
                // clear error message
                dlerror();
            }
        }
        if self.load_ptr.is_null() && self.run_ptr.is_null() {
            self.controls.err("No load or run function found in script object\n");
            self.close();
            return false;
        }
        // success!
        return true;
        
    }
}

#[link(name = "dl")]
extern {
    fn dlopen(filename:*const c_char, flag:c_int) -> *const c_void;
    fn dlsym(handle:*const c_void, symbol:*const c_char) -> *const c_void;
    fn dlclose(handle:*const c_void) -> c_int;
    fn dlerror() -> *const c_char;
}

