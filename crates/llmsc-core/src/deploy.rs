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
        self.exec(&config_script(provider_env_and_model("openai").1))?;
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

    /// Set the upstream **provider** API key (e.g. OpenAI/Anthropic) — written ONLY into the
    /// LiteLLM container (env file) and the model pointed at that provider. The real credential
    /// never touches `llmsc.toml`; it lives only in the service container (credential isolation).
    pub fn set_provider_key(
        &self,
        provider: &str,
        api_key: &str,
        reporter: &dyn Reporter,
    ) -> Result<()> {
        let (env_var, model) = provider_env_and_model(provider);
        reporter.step(&format!("Configuring provider '{provider}' ({model})"));
        // Write the env file (the key lives here, inside the container, 0600).
        let env_script = format!(
            "umask 077 && mkdir -p /etc/litellm && printf '%s=%s\\n' {env_var} '{}' > /etc/litellm/litellm.env",
            api_key.replace('\'', "")
        );
        let o = self.exec(&env_script)?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "writing provider key: {}",
                o.stderr.trim()
            )));
        }
        self.exec(&config_script(model))?;
        self.exec(&unit_script(self.port))?;
        let _ =
            self.exec("systemctl daemon-reload && systemctl restart litellm 2>/dev/null || true");
        reporter.step("Provider key set (stored only in the LiteLLM container)");
        Ok(())
    }

    /// Read per-key spend from the proxy admin API (`/global/spend/keys`). Best-effort: returns an
    /// empty list if the proxy is unreachable. The endpoint/shape may vary by LiteLLM version.
    pub fn key_usage(&self) -> Result<Vec<KeyUsage>> {
        let curl = format!(
            "curl -sS http://127.0.0.1:{}/global/spend/keys -H 'Authorization: Bearer {}'",
            self.port, MASTER_KEY
        );
        let o = self.exec(&curl)?;
        if !o.ok() {
            return Ok(Vec::new());
        }
        Ok(parse_key_usage(&o.stdout).unwrap_or_default())
    }

    /// Point LiteLLM's tracing at a Phoenix collector (`http://<host>:6006`). Writes the endpoint
    /// to the env file and enables the arize_phoenix callback so every LLM call is traced. Idempotent.
    pub fn enable_phoenix(&self, phoenix_host: &str, reporter: &dyn Reporter) -> Result<()> {
        reporter.step(&format!("Wiring LiteLLM traces → Phoenix ({phoenix_host})"));
        let endpoint = format!("http://{phoenix_host}:6006");
        let script = format!(
            "umask 077 && mkdir -p /etc/litellm && \
             grep -q PHOENIX_COLLECTOR_ENDPOINT /etc/litellm/litellm.env 2>/dev/null || \
             printf 'PHOENIX_COLLECTOR_ENDPOINT=%s\\n' '{endpoint}' >> /etc/litellm/litellm.env"
        );
        let o = self.exec(&script)?;
        if !o.ok() {
            return Err(Error::Incus(format!("wiring Phoenix: {}", o.stderr.trim())));
        }
        let _ = self.exec("systemctl restart litellm 2>/dev/null || true");
        Ok(())
    }
}

/// Per-key spend read back from LiteLLM.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyUsage {
    pub key_alias: String,
    pub spend: f64,
}

/// Parse `/global/spend/keys` output: a bare array or a `{"keys":[...]}` wrapper of
/// `{key_alias, spend}` objects. Entries without an alias are dropped.
pub fn parse_key_usage(json: &str) -> Result<Vec<KeyUsage>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        #[serde(default)]
        key_alias: String,
        #[serde(default)]
        spend: f64,
    }
    #[derive(serde::Deserialize)]
    struct Wrap {
        #[serde(default)]
        keys: Vec<Raw>,
    }
    let raws: Vec<Raw> = if let Ok(arr) = serde_json::from_str::<Vec<Raw>>(json) {
        arr
    } else {
        serde_json::from_str::<Wrap>(json)
            .map_err(|e| Error::Incus(format!("parsing key usage: {e}")))?
            .keys
    };
    Ok(raws
        .into_iter()
        .filter(|r| !r.key_alias.is_empty())
        .map(|r| KeyUsage {
            key_alias: r.key_alias,
            spend: r.spend,
        })
        .collect())
}

