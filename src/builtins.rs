use std::os;
use std::cmp::*;

use script::*;
use util::*;

// Calling convention:
// fn(args:&Vec<String>, u_env:*mut WashEnv) -> Vec<String>
fn source_func(args:&Vec<String>, env:&mut WashEnv) -> Vec<String> {
    // in this case args is line
    if args.len() < 1 {
        env.controls.err("No arguments given");
        return vec![];
    }
    let out = env.load_script(Path::new(args[0].clone()), &args.slice_from(1).to_vec());
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
    match args.len() {
        0 => return vec![],
        v if v <= 1 || args[1] != "=".to_string() =>
            match os::getenv(args[0].as_slice()) {
            Some(val) => return vec![val],
            None => return vec![]
        },
        2 if args[1] == "=".to_string() => {
            os::unsetenv(args[0].as_slice());
            return vec![];
        },
        _ if args[1] == "=".to_string() => {
            os::setenv(args[0].as_slice(), args[2].as_slice());
            return vec![args[2].clone()];
        },
        _ => {
            // something went wrong, this case should never happen
            env.controls.err("Unreachable case reached\n");
            return vec![];
        }
    }
}

pub fn load_builtins(env:&mut WashEnv) {
    env.functions.insert("source".to_string(), source_func);
    env.functions.insert("cd".to_string(), cd_func);
    env.functions.insert("senv".to_string(), senv_func);
}
