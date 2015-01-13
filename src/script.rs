use libc::*;

use sodiumoxide::crypto::hash::sha256;

use serialize::hex::ToHex;

use std::io::process::Command;
use std::io::fs::PathExtensions;
use std::collections::HashMap;

use std::io;
use std::ffi;
use std::str;
use std::mem;
use std::cmp::*;

use controls::*;
use constants::*;
use command::*;

use self::WashArgs::*;

// !!!
// Wash function calling convention
pub type WashFunc = fn(&WashArgs, &mut WashEnv, &mut TermState) -> WashArgs;

// >Dat pointer indirection
// Sorry bro, Rust doesn't have DSTs yet
// Once it does they'll turn into a more compact structure
pub type VarTable = HashMap<String, WashArgs>;
pub type FuncTable = HashMap<String, WashFunc>;
pub type ScriptTable = HashMap<Path, WashScript>;

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
    pub variables: VarTable,
    pub functions: FuncTable,
    pub scripts: ScriptTable,
    pub controls: Controls
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

    pub fn from_vec(vec:Vec<String>) -> WashArgs {
        return Long(vec.iter().map(|x| {Flat(x.clone())}).collect::<Vec<WashArgs>>());
    }
    
    pub fn flatten_with(&self, with:&str) -> String {
        match self {
            &Flat(ref s) => s.clone(),
            &Long(ref v) => {
                let mut out = String::new();
                for item in v.iter() {
                    out.push_str(item.flatten_with(with).as_slice());
                    out.push(NL);
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
            &Flat(ref v) => v.len(),
            &Long(ref v) => v.len(),
            &Empty => 0
        }
    }

    pub fn is_flat(&self) -> bool {
        match self {
            &Empty | &Long(_) => false,
            &Flat(_) => true
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            &Flat(_) | &Long(_) => false,
            &Empty => true
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
        let from = max(0, u_from) as usize;
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
            variables: HashMap::new(),
            functions: HashMap::new(),
            scripts: HashMap::new(),
            controls: Controls::new()
        }
    }

    pub fn hasv(&self, name:&str) -> bool {
        self.variables.contains_key(&name.to_string())
    }

    pub fn hasf(&self, name:&String) -> bool {
        self.functions.contains_key(name)
    }

    pub fn insv(&mut self, name:&str, val:WashArgs) {
        self.variables.insert(name.to_string(), val);
    }

    pub fn insf(&mut self, name:&str, func:WashFunc) {
        self.functions.insert(name.to_string(), func);
    }

    pub fn getv(u_env:*const WashEnv, name:&String) -> Option<WashArgs> {
        // I'm not even returning a pointer calm down rust
        let env = unsafe{u_env.as_ref()}.unwrap();
        return match env.variables.get(name) {
            None => None,
            Some(val) => Some(val.clone())
        };
    }

    pub fn getf<'a>(u_env:*const WashEnv, u_name:*const String) -> Option<&'a WashFunc> {
        let env = unsafe{u_env.as_ref()}.unwrap();
        let name = unsafe{u_name.as_ref()}.unwrap();
        return env.functions.get(name);
    }

    pub fn get_script<'a>(u_env:*mut WashEnv, path:&Path) -> Option<&'a mut WashScript> {
        // this is technically unsafe, but the resulting WashScript has no access to the
        // WashEnv except through arguments we pass to its function
        // So really this isn't a borrow, even though Rust thinks it is
        let env = unsafe{u_env.as_mut()}.unwrap();
        return env.scripts.get_mut(path);
    }

    pub fn load_script(&mut self, path:Path, args:&WashArgs) -> WashArgs {
        if !self.scripts.contains_key(&path) {
            self.scripts.insert(path.clone(), WashScript::new(path.clone()));
        }
        let script = WashEnv::get_script(self, &path).unwrap();
        if !script.is_compiled() && !script.compile() {
            self.controls.err("Failed to compile script\n");
            return Empty;
        }
        self.controls.flush();
        if script.is_runnable() {
            script.run(args, self);
            return Empty;
        } else if script.is_loadable() {
            return script.load(args, self);
        } else {
            self.controls.err("Cannot load or run script\n");
            return Empty;
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