/// Minimal proxy config for a given model. The provider key is supplied separately via
/// [`LiteLlmDeployer::set_provider_key`] (env file), never stored in this config or in llmsc.toml.
fn config_script(model: &str) -> String {
    format!(
        "mkdir -p /etc/litellm && cat > /etc/litellm/config.yaml <<'EOF'\n\
         model_list:\n\
         \x20 - model_name: default\n\
         \x20   litellm_params:\n\
         \x20     model: {model}\n\
         general_settings:\n\
         \x20 master_key: {MASTER_KEY}  # TODO: rotate\n\
         EOF"
    )
}

/// The env-var name and a default model for a provider. Unknown → OpenAI.
fn provider_env_and_model(provider: &str) -> (&'static str, &'static str) {
    match provider.trim().to_lowercase().as_str() {
        "anthropic" => ("ANTHROPIC_API_KEY", "anthropic/claude-3-5-sonnet-latest"),
        "openai" => ("OPENAI_API_KEY", "openai/gpt-4o-mini"),
        _ => ("OPENAI_API_KEY", "openai/gpt-4o-mini"),
    }
}

fn unit_script(port: u16) -> String {
    format!(
        "cat > /etc/systemd/system/litellm.service <<'EOF'\n\
         [Unit]\nDescription=LiteLLM proxy\nAfter=network.target\n\
         [Service]\n\
         EnvironmentFile=-/etc/litellm/litellm.env\n\
         ExecStart=/opt/litellm/bin/litellm --config /etc/litellm/config.yaml --port {port}\n\
         Restart=on-failure\n\
         [Install]\nWantedBy=multi-user.target\nEOF"
    )
}

/// Provisions **Phoenix** (Arize) in its own L2 container — LLM/agent observability (traces,
/// token usage). Mirrors [`LiteLlmDeployer`]; LiteLLM is wired to export traces here via
/// [`LiteLlmDeployer::enable_phoenix`].
pub struct PhoenixDeployer<'a, R: CommandRunner> {
    vm: String,
    container: String,
    image: String,
    port: u16,
    runner: &'a R,
}

impl<'a, R: CommandRunner> PhoenixDeployer<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            container: crate::service::container_name("phoenix"),
            image: "images:debian/12".into(),
            port: 6006,
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

    /// Provision and start Phoenix (pip `arize-phoenix`, systemd `phoenix serve`).
    pub fn deploy(&self, reporter: &dyn Reporter) -> Result<()> {
        reporter.step("Creating Phoenix service container");
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
        let o = self.exec("test -x /opt/phoenix/bin/pip || python3 -m venv /opt/phoenix")?;
        if !o.ok() {
            return Err(Error::Incus(format!("python venv: {}", o.stderr.trim())));
        }
        reporter.step("Installing arize-phoenix (pip)");
        let code = self.exec_streamed(
            "/opt/phoenix/bin/pip install --quiet --upgrade pip && \
             /opt/phoenix/bin/pip install --quiet arize-phoenix",
        )?;
        if code != 0 {
            return Err(Error::Incus(format!(
                "pip install phoenix failed (exit {code})"
            )));
        }
        reporter.step("Writing systemd unit");
        self.exec(&phoenix_unit_script(self.port))?;
        reporter.step("Starting Phoenix");
        self.exec("systemctl daemon-reload && systemctl enable --now phoenix")?;
        reporter.step(&format!(
            "Phoenix deployed — UI/collector in the VM at {}:{}",
            self.container, self.port
        ));
        Ok(())
    }
}

fn phoenix_unit_script(port: u16) -> String {
    format!(
        "cat > /etc/systemd/system/phoenix.service <<'EOF'\n\
         [Unit]\nDescription=Arize Phoenix observability\nAfter=network.target\n\
         [Service]\n\
         Environment=PHOENIX_PORT={port}\n\
         ExecStart=/opt/phoenix/bin/phoenix serve\n\
         Restart=on-failure\n\
         [Install]\nWantedBy=multi-user.target\nEOF"
    )
}

