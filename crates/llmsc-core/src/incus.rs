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
