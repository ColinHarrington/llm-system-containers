//! Command-execution boundary.
//!
//! Drivers shell out to `limactl` / `incus` through [`CommandRunner`] so their logic stays
//! unit-testable: the real impl ([`SystemRunner`]) spawns processes; tests use [`FakeRunner`].

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl RunOutput {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

/// Runs an external command and captures its output.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput>;
}

/// Runs real OS processes.
#[derive(Debug, Default)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput> {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|e| Error::Vm(format!("failed to run {program}: {e}")))?;
        Ok(RunOutput {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
type Handler = Box<dyn Fn(&str, &[&str]) -> RunOutput>;

/// A fake runner for tests: records calls and returns programmed output.
#[cfg(test)]
pub struct FakeRunner {
    pub calls: std::cell::RefCell<Vec<Vec<String>>>,
    handler: Handler,
}

#[cfg(test)]
impl FakeRunner {
    pub fn new(handler: impl Fn(&str, &[&str]) -> RunOutput + 'static) -> Self {
        Self {
            calls: std::cell::RefCell::new(Vec::new()),
            handler: Box::new(handler),
        }
    }

    /// Did any recorded call include an argument containing `needle`?
    pub fn called_with(&self, needle: &str) -> bool {
        self.calls
            .borrow()
            .iter()
            .any(|c| c.iter().any(|a| a.contains(needle)))
    }
}

#[cfg(test)]
impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<RunOutput> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|s| s.to_string()));
        self.calls.borrow_mut().push(call);
        Ok((self.handler)(program, args))
    }
}

/// Build a [`RunOutput`] with the given code and stdout (test helper).
#[cfg(test)]
pub fn out(code: i32, stdout: &str) -> RunOutput {
    RunOutput {
        code,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}