/// Pinned VictoriaMetrics / Loki release versions (GitHub release tarballs). Bump deliberately.
const VICTORIAMETRICS_VERSION: &str = "v1.111.0";
const LOKI_VERSION: &str = "3.3.2";

/// Provisions the **metrics + logs stack** in one L2 container (`svc-grafana`): VictoriaMetrics
/// (metrics, :8428), Loki (logs, :3100), and Grafana (dashboards, :3000) with both wired in as
/// datasources. Representative install — pinned versions / apt repo are validated on the VM later.
pub struct GrafanaStackDeployer<'a, R: CommandRunner> {
    vm: String,
    container: String,
    image: String,
    runner: &'a R,
}

impl<'a, R: CommandRunner> GrafanaStackDeployer<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            container: crate::service::container_name("grafana"),
            image: "images:debian/12".into(),
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

    fn check(&self, cmd: &str, what: &str) -> Result<()> {
        let o = self.exec(cmd)?;
        if !o.ok() {
            return Err(Error::Incus(format!("{what}: {}", o.stderr.trim())));
        }
        Ok(())
    }

    /// Provision and start VictoriaMetrics + Loki + Grafana, with Grafana datasources wired.
    pub fn deploy(&self, reporter: &dyn Reporter) -> Result<()> {
        reporter.step("Creating metrics/logs service container");
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

        reporter.step("Installing Grafana (apt repo)");
        self.check(
            "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y wget ca-certificates gpg tar && \
             wget -qO /usr/share/keyrings/grafana.key https://apt.grafana.com/gpg.key && \
             echo 'deb [signed-by=/usr/share/keyrings/grafana.key] https://apt.grafana.com stable main' > /etc/apt/sources.list.d/grafana.list && \
             apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y grafana",
            "apt install grafana",
        )?;

        reporter.step(&format!(
            "Installing VictoriaMetrics {VICTORIAMETRICS_VERSION}"
        ));
        let arch = "$(dpkg --print-architecture)"; // amd64 / arm64 — matches the VM
        let vm_v = VICTORIAMETRICS_VERSION;
        self.check(
            &format!(
                "wget -qO /tmp/vm.tar.gz https://github.com/VictoriaMetrics/VictoriaMetrics/releases/download/{vm_v}/victoria-metrics-linux-{arch}-{vm_v}.tar.gz && \
                 tar -xzf /tmp/vm.tar.gz -C /usr/local/bin && \
                 mv -f /usr/local/bin/victoria-metrics-prod /usr/local/bin/victoria-metrics"
            ),
            "install victoria-metrics",
        )?;

        reporter.step(&format!("Installing Loki {LOKI_VERSION}"));
        let loki_v = LOKI_VERSION;
        self.check(
            &format!(
                "wget -qO /tmp/loki.zip https://github.com/grafana/loki/releases/download/v{loki_v}/loki-linux-{arch}.zip && \
                 (command -v unzip >/dev/null || apt-get install -y unzip) && \
                 unzip -o /tmp/loki.zip -d /usr/local/bin && mv -f /usr/local/bin/loki-linux-* /usr/local/bin/loki"
            ),
            "install loki",
        )?;

        reporter.step("Writing systemd units + Grafana datasources");
        self.check(&metrics_units_script(), "write units")?;
        self.check(&grafana_datasources_script(), "write datasources")?;

        reporter.step("Starting VictoriaMetrics + Loki + Grafana");
        self.check(
            "systemctl daemon-reload && systemctl enable --now victoria-metrics loki grafana-server",
            "start stack",
        )?;
        reporter
            .step("Metrics/logs stack deployed — Grafana :3000, VictoriaMetrics :8428, Loki :3100");
        Ok(())
    }
}

