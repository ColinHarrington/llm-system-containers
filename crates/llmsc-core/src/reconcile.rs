//! Reconcile declarative config (desired state) against Incus (runtime truth).
//!
//! `Incus` is the source of truth; config is intent. We create sandboxes that are declared but
//! missing, leave declared+present ones, and **surface drift** (present in Incus but not in
//! config) without deleting — destructive actions stay explicit.

use crate::config::{Config, IncusProfile, Sandbox};
use crate::error::Result;
use crate::incus::{ConvergeOp, IncusClient, IncusProfileRecord, InstanceConfig};
use crate::progress::Reporter;
use std::collections::BTreeMap;

/// Compute the steps to converge a live Incus profile toward its TOML-owned intent (config +
/// devices only — profiles carry no `source`/`ephemeral`/profiles-of-profiles). Additive on
/// devices (in-place change is a manual remove+add); unsets/removes drift.
pub fn profile_converge_plan(
    desired: &IncusProfile,
    live: Option<&IncusProfileRecord>,
) -> Vec<ConvergeOp> {
    let empty_cfg: BTreeMap<String, String> = BTreeMap::new();
    let empty_dev: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let (lcfg, ldev) = match live {
        Some(p) => (&p.config, &p.devices),
        None => (&empty_cfg, &empty_dev),
    };
    let mut plan = Vec::new();
    for (k, v) in &desired.config {
        if lcfg.get(k) != Some(v) {
            plan.push(ConvergeOp::SetConfig {
                key: k.clone(),
                value: v.clone(),
            });
        }
    }
    for k in lcfg.keys() {
        if !desired.config.contains_key(k) {
            plan.push(ConvergeOp::UnsetConfig { key: k.clone() });
        }
    }
    for (name, keys) in &desired.devices {
        if !ldev.contains_key(name) {
            plan.push(ConvergeOp::AddDevice {
                name: name.clone(),
                keys: keys.clone(),
            });
        }
    }
    for name in ldev.keys() {
        if !desired.devices.contains_key(name) {
            plan.push(ConvergeOp::RemoveDevice { name: name.clone() });
        }
    }
    plan
}

