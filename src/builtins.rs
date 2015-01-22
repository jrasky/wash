use std::io::process::ProcessExit::*;
use std::os::unix::prelude::*;

use std::os;
use std::cmp::*;

use types::WashArgs::*;
use env::HandlerResult::*;

use util::*;
use constants::*;
use types::*;
use env::*;

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
        let id = try!(env.run_job(&name, &arg_slice.flatten_vec()));
        let out = try!(env.job_output(&id));
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
    if FD_REGEX.is_match(name.as_slice()) {
        // run piped instead
        return pipe_run_func(args, env);
    }
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

pub fn run_outfd_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    // Note: Wash calling convention is for the caller to reduce
    // arguments to literals
    if args.len() < 2 {
        return Err("Give file discriptor and command".to_string());
    }
    let fid = match args.get(0) {
        Flat(v) => {
            if FD_REGEX.is_match(v.as_slice()) {
                str_to_usize(FD_REGEX.captures(v.as_slice()).unwrap().at(1).unwrap()).unwrap()
            } else {
                return Err("Not given a file descriptor".to_string());
            }
        },
        Empty | Long(_) => return Err("File descriptors can only be flat".to_string())
    };  
    let mut arg_slice;
    let mut infd = None;
    let name = match args.get(1) {
        Flat(v) => {
            if FD_REGEX.is_match(v.as_slice()) {
                infd = Some(str_to_usize(FD_REGEX.captures(v.as_slice()).unwrap().at(1).unwrap()).unwrap() as Fd);
                arg_slice = args.slice(3, -1);
                match args.get(2) {
                    Flat(v) => v,
                    _ => return Err("Can only run flat names".to_string())
                }
            } else {
                arg_slice = args.slice(2, -1);
                v
            }
        },
        Empty | Long(_) => return Err("Can only run flat names".to_string())
    };
    env.flush();
    match try!(env.run_command_fd(infd, Some(fid as Fd), Some(STDERR), &name, &arg_slice.flatten_vec())) {
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

pub fn job_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    if args.len() < 1 {
        return Err("No arguments given".to_string());
    }
    let name = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Err("Can only run flat names".to_string())
    };
    if FD_REGEX.is_match(name.as_slice()) {
        // run piped instead
        return pipe_job_func(args, env);
    }
    let arg_slice = args.slice(1, -1);
    let id = try!(env.run_job(&name, &arg_slice.flatten_vec()));
    return Ok(Flat(format!("{}", id)));
}

pub fn pipe_job_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    // takes a directed job number, then arguments for a new job
    // creates this new job with the stdout Fd of the given job as the stdin Fd of the new job
    // this new job is also directed
    if args.len() < 2 {
        return Err("Need at least job number and command".to_string());
    }
    let pipe_str = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Err("File descriptors can only be flat".to_string())
    };
    if !FD_REGEX.is_match(pipe_str.as_slice()) {
        return Err("Not given file descriptior".to_string());
    }
    let pipe = str_to_usize(FD_REGEX.captures(pipe_str.as_slice()).unwrap().at(1).unwrap()).unwrap();
    let name = match args.get(1) {
        Flat(v) => v,
        Empty | Long(_) => return Err("Can only run flat names".to_string())
    };
    let arg_slice = args.slice(2, -1);
    let id = try!(env.run_job_fd(Some(pipe as Fd), None, None, &name, &arg_slice.flatten_vec()));
    return Ok(Flat(format!("{}", id)));
}


pub fn pipe_run_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    // takes a directed job number, then arguments for a new job
    // creates this new job with the stdout Fd of the given job as the stdin Fd of the new job
    // this new job is also directed
    if args.len() < 2 {
        return Err("Need at least a file descriptor and a command".to_string());
    }
    let pipe_str = match args.get(0) {
        Flat(v) => v,
        Empty | Long(_) => return Err("File descriptors can only be flat".to_string())
    };
    if !FD_REGEX.is_match(pipe_str.as_slice()) {
        return Err("Not given file descriptior".to_string());
    }
    let pipe = str_to_usize(FD_REGEX.captures(pipe_str.as_slice()).unwrap().at(1).unwrap()).unwrap();
    let name = match args.get(1) {
        Flat(v) => v,
        Empty | Long(_) => return Err("Can only run flat names".to_string())
    };
    let arg_slice = args.slice(2, -1);
    env.flush();
    match try!(env.run_command_fd(Some(pipe as Fd), Some(STDOUT), Some(STDERR),
                                  &name, &arg_slice.flatten_vec())) {
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

pub fn jobs_func(_:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    let jobs = env.get_jobs();
    if jobs.len() == 0 {
        return Err("No jobs".to_string());
    } else {
        return Ok(env.get_jobs());
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
        } else if path == "pipe".to_string() {
            return Err("Cannot set variable path to job pipes".to_string());
        } else if path == "file".to_string() {
            return Err("Cannot set variable path to files".to_string());
        }
        env.variables = path.clone();
        return Ok(Flat(path))
    }
}

