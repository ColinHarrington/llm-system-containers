//! Incus bootstrap inside the L1 VM (M1): install Incus, `admin init`, set the `.llmsc` DNS
//! domain on the bridge. Codifies the spike's phase-0 setup, idempotently.
//!
//! Commands run *inside* the VM via `limactl shell`, through the [`CommandRunner`] boundary so
//! the flow is unit-testable. (Unprivileged-nesting / apparmor setup for L3 lands in M3.)

use crate::error::{Error, Result};
use crate::process::{CommandRunner, RunOutput};
use crate::progress::Reporter;

/// Bootstraps Incus inside a Lima VM.
pub struct IncusBootstrap<'a, R: CommandRunner> {
    vm: String,
    runner: &'a R,
}

impl<'a, R: CommandRunner> IncusBootstrap<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            runner,
        }
    }

    fn vm_streamed(&self, cmd: &str) -> Result<i32> {
        self.runner
            .run_streamed("limactl", &["shell", self.vm.as_str(), "bash", "-lc", cmd])
    }

    fn vm_run(&self, cmd: &str) -> Result<RunOutput> {
        self.runner
            .run("limactl", &["shell", self.vm.as_str(), "bash", "-lc", cmd])
    }

    /// Idempotent: install Incus (if absent), `admin init --minimal`, set the `.llmsc` DNS
    /// domain on the bridge, then verify.
    pub fn run(&self, reporter: &dyn Reporter) -> Result<()> {
        reporter.step("Installing Incus (if needed)");
        let code = self.vm_streamed(
            "command -v incus >/dev/null || (sudo apt-get update && sudo apt-get install -y incus)",
        )?;
        if code != 0 {
            return Err(Error::Incus(format!(
                "installing Incus failed (exit {code})"
            )));
        }

        reporter.step("Initializing Incus");
        self.vm_streamed("sudo incus admin init --minimal 2>/dev/null || true")?;

        reporter.step("Configuring .llmsc DNS on the bridge");
        self.vm_streamed("sudo incus network set incusbr0 dns.domain llmsc 2>/dev/null || true")?;

        reporter.step("Verifying Incus");
        let o = self.vm_run("sudo incus version")?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "Incus not ready: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{out, FakeRunner};
    use crate::progress::SilentReporter;

    #[test]
    fn runs_install_init_and_dns() {
        let r = FakeRunner::new(|_, _| out(0, "incus 6.0.0"));
        IncusBootstrap::new("llmsc", &r)
            .run(&SilentReporter)
            .unwrap();
        assert!(r.called_with("apt-get install -y incus"));
        assert!(r.called_with("incus admin init"));
        assert!(r.called_with("dns.domain llmsc"));
        assert!(r.called_with("incus version"));
    }

    #[test]
    fn errors_when_incus_not_ready() {
        let r = FakeRunner::new(|_, args| {
            let cmd = args.last().copied().unwrap_or("");
            if cmd.contains("incus version") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        assert!(IncusBootstrap::new("llmsc", &r)
            .run(&SilentReporter)
            .is_err());
    }
}
