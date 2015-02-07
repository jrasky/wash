use regex::Regex;

use std::num::*;

use types::WashArgs::*;
use state::HandlerResult::*;

use util::*;
use constants::*;
use types::*;
use state::*;
use builtins::*;

macro_rules! handler {
    ($name:ident, $pre:pat, $next:pat, $state:pat, $func:block) => {
        fn $name($pre:&mut Vec<WashArgs>, $next:&mut Vec<InputValue>,
                 $state:&mut ShellState) -> Result<HandlerResult, String>
            $func
    }
}

handler!(equal_handler, pre, next, state, {
    // set variable and stop
    let (path, name, _, val) = try!(equal_inner(pre, next, state));
    if path.is_none() {
        try!(state.env.insv(name, val));
    } else {
        try!(state.env.insvp(name, path.unwrap(), val));
    }
    return Ok(Stop);
});

fn equal_inner(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>,
               state:&mut ShellState) -> Result<(Option<String>, String, WashArgs, WashArgs), String> {
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
        val = try!(state.input_to_args(next.remove(0)));
    }
    if EQ_PATH_REGEX.is_match(name.as_slice()) {
        let caps = EQ_PATH_REGEX.captures(name.as_slice()).unwrap();
        let path = caps.at(1).unwrap().to_string();
        let name = caps.at(2).unwrap().to_string();
        if path.is_empty() {
            // use default path
            // this can be used to set a variable
            // with a name containing a colon
            let old = state.env.getv(&name).ok().unwrap_or(Empty);
            return Ok((None, name, old, val));
        } else {
            let old = state.env.getvp(&name, &path).ok().unwrap_or(Empty);
            return Ok((Some(path), name, old, val));
        }
    } else {
        let old = state.env.getv(&name).ok().unwrap_or(Empty);
        return Ok((None, name, old, val));
    }
    // right now equals can only produce Stop.
    // In the future this may not be the case

}

handler!(equalequal_handler, pre, next, state, {
    let comp = try!(state.input_to_args(InputValue::Long(next.clone())));
    if Long(pre.clone()).flatten_vec() == comp.flatten_vec() {
        pre.clear();
        next.clear();
        return Ok(Continue);
    } else {
        return Ok(Stop);
    }
});

handler!(dot_handler, pre, next, state, {
    let last = match pre.pop() {
        None => return Err(format!("Cannot dot with nothing")),
        Some(v) => v
    };
    if next.is_empty() {
        return Err(format!("Cannot dot with nothing"));
    }
    let anext = try!(state.input_to_args(next.remove(0)));
    pre.push(Flat(vec![last.flatten(), anext.flatten()].concat()));
    return Ok(Continue);
});

handler!(tildaequal_handler, pre, next, state, {
    let re = match Regex::new(try!(state.input_to_args(InputValue::Long(next.clone()))).flatten().as_slice()) {
        Err(e) => return Err(format!("{}", e)),
        Ok(v) => v
    };
    if re.is_match(match pre.pop() {
        None => return Err("Nothing to compare to".to_string()),
        Some(v) => v
    }.flatten().as_slice()) {
        pre.clear();
        next.clear();
        return Ok(Continue);
    } else {
        return Ok(Stop);
    }
});

handler!(semiamper_handler, pre, _, state, {
    // effectively the "continue" handler
    // run the part before the line and then continue
    // onto the next one no matter what
    // but one after the other
    match run_func(&Long(pre.clone()), &mut state.env) {
        Err(e) => state.env.errf(format_args!("{}\n", e)),
        Ok(v) => match describe_process_output(&v, &mut state.env) {
            Err(e) => {
                state.env.errf(format_args!("{}\n",e));
            },
            _ => {/* nothing */}
        }
    };
    // ;& does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
});

handler!(amperamper_handler, pre, _, state, {
    // "and then:" run this command and continue only if it succeded
    // onto the next one no matter what
    let out = try!(run_func(&Long(pre.clone()), &mut state.env));
    // will return on error
    try!(describe_process_output(&out, &mut state.env));
    // && does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
});

handler!(amper_handler, pre, _, state, {
    // almost directly calls job
    // ignore errors
    match job_func(&Long(pre.clone()), &mut state.env) {
        Err(e) => state.env.errf(format_args!("{}\n", e)),
        Ok(v) => state.env.outf(format_args!("Started job: {}\n", v.flatten()))
    }
    // & does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
});