pub fn describe_process_output(args:&WashArgs, _:&mut WashEnv) -> Result<WashArgs, String> {
    let argv = args.flatten_vec();
    if args.is_empty() {
        return Err("Command Failed".to_string());
    } else if argv.len() < 2 {
        return Err(format!("Command failed: {}", args.flatten()));
    } else if argv != vec!["status", "0"] {
        return Err(format!("Command failed with {} {}", argv[0], argv[1]));
    } else {
        return Ok(Empty);
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
    let val;
    if next.len() == 0 {
        val = Empty;
    } else {
        // This is O(n), but we need the first value so.
        val = try!(env.input_to_args(next.remove(0)));
    }
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
    // but one after the other
    match run_func(&Long(pre.clone()), env) {
        Err(e) => env.errf(format_args!("{}\n", e)),
        Ok(v) => match describe_process_output(&v, env) {
            Err(e) => {
                env.errf(format_args!("{}\n",e));
            },
            _ => {/* nothing */}
        }
    };
    // ;& does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
}


fn amperamper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // "and then:" run this command and continue only if it succeded
    // onto the next one no matter what
    let out = try!(run_func(&Long(pre.clone()), env));
    // will return on error
    try!(describe_process_output(&out, env));
    // && does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
}

fn amper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // almost directly calls job
    // ignore errors
    match job_func(&Long(pre.clone()), env) {
        Err(e) => env.errf(format_args!("{}\n", e)),
        Ok(v) => env.outf(format_args!("Started job: {}\n", v.flatten()))
    }
    // & does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
}

fn bar_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    if pre.len() < 1 {
        return Err("Cannot pipe nothing".to_string());
    }
    let id = match try!(job_func(&Long(pre.clone()), env)) {
        Flat(v) => match str_to_usize(v.as_slice()) {
            None => return Err("djob did not return a job number".to_string()),
            Some(v) => v
        },
        _ => return Err("djob did not return a job number".to_string())
    };
    pre.clear();
    pre.push(try!(env.getvp(&format!("{}", id), &"pipe".to_string())));
    return Ok(Continue);
}

fn leq_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // file input
    if next.is_empty() {
        return Err("File name must be provided".to_string());
    }
    let fname = match try!(env.input_to_args(next.remove(0))) {
        Flat(s) => s,
        _ => return Err("File name must be flat".to_string())
    };
    let fpath = expand_path(Path::new(fname));
    let fid = try!(env.input_file(&fpath));
    pre.insert(0, try!(env.getvp(&format!("{}", fid), &"file".to_string())));
    return Ok(Continue);
}

fn geq_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, env:&mut WashEnv) -> Result<HandlerResult, String> {
    // file output
    if next.is_empty() {
        return Err("File name must be provided".to_string());
    }
    let fname = match try!(env.input_to_args(next.remove(0))) {
        Flat(s) => s,
        _ => return Err("File name must be flat".to_string())
    };
    let fpath = expand_path(Path::new(fname));
    let fid = try!(env.output_file(&fpath));
    pre.insert(0, try!(env.getvp(&format!("{}", fid), &"file".to_string())));
    let out = try!(run_outfd_func(&Long(pre.clone()), env));
    try!(describe_process_output(&out, env));
    // stop no matter what
    return Err(String::new());
}

fn t_sblock_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, _:&mut WashEnv) -> Result<HandlerResult, String> {
    // test function for More case of HandlerResult
    let block = WashBlock {
        start: Long(pre.clone()),
        close: InputValue::Short("}".to_string()),
        content: vec![]
    };
    return Ok(More(block));
}

fn t_eblock_handler(_:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, _:&mut WashEnv) -> Result<HandlerResult, String> {
    // helper to tell users not to use this in a line
    return Err("Close block in incorrect place".to_string());
}

fn builtins_func(_:&WashArgs, _:&mut WashEnv) -> Result<WashArgs, String> {
    return Ok(Long(vec![
        Flat("$".to_string()),
        Flat("builtins".to_string()),
        Flat("cd".to_string()),
        Flat("get".to_string()),
        Flat("jobs".to_string()),
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
    try!(env.insf("jobs", jobs_func));
    try!(env.insf("pipe_job", pipe_job_func));
    try!(env.insf("pipe_run", pipe_run_func));
    try!(env.insf("job", job_func));

    // commands that aren't really meant to be called by users
    try!(env.insf("describe_process_output", describe_process_output));

    // handlers
    try!(env.insert_handler("=", equal_handler));
    try!(env.insert_handler(";&", semiamper_handler));
    try!(env.insert_handler("&&", amperamper_handler));
    try!(env.insert_handler("&", amper_handler));
    try!(env.insert_handler("|", bar_handler));
    try!(env.insert_handler("<", leq_handler));
    try!(env.insert_handler(">", geq_handler));

    // test cases
    try!(env.insert_handler("{", t_sblock_handler));
    try!(env.insert_handler("}", t_eblock_handler));

    return Ok(Empty);
}