/// Compute the steps to converge a *live* instance toward its *declared* sandbox intent.
///
/// Conservative by design: sets/updates declared config keys and unsets drifted ones (but never
/// touches read-only `image.*`); adds declared devices and removes drifted instance-local ones (by
/// name; in-place device edits are a manual remove+add for now); adds declared profiles and removes
/// drifted ones — but **never removes `default`** (it provides the base eth0/root).
pub fn converge_plan(desired: &Sandbox, live: &InstanceConfig) -> Vec<ConvergeOp> {
    let mut plan = Vec::new();

    // config
    let want = desired.effective_config();
    for (k, v) in &want {
        if live.config.get(k) != Some(v) {
            plan.push(ConvergeOp::SetConfig {
                key: k.clone(),
                value: v.clone(),
            });
        }
    }
    for k in live.config.keys() {
        if !want.contains_key(k) && !k.starts_with("image.") {
            plan.push(ConvergeOp::UnsetConfig { key: k.clone() });
        }
    }

    // devices (instance-local only; profile-inherited are not in live.local_devices)
    for (name, keys) in &desired.devices {
        if !live.local_devices.contains(name) {
            plan.push(ConvergeOp::AddDevice {
                name: name.clone(),
                keys: keys.clone(),
            });
        }
    }
    for name in &live.local_devices {
        if !desired.devices.contains_key(name) {
            plan.push(ConvergeOp::RemoveDevice { name: name.clone() });
        }
    }

    // profiles
    for p in &desired.profiles {
        if !live.profiles.contains(p) {
            plan.push(ConvergeOp::AddProfile { name: p.clone() });
        }
    }
    for p in &live.profiles {
        if p != "default" && !desired.profiles.contains(p) {
            plan.push(ConvergeOp::RemoveProfile { name: p.clone() });
        }
    }

    plan
}

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
    // Sandboxes only — service containers (svc-*) are infrastructure, not drift.
    let actual: Vec<String> = incus.sandboxes()?.into_iter().map(|i| i.name).collect();
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
            ..Default::default()
        }
    }

    fn config_with(names: &[&str]) -> Config {
        Config {
            operator: "operator".into(),
            vm: VmConfig::default(),
            sandboxes: names.iter().map(|n| sandbox(n)).collect(),
            services: vec![],
            incus_profiles: vec![],
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

    #[test]
    fn converge_plan_diffs_config_devices_profiles() {
        use crate::incus::{ConvergeOp, InstanceConfig, InstanceStatus};
        use std::collections::BTreeMap;

        let mut desired = sandbox("web-agent-01");
        desired.nesting = true; // → security.nesting=true in effective_config
        desired
            .config
            .insert("limits.processes".into(), "512".into());
        let mut work = BTreeMap::new();
        work.insert("type".into(), "disk".into());
        work.insert("source".into(), "/h/p".into());
        desired.devices.insert("work".into(), work);
        desired.profiles = vec!["sandbox".into()];

        let live = InstanceConfig {
            name: "web-agent-01".into(),
            status: InstanceStatus::Running,
            description: String::new(),
            ephemeral: false,
            profiles: vec!["default".into(), "old-profile".into()],
            // has the privileged invariant already, an image.* (read-only), and a drifted key
            config: BTreeMap::from([
                ("security.privileged".into(), "false".into()),
                ("image.os".into(), "Alpine".into()),
                ("drifted.key".into(), "x".into()),
            ]),
            devices: BTreeMap::new(),
            local_devices: vec!["stale".into()],
        };

        let plan = converge_plan(&desired, &live);
        assert!(plan.contains(&ConvergeOp::SetConfig {
            key: "security.nesting".into(),
            value: "true".into()
        }));
        assert!(plan.contains(&ConvergeOp::SetConfig {
            key: "limits.processes".into(),
            value: "512".into()
        }));
        assert!(plan.contains(&ConvergeOp::UnsetConfig {
            key: "drifted.key".into()
        }));
        // image.* and the already-correct security.privileged are left alone.
        assert!(!plan
            .iter()
            .any(|op| matches!(op, ConvergeOp::UnsetConfig { key } if key.starts_with("image."))));
        assert!(plan.contains(&ConvergeOp::RemoveDevice {
            name: "stale".into()
        }));
        assert!(plan
            .iter()
            .any(|op| matches!(op, ConvergeOp::AddDevice { name, .. } if name == "work")));
        assert!(plan.contains(&ConvergeOp::AddProfile {
            name: "sandbox".into()
        }));
        assert!(plan.contains(&ConvergeOp::RemoveProfile {
            name: "old-profile".into()
        }));
        // never removes the default profile
        assert!(!plan.contains(&ConvergeOp::RemoveProfile {
            name: "default".into()
        }));
    }

    #[test]
    fn profile_converge_plan_diffs_config_and_devices() {
        use crate::config::IncusProfile;
        use crate::incus::{ConvergeOp, IncusProfileRecord};
        use std::collections::BTreeMap;

        let desired = IncusProfile {
            name: "sandbox".into(),
            description: None,
            config: BTreeMap::from([("security.nesting".into(), "true".into())]),
            devices: BTreeMap::from([(
                "eth0".into(),
                BTreeMap::from([("type".into(), "nic".into())]),
            )]),
        };
        // Missing profile → everything is an add/set.
        let plan = profile_converge_plan(&desired, None);
        assert!(plan.contains(&ConvergeOp::SetConfig {
            key: "security.nesting".into(),
            value: "true".into()
        }));
        assert!(plan
            .iter()
            .any(|op| matches!(op, ConvergeOp::AddDevice { name, .. } if name == "eth0")));

        // Existing with drift → set the changed key, unset the extra, remove the extra device.
        let live = IncusProfileRecord {
            name: "sandbox".into(),
            description: String::new(),
            used_by: 0,
            config: BTreeMap::from([("drift".into(), "x".into())]),
            devices: BTreeMap::from([
                (
                    "eth0".into(),
                    BTreeMap::from([("type".into(), "nic".into())]),
                ),
                ("stale".into(), BTreeMap::new()),
            ]),
        };
        let plan = profile_converge_plan(&desired, Some(&live));
        assert!(plan.contains(&ConvergeOp::SetConfig {
            key: "security.nesting".into(),
            value: "true".into()
        }));
        assert!(plan.contains(&ConvergeOp::UnsetConfig {
            key: "drift".into()
        }));
        assert!(plan.contains(&ConvergeOp::RemoveDevice {
            name: "stale".into()
        }));
        // eth0 already present → not re-added.
        assert!(!plan
            .iter()
            .any(|op| matches!(op, ConvergeOp::AddDevice { name, .. } if name == "eth0")));
    }

    #[test]
    fn service_containers_are_not_drift() {
        // A provisioned service container (svc-*) must not be reported as an undeclared sandbox.
        let incus = FakeIncus::new();
        incus
            .launch(&sandbox("svc-litellm"), &SilentReporter)
            .unwrap();
        let report = reconcile(&config_with(&["a"]), &incus, &SilentReporter).unwrap();
        assert_eq!(report.created, vec!["a"]);
        assert!(
            report.extra.is_empty(),
            "service container leaked into drift"
        );
    }
}
