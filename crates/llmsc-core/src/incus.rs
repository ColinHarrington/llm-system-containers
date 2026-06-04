//! L2 system-container operations — the `IncusClient` boundary.
//!
//! The real client ([`CliIncus`]) drives `incus` inside the VM via `limactl shell` through the
//! [`CommandRunner`] boundary; tests use [`FakeIncus`]. (A native Incus REST client over the
//! socket is a future refinement — see `planning/tech-stack.md`.)

use crate::config::Sandbox;
use crate::error::{Error, Result};
use crate::process::{CommandRunner, RunOutput};
use crate::progress::Reporter;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStatus {
    Running,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub name: String,
    pub status: InstanceStatus,
}

/// A Linux user inside a sandbox — one per agent, plus the human operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopoUser {
    pub name: String,
    pub human: bool,
}

/// A sandbox enriched for the topology view, from real Incus state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxTopology {
    pub name: String,
    pub status: InstanceStatus,
    pub image: String,
    pub nesting: bool,
    pub mem_bytes: u64,
    pub users: Vec<TopoUser>,
}

/// Parse `incus list --format json` into sandbox topology rows (services excluded). Users are
/// left empty here — they require a per-container call, filled in by [`CliIncus::topology`].
pub fn parse_topology(list_json: &str) -> Result<Vec<SandboxTopology>> {
    use std::collections::HashMap;
    #[derive(serde::Deserialize)]
    struct RawMem {
        #[serde(default)]
        usage: u64,
    }
    #[derive(serde::Deserialize)]
    struct RawState {
        #[serde(default)]
        memory: Option<RawMem>,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        status: String,
        #[serde(default)]
        config: HashMap<String, String>,
        #[serde(default)]
        state: Option<RawState>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .filter(|r| !crate::service::is_service_container(&r.name))
        .map(|r| {
            let image = r
                .config
                .get("image.description")
                .cloned()
                .or_else(|| {
                    match (r.config.get("image.os"), r.config.get("image.release")) {
                        (Some(os), Some(rel)) => Some(format!("{os} {rel}")),
                        _ => None,
                    }
                })
                .unwrap_or_else(|| "—".to_string());
            SandboxTopology {
                status: if r.status == "Running" {
                    InstanceStatus::Running
                } else {
                    InstanceStatus::Stopped
                },
                nesting: r.config.get("security.nesting").map(|v| v == "true").unwrap_or(false),
                mem_bytes: r.state.and_then(|s| s.memory).map(|m| m.usage).unwrap_or(0),
                image,
                name: r.name,
                users: Vec::new(),
            }
        })
        .collect())
}

/// A managed Incus network (bridge) in the VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRecord {
    pub name: String,
    pub kind: String,
    pub ipv4: String,
    /// Whether outbound NAT to the host/internet is enabled (`ipv4.nat`).
    pub nat: bool,
    /// Number of *sandboxes* attached (instances, not profiles).
    pub used_by: usize,
}

/// A sandbox's real network attachment(s) and address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxNetwork {
    pub name: String,
    pub status: InstanceStatus,
    pub networks: Vec<String>,
    pub ipv4: String,
}

/// Parse `incus network list --format json` into managed networks (host NICs excluded).
pub fn parse_networks(list_json: &str) -> Result<Vec<NetworkRecord>> {
    use std::collections::HashMap;
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(rename = "type", default)]
        net_type: String,
        #[serde(default)]
        managed: bool,
        #[serde(default)]
        config: HashMap<String, String>,
        #[serde(default)]
        used_by: Vec<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus network list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .filter(|r| r.managed) // unmanaged = host physical NICs, not llmsc networks
        .map(|r| NetworkRecord {
            ipv4: r.config.get("ipv4.address").cloned().unwrap_or_else(|| "—".to_string()),
            nat: r.config.get("ipv4.nat").map(|v| v == "true").unwrap_or(false),
            used_by: r.used_by.iter().filter(|u| u.contains("/instances/")).count(),
            kind: r.net_type,
            name: r.name,
        })
        .collect())
}

