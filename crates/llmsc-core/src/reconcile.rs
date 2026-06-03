//! Reconcile declarative config (desired state) against Incus (runtime truth).
//!
//! `Incus` is the source of truth; config is intent. We create sandboxes that are declared but
//! missing, leave declared+present ones, and **surface drift** (present in Incus but not in
//! config) without deleting — destructive actions stay explicit.

use crate::config::Config;
use crate::error::Result;
use crate::incus::IncusClient;
use crate::progress::Reporter;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// Declared sandboxes that were created.
    pub created: Vec<String>,
    /// Declared sandboxes already present.
    pub existing: Vec<String>,
    /// Present in Incus but not declared in config (drift — left untouched).
    pub extra: Vec<String>,
}

/// Bring Incus in line with the declared sandboxes (additively).
pub fn reconcile(
    config: &Config,
    incus: &dyn IncusClient,
    reporter: &dyn Reporter,
) -> Result<ReconcileReport> {
    let actual: Vec<String> = incus.list()?.into_iter().map(|i| i.name).collect();
    let mut report = ReconcileReport::default();

    for sandbox in &config.sandboxes {
        if actual.contains(&sandbox.name) {
            report.existing.push(sandbox.name.clone());
        } else {
            incus.launch(sandbox, reporter)?;
            report.created.push(sandbox.name.clone());
        }
    }

    let desired: Vec<&String> = config.sandboxes.iter().map(|s| &s.name).collect();
    for name in &actual {
        if !desired.contains(&name) {
            report.extra.push(name.clone());
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Sandbox, VmConfig};
    use crate::incus::FakeIncus;
    use crate::progress::SilentReporter;

    fn sandbox(name: &str) -> Sandbox {
        Sandbox {
            name: name.into(),
            image: "images:debian/13".into(),
            nesting: false,
            users: vec![],
        }
    }

    fn config_with(names: &[&str]) -> Config {
        Config {
            vm: VmConfig::default(),
            sandboxes: names.iter().map(|n| sandbox(n)).collect(),
        }
    }

    #[test]
    fn creates_missing() {
        let incus = FakeIncus::new();
        let report = reconcile(&config_with(&["a", "b"]), &incus, &SilentReporter).unwrap();
        assert_eq!(report.created, vec!["a", "b"]);
        assert!(report.existing.is_empty());
        assert_eq!(incus.list().unwrap().len(), 2);
    }

    #[test]
    fn is_idempotent() {
        let incus = FakeIncus::new();
        let cfg = config_with(&["a", "b"]);
        reconcile(&cfg, &incus, &SilentReporter).unwrap();
        let second = reconcile(&cfg, &incus, &SilentReporter).unwrap();
        assert!(second.created.is_empty());
        assert_eq!(second.existing, vec!["a", "b"]);
    }

    #[test]
    fn reports_extra_drift() {
        let incus = FakeIncus::new();
        incus.launch(&sandbox("rogue"), &SilentReporter).unwrap();
        let report = reconcile(&config_with(&["a"]), &incus, &SilentReporter).unwrap();
        assert_eq!(report.created, vec!["a"]);
        assert_eq!(report.extra, vec!["rogue"]);
    }
}
