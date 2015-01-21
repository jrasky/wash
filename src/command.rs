use libc::*;

use std::io::process::{Command, ProcessOutput, ProcessExit, Process};
use std::io::process::StdioContainer::*;
use std::io::{IoError, IoErrorKind, IoResult};
use std::io::IoErrorKind::*;
use std::collections::VecMap;
use std::os::unix::prelude::*;
use std::io::{File, Append, Open, Read, Write};

use controls::*;
use constants::*;
use termios::*;
use signal::*;

// start off as null pointer
static mut uglobal_term:*mut TermState = 0 as *mut TermState;

unsafe extern fn term_sigint(_:c_int, _:*const SigInfo,
                             _:*const c_void) {
    let term:&mut TermState = match uglobal_term.as_mut() {
        Some(v) => v,
        None => {
            // this handler shouldn't be called when Term isn't active
            panic!("Term signal interrupt called when Term not active");
        }
    };
    // delete "^C"
    term.controls.outc(BS);
    term.controls.outc(BS);
    term.controls.outc(SPC);
    term.controls.outc(SPC);
    term.controls.outs("\nInterrupt\n");
    // pass on to foreground job, if there is one
    match term.fg_job {
        None => {/* nothing */},
        Some(id) => {
            match term.interrupt_job(&id) {
                Err(e) => {
                    term.controls.errf(format_args!("Could not interrupt job: {}", e));
                },
                Ok(_) => {/* nothing */}
            }
        }
    }
}

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios,
    jobs: VecMap<(String, Process, Vec<usize>)>,
    fg_job: Option<usize>,
    files: VecMap<File>,
    next_file: Option<usize>
}

impl TermState {
    pub fn new() -> TermState {
        let mut controls = Controls::new();
        let mut tios = match Termios::get() {
            Some(t) => t,
            None => {
                controls.err("Warning: Could not get terminal mode\n");
                Termios::new()
            }
        };
        let old_tios = tios.clone();
        tios.fdisable(0, 0, ICANON|ECHO, 0);
        
        return TermState {
            controls: controls,
            tios: tios,
            old_tios: old_tios,
            jobs: VecMap::new(),
            fg_job: None,
            files: VecMap::new(),
            next_file: None
        };
    }

    pub fn update_terminal(&mut self) {
        if !Termios::set(&self.tios) {
            self.controls.err("Warning: Could not set terminal mode\n");
        }
    }

    pub fn restore_terminal(&mut self) {
        if !Termios::set(&self.old_tios) {
            self.controls.err("Warning: Could not set terminal mode\n");
        }
    }

    fn set_pointer(&mut self) {
        unsafe {
            if !uglobal_term.is_null() {
                panic!("Tried to set Term location twice");
            }
            uglobal_term = self as *mut TermState;
        }
    }

    fn unset_pointer(&mut self) {
        unsafe {
            if uglobal_term.is_null() {
                panic!("Tried to unset Term location twice");
            }
            uglobal_term = 0 as *mut TermState;
        }
    }

    fn handle_sigint(&mut self) {
        self.set_pointer();
        let mut sa = SigAction {
            handler: term_sigint,
            mask: [0; SIGSET_NWORDS],
            flags: SA_RESTART | SA_SIGINFO,
            restorer: 0 // null pointer
        };
        let mask = full_sigset().expect("Could not get a full sigset");
        sa.mask = mask;
        if !signal_handle(SIGINT, &sa) {
            self.controls.err("Warning: could not set handler for SIGINT\n");
        }
    }
    
    fn unhandle_sigint(&mut self) {
        self.unset_pointer();
        if !signal_ignore(SIGINT) {
            self.controls.err("Warning: could not unset handler for SIGINT\n");
        }
    }

    pub fn interrupt_job(&mut self, id:&usize) -> IoResult<()> {
        match self.jobs.get_mut(id) {
            None => return Err(IoError {
                kind: OtherIoError,
                desc: "Job not found",
                detail: None
            }),
            Some(&mut (_, ref mut job, _)) => {
                return job.signal(SIGINT as isize);
            }
        }
    }

    fn find_jobs_hole(&self) -> usize {
        // find a hole in the job map
        let mut last = 0;
        for key in self.jobs.keys() {
            if key - last != 1 {
                // we've found a hole
                return key - 1;
            } else {
                last = key;
            }
        }
        // job list is full
        return last + 1;
    }

    
    fn find_files_hole(&self) -> usize {
        // find a hole in the file map
        let mut last = 0;
        for key in self.files.keys() {
            if key - last != 1 {
                // we've found a hole
                return key - 1;
            } else {
                last = key;
            }
        }
        // file list is full
        return last + 1;
    }

