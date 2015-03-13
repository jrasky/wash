use std::collections::*;

use constants::*;
use types::*;
use ast::*;

use types::Action::*;
use types::HandlerResult::*;
use types::InputValue::*;

macro_rules! handler {
    ($name:ident, $contents:pat, $count:pat,
     $out:pat, $ast:pat, $func:block) => {
        fn $name($contents:&mut LinkedList<InputValue>, $count:&mut usize,
                 $out:&mut LinkedList<Action>, $ast:&mut AST) -> AstResult
            $func
    }
}

handler!(handle_equal, contents, count, out, ast, {
    // since this is a Long, the name/path combo
    // should already be on VS at this point
    // if the variable is named directly, it could be that
    // two instructions up is a Load
    // remove this so you don't have to "$name" every time
    let back1 = out.pop_back();
    let back2 = out.pop_back();
    if back1 == Some(Temp) && back2 == Some(Load) {
        out.push_back(Temp);
    } else {
        match back2 {
            Some(v) => out.push_back(v),
            _ => {}
        }
        match back1 {
            Some(v) => out.push_back(v),
            _ => {}
        }
    }
    // now evaluate the value
    let mut newacs = LinkedList::new();
    if contents.is_empty() {
        newacs.push_back(Set(WashArgs::Empty));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value, false));
        if aclist.is_empty() {
            newacs.push_back(Set(WashArgs::Empty));
        } else {
            newacs.append(&mut aclist);
        }
    }
    // now the value is on CFV
    // name is hopefully on the top of VS
    newacs.push_back(Store);
    while match contents.front() {
        Some(&Split(_)) => true,
        _ => false
    } {
        contents.pop_front();
    }
    if !contents.is_empty() {
        // there are other things on the line, so this variable
        // should be unset at the end.
        out.push_back(ReInsert);
        out.push_back(Top);
        out.push_back(Load);
        out.push_back(Swap);
        out.push_back(Temp);
        ast.add_endline(Pull);
        ast.add_endline(Store);
    }
    out.append(&mut newacs);
    // in either case the end result is one item
    // is consumed from the original, given VS
    *count -= 1;
    return Ok(Continue);
});

handler!(equalequal_inner, contents, count, out, ast, {
    // LHS is already partially evaluated into VS
    if *count > 1 {
        // more than one element means we need to join
        // them and re-push them back
        out.push_back(Join(*count));
        out.push_back(Temp);
    }
    if contents.is_empty() {
        out.push_back(Insert(WashArgs::Empty));
    } else {
        while match contents.front() {
            Some(&Split(_)) => true,
            _ => false
        } {
            contents.pop_front();
        }
        let mut aclist = {
            if contents.len() > 1 {
                let mut v = vec![];
                loop {
                    match contents.pop_front() {
                        None => break,
                        Some(val) => v.push(val)
                    }
                }
                try!(ast.process(&mut Long(v), false))
            } else {
                try!(ast.process(contents.front_mut().unwrap(), false))
            }
        };
        out.append(&mut aclist);
        out.push_back(Temp);
    }
    // now the two arguments we're interested in are at the top
    // of the VS
    out.push_back(Join(2));
    // VS has been emptied as a result
    *count = 0;
    return Ok(Continue);
});

handler!(handle_equalequal, contents, count, out, ast, {
    try!(equalequal_inner(contents, count, out, ast));
    out.push_back(Call(format!("equal?")));
    return Ok(Stop);
});

handler!(handle_tildaequal, contents, count, out, ast, {
    try!(equalequal_inner(contents, count, out, ast));
    out.push_back(Call(format!("re_equal?")));
    return Ok(Stop);
});

handler!(handle_dot, contents, count, out, ast, {
    if contents.is_empty() {
        out.push_back(Insert(WashArgs::Empty));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value, false));
        if aclist.is_empty() {
            out.push_back(Insert(WashArgs::Empty));
        } else {
            out.append(&mut aclist);
            out.push_back(Temp);
        }
    }
    out.push_back(Join(2));
    out.push_back(Call(format!("dot")));
    if !contents.is_empty() || *count > 1 {
        // Only temp if there's something else on the line
        out.push_back(Temp);
    } else {
        *count -= 1;
    }
    return Ok(Continue);
});

handler!(handle_semiamper, _, count, out, _, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 0;
    }
    out.push_back(Call(format!("run")));
    return Ok(Continue);
});