fn metrics_units_script() -> String {
    "mkdir -p /var/lib/victoria-metrics /var/lib/loki && \
     cat > /etc/loki.yaml <<'EOF'\n\
     auth_enabled: false\n\
     server: {http_listen_port: 3100}\n\
     common: {ring: {kvstore: {store: inmemory}}, replication_factor: 1, path_prefix: /var/lib/loki}\n\
     schema_config: {configs: [{from: 2020-01-01, store: tsdb, object_store: filesystem, schema: v13, index: {prefix: index_, period: 24h}}]}\n\
     EOF\n\
     cat > /etc/systemd/system/victoria-metrics.service <<'EOF'\n\
     [Unit]\nDescription=VictoriaMetrics\nAfter=network.target\n\
     [Service]\nExecStart=/usr/local/bin/victoria-metrics --storageDataPath=/var/lib/victoria-metrics --httpListenAddr=:8428\nRestart=on-failure\n\
     [Install]\nWantedBy=multi-user.target\nEOF\n\
     cat > /etc/systemd/system/loki.service <<'EOF'\n\
     [Unit]\nDescription=Loki\nAfter=network.target\n\
     [Service]\nExecStart=/usr/local/bin/loki -config.file=/etc/loki.yaml\nRestart=on-failure\n\
     [Install]\nWantedBy=multi-user.target\nEOF"
        .to_string()
}

fn grafana_datasources_script() -> String {
    "mkdir -p /etc/grafana/provisioning/datasources && \
     cat > /etc/grafana/provisioning/datasources/llmsc.yaml <<'EOF'\n\
     apiVersion: 1\n\
     datasources:\n\
     \x20 - name: VictoriaMetrics\n\
     \x20   type: prometheus\n\
     \x20   access: proxy\n\
     \x20   url: http://127.0.0.1:8428\n\
     \x20 - name: Loki\n\
     \x20   type: loki\n\
     \x20   access: proxy\n\
     \x20   url: http://127.0.0.1:3100\n\
     EOF"
    .to_string()
}

/// Pinned SeaweedFS release version (GitHub release tarball). Bump deliberately.
const SEAWEEDFS_VERSION: &str = "3.80";
/// The Incus storage pool + custom volume backing shared storage (attached to svc-seaweedfs and
/// to sandboxes via [`crate::incus::CliIncus::attach_shared_volume`]).
pub const SHARED_POOL: &str = "default";
pub const SHARED_VOLUME: &str = "llmsc-shared";

/// Provisions **SeaweedFS** in its own L2 container — S3-compatible shared storage. The data dir
/// is an Incus custom volume ([`SHARED_VOLUME`]) that also attaches to sandboxes, so files are
/// shared host ↔ container and container ↔ container. Representative install (pinned release).
pub struct SeaweedFsDeployer<'a, R: CommandRunner> {
    vm: String,
    container: String,
    image: String,
    runner: &'a R,
}

impl<'a, R: CommandRunner> SeaweedFsDeployer<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            container: crate::service::container_name("seaweedfs"),
            image: "images:debian/12".into(),
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

    fn check(&self, cmd: &str, what: &str) -> Result<()> {
        let o = self.exec(cmd)?;
        if !o.ok() {
            return Err(Error::Incus(format!("{what}: {}", o.stderr.trim())));
        }
        Ok(())
    }

    /// Provision and start SeaweedFS with the S3 gateway, backed by the shared custom volume.
    pub fn deploy(&self, reporter: &dyn Reporter) -> Result<()> {
        let dup = |o: &RunOutput| {
            format!("{} {}", o.stderr, o.stdout)
                .to_lowercase()
                .contains("already exists")
        };
        reporter.step("Creating shared storage volume");
        let c = self.incus(&["storage", "volume", "create", SHARED_POOL, SHARED_VOLUME])?;
        if !c.ok() && !dup(&c) {
            return Err(Error::Incus(format!(
                "creating shared volume: {}",
                c.stderr.trim()
            )));
        }

        reporter.step("Creating SeaweedFS service container");
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
        // Back /data with the shared volume (idempotent).
        let o = self.incus(&[
            "config",
            "device",
            "add",
            self.container.as_str(),
            "shared",
            "disk",
            "pool=default",
            "source=llmsc-shared",
            "path=/data",
        ])?;
        if !o.ok() && !dup(&o) {
            return Err(Error::Incus(format!(
                "attaching shared volume: {}",
                o.stderr.trim()
            )));
        }

        reporter.step(&format!("Installing SeaweedFS {SEAWEEDFS_VERSION}"));
        let arch = "$(dpkg --print-architecture)";
        let v = SEAWEEDFS_VERSION;
        self.check(
            &format!(
                "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y wget tar && \
                 wget -qO /tmp/weed.tar.gz https://github.com/seaweedfs/seaweedfs/releases/download/{v}/linux_{arch}.tar.gz && \
                 tar -xzf /tmp/weed.tar.gz -C /usr/local/bin weed"
            ),
            "install weed",
        )?;
        reporter.step("Writing systemd unit");
        self.check(&seaweedfs_unit_script(), "write unit")?;
        reporter.step("Starting SeaweedFS");
        self.check(
            "systemctl daemon-reload && systemctl enable --now seaweedfs",
            "start seaweedfs",
        )?;
        reporter.step(&format!(
            "SeaweedFS deployed — S3 gateway in the VM at {}:8333 (shared volume at /data)",
            self.container
        ));
        Ok(())
    }
}

