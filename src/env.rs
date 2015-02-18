use libc::*;

use std::old_io::process::{ProcessOutput, ProcessExit};
use std::old_io::process::ProcessExit::*;
use std::collections::HashMap;
use std::os::unix::prelude::*;

use std::num::*;
use std::mem;
use std::env;
use std::fmt;

use types::WashArgs::*;

use command::*;
use types::*;
use script::*;
use signal::*;
use constants::*;
use ioctl::*;
use util::*;

use self::FuncEntry::*;

// !!!
// Wash function calling convention
pub type WashFunc = fn(&WashArgs, &mut WashEnv) -> Result<WashArgs, String>;
pub enum FuncEntry {
    Direct(WashFunc),
    #[allow(dead_code)]
    // TODO: rewrite this to be AST-aware
    Indirect(WashBlock)
}

// >Dat pointer indirection
// Sorry bro, Rust doesn't have DSTs yet
// Once it does they'll turn into a more compact structure
pub type VarTable = HashMap<String, WashArgs>;
pub type FuncTable = HashMap<String, FuncEntry>;
pub type ScriptTable = HashMap<Path, WashScript>;
pub type PathTable = HashMap<String, VarTable>;

// WashLoad returns two lists, the first of initialized functions,
// the second the same of variables
type WashLoad = extern fn(*const WashArgs, *mut WashEnv) -> Result<WashArgs, String>;
type WashRun = extern fn(*const WashArgs, *mut WashEnv) -> Result<WashArgs, String>;

// global stop check
static mut uexec_stop:bool = false;
static mut uglobal_env:*mut WashEnv = 0 as *mut WashEnv;

unsafe extern fn env_sigint(_:c_int, _:*const SigInfo,
                            _:*const c_void) {
    // get env pointer
    let env:&mut WashEnv = match uglobal_env.as_mut() {
        Some(v) => v,
        None => {
            // this handler shouldn't be called when the env pointer is null
            panic!("Env signal interrupt called when Env not active");
        }
    };
    // delete the ^C
    env.outc(BS);
    env.outc(BS);
    env.outc(SPC);
    env.outc(SPC);
    env.outc(BS);
    env.outc(BS);
    // set the stop global to true
    uexec_stop = true;
}

pub struct WashEnv {
    pub paths: PathTable,
    pub variables: String,
    pub functions: FuncTable,
    pub scripts: ScriptTable,
    pub term: TermState,
    pub catch_sigint: bool
}

impl Drop for WashEnv {
    fn drop(&mut self) {
        // guarentee that the terminal is restored on exit
        self.restore_terminal();
    }
}

impl WashEnv {
    pub fn new() -> WashEnv {
        WashEnv {
            paths: HashMap::new(),
            variables: String::new(),
            functions: HashMap::new(),
            scripts: HashMap::new(),
            term: TermState::new(),
            catch_sigint: true
        }
    }

    pub fn update_terminal(&mut self) {
        self.term.update_terminal();
        // these two steps happen here so the pointer is correctly set
        // TODO: change signal handling to not depend on global pointers
        self.term.set_pointer();
        self.term.handle_signals();
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

    pub fn errf(&mut self, args:fmt::Arguments) {
        self.term.controls.errf(args);
    }

    pub fn flush(&mut self) {
        self.term.controls.flush();
    }

    pub fn restart_job(&mut self, id:&usize) -> Result<(), String> {
        self.term.restart_job(id)
    }

    pub fn front_job(&mut self) -> Result<usize, String> {
        match self.term.front_job() {
            None => return Err(format!("No front job")),
            Some(u) => Ok(u)
        }
    }
    
    pub fn run_job_fd(&mut self, stdin:Option<Fd>, stdout:Option<Fd>, stderr:Option<Fd>,
                      name:&String, args:&Vec<String>,
                      envs:&Vec<(String, Option<String>)>) -> Result<usize, String> {
        self.term.run_job_fd(stdin, stdout, stderr, name, args, envs)
    }

    pub fn run_job(&mut self, name:&String, args:&Vec<String>) -> Result<usize, String> {
        self.term.run_job(name, args)
    }

    pub fn get_job(&self, id:&usize) -> Result<&Job, String> {
        self.term.get_job(id)
    }

    pub fn has_job(&self, id:&usize) -> bool {
        self.term.jobs.contains_key(id)
    }

    pub fn job_output(&mut self, id:&usize) -> Result<ProcessOutput, String> {
        self.term.job_output(id)
    }
    
    pub fn run_command_fd(&mut self, stdin:Option<Fd>, stdout:Option<Fd>, stderr:Option<Fd>,
                          name:&String, args:&Vec<String>,
                          envs:&Vec<(String, Option<String>)>) -> Result<ProcessExit, String> {
        self.term.run_command_fd(stdin, stdout, stderr, name, args, envs)
    }

    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        self.term.run_command(name, args)
    }

