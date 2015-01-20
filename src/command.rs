use std::io::process::{Command, ProcessOutput, ProcessExit, Process};
use std::io::process::StdioContainer::*;
use std::io::{IoError, IoErrorKind, IoResult};
use std::collections::VecMap;

use controls::*;
use constants::*;
use termios::*;

pub struct TermState {
    pub controls: Controls,
    tios: Termios,
    old_tios: Termios,
    jobs: VecMap<(String, Process)>,
    fg_job: Option<usize>
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
            fg_job: None
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

    fn find_jobs_hole(&self) -> usize {
        // find a hole in the job map
        if self.jobs.len() < 20 {
            // with less than twenty jobs just be lazy
            return self.jobs.len() + 1;
        }
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

    pub fn run_job(&mut self, name:&String, args:&Vec<String>) -> Result<(usize, String), String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        // running as job means no stdin handle
        process.stdin(Ignored);
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
        match self.jobs.insert(id.clone(), (name.clone(), child)) {
            Some(_) => panic!("Overwrote job"),
            _ => {/* nothing */}
        }
        return Ok((id, name.clone()));
    }

    pub fn clean_jobs(&mut self) -> Vec<(usize, String, IoResult<ProcessExit>)> {
        let mut out = vec![];
        let mut remove = vec![];
        for (id, &mut (ref mut name, ref mut child)) in self.jobs.iter_mut() {
             match child.wait() {
                 Err(IoError{kind: IoErrorKind::TimedOut, desc: _, detail: _}) => {
                     // this is expected, do nothing
                 },
                 v => {
                     // all other outputs mean the job is done
                     // if it isn't it'll be cleaned up in drop
                     child.set_timeout(None);
                     remove.push(id);
                     out.push((id, name.clone(), v));
                 }
             }
        }
        for id in remove.iter() {
            self.jobs.remove(id);
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
        let val = (name.clone(), match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        });
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
        // restore settings for Wash
        self.update_terminal();
        return Ok(out);
    }

    pub fn run_command_directed(&mut self, name:&String,
                                args:&Vec<String>) -> Result<ProcessOutput, String> {
        let mut process = Command::new(name);
        process.args(args.as_slice());
        let id = self.find_jobs_hole();
        let val = (name.clone(), match process.spawn() {
            Err(e) => {
                self.update_terminal();
                return Err(format!("Couldn't spawn {}: {}", name, e));
            },
            Ok(v) => v
        });
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
        return Ok(output);
    }
}

