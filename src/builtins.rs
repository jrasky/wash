use std::os;
use std::cmp::*;

use script::*;
use util::*;

// Calling convention:
// fn(args:&Vec<String>, u_env:*mut WashEnv) -> Vec<String>
fn source(args:&Vec<String>, u_env:*mut WashEnv) -> Vec<String> {
    // in this case args is line
    let env = unsafe {
        u_env.as_mut().unwrap()
    };
    if !env.scripts.contains_key(&args.clone()[0]) {
        // only access script objects by borrowing from the hash map
        // this is to ensure lifetimes
        env.scripts.insert(args[0].clone(),
                           WashScript::new(Path::new(&args[0])));
    }
    let script = env.scripts.get_mut(args[0].as_slice()).unwrap();
    if !script.is_compiled() && !script.compile() {
        env.controls.err("Failed to compile script\n");
        return Vec::new();
    }
    env.controls.flush();
    if script.is_runnable() {
        script.run(&args.slice_from(1).to_vec(), env);
    } else if script.is_loadable() {
        script.load(&args.slice_from(1).to_vec(), env);
    } else {
        env.controls.err("Cannot load or run script\n");
    }
    return Vec::new();
}

fn cd(args:&Vec<String>, u_env:*mut WashEnv) -> Vec<String> {
    let env = unsafe {
        u_env.as_mut().unwrap()
    };
    let newp = {
        if args.len() == 0 {
            expand_path(Path::new("~"))
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

pub fn load_builtins(env:&mut WashEnv) {
    env.functions.insert(String::from_str("source"), source);
    env.functions.insert(String::from_str("cd"), cd);
}
