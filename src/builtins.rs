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
    let name = match args {
        &Empty => return Err("No arguments given".to_string()),
        &Long(_) => return Err("Can only source flat names".to_string()),
        &Flat(ref v) => v.clone()
    };
    env.load_script(Path::new(name), &args.slice(1, -1))
}

fn cd_func(args:&WashArgs, _:&mut WashEnv) -> Result<WashArgs, String> {
    let newp = {
        if args.len() == 0 {
            expand_path(Path::new("~"))
        } else if &args.get_flat(0)[..min(args.get_flat(0).len(), 2)] == "./" {
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

fn job_args(args:&WashArgs) -> Result<(Option<Fd>, Option<Fd>, Option<Fd>,
                                       String, Vec<String>), String> {
    // turns arguments into file descriptor options, command name and args
    // utility function because job_func and run_func use this same code
    let (mut stdin, mut stdout, mut stderr) = (None, None, None);
    let mut argc = args.flatten_vec();
    let mut name;
    loop {
        // fail if only file descriptors given
        if argc.is_empty() {
            return Err("Don't know what to do with file descriptors".to_string());
        }
        // pop out arguments from the front until no more file descriptors remain
        name = argc.remove(0);
        if !FD_REGEX.is_match(name.as_slice()) {
            // we've reached the end of file descriptors
            break;
        } else {
            let caps = FD_REGEX.captures(name.as_slice()).unwrap();
            match str_to_usize(caps.at(2).unwrap()) {
                None => return Err(format!("{} could not be turned into usize", caps.at(2).unwrap())),
                Some(fd) => match caps.at(1).unwrap() {
                    path if path.is_empty() || path == "in" =>
                        // default to stdin
                        stdin = Some(fd as Fd),
                    path if path == "out" =>
                        stdout = Some(fd as Fd),
                    path if path == "err" =>
                        stderr = Some(fd as Fd),
                    _ => return Err(format!("{} is not a valid standard output", caps.at(1).unwrap()))
                }
            }
        }
    }
    return Ok((stdin, stdout, stderr, name, argc));
}

pub fn job_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    let id;
    if args.is_empty() || args.len() < 1 {
        return Err("No arguments given".to_string());
    } else if args.is_flat() {
        // easy case, no arguments to the function
        id = try!(env.run_job(&args.flatten(), &vec![]));
    } else if !args.get(0).is_flat() {
        return Err("Can only run flat names".to_string());
    } else if !FD_REGEX.is_match(args.get(0).flatten().as_slice()) {
        // easy case, no file descriptors given
        let args_slice = args.slice(1, -1);
        id = try!(env.run_job(&args.get(0).flatten(), &args_slice.flatten_vec()));
    } else {
        // hard case, file descriptors given
        let (stdin, stdout, stderr, name, argc) = try!(job_args(args));
        id = try!(env.run_job_fd(stdin, stdout, stderr, &name, &argc));
    }
    return Ok(Flat(format!("{}", id)));
}

pub fn job_output_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    let arg = args.get(0);
    if !arg.is_flat() {
        return Err("Give a job number".to_string());
    }
    let id = match str_to_usize(arg.flatten().as_slice()) {
        None => return Err(format!("Couldn't turn {} into a job number", arg.flatten())),
        Some(num) => num
    };
    let out = try!(env.job_output(&id));
    if !out.status.success() {
        return Err(String::from_utf8_lossy(out.error.as_slice()).into_owned());
    } else {
        return Ok(Flat(String::from_utf8_lossy(out.output.as_slice()).into_owned()));
    }
}

pub fn directed_job_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    return job_output_func(&try!(job_func(args, env)), env);
}

pub fn run_func(args:&WashArgs, env:&mut WashEnv) -> Result<WashArgs, String> {
    let out;
    if args.is_empty() || args.len() < 1 {
        return Err("No arguments given".to_string());
    } else if args.is_flat() {
        // easy case, no arguments to the function
        out = try!(env.run_command(&args.flatten(), &vec![]));
    } else if !args.get(0).is_flat() {
        return Err("Can only run flat names".to_string());
    } else if !FD_REGEX.is_match(args.get(0).flatten().as_slice()) {
        // easy case, no file descriptors given
        let args_slice = args.slice(1, -1);
        out = try!(env.run_command(&args.get(0).flatten(), &args_slice.flatten_vec()));
    } else {
        // hard case, file descriptors given
        let (stdin, stdout, stderr, name, argc) = try!(job_args(args));
        out = try!(env.run_command_fd(stdin, stdout, stderr, &name, &argc));
    }
    return match out {
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
        } else {
            env.variables = path.clone();
            return Ok(Flat(path));
        }
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
    pre.insert(0, Flat(format!("@{}", fid)));
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
    pre.insert(0, Flat(format!("@out:{}", fid)));
    let out = try!(run_func(&Long(pre.clone()), env));
    try!(describe_process_output(&out, env));
    // stop no matter what
    return Err(String::new());
}

fn create_content(next:&mut Vec<InputValue>) -> Result<Vec<InputValue>, String> {
    let mut one_line = false;
    let mut line = vec![];
    loop {
        match next.pop() {
            Some(InputValue::Short(ref s)) if *s == "{".to_string() => break,
            Some(InputValue::Short(ref s)) if *s == "}".to_string() && !one_line => {
                // one-line block
                one_line = true;
            },
            Some(InputValue::Split(_)) if !one_line => continue,
            Some(ref v) if one_line => {
                line.insert(0, v.clone())
            }
            _ => return Err("Malformed block".to_string())
        }
    }
    if line.is_empty() {
        return Ok(vec![]);
    } else {
        return Ok(vec![InputValue::Long(line)]);
    }
}

fn act_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, _:&mut WashEnv) -> Result<HandlerResult, String> {
    if !pre.is_empty() {
        return Err("Malformed block".to_string());
    }
    let content = try!(create_content(next));
    let close;
    if content.is_empty() {
        close = Some(InputValue::Short("}".to_string()));
    } else {
        close = None;
    }
    // test function for More case of HandlerResult
    let block = WashBlock {
        start: "act".to_string(),
        next: next.clone(),
        close: close,
        content: content
    };
    return Ok(More(block));
}

fn end_block_handler(_:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, _:&mut WashEnv) -> Result<HandlerResult, String> {
    // helper to tell users not to use this in a line
    return Err("Malformed block".to_string());
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
    try!(env.insf("$", directed_job_func));
    try!(env.insf("run", run_func));
    try!(env.insf("get", get_func));
    try!(env.insf("setp", setp_func));
    try!(env.insf("jobs", jobs_func));
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

    // block start/end
    try!(env.insert_handler("act!", act_handler));
    try!(env.insert_handler("}", end_block_handler));

    return Ok(Empty);
}
