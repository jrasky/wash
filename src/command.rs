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

pub struct Job {
    pub command: String,
    pub process: Process,
    pub files: Vec<usize>
}

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios,
    pub jobs: VecMap<Job>,
    fg_job: Option<usize>,
    pub files: VecMap<File>,
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

    pub fn get_file(&self, id:&usize) -> Result<&File, String> {
        match self.files.get(id) {
            None => Err("File not found".to_string()),
            Some(file) => Ok(file)
        }
    }

    pub fn get_job(&self, id:&usize) -> Result<&Job, String> {
        match self.jobs.get(id) {
            None => Err("Job not found".to_string()),
            Some(job) => Ok(job)
        }
    }

    pub fn get_job_mut(&mut self, id:&usize) -> Result<&mut Job, String> {
        match self.jobs.get_mut(id) {
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
        let child = match process.spawn() {
            Err(e) => {
                self.next_file = None;
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        let id = self.find_jobs_hole();
        match self.jobs.insert(id.clone(), Job {
            command: name.clone(),
            process: child,
            files: match self.next_file {
                None => vec![],
                Some(id) => vec![id]
            }}) {
            Some(_) => panic!("Overwrote job"),
            _ => {/* nothing */}
        }
        self.next_file = None;
        return Ok(id);
    }

    pub fn wait_job(&mut self, id:&usize) -> Result<ProcessExit, String> {
        let mut child = try!(self.get_job_mut(id));
        // clear any timeouts
        child.process.set_timeout(None);
        match child.process.wait() {
            Err(e) => Err(format!("Couldn't wait for child to exit: {}", e)),
            Ok(status) => Ok(status)
        }
    }

    pub fn job_output(&mut self, id:&usize) -> Result<ProcessOutput, String> {
        let mut child = match self.jobs.remove(id) {
            None => return Err("Job not found".to_string()),
            Some(job) => job
        };
        child.process.set_timeout(None);
        let out = child.process.wait_with_output();
        for id in child.files.iter() {
            self.files.remove(id);
        }
        match out {
            Err(e) => return Err(format!("Could not get job output: {}", e)),
            Ok(o) => Ok(o)
        }
    }

    pub fn start_command(&mut self, stdin:StdioContainer, stdout:StdioContainer, stderr:StdioContainer,
                         name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        // set terminal settings for process
        self.restore_terminal();
        // handle interrupts
        self.handle_sigint();
        // start job
        let id = match self.start_job(stdin, stdout, stderr, name, args) {
            Err(e) => {
                self.update_terminal();
                self.unhandle_sigint();
                return Err(e);
            },
            Ok(id) => id
        };
        self.fg_job = Some(id);
        let out = match self.wait_job(&id) {
            Err(e) => {
                // unset foreground job
                self.fg_job = None;
                // remove job
                let out = self.jobs.remove(&id);
                // close files
                if out.is_some() {
                    for id in out.unwrap().files.iter() {
                        self.files.remove(id);
                    }
                }
                self.update_terminal();
                self.unhandle_sigint();
                return Err(e);
            },
            Ok(v) => v
        };
        // unset forground job
        self.fg_job = None;
        // remove job
        let old_job = self.jobs.remove(&id);
        // close files
        if old_job.is_some() {
            for id in old_job.unwrap().files.iter() {
                self.files.remove(id);
            }
        }
        // unhandle sigint
        self.unhandle_sigint();
        // restore settings for Wash
        self.update_terminal();
        return Ok(out);
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
        self.start_command(stdin_o, stdout_o, stderr_o, name, args)
    }
    
    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        // run the command on stdin/out/err
        self.run_command_fd(Some(STDIN), Some(STDOUT), Some(STDERR), name, args)
    }

    pub fn clean_jobs(&mut self) -> Vec<(usize, String, IoResult<ProcessExit>)> {
        let mut out = vec![];
        let mut remove = vec![];
        let mut remove_files = vec![];
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
                    for item in child.files.iter() {
                        remove_files.push(item.clone());
                    }
                    out.push((id, child.command.clone(), v));
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
    
}