/// Parse `incus list --format json` into per-sandbox network attachments (services excluded).
pub fn parse_sandbox_networks(list_json: &str) -> Result<Vec<SandboxNetwork>> {
    use std::collections::HashMap;
    #[derive(serde::Deserialize)]
    struct RawAddr {
        family: String,
        address: String,
        #[serde(default)]
        scope: String,
    }
    #[derive(serde::Deserialize)]
    struct RawIface {
        #[serde(default)]
        addresses: Vec<RawAddr>,
    }
    #[derive(serde::Deserialize)]
    struct RawState {
        #[serde(default)]
        network: Option<HashMap<String, RawIface>>,
    }
    #[derive(serde::Deserialize)]
    struct RawDev {
        #[serde(rename = "type", default)]
        dev_type: String,
        #[serde(default)]
        network: String,
        #[serde(default)]
        parent: String,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        status: String,
        #[serde(default)]
        expanded_devices: HashMap<String, RawDev>,
        #[serde(default)]
        state: Option<RawState>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .filter(|r| !crate::service::is_service_container(&r.name))
        .map(|r| {
            let mut networks: Vec<String> = r
                .expanded_devices
                .values()
                .filter(|d| d.dev_type == "nic")
                .map(|d| if d.network.is_empty() { d.parent.clone() } else { d.network.clone() })
                .filter(|n| !n.is_empty())
                .collect();
            networks.sort();
            networks.dedup();
            let ipv4 = r
                .state
                .and_then(|s| s.network)
                .and_then(|ifaces| {
                    ifaces
                        .into_values()
                        .flat_map(|i| i.addresses)
                        .find(|a| a.family == "inet" && a.scope == "global")
                        .map(|a| a.address)
                })
                .unwrap_or_else(|| "—".to_string());
            SandboxNetwork {
                status: if r.status == "Running" {
                    InstanceStatus::Running
                } else {
                    InstanceStatus::Stopped
                },
                networks,
                ipv4,
                name: r.name,
            }
        })
        .collect())
}

/// An Incus image — either locally cached (base/custom) or from a remote catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRecord {
    pub name: String,
    pub description: String,
    pub base: String,
    pub arch: String,
    pub size_bytes: u64,
    pub used_by: usize,
    pub uploaded: String,
}

/// Parse `incus image list [remote:] --format json` into image records.
pub fn parse_images(list_json: &str) -> Result<Vec<ImageRecord>> {
    #[derive(serde::Deserialize)]
    struct RawAlias {
        name: String,
    }
    #[derive(serde::Deserialize, Default)]
    struct RawProps {
        #[serde(default)]
        description: String,
        #[serde(default)]
        os: String,
        #[serde(default)]
        release: String,
        #[serde(default)]
        architecture: String,
    }
    #[derive(serde::Deserialize)]
    struct RawImg {
        #[serde(default)]
        fingerprint: String,
        #[serde(default)]
        aliases: Vec<RawAlias>,
        #[serde(default)]
        properties: RawProps,
        #[serde(default)]
        architecture: String,
        #[serde(default)]
        size: u64,
        #[serde(default)]
        uploaded_at: String,
        #[serde(default)]
        used_by: Vec<String>,
    }
    let raw: Vec<RawImg> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus image list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|r| {
            let name = r
                .aliases
                .first()
                .map(|a| a.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| r.fingerprint.chars().take(12).collect());
            let base = match (r.properties.os.as_str(), r.properties.release.as_str()) {
                ("", "") => "—".to_string(),
                (os, rel) => format!("{os} {rel}").trim().to_string(),
            };
            let arch = if r.properties.architecture.is_empty() {
                r.architecture
            } else {
                r.properties.architecture
            };
            ImageRecord {
                name,
                description: if r.properties.description.is_empty() {
                    "—".to_string()
                } else {
                    r.properties.description
                },
                base,
                arch: if arch.is_empty() { "—".to_string() } else { arch },
                size_bytes: r.size,
                used_by: r.used_by.len(),
                uploaded: r.uploaded_at.chars().take(10).collect(), // YYYY-MM-DD
            }
        })
        .collect())
}