fn seaweedfs_unit_script() -> String {
    "mkdir -p /data && cat > /etc/systemd/system/seaweedfs.service <<'EOF'\n\
     [Unit]\nDescription=SeaweedFS (S3 shared storage)\nAfter=network.target\n\
     [Service]\n\
     ExecStart=/usr/local/bin/weed server -dir=/data -s3 -ip.bind=0.0.0.0\n\
     Restart=on-failure\n\
     [Install]\nWantedBy=multi-user.target\nEOF"
        .to_string()
}

/// The mitmproxy egress proxy port (HTTP(S) interception).
pub const MITMPROXY_PORT: u16 = 8080;

/// Provisions **mitmproxy** in its own L2 container — the HTTP(S) egress proxy enforcing the L7
/// domain allowlist. Mirrors [`LiteLlmDeployer`].
///
/// **Honest scope:** sandboxes are pointed at this proxy via `HTTP(S)_PROXY`, but for HTTPS
/// interception the sandbox must also trust mitmproxy's CA, and to be non-bypassable the traffic
/// must be *forced* through it (Tetragon/iptables redirect) — both are follow-ups. Today this
/// blocks plain HTTP to non-allowlisted hosts and HTTPS for proxy-respecting clients.
pub struct MitmproxyDeployer<'a, R: CommandRunner> {
    vm: String,
    container: String,
    image: String,
    port: u16,
    runner: &'a R,
}

impl<'a, R: CommandRunner> MitmproxyDeployer<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            container: crate::service::container_name("mitmproxy"),
            image: "images:debian/12".into(),
            port: MITMPROXY_PORT,
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

    /// Provision and start mitmproxy with an initial (empty) allowlist addon.
    pub fn deploy(&self, reporter: &dyn Reporter) -> Result<()> {
        reporter.step("Creating mitmproxy service container");
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
        reporter.step("Installing mitmproxy (apt)");
        let o = self.exec(
            "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y mitmproxy",
        )?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "apt install mitmproxy: {}",
                o.stderr.trim()
            )));
        }
        reporter.step("Writing allowlist addon + systemd unit");
        self.exec(&mitm_addon_script(&[]))?;
        self.exec(&mitm_unit_script(self.port))?;
        reporter.step("Starting mitmproxy");
        self.exec("systemctl daemon-reload && systemctl enable --now mitmproxy")?;
        reporter.step(&format!(
            "mitmproxy deployed — egress proxy in the VM at {}:{}",
            self.container, self.port
        ));
        Ok(())
    }

    /// Rewrite the allowlist addon with the given domains and reload mitmproxy.
    pub fn sync_allowlist(&self, domains: &[String], reporter: &dyn Reporter) -> Result<()> {
        reporter.step(&format!(
            "Syncing mitmproxy allowlist ({} domains)",
            domains.len()
        ));
        let o = self.exec(&mitm_addon_script(domains))?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "writing mitmproxy allowlist: {}",
                o.stderr.trim()
            )));
        }
        let _ = self.exec("systemctl restart mitmproxy 2>/dev/null || true");
        Ok(())
    }
}

