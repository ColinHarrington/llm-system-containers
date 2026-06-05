//! Service provisioning (M5).
//!
//! Deployers stand a service up inside the VM (in its own L2 container by default), driving
//! `incus` via `limactl shell` through the [`CommandRunner`] boundary.
//!
//! Verified on the VM: deploys and starts LiteLLM in its own **debian/12** L2 container
//! (the proxy comes up and `/health/liveliness` responds). Still TODO for real use: supply a
//! provider API key + a real model, and mint per-agent **virtual keys** — the `master_key`
//! and model in the generated config are placeholders.

use crate::error::{Error, Result};
use crate::process::{CommandRunner, RunOutput};
use crate::progress::Reporter;

/// Pinned LiteLLM version. PyPI `litellm` **1.82.7 / 1.82.8 shipped credential-stealing malware**
/// (BerriAI/litellm#24518); a proxy that will hold real provider/subscription credentials must
/// never float to an arbitrary version. Bump deliberately after vetting a release.
const LITELLM_VERSION: &str = "1.87.0";

/// LiteLLM admin master key (placeholder — TODO rotate; provider key via environment). Used both
/// in the generated config and to authenticate virtual-key minting.
const MASTER_KEY: &str = "sk-llmsc-master";

/// Provisions LiteLLM in its own L2 container.
pub struct LiteLlmDeployer<'a, R: CommandRunner> {
    vm: String,
    container: String,
    image: String,
    port: u16,
    runner: &'a R,
}

