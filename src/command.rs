use libc::*;

use std::io::process::{Command, ProcessOutput, ProcessExit,
                       Process, StdioContainer};
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
        None => {
            term.controls.err("No running job found");
        },
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

pub struct Job {
    pub command: String,
    pub process: Process,
    pub files: Vec<File>
}

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios,
    pub jobs: VecMap<Job>,
    fg_job: Option<usize>,
    files: VecMap<File>
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
            files: VecMap::new()
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
            Some(ref mut job) => {
                return job.process.signal(SIGINT as isize);
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
        
    pub fn output_file(&mut self, path:&Path) -> IoResult<Fd> {
        // files are opened before they are attached to processes
        // this allows the next *job function to attach to the file
        // index, so it can be freed when the job is pruned.
        let file = try!(File::open_mode(path, Append, Write));
        let fid = file.as_raw_fd();
        self.files.insert(fid as usize, file);
        return Ok(fid);
    }

    pub fn input_file(&mut self, path:&Path) -> IoResult<Fd> {
        let file = try!(File::open_mode(path, Open, Read));
        let fid = file.as_raw_fd();
        self.files.insert(fid as usize, file);
        return Ok(fid);
    }

    pub fn get_job(&self, id:&usize) -> Result<&Job, String> {
        match self.jobs.get(id) {
            None => Err("Job not found".to_string()),
            Some(job) => Ok(job)
        }
    }

    pub fn start_job(&mut self, stdin:StdioContainer, stdout:StdioContainer, stderr:StdioContainer,
                     name:&String, args:&Vec<String>) -> Result<usize, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdin(stdin);
        process.stdout(stdout);
        process.stderr(stderr);
        // reset signal to default before spawning
        signal_default(SIGINT);
        let child = match process.spawn() {
            Err(e) => {
                signal_ignore(SIGINT);
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        // re-ignore signal
        signal_ignore(SIGINT);
        let id = self.find_jobs_hole();
        let mut job =  Job {
            command: name.clone(),
            process: child,
            files: vec![]
        };
        // claim file descriptors if they exist in the file table
        match stdin {
            InheritFd(fd) if self.files.contains_key(&(fd as usize)) =>
                job.files.push(self.files.remove(&(fd as usize)).unwrap()),
            _ => {}
        }
        match stdout {
            InheritFd(fd) if self.files.contains_key(&(fd as usize)) =>
                job.files.push(self.files.remove(&(fd as usize)).unwrap()),
            _ => {}
        }
        match stderr {
            InheritFd(fd) if self.files.contains_key(&(fd as usize)) =>
                job.files.push(self.files.remove(&(fd as usize)).unwrap()),
            _ => {}
        }
        match self.jobs.insert(id.clone(), job) {
            Some(_) => panic!("Overwrote job"),
            _ => {/* nothing */}
        }
        return Ok(id);
    }

    pub fn wait_job(&mut self, id:&usize) -> Result<ProcessExit, String> {
        // set the foreground job (before borrowing self)
        self.fg_job = Some(id.clone());
        // handle sigint (before borrowing self)
        self.handle_sigint();
        let mut out;
        match self.jobs.get_mut(id) {
            None => out = Err("Job not found".to_string()),
            Some(child) => {
                // clear any timeouts
                child.process.set_timeout(None);
                out = match child.process.wait() {
                    Err(e) => Err(format!("Couldn't wait for child to exit: {}", e)),
                    Ok(status) => Ok(status)
                };
            }
        };
        // unhandle sigint
        self.unhandle_sigint();
        // unset foreground job
        self.fg_job = None;
        return out;
    }

    pub fn job_output(&mut self, id:&usize) -> Result<ProcessOutput, String> {
        // set the foreground job (before borrowing self)
        self.fg_job = Some(id.clone());
        let mut child = match self.jobs.remove(id) {
            None => {
                self.fg_job = None;
                return Err("Job not found".to_string());
            },
            Some(job) => job
        };
        child.process.set_timeout(None);
        // handle sigint
        self.handle_sigint();
        let out = match child.process.wait_with_output() {
            Err(e) => return Err(format!("Could not get job output: {}", e)),
            Ok(o) => Ok(o)
        };
        // unset foreground job
        self.fg_job = None;
        // unhandle sigint
        self.unhandle_sigint();
        return out;
    }

    pub fn start_command(&mut self, stdin:StdioContainer, stdout:StdioContainer, stderr:StdioContainer,
                         name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        // set terminal settings for process
        // do this before we spawn the process
        self.restore_terminal();
        // start job
        let id = match self.start_job(stdin, stdout, stderr, name, args) {
            Err(e) => {
                // reset terminal to original state if spawning failed
                self.update_terminal();
                return Err(e);
            },
            Ok(id) => id
        };
        // wait for job to finish
        let out = self.wait_job(&id);
        // remove job
        self.jobs.remove(&id);
        // restore settings for Wash
        self.update_terminal();
        return out;
    }

    pub fn run_job_fd(&mut self, stdin:Option<Fd>, stdout:Option<Fd>, stderr:Option<Fd>,
                      name:&String, args:&Vec<String>) -> Result<usize, String> {
        let stdin_o = match stdin {
            Some(fd) => InheritFd(fd),
            None => CreatePipe(true, false)
        };
        let stdout_o = match stdout {
            Some(fd) => InheritFd(fd),
            None => CreatePipe(false, true)
        };
        let stderr_o = match stderr {
            Some(fd) => InheritFd(fd),
            None => CreatePipe(false, true)
        };
        self.start_job(stdin_o, stdout_o, stderr_o, name, args)
    }
    
    pub fn run_job(&mut self, name:&String, args:&Vec<String>) -> Result<usize, String> {
        // run the job directed
        self.run_job_fd(None, None, None, name, args)
    }

    pub fn run_command_fd(&mut self, stdin:Option<Fd>, stdout:Option<Fd>, stderr:Option<Fd>,
                          name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        // commands can only run on existing pipes
        // to run a command on a new one, use a job
        let stdin_o = match stdin {
            Some(fd) => InheritFd(fd),
            None => InheritFd(STDIN)
        };
        let stdout_o = match stdout {
            Some(fd) => InheritFd(fd),
            None => InheritFd(STDOUT)
        };
        let stderr_o = match stderr {
            Some(fd) => InheritFd(fd),
            None => InheritFd(STDERR)
        };
        self.start_command(stdin_o, stdout_o, stderr_o, name, args)
    }
    
    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        // run the command on stdin/out/err
        self.run_command_fd(None, None, None, name, args)
    }

    pub fn clean_jobs(&mut self) -> Vec<(usize, String, IoResult<ProcessExit>)> {
        let mut out = vec![];
        let mut remove = vec![];
        for (id, child) in self.jobs.iter_mut() {
            child.process.set_timeout(Some(0)); // don't block on wait
            match child.process.wait() {
                Err(IoError{kind: IoErrorKind::TimedOut, desc: _, detail: _}) => {
                    // this is expected, do nothing
                    child.process.set_timeout(None);
                },
                v => {
                    // all other outputs mean the job is done
                    // if it isn't it'll be cleaned up in drop
                    child.process.set_timeout(None);
                    remove.push(id);
                    out.push((id, child.command.clone(), v));
                }
            }
        }
        for id in remove.iter() {
            self.jobs.remove(id);
        }
        return out;
    }
    
}

