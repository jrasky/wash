#![allow(unstable)]
#![feature(plugin)]
#[plugin] #[no_link]
extern crate regex_macros;
extern crate regex;

use std::io::fs::PathExtensions;
use std::io;
use std::os;

const TOP:&'static str = "
#![crate_type = \"dylib\"]
extern crate libc;

use libc::*;
use std::io::stdio::*;
use std::collections::HashMap;

const NOOP_FUNC:&'static WashFunc = &(noop as WashFunc);

pub type WashFunc = fn(&Vec<String>, &mut WashEnv) -> Vec<String>;
pub type VarTable = HashMap<String, String>;
pub type FuncTable = HashMap<String, WashFunc>;
pub type ScriptTable = HashMap<Path, WashScript>;

pub struct WashEnv {
    variables: VarTable,
    functions: FuncTable,
    scripts: ScriptTable,
    controls: Controls
}

pub struct WashScript {
    path: Path,
    hash: String,
    controls: Controls,
    handle: *const c_void,
    run_ptr: *const c_void,
    load_ptr: *const c_void,
    loaded: bool
}

pub struct Controls {
    stdin: StdinReader,
    stdout: StdWriter,
    stderr: StdWriter
}

impl WashEnv {
    pub fn hasv(&self, name:String) -> bool {
        self.variables.contains_key(&name)
    }

    pub fn hasf(&self, name:String) -> bool {
        self.functions.contains_key(&name)
    }

    pub fn insv(&mut self, name:String, val:String) {
        self.variables.insert(name, val);
    }

    pub fn insf(&mut self, name:String, func:WashFunc) {
        self.functions.insert(name, func);
    }

    pub fn getv(u_env:*const WashEnv, name:String) -> String {
        let env = unsafe{u_env.as_ref()}.unwrap();
        return match env.variables.get(&name) {
            None => \"\".to_string(),
            Some(val) => val.clone()
        };
    }

    pub fn getf<'a>(u_env:*const WashEnv, name:String) -> &'a WashFunc {
        let env = unsafe{u_env.as_ref()}.unwrap();
        return match env.functions.get(&name) {
            None => NOOP_FUNC,
            Some(ptr) => ptr
        }
    }
}

fn noop(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    return vec![];
}

fn sgetenv(name:&str) -> String {
    return std::os::getenv(name).unwrap_or(\"\".to_string()).to_string();
}";

const RUN_TOP:&'static str = "
#[no_mangle]
pub extern fn wash_run(u_args:*const Vec<String>, u_env:*mut WashEnv) {
    let args = unsafe{u_args.as_ref()}.unwrap();
    let mut env = unsafe{u_env.as_mut()}.unwrap();
";

const RUN_BOTTOM:&'static str = "
}";

fn process_line(line:String) -> String {
    if regex!("=").is_match(line.as_slice()) {
        let parts = regex!("=").splitn(line.as_slice(), 2).collect::<Vec<&str>>();
        return process_lvalue(parts[0].to_string()).connect(process_rvalue(parts[1].to_string()).as_slice());
    } else {
        return process_rvalue(line);
    }
}

fn process_lvalue(val:String) -> Vec<String> {
    if regex!("\\$([^ \t\n\r,\"()]+):([^ \t\n\r,\"()]+)").is_match(val.as_slice()) {
        let caps = regex!("\\$([^ \t\n\r,\"()]+):([^ \t\n\r,\"()]+)").captures(val.as_slice()).unwrap();
        match caps.at(1).unwrap() {
            "env" => {
                let val = caps.at(2).unwrap();
                let pre = format!("std::os::setenv(\"{}\", ", val).to_string();
                return vec![pre, ".as_slice());".to_string()];
            },
            _ => panic!("Unknown variable identifier")
        }
    } else if regex!("\\$([^ \t\n\r,\"()]+)").is_match(val.as_slice()) {
        let val = regex!("\\$([^ \t\n\r,\"()]+)").captures(val.as_slice()).unwrap().at(1).unwrap();
        let pre = format!("env.insv(\"{}\".to_string(), ", val).to_string();
        return vec![pre, ");".to_string()];
    } else {
        return vec![];
    }
}

fn process_rvalue(val:String) -> String {
    if regex!("^\\s*\"[^\"]*\"\\s*$").is_match(val.as_slice()) {
        let val = regex!("\"[^\"]*\"").captures(val.as_slice()).unwrap().at(0).unwrap();
        return format!("{}.to_string()", val.to_string());
    } if regex!("^\\s*\\$([^ \t\n\r,\"()]+):([^ \t\n\r,\"()]+)\\s*$").is_match(val.as_slice()) {
        let caps = regex!("\\$([^ \t\n\r,\"()]+):([^ \t\n\r,\"()]+)").captures(val.as_slice()).unwrap();
        match caps.at(1).unwrap() {
            "env" => {
                let val = caps.at(2).unwrap();
                return format!("sgetenv(\"{}\")", val).to_string();
            },
            _ => panic!("Unknown variable identifier")
        }
    } else if regex!("^\\s*\\$([^ \t\n\r,\"()]+)\\s*$").is_match(val.as_slice()) {
        let val = regex!("\\$([^ \t\n\r,\"()]+)").captures(val.as_slice()).unwrap().at(1).unwrap();
        return format!("WashEnv::getv(env, \"{}\".to_string())", val).to_string();
    } else if regex!("^\\s*([^ \t\n\r,\"()]+)\\((.*)\\)\\s*$").is_match(val.as_slice()) {
        let caps = regex!("([^ \t\n\r,\"()]+)\\((.*)\\)").captures(val.as_slice()).unwrap();
        let name = caps.at(1).unwrap();
        let re = regex!("(\".+\"|[^ \t\n\r,\"()]+)");
        let args_cap = caps.at(2).unwrap();
        let mut args = Vec::<String>::new();
        for cap in re.captures_iter(args_cap) {
            args.push(process_rvalue(cap.at(1).unwrap().to_string()));
        }
        return format!("WashEnv::getf(env, \"{}\".to_string())(&vec![{}], env);", name, args.connect(", ")).to_string()
    } else {
        return "".to_string();
    }
}

pub fn main() {
    let args = os::args();
    if args.len() == 1 {
        panic!("No arguments given");
    }
    let inp = Path::new(args[1].to_string());
    if !inp.exists() {
        panic!("File does not exist");
    }
    let mut stdout = io::stdout();
    let mut inf = io::BufferedReader::new(io::File::open(&inp));
    stdout.write_str(TOP).unwrap();
    stdout.write_str(RUN_TOP).unwrap();
    loop {
        match inf.read_line() {
            Err(_) => break,
            Ok(line) => {
                stdout.write_str(process_line(line).as_slice()).unwrap();
                stdout.write_str("\n").unwrap();
            }
        }
    }
    stdout.write_str(RUN_BOTTOM).unwrap();
    stdout.write_str("\n").unwrap();
}
