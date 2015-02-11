use std::old_io::process::ProcessExit::*;
use std::os::unix::prelude::*;

use std::env;
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
    env.load_script(Path::new(name), &args.slice(1, -1))
});

builtin!(cd_func, args, _, {
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
    match env::set_current_dir(&newp) {
        Err(e) => return Err(e.desc.to_string()),
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
        id = try!(env.run_job(&args.flatten(), &vec![]));
    } else if !args.get(0).is_flat() {
        return Err("Can only run flat names".to_string());
    } else if !FD_REGEX.is_match(args.get(0).flatten().as_slice()) &&
        !EQ_TEMP_REGEX.is_match(args.get(0).flatten().as_slice()) {
        // easy case, just a command
        let args_slice = args.slice(1, -1);
        id = try!(env.run_job(&args.get(0).flatten(), &args_slice.flatten_vec()));
    } else {
        // hard case, full argument set
        let (stdin, stdout, stderr, name, argc, envs) = try!(job_args(args, env));
        id = try!(env.run_job_fd(stdin, stdout, stderr, &name, &argc, &envs));
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
        out = try!(env.run_command(&args.flatten(), &vec![]));
    } else if !args.get(0).is_flat() {
        return Err("Can only run flat names".to_string());
    } else if !FD_REGEX.is_match(args.get(0).flatten().as_slice()) &&
        !EQ_TEMP_REGEX.is_match(args.get(0).flatten().as_slice()) {
            // easy case, just a command
            let args_slice = args.slice(1, -1);
            out = try!(env.run_command(&args.get(0).flatten(), &args_slice.flatten_vec()));
    } else {
        // hard case, full argument set
        let (stdin, stdout, stderr, name, argc, envs) = try!(job_args(args, env));
        out = try!(env.run_command_fd(stdin, stdout, stderr, &name, &argc, &envs));
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
        return Ok(Flat(String::new()));
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
            return Ok(Flat(path));
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
    try!(env.insf("fg", fg_func));
    try!(env.insf("ftime", ftime_func));
    try!(env.insf("dot", dot_func));
    try!(env.insf("prompt", prompt_func));
    try!(env.insf("subprompt", subprompt_func));

    // commands that aren't really meant to be called by users
    try!(env.insf("describe_process_output", describe_process_output));

    return Ok(Empty);
}
