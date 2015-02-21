use libc::*;

use sodiumoxide::crypto::hash::sha256;

use serialize::hex::ToHex;

use std::process::Command;
use std::fs::{File, PathExt};
use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use std::ffi::AsOsStr;
use std::os::unix::OsStrExt;

use std::ffi;
use std::fs;
use std::old_io;

use constants::*;

pub struct WashScript {
    pub path: PathBuf,
    pub hash: String,
    handle: *const c_void,
    run_ptr: *const c_void,
    load_ptr: *const c_void,
    pub loaded: bool
}

impl Drop for WashScript {
    fn drop(&mut self) {
        match self.close() {
            Ok(_) => {},
            Err(e) => {
                old_io::stdio::stderr().write_str(e.as_slice()).unwrap();
            }
        }
    }
}

impl WashScript {
    pub fn new(path:&Path) -> WashScript {
        WashScript {
            path: path.to_path_buf(),
            hash: String::new(),
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

    pub unsafe fn get_run(&self) -> Result<&c_void, String> {
        match self.run_ptr.as_ref() {
            Some(f) => return Ok(f),
            None => {
                return Err("Script cannot be run directly".to_string());
            }
        }
    }

    pub unsafe fn get_load(&self) -> Result<&c_void, String> {
        match self.load_ptr.as_ref() {
            Some(f) => return Ok(f),
            None => {
                return Err("Script has no load actions".to_string());
            }
        }
    }

    pub fn close(&mut self) -> Result<(), String> {
        if self.is_compiled() {
            // prevent memory leaks
            unsafe {
                match dlclose(self.handle) {
                    0 => {
                        // nothing
                    },
                    _ => {
                        let c = dlerror();
                        let e = String::from_utf8_lossy(ffi::CStr::from_ptr(c).to_bytes());
                        return Err(format!("Couldn't unload wash script: {}\n", e));
                    }
                }
            }
        }
        self.handle = 0 as *const c_void;
        self.run_ptr = 0 as *const c_void;
        self.load_ptr = 0 as *const c_void;
        return Ok(());
    }

    pub fn compile(&mut self) -> Result<bool, String> {
        if self.is_compiled() {
            // script is already compiled
            return Ok(true);
        }
        if !self.path.exists() {
            return Err(format!("Could not find {}", self.path.display()));
        }
        let mut inf = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                return Err(format!("File error: {}", e));
            }
        };
        let mut content_s = String::new();
        tryf!(inf.read_to_string(&mut content_s), "{err}");
        let contents = content_s.as_bytes();
        self.hash = sha256::hash(contents).0.to_hex();
        let outp = {
            let mut outname = self.hash.clone();
            outname.push_str(".wo");
            Path::new(WO_PATH).join(Path::new(outname.as_slice()))
        };
        if !outp.exists() {
            // scripts needs to be compiled
            match fs::create_dir_all(Path::new(WO_PATH)) {
                Ok(_) => {
                    // nothing
                },
                Err(e) => {
                    return Err(format!("Couldn't create wash script cache directory: {}", e));
                }
            }
            let mut command = Command::new("rustc");
            command.args(&["-o", outp.as_os_str().to_str().unwrap(), "-"]);
            let mut child = match command.spawn() {
                Err(e) => {
                    return Err(format!("Couldn't start compiler: {}", e));
                },
                Ok(c) => c
            };
            {
                // TODO: maybe write match statements instead of
                // just calling unwrap
                let mut input = child.stdin.as_mut().unwrap();
                input.write_all(contents).unwrap();
                input.flush().unwrap();
            }

            match child.wait_with_output() {
                Err(e) => {
                    return Err(format!("Compiler failed to run: {}", e));
                },
                Ok(o) => {
                    if !o.status.success() {
                        return Err(format!("Couldn't compile script: {}",
                                           String::from_utf8_lossy(o.stderr.as_slice())));
                    }
                }
            }
        }

        let path_cstr = outp.as_os_str().to_cstring().ok().unwrap();
        let run_cstr = tryf!(ffi::CString::new(WASH_RUN_SYMBOL), "{err}");
        let load_cstr = tryf!(ffi::CString::new(WASH_LOAD_SYMBOL), "{err}");
        unsafe {
            self.handle = dlopen(path_cstr.as_ptr(), RTLD_LAZY|RTLD_LOCAL);
            if self.handle.is_null() {
                let c = dlerror();
                let e = String::from_utf8_lossy(ffi::CStr::from_ptr(c).to_bytes());
                return Err(format!("Could not load script object: {}", e));
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
            try!(self.close());
            return Err("No load or run function found in script object".to_string());
        }
        // success!
        return Ok(true);
    }
}

#[link(name = "dl")]
extern {
    fn dlopen(filename:*const c_char, flag:c_int) -> *const c_void;
    fn dlsym(handle:*const c_void, symbol:*const c_char) -> *const c_void;
    fn dlclose(handle:*const c_void) -> c_int;
    fn dlerror() -> *const c_char;
}

