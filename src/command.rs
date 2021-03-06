use libc::*;

// use old io, "The equivalent of InheritFd will be added at a later point"
use std::old_io::process::{Command, ProcessOutput, ProcessExit,
                           Process, StdioContainer};
use std::old_io::process::StdioContainer::*;
use std::collections::VecMap;
use std::os::unix::prelude::*;

use std::path;
use std::io;
use std::fs;

use controls::*;
use constants::*;
use termios::*;
use signal::*;

// start off as null pointer
static mut uglobal_term:*mut TermState = 0 as *mut TermState;

unsafe extern fn term_signal(signo:c_int, u_info:*const SigInfo,
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
    match signo {
        SIGCHLD => {
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
                    job.exit = Some(exit);
                    return;
                }
            }
            if !term.spawning {
                // child was not found
                term.controls.errf(format_args!("\nSent SIGCHLD for process not found in job table: {}\n", fields.pid));
            } // failed process spawns cause SIGCHLD before we can put the process into the job table
        },
        SIGTSTP => {
            // ignore background SIGTSTP
        },
        _ => term.controls.errf(format_args!("\nTerm caught unexpected signal: {}\n", signo))
    }
}

pub struct Job {
    pub command: String,
    pub process: Process,
    pub files: Vec<fs::File>,
    pub exit: Option<ProcessExit>
}

impl Job {
    pub fn wait(&mut self, timeout:Option<usize>) -> io::Result<ProcessExit> {
        if self.check_exit() {
            // we're already dead
            return Ok(self.exit.clone().unwrap());
        }
        let mut set = try!(empty_sigset());
        try!(sigset_add(&mut set, SIGCHLD));
        // set a process mask
        let old_set = try!(signal_proc_mask(SIG_BLOCK, &set));
        let out = self.wait_signal(timeout, &set);
        // unset the mask
        try!(signal_proc_mask(SIG_SETMASK, &old_set));
        return out;
    }

    fn wait_signal(&mut self, timeout:Option<usize>, set:&SigSet) -> io::Result<ProcessExit> {
        let mut info; let mut fields;
        loop {
            info = try!(signal_wait_set(set, timeout));
            fields = match info.determine_sigfields() {
                SigFields::SigChld(f) => f,
                _ => return Err(io::Error::new(io::ErrorKind::Other, "Didn't catch SIGCHLD",
                                               Some(format!("Caught signal {} instead", info.signo))))
            };
            if fields.pid == self.process.id() {
                // we're dead (or stopped, but that comes later)
                let exit = match info.code {
                    CLD_EXITED => ProcessExit::ExitStatus(fields.status as isize),
                    _ => ProcessExit::ExitSignal(fields.status as isize)
                };
                self.exit = Some(exit.clone());
                return Ok(exit);
            }
        }
    }

    pub fn check_exit(&self) -> bool {
        match self.exit {
            Some(ProcessExit::ExitSignal(v))
                if v == SIGTSTP as isize ||
                v == SIGSTOP as isize ||
                v == SIGCONT as isize => return false,
            Some(_) => return true,
            None => return false
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        match self.wait(Some(0)) {
            Ok(_) => return,
            Err(_) => {/* continue */}
        }
        
        match self.process.signal_exit() {
            Err(e) => println!("Could not signal {} to exit: {}", self.process.id(), e),
            _ => {/* ok */}
        }
        match self.wait(Some(1000)) {
            Ok(_) => return,
            Err(_) => {/* continue */}
        }
        
        match self.process.signal_kill() {
            Err(e) => println!("Could not kill {}: {}", self.process.id(), e),
            _ => {/* ok */}
        }
    }
}

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios,
    pub jobs: VecMap<Job>,
    files: VecMap<fs::File>,
    jobstack: Vec<usize>,
    spawning: bool
}

impl Drop for TermState {
    fn drop (&mut self) {
        // drop our signal handlers
        self.unhandle_signals();
        self.unset_pointer();
        // then drop all of our jobs
        let ids:Vec<usize> = self.jobs.keys().collect();
        for id in ids.iter() {
            self.controls.errf(format_args!("Dropping job {}...\n", &id));
            self.jobs.remove(id);
        }
    }
}