handler!(handle_amper, _, count, out, _, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 0;
    }
    out.push_back(Call(format!("job")));
    return Ok(Continue);
});

handler!(handle_amperamper, contents, count, out, ast, {
    // amperamper is an extension of semiamper
    try!(handle_semiamper(contents, count, out, ast));
    out.push_back(Call(format!("run_failed?")));
    let old_section = ast.new_section();
    let new_num = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't a numbered one")
    };
    ast.current_section().push_back(Fail(STOP.to_string()));
    ast.move_to(old_section);
    out.push_back(Branch(new_num));
    return Ok(Continue);
});

handler!(handle_bar, contents, count, out, ast, {
    // extension of amper
    try!(handle_amper(contents, count, out, ast));
    out.push_back(Insert(WashArgs::Flat(format!("$pipe:"))));
    out.push_back(Temp);
    out.push_back(Join(2));
    out.push_back(Call(format!("dot")));
    out.push_back(Load);
    out.push_back(Temp);
    *count += 1;
    return Ok(Continue);
});

handler!(handle_geq, contents, count, out, ast, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 1;
    }
    out.push_back(Temp);
    if contents.is_empty() {
        return Err(format!("No file name given"));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value, false));
        if aclist.is_empty() {
            return Err(format!("No file name given"));
        }
        out.append(&mut aclist);
        out.push_back(Call(format!("open_output")));
        out.push_back(Insert(WashArgs::Flat(format!("@out:"))));
        out.push_back(Temp);
        out.push_back(Join(2));
        out.push_back(Call(format!("dot")));
        return Ok(Continue);
    }
});

handler!(handle_leq, contents, count, out, ast, {
    if *count > 0 {
        out.push_back(Join(*count));
        *count = 1;
    }
    out.push_back(Temp);
    if contents.is_empty() {
        return Err(format!("No file name given"));
    } else {
        let mut value = match contents.pop_front().unwrap() {
            Split(_) if !contents.is_empty() => contents.pop_front().unwrap(),
            v => v
        };
        let mut aclist = try!(ast.process(&mut value, false));
        if aclist.is_empty() {
            return Err(format!("No file name given"));
        }
        out.append(&mut aclist);
        out.push_back(Call(format!("open_input")));
        out.push_back(Insert(WashArgs::Flat(format!("@"))));
        out.push_back(Temp);
        out.push_back(Join(2));
        out.push_back(Call(format!("dot")));
        return Ok(Continue);
    }
});

handler!(handle_if, contents, count, out, ast, {
    ast.current_section().append(out);
    let mut values = vec![];
    loop {
        match contents.pop_front() {
            None => break,
            Some(Short(ref s)) if *s == "{" => break,
            Some(v) => values.push(v)
        }
    }
    let mut aclist = try!(ast.process(&mut Long(values), false));
    let old_section = ast.new_section();
    let secnum = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.new_section();
    let finalsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.new_section();
    let elifsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.move_to(old_section);
    aclist.push_back(Branch(secnum));
    aclist.push_back(Jump(elifsec));
    ast.current_section().append(&mut aclist);
    ast.move_to(SectionType::Number(elifsec));
    ast.elif = Some(SectionType::Number(elifsec));
    ast.current_section().push_back(Jump(finalsec));
    *count = 0;
    ast.move_to(SectionType::Number(secnum));
    return Ok(More(SectionType::Number(finalsec)));
});

handler!(handle_elif, contents, count, out, ast, {
    let old_section = match ast.elif {
        None => return Err(format!("No proceeding if block for elif")),
        Some(s) => s
    };
    ast.current_section().append(out);
    let mut values = vec![];
    loop {
        match contents.pop_front() {
            None => break,
            Some(Short(ref s)) if *s == "{" => break,
            Some(v) => values.push(v)
        }
    }
    let mut aclist = try!(ast.process(&mut Long(values), false));
    ast.new_section();
    let secnum = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.new_section();
    let elifsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.move_to(old_section);
    let finalsec = match ast.current_section().pop_back() {
        Some(Jump(n)) => n,
        _ => panic!("Elif section malformed")
    };
    aclist.push_back(Branch(secnum));
    aclist.push_back(Jump(elifsec));
    ast.current_section().append(&mut aclist);
    ast.move_to(SectionType::Number(elifsec));
    ast.elif = Some(SectionType::Number(elifsec));
    ast.current_section().push_back(Jump(finalsec));
    *count = 0;
    ast.move_to(SectionType::Number(secnum));
    return Ok(More(SectionType::Number(finalsec)));
});

