use regex::Regex;

use std::old_io::process::ProcessExit::*;
use std::os::unix::prelude::*;
use std::path::PathBuf;
use std::ffi::AsOsStr;

use std::env;
use std::old_path;
use std::cmp::*;
use std::num::*;

use types::WashArgs::*;

use util::*;
use constants::*;
use types::*;
use env::*;
use ioctl::*;

macro_rules! builtin {
    ($name:ident, $args:pat, $env:pat, $func:block) => {
        pub fn $name($args:&WashArgs, $env:&mut WashEnv) -> Result<WashArgs, String>
            $func
    }
}

builtin!(source_func, args, env, {
    let name = match args {
        &Empty => return Err("No arguments given".to_string()),
        &Long(_) => return Err("Can only source flat names".to_string()),
        &Flat(ref v) => v.clone()
    };
    env.load_script(PathBuf::new(&name), &args.slice(1, -1))
});

builtin!(getall_func, args, env, {
    match args {
        &Empty => env.getall(),
        &Flat(ref p) => env.getallp(p),
        _ => Err(format!("Path must be Flat or Empty"))
    }
});

builtin!(flatten_eqlist_func, args, _, {
    Ok(Flat(args.flatten_with_inner("\n", "=")))
});

builtin!(cd_func, args, _, {
    let newp = {
        if args.len() == 0 {
            expand_path(PathBuf::new("~"))
        } else if &args.get_flat(0)[..min(args.get_flat(0).len(), 2)] == "./" {
            // this specifical case can't be put through expand_path
            PathBuf::new(&args.get_flat(0))
        } else {
            expand_path(PathBuf::new(&args.get_flat(0)))
        }
    };
    match env::set_current_dir(&old_path::Path::new(newp.as_os_str().to_str().unwrap())) {
        Err(e) => return Err(format!("{}", e)),
        Ok(_) => return Ok(Empty)
    }
});

builtin!(outs_func, args, env, {
    let mut argf = args.flatten();
    env.outs(argf.as_slice());
    if argf.pop() != Some(NL) {
        env.outc(NL);
    }
    return Ok(Empty);
});

builtin!(equal_func, args, _, {
    if !args.is_long() || !args.len() == 2 {
        Err(format!("Invalid arguments to equals?"))
    } else if args.get(0) == args.get(1) {
        Ok(Empty)
    } else {
        Ok(Flat(format!("not equal")))
    }
});

builtin!(re_equal_func, args, _, {
    if !args.is_long() || !args.len() == 2 {
        Err(format!("Invalid arguments to equals?"))
    } else if args.get(1).is_flat() {
        let re = tryf!(Regex::new(args.get(1).flatten().as_slice()), "{err}");
        if re.is_match(args.get(0).flatten().as_slice()) {
            Ok(Empty)
        } else {
            Ok(Flat(format!("not equal")))
        }
    } else {
        Err(format!("Right-hand side must be flat (regex)"))
    }
});

builtin!(not_func, args, _, {
    if args.is_empty() {
        Ok(Flat(format!("empty")))
    } else {
        Ok(Empty)
    }
});