/// A mitmproxy addon that blocks any host not on `domains` (suffix match). Empty list = block all.
fn mitm_addon_script(domains: &[String]) -> String {
    let list = domains
        .iter()
        .map(|d| format!("\"{}\"", d.replace('"', "")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "mkdir -p /etc/mitmproxy && cat > /etc/mitmproxy/allowlist.py <<'LLMSC_EOF'\n\
         from mitmproxy import http\n\
         ALLOW = [{list}]\n\
         def _ok(host: str) -> bool:\n\
         \x20   return any(host == d or host.endswith('.' + d) for d in ALLOW)\n\
         def request(flow: http.HTTPFlow) -> None:\n\
         \x20   if not _ok(flow.request.pretty_host):\n\
         \x20       flow.response = http.Response.make(403, b'blocked by llmsc egress allowlist')\n\
         LLMSC_EOF"
    )
}

fn mitm_unit_script(port: u16) -> String {
    format!(
        "cat > /etc/systemd/system/mitmproxy.service <<'EOF'\n\
         [Unit]\nDescription=mitmproxy egress allowlist\nAfter=network.target\n\
         [Service]\n\
         ExecStart=/usr/bin/mitmdump -s /etc/mitmproxy/allowlist.py --listen-port {port} --set block_global=false\n\
         Restart=on-failure\n\
         [Install]\nWantedBy=multi-user.target\nEOF"
    )
}

/// Loads Tetragon TracingPolicies into the **L1 VM** (Tetragon runs in the VM, not a container —
/// `planning/security-model.md`). Policies live under `/etc/tetragon/tetragon.tp.d/`.
///
/// **Scaffold:** this assumes Tetragon is already installed and running in the VM; installing it
/// (and validating the generated policy schema) is follow-up work. The write+reload path is wired
/// and tested so the GUI/CLI can drive it once Tetragon is present.
pub struct TetragonDeployer<'a, R: CommandRunner> {
    vm: String,
    runner: &'a R,
}