handler!(handle_else, contents, count, out, ast, {
    ast.current_section().append(out);
    let old_section = match ast.elif {
        None => return Err(format!("No proceeding if block for else")),
        Some(s) => s
    };
    loop {
        match contents.pop_front() {
            None => break,
            Some(Short(ref s)) if *s == "{" => break,
            Some(_) => {}
        }
    }
    ast.move_to(old_section);
    let finalsec = match ast.current_section().pop_back() {
        Some(Jump(n)) => n,
        _ => panic!("Elif section malformed")
    };
    ast.elif = None;
    *count = 0;
    return Ok(More(SectionType::Number(finalsec)));
});

handler!(handle_while, contents, count, out, ast, {
    ast.current_section().append(out);
    let mut values = vec![];
    loop {
        match contents.pop_front() {
            None => break,
            Some(Short(ref s)) if *s == "{" => break,
            Some(v) => values.push(v)
        }
    }
    let mut aclist = try!(ast.process(&mut Long(values), false));
    let old_sec = ast.new_section();
    let newsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.new_section();
    let finalsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.move_to(old_sec);
    ast.current_section().push_back(Jump(newsec));
    ast.move_to(SectionType::Number(newsec));
    aclist.push_back(Call(format!("not?")));
    aclist.push_back(Branch(finalsec));
    ast.current_section().append(&mut aclist);
    *count = 0;
    ast.sec_loop = true;
    return Ok(More(SectionType::Number(finalsec)));
});

handler!(handle_func, contents, count, out, ast, {
    ast.current_section().append(out);
    let mut values = vec![];
    loop {
        match contents.pop_front() {
            None => break,
            Some(Short(ref s)) if *s == "{" => break,
            Some(v) => values.push(v)
        }
    }
    let mut aclist = try!(ast.process(&mut Long(values), false));
    let old_sec = ast.new_section();
    let newsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.move_to(old_sec);
    aclist.push_back(Save(newsec));
    ast.current_section().append(&mut aclist);
    ast.move_to(SectionType::Number(newsec));
    *count = 0;
    ast.no_jump = true;
    return Ok(More(old_sec));
});

handler!(handle_act, contents, count, out, ast, {
    ast.current_section().append(out);
    loop {
        match contents.pop_front() {
            None => break,
            Some(Short(ref s)) if *s == "{" => break,
            Some(_) => {}
        }
    }
    let old_sec = ast.new_section();
    let newsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.new_section();
    let finalsec = match ast.get_position() {
        SectionType::Number(n) => n,
        _ => panic!("New section wasn't numbered")
    };
    ast.move_to(old_sec);
    ast.current_section().push_back(Jump(newsec));
    ast.move_to(SectionType::Number(newsec));
    *count = 0;
    return Ok(More(SectionType::Number(finalsec)));
});

handler!(handle_endblock, _, count, out, ast, {
    {
        let mut sec = ast.current_section();
        sec.append(out);
        if *count > 0 {
            if *count > 1 {
                sec.push_back(Join(*count));
            } else {
                sec.push_back(Pull);
            }
            sec.push_back(Call(format!("run")));
            sec.push_back(Call(format!("describe_process_output")));
        }
    }
    try!(ast.end_block());
    *count = 0;
    return Ok(Continue);
});

pub fn load_handlers(ast:&mut AST) {
    ast.add_handler("=", handle_equal);
    ast.add_handler("==", handle_equalequal);
    ast.add_handler("~=", handle_tildaequal);
    ast.add_handler(".", handle_dot);
    ast.add_handler("&;", handle_semiamper);
    ast.add_handler("&", handle_amper);
    ast.add_handler("&&", handle_amperamper);
    ast.add_handler("|", handle_bar);
    ast.add_handler(">", handle_geq);
    ast.add_handler("<", handle_leq);

    ast.add_handler("act!", handle_act);
    ast.add_handler("if!", handle_if);
    ast.add_handler("elif!", handle_elif);
    ast.add_handler("else!", handle_else);
    ast.add_handler("while!", handle_while);
    ast.add_handler("func!", handle_func);
    ast.add_handler("}", handle_endblock);
}
