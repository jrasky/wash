use std::os;
use std::cmp::*;

use script::*;
use util::*;

// Calling convention:
// fn(args:&Vec<String>, u_env:*mut WashEnv) -> Vec<String>
fn load_func(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    // in this case args is line
    if args.len() < 1 {
        env.controls.err("No arguments given");
        return vec![];
    }
    env.load_script(Path::new(args[0].clone()), &args.slice_from(1).to_vec())
}

fn source_func(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    // in this case args is line
    let out = load_func(args, env);
    return out.slice_from(min(2, out.len())).to_vec();
}

fn cd_func(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    let newp = {
        if args.len() == 0 {
            expand_path(Path::new("~"))
        } else if args[0].slice_to(min(args[0].len(), 2)) == "./" {
            // this specifical case can't be put through expand_path
            Path::new(&args[0])
        } else {
            expand_path(Path::new(&args[0]))
        }
    };
    match os::change_dir(&newp) {
        Ok(_) => {},
        Err(e) => {
            env.controls.errf(format_args!("Failed: {}: {}\n", newp.display(), e));
        }
    }
    return Vec::new();
}

fn senv_func(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    if args.len() == 0 {
        return vec![];
    }
    if regex!(r"^\S+[=]").is_match(args[0].as_slice()) {
        let re = regex!("=");
        let parts = collect(re.splitn(args[0].as_slice(), 2));
        // note: parts will have length of exactly 2
        if parts[1] == "" {
            os::unsetenv(parts[0]);
            return vec![];
        } else {
            os::setenv(parts[0], parts[1]);
            return vec![parts[1].to_string()];
        }
    } else if regex!(r"^\S+$").is_match(args[0].as_slice()) {
        match os::getenv(args[0].as_slice()) {
            None => return vec![],
            Some(val) => return vec![val]
        }
    } else {
        // user supplied some variable name with spaces in it
        // which isn't allowed
        return vec![];
    }
}

#[allow(unused_variables)]
fn builtins_func(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    return vec![
        "builtins".to_string(),
        "cd".to_string(),
        "load".to_string(),
        "senv".to_string(),
        "source".to_string()];
}

pub fn load_builtins(env:&mut WashEnv) {
    env.insf("source", source_func);
    env.insf("load", load_func);
    env.insf("cd", cd_func);
    env.insf("senv", senv_func);
    env.insf("builtins", builtins_func);
}
