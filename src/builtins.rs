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
        env.controls.err("No arguments given");
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
            env.controls.errf(format_args!("Failed: {}: {}\n", newp.display(), e));
        }
    }
    return Empty;
}

fn senv_func(args:&WashArgs, _:&mut WashEnv) -> WashArgs {
    let farg = args.get_flat(0);
    let arg = farg.as_slice();
    if regex!(r"^\S+=").is_match(arg) {
        let re = regex!("=");
        let parts = re.splitn(arg, 2).collect::<Vec<&str>>();
        os::setenv(parts[0], parts[1]);
        return Flat(parts[1].to_string());
    } else if regex!(r"^\S+$").is_match(arg) {
        os::unsetenv(arg);
        return Empty;
    } else {
        // user supplied some variable name with spaces in it
        // which isn't allowed
        return Empty;
    }
}

fn genv_func(args:&WashArgs, _:&mut WashEnv) -> WashArgs {
    let fname = args.get_flat(0);
    let name = fname.as_slice();
    if !regex!(r"^\S+$").is_match(name) {
        // takes care of all bad arguments
        return Empty;
    }
    match os::getenv(name) {
        None => return Empty,
        Some(val) => return Flat(val)
    }
}

fn outs_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    let mut argf = args.flatten();
    env.controls.outs(argf.as_slice());
    if argf.pop() != Some(NL) {
        env.controls.outc(NL);
    }
    return Empty;
}

#[allow(unused_variables)]
fn builtins_func(args:&WashArgs, env:&mut WashEnv) -> WashArgs {
    return Long(vec![
        Flat("builtins".to_string()),
        Flat("cd".to_string()),
        Flat("genv".to_string()),
        Flat("senv".to_string()),
        Flat("source".to_string())]);
}

pub fn load_builtins(env:&mut WashEnv) {
    env.insf("source", source_func);
    env.insf("cd", cd_func);
    env.insf("senv", senv_func);
    env.insf("builtins", builtins_func);
    env.insf("genv", genv_func);
    env.insf("outs", outs_func);
}