/// Parse `getent passwd` output into sandbox users: real login users with uid ≥ 1000 (an agent
/// per user, plus the human `operator`). System/service accounts and nologin shells are skipped.
pub fn parse_users(passwd: &str) -> Vec<TopoUser> {
    passwd
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() < 7 {
                return None;
            }
            let (name, shell) = (f[0], f[6]);
            let uid: u32 = f[2].parse().ok()?;
            if uid < 1000 || uid >= 65000 {
                return None;
            }
            if shell.ends_with("nologin") || shell.ends_with("false") {
                return None;
            }
            Some(TopoUser {
                name: name.to_string(),
                human: name == "operator",
            })
        })
        .collect()
}

/// Manages L2 system containers. `&self` (real impls hit the REST API; fakes use interior mut).
pub trait IncusClient {
    /// All Incus instances in the VM — sandboxes **and** service containers.
    fn list(&self) -> Result<Vec<Instance>>;
    /// Only sandboxes: [`list`](Self::list) minus service containers (`svc-*`). Services are
    /// shared infrastructure, never sandboxes, so they must not appear in sandbox views.
    fn sandboxes(&self) -> Result<Vec<Instance>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|i| !crate::service::is_service_container(&i.name))
            .collect())
    }
    fn exists(&self, name: &str) -> Result<bool>;
    /// Create the sandbox (image pull can be slow → reports progress), enabling nesting and
    /// creating its users per the spec.
    fn launch(&self, spec: &Sandbox, reporter: &dyn Reporter) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
    /// Open an interactive shell in the sandbox as `user`; returns the shell's exit code.
    fn shell(&self, user: &str, name: &str) -> Result<i32>;
}

/// In-memory fake for unit tests.
#[derive(Debug, Default)]
pub struct FakeIncus {
    instances: RefCell<Vec<Instance>>,
}

impl FakeIncus {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IncusClient for FakeIncus {
    fn list(&self) -> Result<Vec<Instance>> {
        Ok(self.instances.borrow().clone())
    }
    fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.instances.borrow().iter().any(|i| i.name == name))
    }
    fn launch(&self, spec: &Sandbox, _reporter: &dyn Reporter) -> Result<()> {
        if self.exists(&spec.name)? {
            return Err(Error::Incus(format!(
                "instance already exists: {}",
                spec.name
            )));
        }
        self.instances.borrow_mut().push(Instance {
            name: spec.name.clone(),
            status: InstanceStatus::Running,
        });
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<()> {
        let mut v = self.instances.borrow_mut();
        let before = v.len();
        v.retain(|i| i.name != name);
        if v.len() == before {
            return Err(Error::NotFound(name.to_string()));
        }
        Ok(())
    }
    fn shell(&self, _user: &str, _name: &str) -> Result<i32> {
        Ok(0)
    }
}

/// Real client: drives `incus` inside the VM via `limactl shell`.
pub struct CliIncus<'a, R: CommandRunner> {
    vm: String,
    runner: &'a R,
}

impl<'a, R: CommandRunner> CliIncus<'a, R> {
    pub fn new(vm: impl Into<String>, runner: &'a R) -> Self {
        Self {
            vm: vm.into(),
            runner,
        }
    }

    /// Run `incus <args>` inside the VM, captured.
    fn incus_run(&self, args: &[&str]) -> Result<RunOutput> {
        let mut full = vec!["shell", self.vm.as_str(), "sudo", "incus"];
        full.extend_from_slice(args);
        self.runner.run("limactl", &full)
    }

    /// Run `incus <args>` inside the VM with streamed output (for slow ops like image pulls).
    fn incus_streamed(&self, args: &[&str]) -> Result<i32> {
        let mut full = vec!["shell", self.vm.as_str(), "sudo", "incus"];
        full.extend_from_slice(args);
        self.runner.run_streamed("limactl", &full)
    }

    /// Real topology: every sandbox (services excluded) with its status, image, nesting flag,
    /// memory use, and Linux users (one per agent + the human operator). Users come from
    /// `getent passwd` inside each running sandbox; stopped sandboxes report none.
    pub fn topology(&self) -> Result<Vec<SandboxTopology>> {
        let o = self.incus_run(&["list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("list: {}", o.stderr.trim())));
        }
        let mut sandboxes = parse_topology(&o.stdout)?;
        for sb in sandboxes.iter_mut() {
            if sb.status != InstanceStatus::Running {
                continue;
            }
            if let Ok(u) = self.incus_run(&["exec", &sb.name, "--", "getent", "passwd"]) {
                if u.ok() {
                    sb.users = parse_users(&u.stdout);
                }
            }
        }
        Ok(sandboxes)
    }

    /// Images cached locally in the VM (base distros pulled on first use + custom builds).
    pub fn images(&self) -> Result<Vec<ImageRecord>> {
        let o = self.incus_run(&["image", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("image list: {}", o.stderr.trim())));
        }
        parse_images(&o.stdout)
    }

