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

use controls::*;
use constants::*;

// !!!
// Wash function calling convention
pub type WashFunc = fn(&Vec<String>, &mut Controls) -> Vec<String>;

// >Dat pointer indirection
// Sorry bro, Rust doesn't have DSTs yet
// Once it does they'll turn into a more compact structure
pub type FuncTable = HashMap<String, WashFunc>;
pub type ScriptTable = HashMap<String, WashScript>;

type WashLoad = extern fn(*const Vec<String>, *mut FuncTable, *mut Controls);
type WashRun = extern fn(*const Vec<String>, *mut FuncTable, *mut Controls);

pub struct WashScript {
    pub path: Path,
    pub hash: String,
    controls: Controls,
    handle: *const c_void,
    run_ptr: *const c_void,
    load_ptr: *const c_void,
    pub loaded: bool
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

    pub fn run(&mut self, args:&Vec<String>, functions:&mut FuncTable) {
        if !self.is_compiled() {
            self.controls.err("Script not compiled\n");
            return;
        }
        
        let run_func:WashRun = unsafe {
            match self.run_ptr.as_ref() {
                Some(f) => mem::transmute(f),
                None => {
                    self.controls.err("Script cannot be run directly\n");
                    return;
                }
            }
        };

        if !self.loaded && self.is_loadable() {
            self.load(args, functions);
        }

        run_func(args, functions, &mut self.controls);
    }

    pub fn load(&mut self, args:&Vec<String>, functions:&mut FuncTable) {
        if !self.is_compiled() {
            self.controls.err("Script is not compiled\n");
            return;
        }
        
        let load_func:WashLoad = unsafe {
            match self.load_ptr.as_ref() {
                Some(f) => mem::transmute(f),
                None => {
                    self.controls.err("Script has no load actions\n");
                    return;
                }
            }
        };

        if self.loaded {
            self.controls.err("Script already loaded\n");
        }

        load_func(args, functions, &mut self.controls);
        self.loaded = true;
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

        let path_cstr = ffi::CString::from_slice(outp.as_str().unwrap().as_bytes()).as_ptr();
        let run_cstr = ffi::CString::from_slice(WASH_RUN_SYMBOL.as_bytes()).as_ptr();
        let load_cstr = ffi::CString::from_slice(WASH_LOAD_SYMBOL.as_bytes()).as_ptr();
        unsafe {
            self.handle = dlopen(path_cstr, RTLD_LAZY|RTLD_LOCAL);
            if self.handle.is_null() {
                let c = dlerror();
                let e = str::from_utf8(ffi::c_str_to_bytes(&c)).unwrap();
                self.controls.errf(format_args!("Could not load script object: {}\n", e));
                return false;
            }
            
            self.run_ptr = dlsym(self.handle, run_cstr);
            if self.run_ptr.is_null() {
                // this script isn't run directly
                // clear error message
                dlerror();
            }
            
            self.load_ptr = dlsym(self.handle, load_cstr);
            if self.load_ptr.is_null() {
                // this script is only run directly
                // clear error message
                dlerror();
            }
        }
        if self.load_ptr.is_null() && self.run_ptr.is_null() {
            self.controls.err("No load or run function found in script object");
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
