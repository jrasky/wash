use std::old_io::process::ProcessExit::*;
use std::os::unix::prelude::*;

use std::os;
use std::cmp::*;

use types::WashArgs::*;

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

fn job_args(args:&WashArgs, env:&mut WashEnv) -> Result<(Option<Fd>, Option<Fd>, Option<Fd>,
                                                         String, Vec<String>), String> {
    // turns arguments into file descriptor options, command name and args
    // utility function because job_func and run_func use this same code
    let (mut stdin, mut stdout, mut stderr) = (None, None, None);
    let mut argc = args.flatten_vec();
    let mut name;
    loop {
        // check for stop
        try!(env.func_stop());
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
        let (stdin, stdout, stderr, name, argc) = try!(job_args(args, env));
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
        let (stdin, stdout, stderr, name, argc) = try!(job_args(args, env));
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

    return Ok(Empty);
}