impl<'a, R: CommandRunner> TetragonDeployer<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            runner,
        }
    }

    /// Run a command directly in the VM (sudo) — Tetragon is an L1-host daemon, not in a container.
    fn vm_sh(&self, cmd: &str) -> Result<RunOutput> {
        self.runner.run(
            "limactl",
            &["shell", self.vm.as_str(), "sudo", "bash", "-lc", cmd],
        )
    }

    /// Write a TracingPolicy file and reload Tetragon. `name` is the policy name (file stem).
    pub fn apply_policy(&self, name: &str, yaml: &str) -> Result<()> {
        let dir = "/etc/tetragon/tetragon.tp.d";
        let write =
            format!("mkdir -p {dir} && cat > {dir}/{name}.yaml <<'LLMSC_EOF'\n{yaml}\nLLMSC_EOF",);
        let o = self.vm_sh(&write)?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "writing Tetragon policy {name}: {}",
                o.stderr.trim()
            )));
        }
        // Reload is best-effort: Tetragon may not be installed yet (scaffold).
        let _ = self.vm_sh("systemctl reload tetragon 2>/dev/null || systemctl restart tetragon 2>/dev/null || true");
        Ok(())
    }

    /// Apply many policies (one per agent). Returns the names applied.
    pub fn apply_policies(
        &self,
        policies: &[crate::tetragon::TetragonPolicy],
        reporter: &dyn Reporter,
    ) -> Result<Vec<String>> {
        let mut applied = Vec::new();
        for p in policies {
            reporter.step(&format!("Loading Tetragon policy {}", p.name));
            self.apply_policy(&p.name, &p.to_tracing_policy_yaml())?;
            applied.push(p.name.clone());
        }
        Ok(applied)
    }
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
    fn set_provider_key_writes_env_only_in_container() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        LiteLlmDeployer::new("llmsc", &r)
            .set_provider_key("anthropic", "sk-ant-123", &SilentReporter)
            .unwrap();
        assert!(r.called_with("litellm.env"));
        assert!(r.called_with("ANTHROPIC_API_KEY"));
        assert!(r.called_with("sk-ant-123"));
        assert!(r.called_with("anthropic/claude-3-5-sonnet-latest"));
        // The key is only ever written inside the svc-litellm container.
        assert!(r.called_with("svc-litellm"));
    }

    #[test]
    fn parse_key_usage_handles_array_and_wrapper() {
        let arr = r#"[{"key_alias":"llmsc-a-x","spend":1.25},{"key_alias":"","spend":9}]"#;
        let u = parse_key_usage(arr).unwrap();
        assert_eq!(u.len(), 1); // empty alias dropped
        assert_eq!(u[0].key_alias, "llmsc-a-x");
        assert_eq!(u[0].spend, 1.25);
        let wrap = r#"{"keys":[{"key_alias":"llmsc-b-y","spend":0.5}]}"#;
        assert_eq!(parse_key_usage(wrap).unwrap()[0].spend, 0.5);
    }

    #[test]
    fn seaweedfs_deploy_creates_volume_and_starts() {
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        SeaweedFsDeployer::new("llmsc", &r)
            .deploy(&SilentReporter)
            .unwrap();
        assert!(r.called_with("svc-seaweedfs"));
        assert!(r.called_with("llmsc-shared")); // shared volume
        assert!(r.called_with("source=llmsc-shared")); // attached to /data
        assert!(r.called_with("weed"));
        assert!(r.called_with("seaweedfs.service"));
    }

    #[test]
    fn grafana_stack_deploy_installs_all_three() {
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        GrafanaStackDeployer::new("llmsc", &r)
            .deploy(&SilentReporter)
            .unwrap();
        assert!(r.called_with("svc-grafana"));
        assert!(r.called_with("grafana"));
        assert!(r.called_with("VictoriaMetrics"));
        assert!(r.called_with("loki"));
        assert!(r.called_with("llmsc.yaml")); // datasource provisioning
        assert!(r.called_with("victoria-metrics loki grafana-server"));
    }

    #[test]
    fn phoenix_deploy_runs_expected_steps() {
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        PhoenixDeployer::new("llmsc", &r)
            .deploy(&SilentReporter)
            .unwrap();
        assert!(r.called_with("svc-phoenix"));
        assert!(r.called_with("arize-phoenix"));
        assert!(r.called_with("phoenix.service"));
        assert!(r.called_with("systemctl"));
    }

    #[test]
    fn enable_phoenix_writes_collector_endpoint() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        LiteLlmDeployer::new("llmsc", &r)
            .enable_phoenix("10.21.32.9", &SilentReporter)
            .unwrap();
        assert!(r.called_with("PHOENIX_COLLECTOR_ENDPOINT"));
        assert!(r.called_with("10.21.32.9"));
    }

    #[test]
    fn mitmproxy_deploy_and_allowlist_sync() {
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        let d = MitmproxyDeployer::new("llmsc", &r);
        d.deploy(&SilentReporter).unwrap();
        assert!(r.called_with("svc-mitmproxy"));
        assert!(r.called_with("mitmproxy")); // apt install + unit
        assert!(r.called_with("allowlist.py"));
        assert!(r.called_with("systemctl"));

        let r2 = FakeRunner::new(|_, _| out(0, ""));
        MitmproxyDeployer::new("llmsc", &r2)
            .sync_allowlist(&["github.com".into(), "pypi.org".into()], &SilentReporter)
            .unwrap();
        assert!(r2.called_with("allowlist.py"));
        assert!(r2.called_with("\"github.com\""));
    }

    #[test]
    fn tetragon_apply_writes_policy_and_reloads() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let pols = vec![crate::tetragon::agent_policy(
            "web-agent-01",
            "agent-claude",
            None,
        )];
        let applied = TetragonDeployer::new("llmsc", &r)
            .apply_policies(&pols, &SilentReporter)
            .unwrap();
        assert_eq!(applied, vec!["llmsc-web-agent-01-agent-claude"]);
        assert!(r.called_with("tetragon.tp.d"));
        assert!(r.called_with("TracingPolicy"));
        assert!(r.called_with("tetragon")); // reload
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
