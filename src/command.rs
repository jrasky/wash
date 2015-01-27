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

unsafe extern fn term_sigchld(_:c_int, u_info:*const SigInfo,
                              _:*const c_void) {
    let term:&mut TermState = match uglobal_term.as_mut() {
        Some(v) => v,
        None => {
            // this handler shouldn't be called when Term isn't active
            panic!("Term signal interrupt called when Term not active");
        }
    };
    let info:&SigInfo = match u_info.as_ref() {
        Some(v) => v,
        None => panic!("Given a null pointer for signal info")
    };
    let fields = match info.determine_sigfields() {
        SigFields::SigChld(f) => f,
        _ => panic!("Signal wasn't a SIGCHLD")
    };
    // find the child by pid
    for (_, ref mut job) in term.jobs.iter_mut() {
        if job.process.id() == fields.pid {
            let exit = match info.code {
                CLD_EXITED => ProcessExit::ExitStatus(fields.status as isize),
                _ => ProcessExit::ExitSignal(fields.status as isize)
            };
            job.exit = Some(Ok(exit));
            return;
        }
    }
    // child was not found
    term.controls.errf(format_args!("\nSent SIGCHLD for process not found in job table: {}\n", fields.pid));
}

pub struct Job {
    pub command: String,
    pub process: Process,
    pub files: Vec<File>,
    pub exit: Option<IoResult<ProcessExit>>
}

impl Job {
    pub fn wait(&mut self, timeout:Option<usize>) -> Result<ProcessExit, String> {
        return Err("Don't call this yet".to_string());
    }

    pub fn check_exit(&mut self) -> bool {
        return self.exit.is_some();
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.process.set_timeout(Some(0));
        match self.process.wait() {
            Ok(_) => return,
            Err(_) => {/* continue */}
        }
        
        self.process.set_timeout(Some(1000));
        match self.process.signal_exit() {
            Err(e) => println!("Could not signal {} to exit: {}", self.process.id(), e),
            _ => {/* ok */}
        }
        match self.process.wait() {
            Ok(_) => return,
            Err(_) => {/* continue */}
        }
        
        match self.process.signal_kill() {
            Err(e) => println!("Could not kill {}: {}", self.process.id(), e),
            _ => {/* ok */}
        }
        self.process.set_timeout(None);
    }
}

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios,
    pub jobs: VecMap<Job>,
    fg_job: Option<usize>,
    files: VecMap<File>,
    pub catch_sigint: bool
}

impl Drop for TermState {
    fn drop (&mut self) {
        self.unhandle_sigchld();
        self.unset_pointer();
    }
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
        
        TermState {
            controls: controls,
            tios: tios,
            old_tios: old_tios,
            jobs: VecMap::new(),
            fg_job: None,
            files: VecMap::new(),
            catch_sigint: true
        }
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

    pub fn set_pointer(&mut self) {
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

    pub fn handle_sigchld(&mut self) {
        let sa = SigAction::handler(term_sigchld);
        if !signal_handle(SIGCHLD, &sa) {
            self.controls.err("Warning: could not set handlher for SIGCHLD\n");
        }
    }
    
    pub fn unhandle_sigchld(&mut self) {
        if !signal_default(SIGCHLD) {
            self.controls.err("Warning: could not unset handler for SIGCHLD\n");
        }
    }


    fn handle_sigint(&mut self) {
        if !self.catch_sigint {return}
        let sa = SigAction::handler(term_sigint);
        if !signal_handle(SIGINT, &sa) {
            self.controls.err("Warning: could not set handler for SIGINT\n");
        }
    }
    
    fn unhandle_sigint(&mut self) {
        if !self.catch_sigint {return}
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
        // set a signal handler while spawning the process
        self.handle_sigint();
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdin(stdin);
        process.stdout(stdout);
        process.stderr(stderr);
        let child = match process.spawn() {
            Err(e) => {
                signal_ignore(SIGINT);
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        };
        let id = self.find_jobs_hole();
        let mut job =  Job {
            command: name.clone(),
            process: child,
            files: vec![],
            exit: None
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
        // unhandle sigint
        self.unhandle_sigint();
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
        let exit = child.process.wait();
        // unset foreground job
        self.fg_job = None;
        // unhandle sigint
        self.unhandle_sigint();
        let status = match exit {
            Err(e) => return Err(format!("Could not get job output: {}", e)),
            Ok(s) => s
        };
        let stdout = match child.process.stdout.as_mut() {
            None => return Err("Child had no stdout".to_string()),
            Some(st) => match st.read_to_end() {
                Err(e) => return Err(format!("Could not read stdout: {}", e)),
                Ok(v) => v
            }
        };
        let stderr = match child.process.stderr.as_mut() {
            None => return Err("Child had no stderr".to_string()),
            Some(st) => match st.read_to_end() {
                Err(e) => return Err(format!("Could not read stderr: {}", e)),
                Ok(v) => v
            }
        };
        return Ok(ProcessOutput {
            status: status,
            output: stdout,
            error: stderr
        });
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
        // unhandle sigchld because we're calling wait
        // which will register another handler
        // which doesn't use SIGINFO
        // which will panic when it tries to run the old handler
        self.unhandle_sigchld();
        for (id, child) in self.jobs.iter_mut() {
            if child.check_exit() {
                out.push((id, child.command.clone(), child.exit.clone().unwrap()));
                remove.push(id);
            }
        }
        self.handle_sigchld();
        for id in remove.iter() {
            self.jobs.remove(id);
        }
        return out;
    }
    
}