    pub fn wait_job(&mut self, id:&usize) -> Result<ProcessExit, String> {
        self.term.wait_job(id)
    }

    pub fn remove_if_done(&mut self, id:&usize) -> Result<bool, String> {
        self.term.remove_if_done(id)
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
            if path == "sys" {
                return Err(format!("System variables are read-only"));
            } else if path == "pipe" {
                return Err("Pipes are read-only variables".to_string())
            } else if path == "env" {
                env::remove_var(name.as_slice());
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
            if path == "pipe" {
                return Err(format!("Pipes are read-only variales"));
            } else if path == "sys" {
                return Err(format!("System variables are read-only"))
            } else if path == "env" {
                if !val.is_flat() {
                    return Err("Environment variables can only be flat".to_string());
                }
                env::set_var(name.as_slice(), val.flatten().as_slice());
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
        self.functions.insert(name.to_string(), Direct(func));
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
        if *path == "sys" {
            return Err(format!("Cannot get all system variables"));
        } else if *path == "env" {
            let mut out = vec![];
            let envs = env::vars();
            for (name, value) in envs {
                out.push(Long(vec![Flat(name), Flat(value)]));
            }
            return Ok(Long(out));
        } else if *path == "pipe" {
            // list of non-background jobs (which can be piped)
            let mut out = vec![];
            for (id, job) in self.term.jobs.iter() {
                match job.process.stdout {
                    None => {/* don't include this job */},
                    Some(_) => {
                        // include this job
                        out.push(Long(vec![Flat(format!("@{}", id)),
                                           Flat(job.command.clone())]));
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
        if *path == "sys" {
            // special variables like usernames and things
            if *name == "login" {
                return Ok(Flat(tryf!(get_login(),
                                     "Could not get username: {err}")));
            } else if *name == "hostname" {
                return Ok(Flat(tryf!(get_hostname(),
                         "Could not get hostname: {err}")));
            } else if *name == "args" {
                let mut out = vec![];
                for arg in env::args() {
                    out.push(Flat(arg));
                }
                return Ok(Long(out));
            } else if *name == "cwd" {
                let cwd = tryf!(env::current_dir(),
                                "Couldn't get current directory: {err}");
                return Ok(Flat(format!("{}", cwd.display())));
            } else if *name == "scwd" {
                let cwd = tryf!(env::current_dir(),
                                "Couldn't get current directory: {err}");
                return Ok(Flat(format!("{}", condense_path(cwd).display())));
            } else {
                return Err(format!("System variable not found"));
            }
        } else if *path == "env" {
            // environment variables
            return match env::var(name.as_slice()) {
                Err(e) => Err(format!("{}", e)),
                Ok(s) => Ok(Flat(s))
            }
        } else if *path == "pipe" {
            // pipe Fd's
            let from = try!(self.get_job(&match from_str_radix(name.as_slice(), 10) {
                Err(e) => return Err(format!("Did not give job number: {}", e)),
                Ok(v) => v
            }));
            match from.process.stdout {
                None => return Err("Job has no output handles".to_string()),
                Some(ref p) => Ok(Flat(format!("@{}", p.as_raw_fd())))
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
    
    fn set_pointer(&mut self) {
        unsafe {
            if !uglobal_env.is_null() {
                panic!("Tried to set Env location twice");
            }
            uglobal_env = self as *mut WashEnv;
        }
    }

    fn unset_pointer(&mut self) {
        unsafe {
            if uglobal_env.is_null() {
                panic!("Tried to unset Env location twice");
            }
            uglobal_env = 0 as *mut WashEnv;
        }
    }

    pub fn handle_sigint(&mut self) {
        // sigint handling in env may be disabled from above
        if !self.catch_sigint {return}
        self.func_unstop();
        self.set_pointer();
        let mut sa = SigAction {
            handler: env_sigint,
            mask: [0; SIGSET_NWORDS],
            flags: SA_RESTART | SA_SIGINFO,
            restorer: 0 // null pointer
        };
        let mask = full_sigset().unwrap();
        sa.mask = mask;
        match signal_handle(SIGINT, &sa) {
            Err(e) => self.errf(format_args!("Could not set handler for SIGINT: {}\n", e)),
            _ => {}
        }
    }

    pub fn unhandle_sigint(&mut self) {
        // sigint handling in env may be disabled from above
        if !self.catch_sigint {return}
        self.func_unstop();
        self.unset_pointer();
        match signal_ignore(SIGINT) {
            Err(e) => self.errf(format_args!("Could not unset handler for SIGINT: {}\n", e)),
            _ => {}
        }
    }

    pub fn func_unstop(&self) {
        unsafe {
            uexec_stop = false;
        }
    }

    pub fn func_stop(&self) -> Result<(), String> {
        unsafe {
            if uexec_stop {
                return Err("Interrupt".to_string());
            } else {
                return Ok(());
            }
        }
    }

    pub fn runf(&mut self, name:&String, args:&WashArgs) -> Result<WashArgs, String> {
        let func = match self.functions.get(name) {
            None => return Err("Function not found".to_string()),
            Some(&Direct(ref func)) => func.clone(),
            Some(_) => return Err(format!("Cannot run indirect functions this way"))
        };
        self.handle_sigint();
        // in the case a function calls other functions
        let do_unhandle;
        if self.catch_sigint {
            self.catch_sigint = false;
            do_unhandle = true;
        } else {
            do_unhandle = false;
        }
        let out = func(args, self);
        if do_unhandle {
            self.catch_sigint = true;
            self.unhandle_sigint();
        }
        return out;
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

    pub fn output_file(&mut self, path:&Path) -> Result<Fd, String> {
        Ok(tryf!(self.term.output_file(path),
                 "Couldn't open file: {err}"))
    }

    pub fn input_file(&mut self, path:&Path) -> Result<Fd, String> {
        Ok(tryf!(self.term.input_file(path),
                 "Couldn't open file: {err}"))
    }

    pub fn get_jobs(&mut self) -> WashArgs {
        let mut out = vec![];
        for (id, job) in self.term.jobs.iter() {
            match job.process.stdout {
                None => {
                    // background job
                    out.push(Flat(format!("{}: background job {}", job.command, id)));
                },
                Some(_) => {
                    // foreground job
                    out.push(Flat(format!("{}: job {}", job.command, id)));
                }
            }
        }
        return Long(out);
    }

    pub fn clean_jobs(&mut self) -> WashArgs {
        let mut out = vec![];
        let result = self.term.clean_jobs();
        for &(ref id, ref job) in result.iter() {
            match job.exit {
                Some(v) if job.check_exit() => {
                    if v.success() {
                        out.push(Flat(format!("Job {} ({}) finished", id, job.command)));
                    } else {
                        match v {
                            ExitSignal(sig) => {
                                out.push(Flat(format!("Job {} ({}) failed: signal {}", id, job.command, sig)));
                            },
                            ExitStatus(status) => {
                                out.push(Flat(format!("Job {} ({}) failed: status {}", id, job.command, status)));
                            }
                        }
                    }
                },
                _ => panic!("Removed running job"),
            }
        }
        return Long(out);
    }

}
