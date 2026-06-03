//! L2 system-container operations — the `IncusClient` boundary.
//!
//! The real client (M2) talks to the Incus REST API; tests use [`FakeIncus`].

use crate::config::Sandbox;
use crate::error::{Error, Result};
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
    fn launch(&self, spec: &Sandbox) -> Result<()>;
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
    fn launch(&self, spec: &Sandbox) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        c.launch(&sb("web-agent-01")).unwrap();
        assert!(c.exists("web-agent-01").unwrap());
        assert_eq!(c.list().unwrap().len(), 1);
        c.delete("web-agent-01").unwrap();
        assert!(c.list().unwrap().is_empty());
    }

    #[test]
    fn launch_duplicate_errors() {
        let c = FakeIncus::new();
        c.launch(&sb("a")).unwrap();
        assert!(c.launch(&sb("a")).is_err());
    }

    #[test]
    fn delete_missing_is_not_found() {
        let c = FakeIncus::new();
        assert!(matches!(c.delete("nope"), Err(Error::NotFound(_))));
    }
}