impl<'a, R: CommandRunner> LiteLlmDeployer<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            container: crate::service::container_name("litellm"),
            // debian/13 (trixie) systemd hangs at boot under this Incus → no networking; bookworm works.
            image: "images:debian/12".into(),
            port: 4000,
            runner,
        }
    }

    fn incus(&self, args: &[&str]) -> Result<RunOutput> {
        let mut full = vec!["shell", self.vm.as_str(), "sudo", "incus"];
        full.extend_from_slice(args);
        self.runner.run("limactl", &full)
    }

    fn incus_streamed(&self, args: &[&str]) -> Result<i32> {
        let mut full = vec!["shell", self.vm.as_str(), "sudo", "incus"];
        full.extend_from_slice(args);
        self.runner.run_streamed("limactl", &full)
    }

    fn exec(&self, cmd: &str) -> Result<RunOutput> {
        self.incus(&["exec", self.container.as_str(), "--", "bash", "-lc", cmd])
    }

    fn exec_streamed(&self, cmd: &str) -> Result<i32> {
        self.incus_streamed(&["exec", self.container.as_str(), "--", "bash", "-lc", cmd])
    }

    /// Provision and start LiteLLM. Idempotent-ish (skips container creation if present).
    pub fn deploy(&self, reporter: &dyn Reporter) -> Result<()> {
        reporter.step("Creating LiteLLM service container");
        if !self.incus(&["info", self.container.as_str()])?.ok() {
            let code =
                self.incus_streamed(&["launch", self.image.as_str(), self.container.as_str()])?;
            if code != 0 {
                return Err(Error::Incus(format!(
                    "creating {} failed (exit {code})",
                    self.container
                )));
            }
        }

        reporter.step("Installing Python (apt)");
        // Always install python3-venv: the base image ships python3 but not the venv module.
        let o = self.exec(
            "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y python3 python3-venv",
        )?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "apt install python: {}",
                o.stderr.trim()
            )));
        }

        reporter.step("Creating virtualenv");
        let o = self.exec("test -x /opt/litellm/bin/pip || python3 -m venv /opt/litellm")?;
        if !o.ok() {
            return Err(Error::Incus(format!("python venv: {}", o.stderr.trim())));
        }

        reporter.step(&format!(
            "Installing LiteLLM {LITELLM_VERSION} (pip, pinned)"
        ));
        let install = format!(
            "/opt/litellm/bin/pip install --quiet --upgrade pip && \
             /opt/litellm/bin/pip install --quiet 'litellm[proxy]=={LITELLM_VERSION}'"
        );
        let code = self.exec_streamed(&install)?;
        if code != 0 {
            return Err(Error::Incus(format!(
                "pip install litellm failed (exit {code})"
            )));
        }

        reporter.step("Writing config + systemd unit");
        self.exec(&config_script())?;
        self.exec(&unit_script(self.port))?;

        reporter.step("Starting LiteLLM");
        self.exec("systemctl daemon-reload && systemctl enable --now litellm")?;

        reporter.step(&format!(
            "LiteLLM deployed — reachable in the VM at {}:{} (TODO: set a provider key + virtual keys)",
            self.container, self.port
        ));
        Ok(())
    }

    /// Mint/refresh per-agent virtual keys (compiled by `enforce::virtual_key_specs`) against the
    /// running proxy's admin API. Idempotent-ish: a duplicate `key_alias` is treated as success
    /// (true upsert + usage read-back is a follow-up). Requires the proxy to be up. Returns the
    /// aliases synced.
    pub fn sync_virtual_keys(
        &self,
        specs: &[crate::enforce::VirtualKeySpec],
        reporter: &dyn Reporter,
    ) -> Result<Vec<String>> {
        let mut synced = Vec::new();
        for s in specs {
            reporter.step(&format!(
                "Virtual key {} — ${:.0}/{}",
                s.key_alias, s.max_budget_usd, s.budget_duration
            ));
            let models = if s.models.is_empty() {
                "[]".to_string()
            } else {
                format!(
                    "[{}]",
                    s.models
                        .iter()
                        .map(|m| format!("\"{m}\""))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            let body = format!(
                "{{\"key_alias\":\"{}\",\"max_budget\":{},\"budget_duration\":\"{}\",\"models\":{}}}",
                s.key_alias, s.max_budget_usd, s.budget_duration, models
            );
            let curl = format!(
                "curl -sS -X POST http://127.0.0.1:{}/key/generate \
                 -H 'Authorization: Bearer {}' -H 'Content-Type: application/json' -d '{}'",
                self.port, MASTER_KEY, body
            );
            let o = self.exec(&curl)?;
            let combined = format!("{} {}", o.stdout, o.stderr).to_lowercase();
            let dup = combined.contains("already exists") || combined.contains("duplicate");
            if !o.ok() && !dup {
                return Err(Error::Incus(format!(
                    "minting virtual key {}: {}",
                    s.key_alias,
                    o.stderr.trim()
                )));
            }
            synced.push(s.key_alias.clone());
        }
        Ok(synced)
    }
}

/// Minimal proxy config. A real provider key + model must still be supplied (integration TODO).
fn config_script() -> String {
    format!(
        "mkdir -p /etc/litellm && cat > /etc/litellm/config.yaml <<'EOF'\n\
         model_list:\n\
         \x20 - model_name: default\n\
         \x20   litellm_params:\n\
         \x20     model: openai/gpt-4o-mini\n\
         general_settings:\n\
         \x20 master_key: {MASTER_KEY}  # TODO: rotate; provider key via environment\n\
         EOF"
    )
}

fn unit_script(port: u16) -> String {
    format!(
        "cat > /etc/systemd/system/litellm.service <<'EOF'\n\
         [Unit]\nDescription=LiteLLM proxy\nAfter=network.target\n\
         [Service]\n\
         ExecStart=/opt/litellm/bin/litellm --config /etc/litellm/config.yaml --port {port}\n\
         Restart=on-failure\n\
         [Install]\nWantedBy=multi-user.target\nEOF"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{out, FakeRunner};
    use crate::progress::SilentReporter;

    #[test]
    fn deploy_runs_expected_steps() {
        // `info` -> non-zero (container absent) so it creates; everything else ok.
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        LiteLlmDeployer::new("llmsc", &r)
            .deploy(&SilentReporter)
            .unwrap();
        assert!(r.called_with("launch"));
        assert!(r.called_with("svc-litellm"));
        assert!(r.called_with("litellm[proxy]"));
        assert!(r.called_with("config.yaml"));
        assert!(r.called_with("litellm.service"));
        assert!(r.called_with("systemctl"));
    }

    #[test]
    fn sync_virtual_keys_posts_specs_to_admin_api() {
        let r = FakeRunner::new(|_, _| out(0, "{\"key\":\"sk-...\"}"));
        let specs = vec![crate::enforce::virtual_key_spec(
            "web-agent-01",
            "agent-claude",
            "small",
        )];
        let synced = LiteLlmDeployer::new("llmsc", &r)
            .sync_virtual_keys(&specs, &SilentReporter)
            .unwrap();
        assert_eq!(synced, vec!["llmsc-web-agent-01-agent-claude"]);
        assert!(r.called_with("key/generate"));
        assert!(r.called_with("llmsc-web-agent-01-agent-claude"));
        assert!(r.called_with("\"max_budget\":5"));
    }

    #[test]
    fn sync_virtual_keys_tolerates_duplicate_alias() {
        let r = FakeRunner::new(|_, _| out(1, "Error: key_alias already exists"));
        let specs = vec![crate::enforce::virtual_key_spec("sb", "agent-x", "medium")];
        // duplicate → treated as success.
        LiteLlmDeployer::new("llmsc", &r)
            .sync_virtual_keys(&specs, &SilentReporter)
            .unwrap();
    }

    #[test]
    fn deploy_errors_if_pip_fails() {
        // info -> 0 (container "exists", skip create); pip install -> non-zero (fail).
        let r = FakeRunner::new(|_, args| {
            let cmd = args.last().copied().unwrap_or("");
            if cmd.contains("pip install") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        assert!(LiteLlmDeployer::new("llmsc", &r)
            .deploy(&SilentReporter)
            .is_err());
    }
}
