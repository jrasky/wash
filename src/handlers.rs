use regex::Regex;

use types::WashArgs::*;
use state::HandlerResult::*;

use util::*;
use constants::*;
use types::*;
use state::*;
use builtins::*;

fn equal_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
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
            try!(state.env.insv(name, val.clone()));
            return Err(STOP.to_string());
        } else {
            try!(state.env.insvp(name, path, val.clone()));
            return Err(STOP.to_string());
        }
    } else {
        try!(state.env.insv(name, val.clone()));
        return Err(STOP.to_string());
    }
    // right now equals can only produce Stop.
    // In the future this may not be the case
}

fn equalequal_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    let comp = try!(state.input_to_args(InputValue::Long(next.clone())));
    if Long(pre.clone()).flatten_vec() == comp.flatten_vec() {
        pre.clear();
        next.clear();
        return Ok(Continue);
    } else {
        return Ok(Stop);
    }
}

fn tildaequal_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
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
}

fn semiamper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
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
}


fn amperamper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    // "and then:" run this command and continue only if it succeded
    // onto the next one no matter what
    let out = try!(run_func(&Long(pre.clone()), &mut state.env));
    // will return on error
    try!(describe_process_output(&out, &mut state.env));
    // && does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
}

fn amper_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    // almost directly calls job
    // ignore errors
    match job_func(&Long(pre.clone()), &mut state.env) {
        Err(e) => state.env.errf(format_args!("{}\n", e)),
        Ok(v) => state.env.outf(format_args!("Started job: {}\n", v.flatten()))
    }
    // & does not pass on the value of the previous command
    pre.clear();
    return Ok(Continue);
}

fn bar_handler(pre:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    if pre.len() < 1 {
        return Err("Cannot pipe nothing".to_string());
    }
    let id = match try!(job_func(&Long(pre.clone()), &mut state.env)) {
        Flat(v) => match str_to_usize(v.as_slice()) {
            None => return Err("djob did not return a job number".to_string()),
            Some(v) => v
        },
        _ => return Err("djob did not return a job number".to_string())
    };
    pre.clear();
    pre.push(try!(state.env.getvp(&format!("{}", id), &"pipe".to_string())));
    return Ok(Continue);
}

fn leq_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
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
}

fn geq_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
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
}

fn block_handler(name:String, pre:&mut Vec<WashArgs>,
               next:&mut Vec<InputValue>, _:&mut ShellState) -> Result<HandlerResult, String> {
    if !pre.is_empty() {
        return Err("Malformed block".to_string());
    }
    let content = try!(create_content(next));
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

fn end_block_handler(_:&mut Vec<WashArgs>, _:&mut Vec<InputValue>, _:&mut ShellState) -> Result<HandlerResult, String> {
    // helper to tell users not to use this in a line
    return Err("Malformed block".to_string());
}

fn act_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    block_handler("act".to_string(), pre, next, state)
}

fn if_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    block_handler("if".to_string(), pre, next, state)
}

fn else_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    block_handler("else".to_string(), pre, next, state)
}

fn loop_handler(pre:&mut Vec<WashArgs>, next:&mut Vec<InputValue>, state:&mut ShellState) -> Result<HandlerResult, String> {
    block_handler("loop".to_string(), pre, next, state)
}

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

    // block start/end
    try!(state.insert_handler("act!", act_handler));
    try!(state.insert_handler("if!", if_handler));
    try!(state.insert_handler("else!", else_handler));
    try!(state.insert_handler("loop!", loop_handler));
    try!(state.insert_handler("}", end_block_handler));

    return Ok(Empty);
}
