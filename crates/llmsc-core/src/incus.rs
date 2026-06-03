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

/// Manages L2 system containers. `&self` (real impls hit the REST API; fakes use interior mut).
pub trait IncusClient {
    fn list(&self) -> Result<Vec<Instance>>;
    fn exists(&self, name: &str) -> Result<bool>;
    /// Create the sandbox (image pull can be slow → reports progress), enabling nesting and
    /// creating its users per the spec.
    fn launch(&self, spec: &Sandbox, reporter: &dyn Reporter) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
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
}