impl TermState {
    pub fn new() -> TermState {
        let mut controls = Controls::new();
        let mut tios = match Termios::get() {
            Ok(t) => t,
            Err(e) => {
                controls.errf(format_args!("Warning: Could not get terminal mode: {}\n", e));
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
            files: VecMap::new(),
            jobstack: vec![],
            spawning: false
        }
    }

    pub fn update_terminal(&mut self) {
        match Termios::set(&self.tios) {
            Err(e) =>
                self.controls.errf(format_args!("Warning: Could not set terminal mode: {}\n", e)),
            Ok(_) => {}
        }
    }

    pub fn restore_terminal(&mut self) {
        match Termios::set(&self.old_tios) {
            Err(e) =>
                self.controls.errf(format_args!("Warning: Could not restore terminal mode: {}\n", e)),
            Ok(_) => {}
        }
    }

    pub fn set_pointer(&mut self) {
        unsafe {
            uglobal_term = self as *mut TermState;
        }
    }

    pub fn unset_pointer(&mut self) {
        unsafe {
            uglobal_term = 0 as *mut TermState;
        }
    }

    pub fn handle_signals(&mut self) {
        let sa = SigAction::handler(term_signal);
        match signal_handle(SIGCHLD, &sa) {
            Err(e) => self.controls.errf(format_args!("Could not set handler for SIGCHLD: {}\n", e)),
            _ => {}
        }
        match signal_handle(SIGTSTP, &sa) {
            Err(e) => self.controls.errf(format_args!("Could not set handler for SIGTSTP: {}\n", e)),
            _ => {}
        }
    }
    
    pub fn unhandle_signals(&mut self) {
        match signal_default(SIGCHLD) {
            Err(e) => self.controls.errf(format_args!("Could not unset handler for SIGCHLD: {}\n", e)),
            _ => {}
        }
        match signal_default(SIGTSTP) {
            Err(e) => self.controls.errf(format_args!("Could not unset handler for SIGTSTP: {}\n", e)),
            _ => {}
        }
    }

    pub fn remove_if_done(&mut self, id:&usize) -> Result<bool, String> {
        if !self.jobs.contains_key(id) {
            return Err(format!("Job not found"));
        }
        if self.jobs.get(id).unwrap().check_exit() {
            self.jobs.remove(id);
            return Ok(true);
        } else {
            return Ok(false);
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
        
    pub fn output_file(&mut self, path:&path::Path) -> io::Result<Fd> {
        // files are opened before they are attached to processes
        // this allows the next *job function to attach to the file
        // index, so it can be freed when the job is pruned.
        let mut options = fs::OpenOptions::new();
        options.append(true).write(true);
        let file = try!(options.open(path));
        let fid = file.as_raw_fd();
        self.files.insert(fid as usize, file);
        return Ok(fid);
    }

    pub fn input_file(&mut self, path:&path::Path) -> io::Result<Fd> {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        let file = try!(options.open(path));
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

    pub fn front_job(&mut self) -> Option<usize> {
        self.jobstack.pop()
    }

    pub fn restart_job(&mut self, id:&usize) -> Result<(), String> {
        let mut job = match self.jobs.get_mut(id) {
            None => return Err(format!("Job not found")),
            Some(job) => job
        };
        tryf!(job.process.signal(SIGCONT as isize),
              "Couldn't restart process: {err}");
        return Ok(());
    }

    pub fn start_job(&mut self, stdin:StdioContainer, stdout:StdioContainer, stderr:StdioContainer,
                     name:&String, args:&Vec<String>,
                     envs:&Vec<(String, Option<String>)>) -> Result<usize, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        process.stdin(stdin);
        process.stdout(stdout);
        process.stderr(stderr);
        for &(ref env, ref val) in envs.iter() {
            match val {
                &None => process.env_remove(env),
                &Some(ref val) => process.env(env, val)
            };
        }
        // for some reason a sigprocmask doesn't work here
        self.spawning = true;
        let out = process.spawn();
        self.spawning = false;
        let child = tryf!(out, "Couldn't spawn {name}: {err}", name=name);
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
        return Ok(id);
    }

    fn wait_job_signal(&mut self, id:&usize, set:&SigSet) -> Result<ProcessExit, String> {
        let mut info; let mut fields;
        loop {
            info = match signal_wait_set(set, None) {
                Ok(i) => i,
                Err(ref err) if err.detail() == Some("interrupted system call".to_string()) => {
                    // our waiting was interrupted, try again
                    continue;
                },
                Err(e) => return Err(format!("Couldn't wait for child to exit: {}", e))
            };
            match info.signo {
                SIGINT => {
                    // delete "^C"
                    self.controls.outc(BS);
                    self.controls.outc(BS);
                    self.controls.outc(SPC);
                    self.controls.outc(SPC);
                    self.controls.outc(BS);
                    self.controls.outc(BS);
                    self.controls.outs("\nInterrupt\n");
                    continue;
                },
                SIGTSTP => {
                    // delete "^Z"
                    self.controls.outc(BS);
                    self.controls.outc(BS);
                    self.controls.outc(SPC);
                    self.controls.outc(SPC);
                    self.controls.outc(BS);
                    self.controls.outc(BS);
                    self.controls.outs("\nStop\n");
                    continue;
                },
                SIGCHLD => {
                    fields = match info.determine_sigfields() {
                        SigFields::SigChld(f) => f,
                        _ => return Err(format!("Caught signal {} instead of SIGCHLD", info.signo))
                    };
                    if fields.pid == self.jobs.get_mut(id).unwrap().process.id() {
                        // process of interest died
                        let exit = match info.code {
                            CLD_EXITED => ProcessExit::ExitStatus(fields.status as isize),
                            _ => ProcessExit::ExitSignal(fields.status as isize)
                        };
                        self.jobs.get_mut(id).unwrap().exit = Some(exit.clone());
                        if info.code != CLD_EXITED && fields.status == SIGCONT {
                            continue;
                        } else {
                            return Ok(exit);
                        }
                    } else {
                        // some other job finished
                        // find the child by pid
                        for (_, ref mut job) in self.jobs.iter_mut() {
                            if job.process.id() == fields.pid {
                                let exit = match info.code {
                                    CLD_EXITED => ProcessExit::ExitStatus(fields.status as isize),
                                    _ => ProcessExit::ExitSignal(fields.status as isize)
                                };
                                job.exit = Some(exit);
                                break;
                            }
                        }
                    }
                }, _ => return Err(format!("Caught unexpected signal: {}", info.signo))
            }
        }
    }

    pub fn wait_job(&mut self, id:&usize) -> Result<ProcessExit, String> {
        if !self.jobs.contains_key(id) {
            return Err("Job not found".to_string());
        }
        if self.jobs.get(id).unwrap().check_exit() {
            // child has already exited
            return Ok(self.jobs.get(id).unwrap().exit.clone().unwrap());
        }
        let mut set = tryf!(empty_sigset(),
                            "Couldn't get empty sigset: {err}");
        tryf!(sigset_add(&mut set, SIGCHLD),
              "Couldn't add SIGCHLD to sigset: {err}");
        tryf!(sigset_add(&mut set, SIGINT),
              "Couldn't add SIGINT to sigset: {err}");
        tryf!(sigset_add(&mut set, SIGTSTP),
              "Couldn't add SIGTSTP to sigset: {err}");
        // set a process mask
        let old_set = tryf!(signal_proc_mask(SIG_BLOCK, &set),
                            "Couldn't set process mask: {err}");
        let out = self.wait_job_signal(id, &set);
        // unset the mask
        tryf!(signal_proc_mask(SIG_SETMASK, &old_set),
              "Couldn't unset process mask: {err}");
        return out;
    }

    pub fn job_output(&mut self, id:&usize) -> Result<ProcessOutput, String> {
        // set the foreground job (before borrowing self)
        let status = try!(self.wait_job(id));
        let mut child = self.jobs.remove(id).unwrap();
        let stdout = match child.process.stdout.as_mut() {
            None => return Err("Child had no stdout".to_string()),
            Some(st) => tryf!(st.read_to_end(),
                              "Could not read stdout: {err}")
        };
        let stderr = match child.process.stderr.as_mut() {
            None => return Err("Child had no stderr".to_string()),
            Some(st) => tryf!(st.read_to_end(),
                              "Could not read stderr: {err}")
        };
        return Ok(ProcessOutput {
            status: status,
            output: stdout,
            error: stderr
        });
    }

    pub fn start_command(&mut self, stdin:StdioContainer, stdout:StdioContainer, stderr:StdioContainer,
                         name:&String, args:&Vec<String>,
                         envs:&Vec<(String, Option<String>)>) -> Result<ProcessExit, String> {
        // set terminal settings for process
        // do this before we spawn the process
        self.restore_terminal();
        // start job
        let id = match self.start_job(stdin, stdout, stderr, name, args, envs) {
            Err(e) => {
                // reset terminal to original state if spawning failed
                self.update_terminal();
                return Err(e);
            },
            Ok(id) => id
        };
        // wait for job to finish
        let out = self.wait_job(&id);
        if self.jobs.get(&id).unwrap().check_exit() {
            // job is done
            self.jobs.remove(&id);
        } else {
            // job is stopped
            self.jobstack.push(id);
        }
        // restore settings for Wash
        self.update_terminal();
        return out;
    }

    pub fn run_job_fd(&mut self, stdin:Option<Fd>, stdout:Option<Fd>, stderr:Option<Fd>,
                      name:&String, args:&Vec<String>,
                      envs:&Vec<(String, Option<String>)>) -> Result<usize, String> {
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
        self.start_job(stdin_o, stdout_o, stderr_o, name, args, envs)
    }
    
    pub fn run_job(&mut self, name:&String, args:&Vec<String>) -> Result<usize, String> {
        // run the job directed
        self.run_job_fd(None, None, None, name, args, &vec![])
    }

    pub fn run_command_fd(&mut self, stdin:Option<Fd>, stdout:Option<Fd>, stderr:Option<Fd>,
                          name:&String, args:&Vec<String>,
                          envs:&Vec<(String, Option<String>)>) -> Result<ProcessExit, String> {
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
        let out = self.start_command(stdin_o, stdout_o, stderr_o, name, args, envs);
        // clean jobs so we don't get a bunch of "job finished" messages
        self.clean_jobs();
        return out;
    }
    
    pub fn run_command(&mut self, name:&String, args:&Vec<String>) -> Result<ProcessExit, String> {
        // run the command on stdin/out/err
        self.run_command_fd(None, None, None, name, args, &vec![])
    }

    pub fn clean_jobs(&mut self) -> Vec<(usize, Job)> {
        let mut remove = vec![];
        for (id, child) in self.jobs.iter_mut() {
            if child.check_exit() {
                remove.push(id);
            }
        }
        let mut out = vec![];
        for id in remove.iter() {
            out.push((id.clone(), self.jobs.remove(id).unwrap()));
        }
        return out;
    }
    
}

