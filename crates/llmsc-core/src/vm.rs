//! L1 VM lifecycle abstraction — the `VmDriver` boundary.
//!
//! The real driver (Lima, M1) shells out to `limactl`; tests use [`FakeVmDriver`].

use crate::config::VmConfig;
use crate::error::{Error, Result};
use crate::process::CommandRunner;
use crate::progress::Reporter;
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmStatus {
    NotCreated,
    Stopped,
    Starting,
    Running,
}

/// Drives the L1 VM. `&self` methods (real impls shell out; fakes use interior mutability).
pub trait VmDriver {
    fn status(&self) -> Result<VmStatus>;
    /// Bring the VM up, reporting each step (creation can take minutes).
    fn up(&self, reporter: &dyn Reporter) -> Result<()>;
    fn down(&self) -> Result<()>;
}

/// In-memory fake for unit tests.
#[derive(Debug)]
pub struct FakeVmDriver {
    status: Cell<VmStatus>,
}

impl FakeVmDriver {
    pub fn new() -> Self {
        Self {
            status: Cell::new(VmStatus::NotCreated),
        }
    }
}

impl Default for FakeVmDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl VmDriver for FakeVmDriver {
    fn status(&self) -> Result<VmStatus> {
        Ok(self.status.get())
    }
    fn up(&self, _reporter: &dyn Reporter) -> Result<()> {
        self.status.set(VmStatus::Running);
        Ok(())
    }
    fn down(&self) -> Result<()> {
        self.status.set(VmStatus::Stopped);
        Ok(())
    }
}

/// Real driver: manages the L1 VM via `limactl` (M1).
pub struct LimaVmDriver<R: CommandRunner> {
    cfg: VmConfig,
    runner: R,
}

impl<R: CommandRunner> LimaVmDriver<R> {
    pub fn new(cfg: VmConfig, runner: R) -> Self {
        Self { cfg, runner }
    }
}

impl<R: CommandRunner> VmDriver for LimaVmDriver<R> {
    fn status(&self) -> Result<VmStatus> {
        let o = self.runner.run(
            "limactl",
            &["list", "--format", "{{.Status}}", &self.cfg.name],
        )?;
        Ok(match o.stdout.trim() {
            "" => VmStatus::NotCreated,
            "Running" => VmStatus::Running,
            "Stopped" => VmStatus::Stopped,
            _ => VmStatus::Starting,
        })
    }

    fn up(&self, reporter: &dyn Reporter) -> Result<()> {
        reporter.step(&format!("Checking status of VM '{}'", self.cfg.name));
        match self.status()? {
            VmStatus::Running => {
                reporter.step("VM is already running");
                Ok(())
            }
            VmStatus::NotCreated => {
                reporter.step(&format!(
                    "Creating VM '{}' ({} CPU, {} GiB) — downloading image, this can take a few minutes",
                    self.cfg.name, self.cfg.cpus, self.cfg.memory_gib
                ));
                let name = format!("--name={}", self.cfg.name);
                let cpus = format!("--cpus={}", self.cfg.cpus);
                let mem = format!("--memory={}", self.cfg.memory_gib);
                let code = self.runner.run_streamed(
                    "limactl",
                    &[
                        "start",
                        &name,
                        &cpus,
                        &mem,
                        "--tty=false",
                        "template://default",
                    ],
                )?;
                if code != 0 {
                    return Err(Error::Vm(format!(
                        "limactl start (create) exited with {code}"
                    )));
                }
                reporter.step("VM created and running");
                Ok(())
            }
            VmStatus::Stopped | VmStatus::Starting => {
                reporter.step("Starting existing VM");
                let code = self
                    .runner
                    .run_streamed("limactl", &["start", &self.cfg.name])?;
                if code != 0 {
                    return Err(Error::Vm(format!("limactl start exited with {code}")));
                }
                reporter.step("VM running");
                Ok(())
            }
        }
    }

    fn down(&self) -> Result<()> {
        let o = self.runner.run("limactl", &["stop", &self.cfg.name])?;
        if !o.ok() {
            return Err(Error::Vm(format!("limactl stop: {}", o.stderr.trim())));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::SilentReporter;

    #[test]
    fn fake_lifecycle() {
        let d = FakeVmDriver::new();
        assert_eq!(d.status().unwrap(), VmStatus::NotCreated);
        d.up(&SilentReporter).unwrap();
        assert_eq!(d.status().unwrap(), VmStatus::Running);
        d.down().unwrap();
        assert_eq!(d.status().unwrap(), VmStatus::Stopped);
    }
}

#[cfg(test)]
mod lima_tests {
    use super::*;
    use crate::config::{VmConfig, VmDriverKind};
    use crate::process::{out, FakeRunner};
    use crate::progress::{Reporter, SilentReporter};
    use std::cell::RefCell;

    #[derive(Default)]
    struct RecordingReporter {
        steps: RefCell<Vec<String>>,
    }
    impl Reporter for RecordingReporter {
        fn step(&self, msg: &str) {
            self.steps.borrow_mut().push(msg.to_string());
        }
    }

    fn cfg() -> VmConfig {
        VmConfig {
            name: "llmsc".into(),
            cpus: 4,
            memory_gib: 8,
            disk_gib: 100,
            driver: VmDriverKind::Lima,
        }
    }

    #[test]
    fn parses_running_status() {
        let d = LimaVmDriver::new(cfg(), FakeRunner::new(|_, _| out(0, "Running\n")));
        assert_eq!(d.status().unwrap(), VmStatus::Running);
    }

    #[test]
    fn empty_status_is_not_created() {
        let d = LimaVmDriver::new(cfg(), FakeRunner::new(|_, _| out(0, "")));
        assert_eq!(d.status().unwrap(), VmStatus::NotCreated);
    }

    #[test]
    fn up_creates_when_absent() {
        // every command returns "" — list "" means NotCreated, so up() should create.
        let d = LimaVmDriver::new(cfg(), FakeRunner::new(|_, _| out(0, "")));
        d.up(&SilentReporter).unwrap();
        assert!(d.runner.called_with("template://default"));
        assert!(d.runner.called_with("--name=llmsc"));
    }

    #[test]
    fn up_reports_steps_when_creating() {
        let d = LimaVmDriver::new(cfg(), FakeRunner::new(|_, _| out(0, "")));
        let rep = RecordingReporter::default();
        d.up(&rep).unwrap();
        let steps = rep.steps.borrow();
        assert!(steps.iter().any(|s| s.contains("Creating VM")));
        assert!(steps.iter().any(|s| s.contains("running")));
    }

    #[test]
    fn up_is_noop_when_running() {
        let d = LimaVmDriver::new(
            cfg(),
            FakeRunner::new(|_, args| {
                if args.first().copied() == Some("list") {
                    out(0, "Running")
                } else {
                    out(0, "")
                }
            }),
        );
        d.up(&SilentReporter).unwrap();
        assert!(!d.runner.called_with("start"));
    }

    #[test]
    fn down_stops() {
        let d = LimaVmDriver::new(cfg(), FakeRunner::new(|_, _| out(0, "")));
        d.down().unwrap();
        assert!(d.runner.called_with("stop"));
    }
}