handler!(bar_handler, pre, _, state, {
    if pre.len() < 1 {
        return Err("Cannot pipe nothing".to_string());
    }
    let id:usize = match try!(job_func(&Long(pre.clone()), &mut state.env)) {
        Flat(v) => match from_str_radix(v.as_slice(), 10) {
            Err(e) => return Err(format!("djob did not return a job number: {}", e)),
            Ok(v) => v
        },
        _ => return Err("djob did not return a job number".to_string())
    };
    pre.clear();
    pre.push(try!(state.env.getvp(&format!("{}", id), &"pipe".to_string())));
    return Ok(Continue);
});

handler!(leq_handler, pre, next, state, {
    // file input
    if next.is_empty() {
        return Err("File name must be provided".to_string());
    }
    let fname = match try!(state.input_to_args(next.remove(0))) {
        Flat(s) => s,
        _ => return Err("File name must be flat".to_string())
    };
    let fpath = expand_path(Path::new(fname));
    let fid = try!(state.env.input_file(&fpath));
    pre.insert(0, Flat(format!("@{}", fid)));
    return Ok(Continue);
});

handler!(geq_handler, pre, next, state, {
    // file output
    if next.is_empty() {
        return Err("File name must be provided".to_string());
    }
    let fname = match try!(state.input_to_args(next.remove(0))) {
        Flat(s) => s,
        _ => return Err("File name must be flat".to_string())
    };
    let fpath = expand_path(Path::new(fname));
    let fid = try!(state.env.output_file(&fpath));
    pre.insert(0, Flat(format!("@out:{}", fid)));
    let out = try!(run_func(&Long(pre.clone()), &mut state.env));
    try!(describe_process_output(&out, &mut state.env));
    // stop no matter what
    return Ok(Stop);
});

fn block_handler(name:String, pre:&mut Vec<WashArgs>,
                 next:&mut Vec<InputValue>, _:&mut ShellState) -> Result<HandlerResult, String> {
    if !pre.is_empty() {
        return Err("Malformed block".to_string());
    }
    let content = try!(create_content(next));
    // shave off final splits in next
    while match get_nm_index(next, next.len() - 1) {
        Some(&InputValue::Split(_)) => true,
        _ => false
    } {
        next.pop();
    }
    let close;
    if content.is_empty() {
        close = vec![InputValue::Short("}".to_string())];
    } else {
        close = vec![];
    }
    // test function for More case of HandlerResult
    let block = WashBlock {
        start: name,
        next: next.clone(),
        close: close,
        content: content
    };
    return Ok(More(block));
}

handler!(end_block_handler, _, _, _, {
    // helper to tell users not to use this in a line
    return Err("Malformed block".to_string());
});

handler!(act_handler, pre, next, state, {
    block_handler("act".to_string(), pre, next, state)
});

handler!(if_handler, pre, next, state, {
    block_handler("if".to_string(), pre, next, state)
});

handler!(else_handler, pre, next, state, {
    block_handler("else".to_string(), pre, next, state)
});

handler!(loop_handler, pre, next, state, {
    block_handler("loop".to_string(), pre, next, state)
});

pub fn load_handlers(state:&mut ShellState) -> Result<WashArgs, String> {
    // handlers
    try!(state.insert_handler("=", equal_handler));
    try!(state.insert_handler(";&", semiamper_handler));
    try!(state.insert_handler("&&", amperamper_handler));
    try!(state.insert_handler("&", amper_handler));
    try!(state.insert_handler("|", bar_handler));
    try!(state.insert_handler("<", leq_handler));
    try!(state.insert_handler(">", geq_handler));
    try!(state.insert_handler("==", equalequal_handler));
    try!(state.insert_handler("~=", tildaequal_handler));
    try!(state.insert_handler(".", dot_handler));

    // block start/end
    try!(state.insert_handler("act!", act_handler));
    try!(state.insert_handler("if!", if_handler));
    try!(state.insert_handler("else!", else_handler));
    try!(state.insert_handler("loop!", loop_handler));
    try!(state.insert_handler("}", end_block_handler));

    return Ok(Empty);
}
