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

/// Live resource usage of the L1 VM (what the host has handed to llmsc-vm). CPU "used" is the
/// 1-minute load average clamped to the core count — a real, if coarse, in-use figure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VmResources {
    pub cpu_used: f64,
    pub cpu_total: u32,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

/// Shell snippet run inside the VM to emit labelled resource lines (see [`parse_resources`]).
const RESOURCES_SCRIPT: &str = "printf 'cpu_total %s\\n' \"$(nproc)\"; \
     printf 'cpu_load %s\\n' \"$(cut -d' ' -f1 /proc/loadavg)\"; \
     free -b | awk '/^Mem:/{printf \"mem %s %s\\n\",$2,$7}'; \
     df -B1 / | awk 'NR==2{printf \"disk %s %s\\n\",$2,$3}'";

/// Parse the labelled output of [`RESOURCES_SCRIPT`] into [`VmResources`].
pub fn parse_resources(output: &str) -> Result<VmResources> {
    let (mut cpu_total, mut cpu_load) = (0u32, 0f64);
    let (mut mem_total, mut mem_avail) = (0u64, 0u64);
    let (mut disk_total, mut disk_used) = (0u64, 0u64);
    for line in output.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.as_slice() {
            ["cpu_total", n] => cpu_total = n.parse().unwrap_or(0),
            ["cpu_load", n] => cpu_load = n.parse().unwrap_or(0.0),
            ["mem", total, avail] => {
                mem_total = total.parse().unwrap_or(0);
                mem_avail = avail.parse().unwrap_or(0);
            }
            ["disk", total, used] => {
                disk_total = total.parse().unwrap_or(0);
                disk_used = used.parse().unwrap_or(0);
            }
            _ => {}
        }
    }
    if cpu_total == 0 || mem_total == 0 || disk_total == 0 {
        return Err(Error::Vm(format!(
            "could not read VM resources (output: {})",
            output.trim()
        )));
    }
    Ok(VmResources {
        cpu_used: cpu_load.min(cpu_total as f64),
        cpu_total,
        mem_used_bytes: mem_total.saturating_sub(mem_avail),
        mem_total_bytes: mem_total,
        disk_used_bytes: disk_used,
        disk_total_bytes: disk_total,
    })
}

/// Drives the L1 VM. `&self` methods (real impls shell out; fakes use interior mutability).
pub trait VmDriver {
    fn status(&self) -> Result<VmStatus>;
    /// Bring the VM up, reporting each step (creation can take minutes).
    fn up(&self, reporter: &dyn Reporter) -> Result<()>;
    fn down(&self) -> Result<()>;
    /// Stop and delete the VM entirely.
    fn destroy(&self) -> Result<()>;
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
    fn destroy(&self) -> Result<()> {
        self.status.set(VmStatus::NotCreated);
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

    /// Read live CPU/memory/disk usage from inside the VM. Requires the VM to be running.
    pub fn resources(&self) -> Result<VmResources> {
        let o = self.runner.run(
            "limactl",
            &["shell", &self.cfg.name, "sh", "-c", RESOURCES_SCRIPT],
        )?;
        if !o.ok() {
            return Err(Error::Vm(format!(
                "reading VM resources: {}",
                o.stderr.trim()
            )));
        }
        parse_resources(&o.stdout)
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

    fn destroy(&self) -> Result<()> {
        let o = self
            .runner
            .run("limactl", &["delete", "--force", &self.cfg.name])?;
        if !o.ok() {
            return Err(Error::Vm(format!("limactl delete: {}", o.stderr.trim())));
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
        d.destroy().unwrap();
        assert_eq!(d.status().unwrap(), VmStatus::NotCreated);
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

    #[test]
    fn destroy_deletes() {
        let d = LimaVmDriver::new(cfg(), FakeRunner::new(|_, _| out(0, "")));
        d.destroy().unwrap();
        assert!(d.runner.called_with("delete"));
    }

    #[test]
    fn parses_resource_output() {
        let r = parse_resources(
            "cpu_total 8\ncpu_load 2.5\nmem 16000000000 6000000000\ndisk 200000000000 80000000000",
        )
        .unwrap();
        assert_eq!(r.cpu_total, 8);
        assert_eq!(r.cpu_used, 2.5);
        assert_eq!(r.mem_total_bytes, 16_000_000_000);
        assert_eq!(r.mem_used_bytes, 10_000_000_000); // total - available
        assert_eq!(r.disk_used_bytes, 80_000_000_000);
    }

    #[test]
    fn cpu_used_is_clamped_to_total() {
        let r = parse_resources("cpu_total 4\ncpu_load 9.9\nmem 8 4\ndisk 100 10").unwrap();
        assert_eq!(r.cpu_used, 4.0);
    }

    #[test]
    fn empty_resource_output_errors() {
        assert!(parse_resources("").is_err());
    }

    #[test]
    fn resources_runs_limactl_shell() {
        let d = LimaVmDriver::new(
            cfg(),
            FakeRunner::new(|_, _| out(0, "cpu_total 4\ncpu_load 1.0\nmem 8 4\ndisk 100 25")),
        );
        let r = d.resources().unwrap();
        assert_eq!(r.cpu_total, 4);
        assert!(d.runner.called_with("shell"));
    }
}
