use std::io::process::ProcessExit::*;

use std::os;
use std::cmp::*;

use script::WashArgs::*;
use script::HandlerResult::*;
use script::*;
use util::*;
use constants::*;
use types::*;

fn source_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    // in this case args is line
    if args.is_empty() {
        return Err("No arguments given".to_string());
    }
    let name = match args {
        &Empty => return Err("No arguments given".to_string()),
        &Long(_) => return Err("Can only source flat names".to_string()),
        &Flat(ref v) => v.clone()
    };
    env.load_script(Path::new(name), &args.slice(1, -1))
}

fn cd_func(args:&WashArgs, _:&mut WashEnv) -> Result<WashArgs, String> {
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
        Err(e) => return Err(e.desc.to_string()),
        Ok(_) => return Ok(Empty)
    }
}

fn outs_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    let mut argf = args.flatten();
    env.outs(argf.as_slice());
    if argf.pop() != Some(NL) {
        env.outc(NL);
    }
    return Ok(Empty);
}

pub fn drun_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    // Note: Wash calling convention is for the caller to reduce
    // arguments to literals
    if args.len() < 1 {
        return Err("No arguments provided".to_string());
    }
    let name = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Err("Can only run flat names".to_string())
    };
    // this could be empty but that's ok
    let arg_slice = args.slice(1, -1);
    if env.hasf(&name) {
        return env.runf(&name, &arg_slice);
    } else {
        let out = try!(env.run_command_directed(&name, &arg_slice.flatten_vec()));
        if !out.status.success() {
            return Err(String::from_utf8_lossy(out.error.as_slice()).into_owned());
        }
        return Ok(Flat(String::from_utf8_lossy(out.output.as_slice()).into_owned()));
    }
}

pub fn run_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    // Note: Wash calling convention is for the caller to reduce
    // arguments to literals
    if args.len() < 1 {
        return Err("No arguments given".to_string());
    }
    let name = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Err("Can only run flat names".to_string())
    };
    // this could be empty but that's ok
    let arg_slice = args.slice(1, -1);
    if env.hasf(&name) {
        // run functions before commands
        let out = try!(env.runf(&name, &arg_slice)).flatten();
        env.outf(format_args!("{}\n", out));
        return Ok(Long(vec![Flat("status".to_string()),
                            Flat("0".to_string())]));
    } else {
        // flush output and run command
        env.flush();
        match try!(env.run_command(&name, &arg_slice.flatten_vec())) {
            ExitSignal(sig) => {
                return Ok(Long(vec![Flat("signal".to_string()),
                                    Flat(format!("{}", sig))]));
            },
            ExitStatus(status) => {
                return Ok(Long(vec![Flat("status".to_string()),
                                    Flat(format!("{}", status))]));
            }
        }
    }
}

pub fn get_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    if args.len() < 1 {
        return Err("No variable name given".to_string());
    }
    let name = match args.get(0) {
        ref v if !v.is_flat() => {
            return Err("Variable names can only be flat".to_string());
        },
        ref v if !EQ_VAR_REGEX.is_match(v.flatten().as_slice()) => {
            return Err("Varibale names cannot contain whitespace, quotes, or parentheses".to_string());
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

pub fn setp_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    if args.len() < 1 {
        env.variables = String::new();
        return Ok(Flat(String::new()));
    } else {
        let path = match args.get(0) {
            ref v if !v.is_flat() => {
                return Err("Variable paths can only be flat".to_string());
            }
            v => v.flatten()
        };
        if path == "env".to_string() {
            return Err("Cannot set variable path to environment variables".to_string());
        }
        env.variables = path.clone();
        return Ok(Flat(path))
    }
}

fn equal_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // other l-values might eventually be supported,
    // for now you can only set variables
    // consume only the first variable before the equals
    let name = match pre.pop() {
        None => {
            return Err("No variable name provided".to_string());
        },
        Some(ref v) if !v.is_flat() => {
            return Err("Variable names can only be flat".to_string());
        },
        Some(ref v) if !EQ_VAR_REGEX.is_match(v.flatten().as_slice()) => {
            return Err("Variable names cannot contain whitespace, quotes, or parentheses".to_string());
        }
        Some(v) => v.flatten()
    };
    // This is O(n), but we need the first value so.
    let val = try!(env.input_to_args(next.remove(0)));
    if EQ_PATH_REGEX.is_match(name.as_slice()) {
        let caps = EQ_PATH_REGEX.captures(name.as_slice()).unwrap();
        let path = caps.at(1).unwrap().to_string();
        let name = caps.at(2).unwrap().to_string();
        if path.is_empty() {
            // use default path
            // this can be used to set a variable
            // with a name containing a colon
            try!(env.insv(name, val.clone()));
            return Err(val.flatten());
        } else {
            try!(env.insvp(name, path, val.clone()));
            return Err(val.flatten());
        }
    } else {
        try!(env.insv(name, val.clone()));
        return Err(val.flatten());
    }
    // right now equals can only produce Stop.
    // In the future this may not be the case
}

fn semiamper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // effectively the "continue" handler
    // run the part before the line and then continue
    // onto the next one no matter what
    let out = match run_func(&Long(pre.clone()), env) {
        Err(_) => vec![],
        Ok(v) => v.flatten_vec()
    };
    if out != vec!["status".to_string(), "0".to_string()] {
        env.err("Command failed\n");
    }
    // ;& does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
}


fn amperamper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // "and then:" run this command and continue only if it succeded
    // onto the next one no matter what
    let out = try!(run_func(&Long(pre.clone()), env)).flatten_vec();
    if out != vec!["status".to_string(), "0".to_string()] {
        return Err("Command failed".to_string());
    } else {
        // && does not pass on the value of the previous command
        pre.clear();
        return Ok(Continue);
    }
}

fn builtins_func(_:&WashArgs, _:&mut WashEnv) -> Result<WashArgs, String> {
    return Ok(Long(vec![
        Flat("$".to_string()),
        Flat("builtins".to_string()),
        Flat("cd".to_string()),
        Flat("get".to_string()),
        Flat("run".to_string()),
        Flat("setp".to_string()),
        Flat("source".to_string())]));
}

pub fn load_builtins(env:&mut WashEnv) -> Result<WashArgs, String> {
    // functions
    try!(env.insf("source", source_func));
    try!(env.insf("cd", cd_func));
    try!(env.insf("builtins", builtins_func));
    try!(env.insf("outs", outs_func));
    try!(env.insf("$", drun_func));
    try!(env.insf("run", run_func));
    try!(env.insf("get", get_func));
    try!(env.insf("setp", setp_func));

    // handlers
    try!(env.insert_handler("=", equal_handler));
    try!(env.insert_handler(";&", semiamper_handler));
    try!(env.insert_handler("&&", amperamper_handler));

    return Ok(Empty);
}