    /// All images available from a remote catalog (e.g. `images:`). Hits the network and can
    /// return a large list — callers should fetch this on demand, not on every refresh.
    pub fn images_remote(&self, remote: &str) -> Result<Vec<ImageRecord>> {
        let target = format!("{remote}:");
        let o = self.incus_run(&["image", "list", &target, "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("image list {target}: {}", o.stderr.trim())));
        }
        parse_images(&o.stdout)
    }

    /// Managed Incus networks (bridges) in the VM.
    pub fn networks(&self) -> Result<Vec<NetworkRecord>> {
        let o = self.incus_run(&["network", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("network list: {}", o.stderr.trim())));
        }
        parse_networks(&o.stdout)
    }

    /// Per-sandbox network attachments and addresses (services excluded).
    pub fn sandbox_networks(&self) -> Result<Vec<SandboxNetwork>> {
        let o = self.incus_run(&["list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("list: {}", o.stderr.trim())));
        }
        parse_sandbox_networks(&o.stdout)
    }
}

impl<R: CommandRunner> IncusClient for CliIncus<'_, R> {
    fn list(&self) -> Result<Vec<Instance>> {
        let o = self.incus_run(&["list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("list: {}", o.stderr.trim())));
        }
        #[derive(serde::Deserialize)]
        struct Raw {
            name: String,
            status: String,
        }
        let raw: Vec<Raw> = serde_json::from_str(&o.stdout)
            .map_err(|e| Error::Incus(format!("parsing `incus list` output: {e}")))?;
        Ok(raw
            .into_iter()
            .map(|r| Instance {
                status: if r.status == "Running" {
                    InstanceStatus::Running
                } else {
                    InstanceStatus::Stopped
                },
                name: r.name,
            })
            .collect())
    }

    fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.incus_run(&["info", name])?.ok())
    }

    fn launch(&self, spec: &Sandbox, reporter: &dyn Reporter) -> Result<()> {
        if self.exists(&spec.name)? {
            return Err(Error::Incus(format!(
                "instance already exists: {}",
                spec.name
            )));
        }
        reporter.step(&format!(
            "Creating sandbox '{}' from {}",
            spec.name, spec.image
        ));
        let code = self.incus_streamed(&["launch", &spec.image, &spec.name])?;
        if code != 0 {
            return Err(Error::Incus(format!("`incus launch` exited with {code}")));
        }
        if spec.nesting {
            reporter.step("Enabling nested containers (L3)");
            let o = self.incus_run(&["config", "set", &spec.name, "security.nesting", "true"])?;
            if !o.ok() {
                return Err(Error::Incus(format!(
                    "setting security.nesting: {}",
                    o.stderr.trim()
                )));
            }
        }
        for u in &spec.users {
            reporter.step(&format!("Creating user '{}'", u.name));
            // "operator" collides with the system group of the same name → fall back to `-g users`.
            let useradd = format!(
                "id {u} 2>/dev/null || useradd -m -s /bin/bash {u} || useradd -m -s /bin/bash -g users {u}",
                u = u.name
            );
            let o = self.incus_run(&["exec", &spec.name, "--", "bash", "-lc", &useradd])?;
            if !o.ok() {
                return Err(Error::Incus(format!(
                    "creating user {}: {}",
                    u.name,
                    o.stderr.trim()
                )));
            }
        }
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let o = self.incus_run(&["delete", "--force", name])?;
        if !o.ok() {
            return Err(Error::Incus(format!("delete: {}", o.stderr.trim())));
        }
        Ok(())
    }

    fn shell(&self, user: &str, name: &str) -> Result<i32> {
        // `-t` forces a pseudo-terminal; limactl shell passes our stdio through for interactivity.
        self.incus_streamed(&["exec", "-t", name, "--", "su", "-", user])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::SilentReporter;

    fn sb(name: &str) -> Sandbox {
        Sandbox {
            name: name.into(),
            image: "images:debian/13".into(),
            nesting: true,
            users: vec![],
        }
    }

    #[test]
    fn launch_list_delete() {
        let c = FakeIncus::new();
        assert!(c.list().unwrap().is_empty());
        c.launch(&sb("web-agent-01"), &SilentReporter).unwrap();
        assert!(c.exists("web-agent-01").unwrap());
        assert_eq!(c.list().unwrap().len(), 1);
        c.delete("web-agent-01").unwrap();
        assert!(c.list().unwrap().is_empty());
    }

    #[test]
    fn launch_duplicate_errors() {
        let c = FakeIncus::new();
        c.launch(&sb("a"), &SilentReporter).unwrap();
        assert!(c.launch(&sb("a"), &SilentReporter).is_err());
    }

    #[test]
    fn delete_missing_is_not_found() {
        let c = FakeIncus::new();
        assert!(matches!(c.delete("nope"), Err(Error::NotFound(_))));
    }

    #[test]
    fn parse_topology_extracts_real_fields_and_excludes_services() {
        let json = r#"[
          {"name":"web-agent-01","status":"Running",
           "config":{"image.description":"Ubuntu 24.04 LTS","security.nesting":"true"},
           "state":{"memory":{"usage":2147483648}}},
          {"name":"scratch-01","status":"Stopped",
           "config":{"image.os":"Alpine","image.release":"3.21"}},
          {"name":"svc-litellm","status":"Running","config":{},"state":{"memory":{"usage":1}}}
        ]"#;
        let t = parse_topology(json).unwrap();
        assert_eq!(t.len(), 2, "service container must be excluded");
        let web = &t[0];
        assert_eq!(web.name, "web-agent-01");
        assert_eq!(web.status, InstanceStatus::Running);
        assert_eq!(web.image, "Ubuntu 24.04 LTS");
        assert!(web.nesting);
        assert_eq!(web.mem_bytes, 2147483648);
        let scratch = &t[1];
        assert_eq!(scratch.image, "Alpine 3.21");
        assert!(!scratch.nesting);
    }

    #[test]
    fn parse_images_extracts_alias_base_and_usage() {
        let json = r#"[
          {"fingerprint":"abc123def456789","aliases":[{"name":"dev-ubuntu-24.04"}],
           "properties":{"description":"Ubuntu 24.04 LTS","os":"Ubuntu","release":"24.04","architecture":"amd64"},
           "size":1503238553,"uploaded_at":"2026-05-30T10:00:00Z",
           "used_by":["/1.0/instances/web-agent-01","/1.0/instances/ci-runner"]},
          {"fingerprint":"deadbeefcafebabe1234","aliases":[],
           "properties":{"os":"Alpine","release":"3.21"},"size":3500000,"uploaded_at":"","used_by":[]}
        ]"#;
        let imgs = parse_images(json).unwrap();
        assert_eq!(imgs.len(), 2);
        assert_eq!(imgs[0].name, "dev-ubuntu-24.04");
        assert_eq!(imgs[0].base, "Ubuntu 24.04");
        assert_eq!(imgs[0].arch, "amd64");
        assert_eq!(imgs[0].used_by, 2);
        assert_eq!(imgs[0].uploaded, "2026-05-30");
        // No alias → falls back to the (truncated) fingerprint.
        assert_eq!(imgs[1].name, "deadbeefcafe");
        assert_eq!(imgs[1].description, "—");
    }

    #[test]
    fn parse_networks_keeps_managed_only_with_nat_and_usage() {
        let json = r#"[
          {"name":"incusbr0","type":"bridge","managed":true,
           "config":{"ipv4.address":"10.71.0.1/24","ipv4.nat":"true"},
           "used_by":["/1.0/instances/web-agent-01","/1.0/profiles/default"]},
          {"name":"eth0","type":"physical","managed":false,"config":{},"used_by":[]}
        ]"#;
        let nets = parse_networks(json).unwrap();
        assert_eq!(nets.len(), 1, "unmanaged host NIC must be excluded");
        assert_eq!(nets[0].name, "incusbr0");
        assert_eq!(nets[0].ipv4, "10.71.0.1/24");
        assert!(nets[0].nat);
        assert_eq!(nets[0].used_by, 1, "profiles must not count as sandboxes");
    }

    #[test]
    fn parse_sandbox_networks_extracts_attachment_and_ip() {
        let json = r#"[
          {"name":"web-agent-01","status":"Running",
           "expanded_devices":{"eth0":{"type":"nic","network":"incusbr0","name":"eth0"},
                               "root":{"type":"disk","network":""}},
           "state":{"network":{"eth0":{"addresses":[
             {"family":"inet","address":"127.0.0.1","scope":"local"},
             {"family":"inet","address":"10.71.0.20","scope":"global"},
             {"family":"inet6","address":"fe80::1","scope":"link"}]},
             "lo":{"addresses":[]}}}},
          {"name":"svc-litellm","status":"Running","expanded_devices":{},"state":null}
        ]"#;
        let sbs = parse_sandbox_networks(json).unwrap();
        assert_eq!(sbs.len(), 1, "service container must be excluded");
        assert_eq!(sbs[0].name, "web-agent-01");
        assert_eq!(sbs[0].networks, vec!["incusbr0"]);
        assert_eq!(sbs[0].ipv4, "10.71.0.20");
    }

    #[test]
    fn parse_users_keeps_agents_and_operator_only() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\n\
                      daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n\
                      operator:x:1000:1000::/home/operator:/bin/bash\n\
                      agent-claude:x:1001:1001::/home/agent-claude:/bin/bash\n\
                      svc:x:999:999::/:/usr/sbin/nologin\n\
                      nobody:x:65534:65534::/:/usr/sbin/nologin";
        let users = parse_users(passwd);
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "operator");
        assert!(users[0].human);
        assert_eq!(users[1].name, "agent-claude");
        assert!(!users[1].human);
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use crate::config::{User, UserRole};
    use crate::process::{out, FakeRunner};
    use crate::progress::SilentReporter;

    fn spec() -> Sandbox {
        Sandbox {
            name: "web-agent-01".into(),
            image: "images:debian/13".into(),
            nesting: true,
            users: vec![
                User {
                    name: "agent-claude".into(),
                    role: UserRole::Agent,
                },
                User {
                    name: "operator".into(),
                    role: UserRole::Human,
                },
            ],
        }
    }

    #[test]
    fn launch_creates_sets_nesting_and_users() {
        // info -> non-zero (does not exist) so launch proceeds; everything else ok.
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        CliIncus::new("llmsc", &r)
            .launch(&spec(), &SilentReporter)
            .unwrap();
        assert!(r.called_with("launch"));
        assert!(r.called_with("security.nesting"));
        assert!(r.called_with("useradd"));
        assert!(r.called_with("agent-claude"));
    }

    #[test]
    fn launch_refuses_existing() {
        let r = FakeRunner::new(|_, _| out(0, "")); // info ok -> exists
        assert!(CliIncus::new("llmsc", &r)
            .launch(&spec(), &SilentReporter)
            .is_err());
    }

    #[test]
    fn list_parses_json() {
        let json =
            r#"[{"name":"web-agent-01","status":"Running"},{"name":"ci","status":"Stopped"}]"#;
        let r = FakeRunner::new(move |_, _| out(0, json));
        let items = CliIncus::new("llmsc", &r).list().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "web-agent-01");
        assert_eq!(items[0].status, InstanceStatus::Running);
        assert_eq!(items[1].status, InstanceStatus::Stopped);
    }

    #[test]
    fn delete_calls_incus_delete() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        CliIncus::new("llmsc", &r).delete("web-agent-01").unwrap();
        assert!(r.called_with("delete"));
    }

    #[test]
    fn shell_execs_su_as_user() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        CliIncus::new("llmsc", &r)
            .shell("agent-claude", "web-agent-01")
            .unwrap();
        assert!(r.called_with("exec"));
        assert!(r.called_with("su"));
        assert!(r.called_with("agent-claude"));
        assert!(r.called_with("web-agent-01"));
    }
}