    pub fn run_job(&mut self, name:&String, args:&Vec<String>) -> Result<(usize, String), String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        // running as job means no input/output handles
        process.stdin(Ignored);
        process.stdout(Ignored);
        process.stderr(Ignored);
        let mut child = match process.spawn() {
            Err(e) => {
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        // set wait timeout to zero so we can check for process exit quickly
        child.set_timeout(Some(0));
        let id = self.find_jobs_hole();
        // panic if we've overwritten a job
        match self.jobs.insert(id.clone(), (name.clone(), child,
                                            match self.next_file {
                                                None => vec![],
                                                Some(id) => vec![id]
                                            })) {
            Some(_) => panic!("Overwrote job"),
            _ => {/* nothing */}
        }
        self.next_file = None;
        return Ok((id, name.clone()));
    }

    pub fn run_command_outfd(&mut self, stdout:Fd, stdin:Option<Fd>, name:&String,
                             args:&Vec<String>) -> Result<ProcessExit, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdout(InheritFd(stdout));
        match stdin {
            None => {
                // user can't respond without stdout
                process.stdin(Ignored);
            },
            Some(fd) => {
                // given stdin
                process.stdin(InheritFd(fd));
            }
        }
        process.stderr(InheritFd(STDERR));
        // set terminal settings for process
        self.restore_terminal();
        // push job into jobs
        let id = self.find_jobs_hole();
        // handle interrupts
        self.handle_sigint();
        let val = (name.clone(), match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        }, vec![]);
        // insert job
        self.jobs.insert(id, val);
        // set forground job
        self.fg_job = Some(id);
        let out = match self.jobs.get_mut(&id).unwrap().1.wait() {
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                self.jobs.remove(&id);
                self.update_terminal();
                return Err(format!("Couldn't wait for child to exit: {}", e));
            },
            Ok(v) => v
        };
        // unset forground job
        self.fg_job = None;
        // remove job
        self.jobs.remove(&id);
        // remove file if there is one
        match self.next_file {
            Some(id) => {
                self.files.remove(&id);
            },
            None => {/* do nothing */}
        }
        self.next_file = None;
        // unhandle sigint
        self.unhandle_sigint();
        // restore settings for Wash
        self.update_terminal();
        return Ok(out);
    }


    pub fn run_job_fd(&mut self, stdin:Fd, name:&String,
                      args:&Vec<String>) -> Result<(usize, String), String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        // given stdin
        process.stdin(InheritFd(stdin));
        // new pipes for others
        let mut child = match process.spawn() {
            Err(e) => {
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        // set wait timeout to zero so we can check for process exit quickly
        child.set_timeout(Some(0));
        let id = self.find_jobs_hole();
        // panic if we've overwritten a job
        match self.jobs.insert(id.clone(), (name.clone(), child,
                                            match self.next_file {
                                                None => vec![],
                                                Some(id) => vec![id]
                                            })) {
            Some(_) => panic!("Overwrote job"),
            _ => {/* nothing */}
        }
        self.next_file = None;
        return Ok((id, name.clone()));
    }
    
    pub fn run_command_fd(&mut self, stdin:Fd, name:&String,
                      args:&Vec<String>) -> Result<ProcessExit, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        // given stdin
        process.stdin(InheritFd(stdin));
        // output to terminal for others
        process.stdout(InheritFd(STDOUT));
        process.stderr(InheritFd(STDERR));
        if stdin == 0 {
            // restore terminal if process is reading from it
            self.restore_terminal();
        }
        // push job into jobs
        let id = self.find_jobs_hole();
        // handle interrupts
        self.handle_sigint();
        let val = (name.clone(), match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        }, vec![]);
        // insert job
        self.jobs.insert(id, val);
        // set forground job
        self.fg_job = Some(id);
        let out = match self.jobs.get_mut(&id).unwrap().1.wait() {
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                self.jobs.remove(&id);
                self.update_terminal();
                return Err(format!("Couldn't wait for child to exit: {}", e));
            },
            Ok(v) => v
        };
        // unset forground job
        self.fg_job = None;
        // remove job
        self.jobs.remove(&id);
        // remove file if there is one
        match self.next_file {
            Some(id) => {
                self.files.remove(&id);
            },
            None => {/* do nothing */}
        }
        self.next_file = None;
        // unhandle sigint
        self.unhandle_sigint();
        if stdin == 0 {
            // restore terminal settings if we changed them earlier
            self.update_terminal();
        }
        return Ok(out);
    }
    
    pub fn run_job_directed(&mut self, name:&String, args:&Vec<String>) -> Result<(usize, String), String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        // directed job means all inputs/outputs are pipes
        // all others are redirected
        let mut child = match process.spawn() {
            Err(e) => {
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        // set wait timeout to zero so we can check for process exit quickly
        child.set_timeout(Some(0));
        let id = self.find_jobs_hole();
        // panic if we've overwritten a job
        match self.jobs.insert(id.clone(), (name.clone(), child,
                                            match self.next_file {
                                                None => vec![],
                                                Some(id) => vec![id]
                                            })) {
            Some(_) => panic!("Overwrote job"),
            _ => {/* nothing */}
        }
        self.next_file = None;
        return Ok((id, name.clone()));
    }

    pub fn get_jobs(&self) -> Vec<(usize, String, &Process)> {
        let mut out = vec![];
        for (id, &(ref name, ref child, _)) in self.jobs.iter() {
            out.push((id, name.clone(), child));
        }
        return out;
    }

    pub fn get_job(&self, id:&usize) -> Option<(String, &Process)> {
        match self.jobs.get(id) {
            None => return None,
            Some(&(ref name, ref child, _)) => {
                return Some((name.clone(), child));
            }
        }
    }

    pub fn get_job_mut(&mut self, id:&usize) -> Option<&mut (String, Process, Vec<usize>)> {
        return self.jobs.get_mut(id);
    }

    pub fn output_file(&mut self, path:&Path) -> IoResult<usize> {
        let fid = self.find_files_hole();
        // files are opened before they are attached to processes
        // this allows the next *job function to attach to the file
        // index, so it can be freed when the job is pruned.
        self.next_file = Some(fid);
        let file = try!(File::open_mode(path, Append, Write));
        self.files.insert(fid, file);
        return Ok(fid);
    }

    pub fn input_file(&mut self, path:&Path) -> IoResult<usize> {
        let fid = self.find_files_hole();
        self.next_file = Some(fid);
        let file = try!(File::open_mode(path, Open, Read));
        self.files.insert(fid, file);
        return Ok(fid);
    }

    pub fn get_file(&self, id:&usize) -> Option<&File> {
        return self.files.get(id);
    }

    pub fn get_files(&self) -> Vec<(usize, &File)> {
        return self.files.iter().collect();
    }

    pub fn clean_jobs(&mut self) -> Vec<(usize, String, IoResult<ProcessExit>)> {
        let mut out = vec![];
        let mut remove = vec![];
        let mut remove_files = vec![];
        for (id, &mut (ref mut name, ref mut child,
                       ref mut files)) in self.jobs.iter_mut() {
            match child.wait() {
                Err(IoError{kind: IoErrorKind::TimedOut, desc: _, detail: _}) => {
                    // this is expected, do nothing
                },
                v => {
                    // all other outputs mean the job is done
                    // if it isn't it'll be cleaned up in drop
                    child.set_timeout(None);
                    remove.push(id);
                    for item in files.iter() {
                        remove_files.push(item.clone());
                    }
                    out.push((id, name.clone(), v));
                }
            }
        }
        for id in remove.iter() {
            self.jobs.remove(id);
        }
        for id in remove_files.iter() {
            self.files.remove(id);
        }
        return out;
    }
    
    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdout(InheritFd(STDOUT));
        process.stdin(InheritFd(STDIN));
        process.stderr(InheritFd(STDERR));
        // set terminal settings for process
        self.restore_terminal();
        // push job into jobs
        let id = self.find_jobs_hole();
        // handle interrupts
        self.handle_sigint();
        let val = (name.clone(), match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        }, vec![]);
        // insert job
        self.jobs.insert(id, val);
        // set forground job
        self.fg_job = Some(id);
        let out = match self.jobs.get_mut(&id).unwrap().1.wait() {
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                self.jobs.remove(&id);
                self.update_terminal();
                return Err(format!("Couldn't wait for child to exit: {}", e));
            },
            Ok(v) => v
        };
        // unset forground job
        self.fg_job = None;
        // remove job
        self.jobs.remove(&id);
        // remove file if there is one
        match self.next_file {
            Some(id) => {
                self.files.remove(&id);
            },
            None => {/* do nothing */}
        }
        self.next_file = None;
        // unhandle sigint
        self.unhandle_sigint();
        // restore settings for Wash
        self.update_terminal();
        return Ok(out);
    }

    pub fn run_command_directed(&mut self, name:&String,
                                args:&Vec<String>) -> Result<ProcessOutput, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        let id = self.find_jobs_hole();
        // handle sigint
        self.handle_sigint();
        let val = (name.clone(), match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        }, vec![]);
        // put job in jobs list
        self.jobs.insert(id, val);
        // set foreground job
        self.fg_job = Some(id);
        let out = match self.jobs.get_mut(&id).unwrap().1.stdout.as_mut().unwrap().read_to_end() {
            Ok(v) => v,
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                self.jobs.remove(&id);
                return Err(format!("Couldn't get stdout: {}", e));
            }
        };
        let err = match self.jobs.get_mut(&id).unwrap().1.stderr.as_mut().unwrap().read_to_end() {
            Ok(v) => v,
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                self.jobs.remove(&id);
                return Err(format!("Couldn't get stderr: {}", e));
            }
        };
        let output = match self.jobs.get_mut(&id).unwrap().1.wait() {
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                self.jobs.remove(&id);
                self.update_terminal();
                return Err(format!("Couldn't wait for child to exit: {}", e));
            },
            Ok(v) => ProcessOutput {
                status: v,
                output: out,
                error: err
            }
        };
        // unset foreground job
        self.fg_job = None;
        // remove job
        self.jobs.remove(&id);
        // remove file if there is one
        match self.next_file {
            Some(id) => {
                self.files.remove(&id);
            },
            None => {/* do nothing */}
        }
        self.next_file = None;
        // unhandle sigint
        self.unhandle_sigint();
        return Ok(output);
    }
}