fn job_args(args:&WashArgs, env:&mut WashEnv) -> Result<(Option<Fd>, Option<Fd>, Option<Fd>,
                                                         String, Vec<String>, Vec<(String, Option<String>)>), String> {
    // turns arguments into file descriptor options, command name and args
    // utility function because job_func and run_func use this same code
    let (mut stdin, mut stdout, mut stderr) = (None, None, None);
    let mut argc = match args {
        &Long(ref v) => v.clone(),
        _ => return Err(format!("Not given Long"))
    };
    let mut envs = vec![];
    let mut name; let mut fname;
    loop {
        // check for stop
        try!(env.func_stop());
        // fail if only file descriptors given
        if argc.is_empty() {
            return Err("No command given".to_string());
        }
        // pop out arguments from the front until no more file descriptors remain
        name = argc.remove(0);
        fname = name.flatten();
        if name.is_empty() {
            // skip empties
            continue;
        } else if FD_REGEX.is_match(fname.as_slice()) {
            let caps = FD_REGEX.captures(fname.as_slice()).unwrap();
            match from_str_radix::<Fd>(caps.at(2).unwrap(), 10) {
                Err(e) => return Err(format!("{} could not be turned into usize: {}", caps.at(2).unwrap(), e)),
                Ok(fd) => match caps.at(1).unwrap() {
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
        } else if EQ_TEMP_REGEX.is_match(fname.as_slice()) {
            // given environment variables for this job
            let caps = EQ_TEMP_REGEX.captures(fname.as_slice()).unwrap();
            let path = caps.at(1).unwrap();
            if path != "env" {
                return Err(format!("Can only set environment variables on commands"));
            }
            let name = caps.at(2).unwrap();
            if argc.is_empty() {
                return Err(format!("Incomplete temporary variable decleration"));
            }
            let val = match argc.remove(0) {
                Empty => None,
                Flat(s) => Some(s),
                _ => return Err(format!("Environment variables can only be flat"))
            };
            envs.push((name.to_string(), val));
        } else {
            // end of special arguments
            break;
        }
    }
    return Ok((stdin, stdout, stderr, fname, Long(argc).flatten_vec(), envs));
}

builtin!(job_func, args, env, {
    let id;
    if args.is_empty() || args.len() < 1 {
        return Err("No arguments given".to_string());
    } else if args.is_flat() {
        // easy case, no arguments to the function
        if env.hasf(&args.flatten()) {
            return Err(format!("Cannot run functions as jobs"));
        } else {
            id = try!(env.run_job(&args.flatten(), &vec![]));
        }
    } else if !args.get(0).is_flat() {
        return Err("Can only run flat names".to_string());
    } else if !FD_REGEX.is_match(args.get(0).flatten().as_slice()) &&
        !EQ_TEMP_REGEX.is_match(args.get(0).flatten().as_slice()) {
            // easy case, just a command
            let args_slice = args.slice(1, -1);
            if env.hasf(&args.get(0).flatten()) {
                return Err(format!("Cannot run functions as jobs"));
            } else {
                id = try!(env.run_job(&args.get(0).flatten(), &args_slice.flatten_vec()));
            }
        } else {
            // hard case, full argument set
            let (stdin, stdout, stderr, name, argc, envs) = try!(job_args(args, env));
            if env.hasf(&name) {
                return Err(format!("Cannot run functions as jobs"));
            } else {
                id = try!(env.run_job_fd(stdin, stdout, stderr, &name, &argc, &envs));
            }
        }
    return Ok(Flat(format!("{}", id)));
});

builtin!(job_output_func, args, env, {
    let arg = args.get(0);
    if !arg.is_flat() {
        return Err("Give a job number".to_string());
    }
    let id = match from_str_radix(arg.flatten().as_slice(), 10) {
        Err(e) => return Err(format!("Couldn't turn {} into a job number: {}", arg.flatten(), e)),
        Ok(num) => num
    };
    let out = try!(env.job_output(&id));
    if !out.status.success() {
        let mut s = String::from_utf8_lossy(out.error.as_slice()).into_owned();
        // remove trailing newlines
        match s.pop() {
            Some(v) if v != NL => s.push(v),
            _ => {}
        };
        return Err(s);
    } else {
        let mut s = String::from_utf8_lossy(out.output.as_slice()).into_owned();
        // remove trailing newlines
        match s.pop() {
            Some(v) if v != NL => s.push(v),
            _ => {}
        };
        return Ok(Flat(s));
    }
});

builtin!(directed_job_func, args, env, {
    return job_output_func(&try!(job_func(args, env)), env);
});

builtin!(run_func, args, env, {
    let out;
    if args.is_empty() || args.len() < 1 {
        return Err("No arguments given".to_string());
    } else if args.is_flat() {
        // easy case, no arguments to the function
        if env.hasf(&args.flatten()) {
            try!(env.runf(&args.flatten(), &Empty));
            out = ExitStatus(0);
        } else {
            out = try!(env.run_command(&args.flatten(), &vec![]));
        }
    } else if !args.get(0).is_flat() {
        return Err("Can only run flat names".to_string());
    } else if !FD_REGEX.is_match(args.get(0).flatten().as_slice()) &&
        !EQ_TEMP_REGEX.is_match(args.get(0).flatten().as_slice()) {
            // easy case, just a command
            let args_slice = args.slice(1, -1);
            if env.hasf(&args.get(0).flatten()) {
                try!(env.runf(&args.get(0).flatten(), &args_slice));
                out = ExitStatus(0);
            } else {
                out = try!(env.run_command(&args.get(0).flatten(), &args_slice.flatten_vec()));
            }
        } else {
            // hard case, full argument set
            let (stdin, stdout, stderr, name, argc, envs) = try!(job_args(args, env));
            if env.hasf(&name) {
                return Err(format!("Cannot redirect function output"));
            } else {
                out = try!(env.run_command_fd(stdin, stdout, stderr, &name, &argc, &envs));
            }
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
});

builtin!(jobs_func, _, env, {
    let jobs = env.get_jobs();
    if jobs.len() == 0 {
        return Err("No jobs".to_string());
    } else {
        return Ok(env.get_jobs());
    }
});

builtin!(fg_func, args, env, {
    let mut id;
    if args.len() < 1 {
        id = try!(env.front_job());
        while !env.has_job(&id) {
            id = try!(env.front_job());
        }
    } else {
        id = match from_str_radix(args.get(0).flatten().as_slice(), 10) {
            Err(e) => return Err(format!("Not given a job number: {}", e)),
            Ok(v) => v
        };
        if !env.has_job(&id) {
            return Err(format!("Job not found"));
        }
    }
    let name = try!(env.get_job(&id)).command.clone();
    env.outf(format_args!("Returning to: {}\n", name));
    try!(env.restart_job(&id));
    let out = try!(env.wait_job(&id));
    try!(env.remove_if_done(&id));
    return describe_process_output(&match out {
        ExitSignal(sig) => Long(vec![Flat("signal".to_string()),
                                     Flat(format!("{}", sig))]),
        ExitStatus(status) => Long(vec![Flat("status".to_string()),
                                        Flat(format!("{}", status))])
    }, env)
});

builtin!(get_func, args, env, {
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
});

builtin!(setp_func, args, env, {
    if args.len() < 1 {
        env.variables = String::new();
        return Ok(Empty);
    } else {
        let path = match args.get(0) {
            ref v if !v.is_flat() => {
                return Err("Variable paths can only be flat".to_string());
            }
            v => v.flatten()
        };
        if path == "cfg" {
            return Err(format!("Cannot set variable path to configuration variables"));
        } else if path == "sys" {
            return Err(format!("Cannot set variable path to system variables"));
        } else if path == "env" {
            return Err("Cannot set variable path to environment variables".to_string());
        } else if path == "pipe" {
            return Err("Cannot set variable path to job pipes".to_string());
        } else {
            env.variables = path.clone();
            return Ok(Empty);
        }
    }
});

builtin!(describe_process_output, args, _, {
    let argv = args.flatten_vec();
    if args.is_empty() {
        return Err("Command Failed".to_string());
    } else if argv.len() < 2 {
        return Err(format!("Command failed: {}", args.flatten()));
    } else if argv == vec!["signal", "19"] || // SIGSTOP
        argv == vec!["signal", "20"] { // SIGTSTP
            return Err(format!("Command stopped"));
        } else if argv != vec!["status", "0"] {
            return Err(format!("Command failed with {} {}", argv[0], argv[1]));
        } else {
            return Ok(Empty);
        }
});

builtin!(run_failed_func, args, _, {
    let argv = args.flatten_vec();
    if argv == vec!["status", "0"] {
        Ok(Flat(format!("succeeded")))
    } else {
        Ok(Empty)
    }
});

builtin!(ftime_func, args, _, {
    let fmt = match args.get(0) {
        Flat(s) => s,
        _ => return Err(format!("Time format must be flat"))
    };
    let lt = match get_time() {
        None => return Err(format!("Could not get current time")),
        Some(t) => t
    };
    return Ok(Flat(strf_time(&fmt, &lt)));
});

builtin!(dot_func, args, _, {
    return Ok(Flat(args.flatten_vec().concat()));
});

builtin!(prompt_func, _, env, {
    return dot_func(&Long(vec![
        try!(env.getvp(&format!("login"), &format!("sys"))),
        Flat(format!("@")),
        try!(env.getvp(&format!("hostname"), &format!("sys"))),
        Flat(format!(":")),
        try!(env.getvp(&format!("scwd"), &format!("sys"))),
        Flat(format!(" => $("))
            ]), env);
});

builtin!(subprompt_func, _, _, {
    return Ok(Flat(format!(" => $(")));
});

builtin!(open_output_func, args, env, {
    let fname = match args {
        &Flat(ref s) => s.clone(),
        _ => return Err(format!("File name must be flat"))
    };
    let fpath = expand_path(PathBuf::new(&fname));
    let fid = try!(env.output_file(&fpath));
    Ok(Flat(format!("{}", fid)))
});

builtin!(open_input_func, args, env, {
    let fname = match args {
        &Flat(ref s) => s.clone(),
        _ => return Err(format!("File name must be flat"))
    };
    let fpath = expand_path(PathBuf::new(&fname));
    let fid = try!(env.input_file(&fpath));
    Ok(Flat(format!("{}", fid)))
});

builtin!(builtins_func, _, _, {
    return Ok(Long(vec![
        Flat("$".to_string()),
        Flat("builtins".to_string()),
        Flat("cd".to_string()),
        Flat("dot".to_string()),
        Flat("fg".to_string()),
        Flat("get".to_string()),
        Flat("jobs".to_string()),
        Flat("run".to_string()),
        Flat("setp".to_string()),
        Flat("source".to_string())]));
});

pub fn load_builtins(env:&mut WashEnv) -> Result<WashArgs, String> {
    // functions
    try!(env.insfd("source", source_func));
    try!(env.insfd("cd", cd_func));
    try!(env.insfd("builtins", builtins_func));
    try!(env.insfd("outs", outs_func));
    try!(env.insfd("$", directed_job_func));
    try!(env.insfd("run", run_func));
    try!(env.insfd("get", get_func));
    try!(env.insfd("setp", setp_func));
    try!(env.insfd("jobs", jobs_func));
    try!(env.insfd("job", job_func));
    try!(env.insfd("fg", fg_func));
    try!(env.insfd("ftime", ftime_func));
    try!(env.insfd("dot", dot_func));
    try!(env.insfd("prompt", prompt_func));
    try!(env.insfd("subprompt", subprompt_func));
    try!(env.insfd("equal?", equal_func));
    try!(env.insfd("not?", not_func));
    try!(env.insfd("re_equal?", re_equal_func));
    try!(env.insfd("open_input", open_input_func));
    try!(env.insfd("open_output", open_output_func));
    try!(env.insfd("run_failed?", run_failed_func));
    try!(env.insfd("getall", getall_func));
    try!(env.insfd("flatten_eqlist", flatten_eqlist_func));

    // commands that aren't really meant to be called by users
    try!(env.insfd("describe_process_output", describe_process_output));

    return Ok(Empty);
}
