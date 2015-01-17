use std::io::process::ProcessExit::*;

use std::os;
use std::cmp::*;

use script::WashArgs::*;
use script::*;
use util::*;
use constants::*;

// Calling convention:
// fn(args:&Vec<String>, u_env:*mut WashEnv) -> Vec<String>
fn source_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    // in this case args is line
    if args.is_empty() {
        env.term.controls.err("No arguments given");
        return Empty;
    }
    let name = match args {
        &Empty => return Empty,
        &Long(_) => return Empty,
        &Flat(ref v) => v.clone()
    };
    env.load_script(Path::new(name), &args.slice(1, -1))
}

fn cd_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    let newp = {
        if args.is_empty() {
            expand_path(Path::new("~"))
        } else if args.get_flat(0).slice_to(min(args.get_flat(0).len(), 2)) == "./" {
            // this specifical case can't be put through expand_path
            Path::new(&args.get_flat(0))
        } else {
            expand_path(Path::new(&args.get_flat(0)))
        }
    };
    match os::change_dir(&newp) {
        Ok(_) => {},
        Err(e) => {
            env.term.controls.errf(format_args!("Failed: {}: {}\n", newp.display(), e));
        }
    }
    return Empty;
}

fn outs_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    let mut argf = args.flatten();
    env.term.controls.outs(argf.as_slice());
    if argf.pop() != Some(NL) {
        env.term.controls.outc(NL);
    }
    return Empty;
}

pub fn drun_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    // Note: Wash calling convention is for the caller to reduce
    // arguments to literals
    if args.len() < 1 {
        return Empty;
    }
    let name = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Empty
    };
    // this could be empty but that's ok
    let arg_slice = args.slice(1, -1);
    if env.hasf(&name) {
        return env.runf(&name, &arg_slice);
    } else {
        let out = match env.term.run_command_directed(&name, &arg_slice.flatten_vec()) {
            None => return Empty,
            Some(v) => v
        };
        if !out.status.success() {
            env.term.controls.err(String::from_utf8_lossy(out.error.as_slice()).as_slice());
            return Empty;
        }
        return Flat(String::from_utf8_lossy(out.output.as_slice()).into_owned());
    }
}

pub fn run_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    // Note: Wash calling convention is for the caller to reduce
    // arguments to literals
    if args.len() < 1 {
        return Empty;
    }
    let name = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Empty
    };
    // this could be empty but that's ok
    let arg_slice = args.slice(1, -1);
    if env.hasf(&name) {
        // run functions before commands
        let out = env.runf(&name, &arg_slice).flatten();
        env.term.controls.outf(format_args!("{}\n", out));
        return Long(vec![Flat("status".to_string()),
                         Flat("0".to_string())]);
    } else {
        // flush output and run command
        env.term.controls.flush();
        match env.term.run_command(&name, &arg_slice.flatten_vec()) {
            None => return Empty,
            Some(ExitSignal(sig)) => {
                return Long(vec![Flat("signal".to_string()),
                                 Flat(format!("{}", sig))]);
            },
            Some(ExitStatus(status)) => {
                return Long(vec![Flat("status".to_string()),
                                 Flat(format!("{}", status))]);
            }
        }
    }
}

pub fn get_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    if args.len() < 1 {
        env.term.controls.err("No variable name given");
        return Empty;
    }
    let name = match args.get(0) {
        ref v if !v.is_flat() => {
            env.term.controls.err("Variables names can only be flat");
            return Empty;
        },
        ref v if !EQ_VAR_REGEX.is_match(v.flatten().as_slice()) => {
            env.term.controls.err("Variable names cannot contain whitespace, quotes, or parentheses");
            return Empty;
        }
        v => v.flatten()
    };
    if EQ_PATH_REGEX.is_match(name.as_slice()) {
        let caps = EQ_PATH_REGEX.captures(name.as_slice()).unwrap();
        let path = caps.at(1).unwrap().to_string();
        let name = caps.at(2).unwrap().to_string();
        if path.is_empty() {
            // use default path
            // this can be used to set a variable
            // with a name containing a colon
            return env.getv(&name);
        } else {
            return env.getvp(&name, &path);
        }
    } else {
        return env.getv(&name);
    }
}

pub fn setp_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    if args.len() < 1 {
        env.variables = String::new();
        return Flat(String::new());
    } else {
        let path = match args.get(0) {
            ref v if !v.is_flat() => {
                env.term.controls.err("Variable paths can only be flat");
                return Empty;
            }
            v => v.flatten()
        };
        if path == "env".to_string() {
            env.term.controls.err("Cannot set variable path to environment variables");
            return Empty;
        }
        env.variables = path.clone();
        return Flat(path)
    }
}

pub fn equals_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    // other l-values might eventually be supported,
    // for now you can only set variables
    if args.len() < 2 {
        env.term.controls.err("Not enough arguments: give variable and value");
        return Empty;
    }
    let name = match args.get(0) {
        ref v if !v.is_flat() => {
            env.term.controls.err("Variables names can only be flat");
            return Empty;
        },
        ref v if !EQ_VAR_REGEX.is_match(v.flatten().as_slice()) => {
            env.term.controls.err("Variable names cannot contain whitespace, quotes, or parentheses");
            return Empty;
        }
        v => v.flatten()
    };
    let val = args.get(1);
    if EQ_PATH_REGEX.is_match(name.as_slice()) {
        let caps = EQ_PATH_REGEX.captures(name.as_slice()).unwrap();
        let path = caps.at(1).unwrap().to_string();
        let name = caps.at(2).unwrap().to_string();
        if path.is_empty() {
            // use default path
            // this can be used to set a variable
            // with a name containing a colon
            env.insv(name, val.clone());
            return val;
        } else {
            env.insvp(name, path, val.clone());
            return val;
        }
    } else {
        env.insv(name, val.clone());
        return val;
    }
}

fn builtins_func(_:&WashArgs, _:&mut WashEnv) -> WashArgs {
    return Long(vec![
        Flat("$".to_string()),
        Flat("=".to_string()),
        Flat("builtins".to_string()),
        Flat("cd".to_string()),
        Flat("get".to_string()),
        Flat("run".to_string()),
        Flat("setp".to_string()),
        Flat("source".to_string())]);
}

pub fn load_builtins(env:&mut WashEnv) {
    env.insf("source", source_func);
    env.insf("cd", cd_func);
    env.insf("builtins", builtins_func);
    env.insf("outs", outs_func);
    env.insf("$", drun_func);
    env.insf("run", run_func);
    env.insf("=", equals_func);
    env.insf("get", get_func);
    env.insf("setp", setp_func);
}
