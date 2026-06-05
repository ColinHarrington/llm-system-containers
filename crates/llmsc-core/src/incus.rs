//! L2 system-container operations — the `IncusClient` boundary.
//!
//! The real client ([`CliIncus`]) drives `incus` inside the VM via `limactl shell` through the
//! [`CommandRunner`] boundary; tests use [`FakeIncus`]. (A native Incus REST client over the
//! socket is a future refinement — see `planning/tech-stack.md`.)

use crate::config::{Sandbox, UserRole};
use crate::error::{Error, Result};
use crate::process::{CommandRunner, RunOutput};
use crate::progress::Reporter;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStatus {
    Running,
    Stopped,
}

/// Deserialize a field that may be `null` (or absent) as `T::default()`. Incus `--format json`
/// emits `null` rather than `[]`/`{}` for empty collections in places (notably remote image
/// `aliases`/`used_by`), which plain `#[serde(default)]` does not tolerate.
fn null_default<'de, D, T>(de: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    use serde::Deserialize;
    Ok(Option::<T>::deserialize(de)?.unwrap_or_default())
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
        #[serde(default, deserialize_with = "null_default")]
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
                .or_else(
                    || match (r.config.get("image.os"), r.config.get("image.release")) {
                        (Some(os), Some(rel)) => Some(format!("{os} {rel}")),
                        _ => None,
                    },
                )
                .unwrap_or_else(|| "—".to_string());
            SandboxTopology {
                status: if r.status == "Running" {
                    InstanceStatus::Running
                } else {
                    InstanceStatus::Stopped
                },
                nesting: r
                    .config
                    .get("security.nesting")
                    .map(|v| v == "true")
                    .unwrap_or(false),
                mem_bytes: r.state.and_then(|s| s.memory).map(|m| m.usage).unwrap_or(0),
                image,
                name: r.name,
                users: Vec::new(),
            }
        })
        .collect())
}

/// Shell snippet that creates Linux user `name` if absent. Tries `useradd` (debian/fedora/…)
/// then `adduser -D` (alpine/busybox); `-g users` covers names that collide with a system group
/// (e.g. "operator"). POSIX `sh`-safe.
/// Render the `incus launch` argv for a sandbox spec: image + name, then the Incus surface as
/// flags (`--ephemeral`, `--description`, `-p` profiles, `-c` effective config incl. the
/// `security.privileged=false` invariant + nesting, `-d` devices). CLI flags are the documented
/// thin convenience subset of the same `InstancesPost` struct (see the research note).
pub fn launch_args(spec: &Sandbox) -> Vec<String> {
    let mut a: Vec<String> = vec!["launch".into(), spec.image.clone(), spec.name.clone()];
    if spec.ephemeral {
        a.push("--ephemeral".into());
    }
    if let Some(d) = spec.description.as_deref().filter(|d| !d.is_empty()) {
        a.push("--description".into());
        a.push(d.to_string());
    }
    for p in &spec.profiles {
        a.push("-p".into());
        a.push(p.clone());
    }
    for (k, v) in spec.effective_config() {
        a.push("-c".into());
        a.push(format!("{k}={v}"));
    }
    for (name, keys) in &spec.devices {
        // -d name,type=<t>,key=value,…  (device type first if present)
        let mut parts = vec![name.clone()];
        if let Some(t) = keys.get("type") {
            parts.push(format!("type={t}"));
        }
        for (k, v) in keys {
            if k != "type" {
                parts.push(format!("{k}={v}"));
            }
        }
        a.push("-d".into());
        a.push(parts.join(","));
    }
    a
}

fn role_word(role: UserRole) -> &'static str {
    match role {
        UserRole::Human => "human",
        UserRole::Agent => "agent",
    }
}

pub fn useradd_script(name: &str) -> String {
    format!(
        "id {n} 2>/dev/null || useradd -m -s /bin/bash {n} || useradd -m -s /bin/bash -g users {n} || adduser -D {n}",
        n = name
    )
}

/// Sanitize an image alias into a valid temporary builder *container* name (`build-<slug>`).
/// Container names allow only letters, digits, and hyphens.
pub fn builder_name(alias: &str) -> String {
    let mut s: String = alias
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    s.truncate(48);
    let s = s.trim_matches('-');
    let s = if s.is_empty() { "img" } else { s };
    format!("build-{s}")
}

/// An instance snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub created: String,
    pub stateful: bool,
}

/// Parse the named instance's snapshots from `incus list <name> --format json`.
pub fn parse_snapshots(list_json: &str, instance: &str) -> Result<Vec<Snapshot>> {
    #[derive(serde::Deserialize)]
    struct RawSnap {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        created_at: String,
        #[serde(default, deserialize_with = "null_default")]
        stateful: bool,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        snapshots: Vec<RawSnap>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus list` output: {e}")))?;
    let inst = raw
        .into_iter()
        .find(|r| r.name == instance)
        .ok_or_else(|| Error::NotFound(format!("instance '{instance}'")))?;
    Ok(inst
        .snapshots
        .into_iter()
        .map(|s| Snapshot {
            // snapshot names may be "<instance>/<snap>" — keep the short name for commands.
            name: s.name.rsplit('/').next().unwrap_or(&s.name).to_string(),
            created: s.created_at.chars().take(10).collect(),
            stateful: s.stateful,
        })
        .collect())
}

/// One step in converging a live instance toward its declared intent (see `reconcile::converge_plan`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvergeOp {
    SetConfig {
        key: String,
        value: String,
    },
    UnsetConfig {
        key: String,
    },
    AddDevice {
        name: String,
        keys: BTreeMap<String, String>,
    },
    RemoveDevice {
        name: String,
    },
    AddProfile {
        name: String,
    },
    RemoveProfile {
        name: String,
    },
}

/// One step in converging a network ACL toward its compiled intent (see `enforce::egress_acl_plan`).
/// Scoped to a single named ACL — `apply_egress` takes the name once. We only manage `egress`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AclOp {
    /// Create the ACL (no-op if it already exists).
    Create,
    /// Add a rule in the given direction (e.g. `egress`).
    AddRule { direction: String, rule: AclRule },
    /// Remove a matching rule in the given direction.
    RemoveRule { direction: String, rule: AclRule },
}

/// A live instance's Incus surface, read back from the server (the round-trip view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceConfig {
    pub name: String,
    pub status: InstanceStatus,
    pub description: String,
    pub ephemeral: bool,
    pub profiles: Vec<String>,
    /// Instance-local `config` keys (`volatile.*` filtered out — never surfaced).
    pub config: BTreeMap<String, String>,
    /// Effective devices (expanded — includes profile-provided eth0/root plus instance-local).
    pub devices: BTreeMap<String, BTreeMap<String, String>>,
    /// Names of instance-local devices (the removable subset; profile-inherited ones are not).
    pub local_devices: Vec<String>,
}

/// Parse `incus list <name> --format json` into the named instance's [`InstanceConfig`].
pub fn parse_instance(list_json: &str, name: &str) -> Result<InstanceConfig> {
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        status: String,
        #[serde(default, deserialize_with = "null_default")]
        description: String,
        #[serde(default, deserialize_with = "null_default")]
        ephemeral: bool,
        #[serde(default, deserialize_with = "null_default")]
        profiles: Vec<String>,
        #[serde(default, deserialize_with = "null_default")]
        config: BTreeMap<String, String>,
        #[serde(default, deserialize_with = "null_default")]
        expanded_devices: BTreeMap<String, BTreeMap<String, String>>,
        #[serde(default, deserialize_with = "null_default")]
        devices: BTreeMap<String, BTreeMap<String, String>>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus list` output: {e}")))?;
    let inst = raw
        .into_iter()
        .find(|r| r.name == name)
        .ok_or_else(|| Error::NotFound(format!("instance '{name}'")))?;
    Ok(InstanceConfig {
        local_devices: inst.devices.keys().cloned().collect(),
        status: if inst.status == "Running" {
            InstanceStatus::Running
        } else {
            InstanceStatus::Stopped
        },
        config: inst
            .config
            .into_iter()
            .filter(|(k, _)| !k.starts_with("volatile."))
            .collect(),
        devices: inst.expanded_devices,
        description: inst.description,
        ephemeral: inst.ephemeral,
        profiles: inst.profiles,
        name: inst.name,
    })
}

/// An Incus project (its config: features / limits / restrictions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub name: String,
    pub description: String,
    pub used_by: usize,
    pub config: BTreeMap<String, String>,
}

/// Parse `incus project list --format json` into project records.
pub fn parse_projects(list_json: &str) -> Result<Vec<ProjectRecord>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        description: String,
        #[serde(default, deserialize_with = "null_default")]
        config: BTreeMap<String, String>,
        #[serde(default, deserialize_with = "null_default")]
        used_by: Vec<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus project list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|r| ProjectRecord {
            used_by: r.used_by.len(),
            name: r.name,
            description: r.description,
            config: r.config,
        })
        .collect())
}

/// A custom storage volume in a pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageVolume {
    pub name: String,
    pub vtype: String,
    pub used_by: usize,
    pub config: BTreeMap<String, String>,
}

/// An Incus storage pool (and its custom volumes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePool {
    pub name: String,
    pub driver: String,
    pub description: String,
    pub used_by: usize,
    pub config: BTreeMap<String, String>,
    pub volumes: Vec<StorageVolume>,
}

/// Parse `incus storage list --format json` into pools (volumes filled separately).
pub fn parse_storage_pools(list_json: &str) -> Result<Vec<StoragePool>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        driver: String,
        #[serde(default, deserialize_with = "null_default")]
        description: String,
        #[serde(default, deserialize_with = "null_default")]
        config: BTreeMap<String, String>,
        #[serde(default, deserialize_with = "null_default")]
        used_by: Vec<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus storage list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|r| StoragePool {
            used_by: r.used_by.len(),
            name: r.name,
            driver: r.driver,
            description: r.description,
            config: r.config,
            volumes: Vec::new(),
        })
        .collect())
}

/// Parse `incus storage volume list <pool> --format json` into **custom** volumes only
/// (instance/image-backing volumes are infrastructure, not user data).
pub fn parse_storage_volumes(list_json: &str) -> Result<Vec<StorageVolume>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(rename = "type", default, deserialize_with = "null_default")]
        vtype: String,
        #[serde(default, deserialize_with = "null_default")]
        config: BTreeMap<String, String>,
        #[serde(default, deserialize_with = "null_default")]
        used_by: Vec<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus storage volume list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .filter(|r| r.vtype == "custom")
        .map(|r| StorageVolume {
            used_by: r.used_by.len(),
            name: r.name,
            vtype: r.vtype,
            config: r.config,
        })
        .collect())
}

/// An Incus profile (a reusable bundle of `config` + `devices`) in the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncusProfileRecord {
    pub name: String,
    pub description: String,
    /// Number of instances using this profile.
    pub used_by: usize,
    pub config: BTreeMap<String, String>,
    pub devices: BTreeMap<String, BTreeMap<String, String>>,
}

/// Parse `incus profile list --format json` into profile records.
pub fn parse_incus_profiles(list_json: &str) -> Result<Vec<IncusProfileRecord>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        description: String,
        #[serde(default, deserialize_with = "null_default")]
        config: BTreeMap<String, String>,
        #[serde(default, deserialize_with = "null_default")]
        devices: BTreeMap<String, BTreeMap<String, String>>,
        #[serde(default, deserialize_with = "null_default")]
        used_by: Vec<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus profile list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|r| IncusProfileRecord {
            used_by: r.used_by.len(),
            name: r.name,
            description: r.description,
            config: r.config,
            devices: r.devices,
        })
        .collect())
}

/// One rule in a network ACL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclRule {
    pub action: String,
    pub source: String,
    pub destination: String,
    pub protocol: String,
    pub port: String,
    pub description: String,
}

/// A network ACL (named allow/deny ruleset applied to nics — the egress-policy layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkAcl {
    pub name: String,
    pub description: String,
    pub used_by: usize,
    pub ingress: Vec<AclRule>,
    pub egress: Vec<AclRule>,
}

/// Parse `incus network acl list --format json` into ACLs with their rules.
pub fn parse_network_acls(list_json: &str) -> Result<Vec<NetworkAcl>> {
    #[derive(serde::Deserialize)]
    struct RawRule {
        #[serde(default, deserialize_with = "null_default")]
        action: String,
        #[serde(default, deserialize_with = "null_default")]
        source: String,
        #[serde(default, deserialize_with = "null_default")]
        destination: String,
        #[serde(default, deserialize_with = "null_default")]
        protocol: String,
        #[serde(default, deserialize_with = "null_default")]
        destination_port: String,
        #[serde(default, deserialize_with = "null_default")]
        description: String,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        #[serde(default, deserialize_with = "null_default")]
        description: String,
        #[serde(default, deserialize_with = "null_default")]
        ingress: Vec<RawRule>,
        #[serde(default, deserialize_with = "null_default")]
        egress: Vec<RawRule>,
        #[serde(default, deserialize_with = "null_default")]
        used_by: Vec<String>,
    }
    let map = |r: RawRule| AclRule {
        action: r.action,
        source: r.source,
        destination: r.destination,
        protocol: r.protocol,
        port: r.destination_port,
        description: r.description,
    };
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus network acl list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|r| NetworkAcl {
            used_by: r.used_by.len(),
            name: r.name,
            description: r.description,
            ingress: r.ingress.into_iter().map(map).collect(),
            egress: r.egress.into_iter().map(map).collect(),
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
        #[serde(rename = "type", default, deserialize_with = "null_default")]
        net_type: String,
        #[serde(default, deserialize_with = "null_default")]
        managed: bool,
        #[serde(default, deserialize_with = "null_default")]
        config: HashMap<String, String>,
        #[serde(default, deserialize_with = "null_default")]
        used_by: Vec<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(list_json)
        .map_err(|e| Error::Incus(format!("parsing `incus network list` output: {e}")))?;
    Ok(raw
        .into_iter()
        .filter(|r| r.managed) // unmanaged = host physical NICs, not llmsc networks
        .map(|r| NetworkRecord {
            ipv4: r
                .config
                .get("ipv4.address")
                .cloned()
                .unwrap_or_else(|| "—".to_string()),
            nat: r
                .config
                .get("ipv4.nat")
                .map(|v| v == "true")
                .unwrap_or(false),
            used_by: r
                .used_by
                .iter()
                .filter(|u| u.contains("/instances/"))
                .count(),
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
        #[serde(default, deserialize_with = "null_default")]
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
        #[serde(default, deserialize_with = "null_default")]
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
                .map(|d| {
                    if d.network.is_empty() {
                        d.parent.clone()
                    } else {
                        d.network.clone()
                    }
                })
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
    /// Distro family (os) — e.g. "Debian", "Ubuntu" — used to group/sort the catalog.
    pub flavor: String,
    pub base: String,
    pub arch: String,
    /// "container" or "virtual-machine".
    pub kind: String,
    pub size_bytes: u64,
    pub used_by: usize,
    pub uploaded: String,
}

/// Parse `incus image list [remote:] --format json` into image records.
pub fn parse_images(list_json: &str) -> Result<Vec<ImageRecord>> {
    #[derive(serde::Deserialize)]
    struct RawAlias {
        #[serde(default, deserialize_with = "null_default")]
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
        #[serde(default, deserialize_with = "null_default")]
        fingerprint: String,
        #[serde(default, deserialize_with = "null_default")]
        aliases: Vec<RawAlias>,
        #[serde(default, deserialize_with = "null_default")]
        properties: RawProps,
        #[serde(default, deserialize_with = "null_default")]
        architecture: String,
        #[serde(rename = "type", default, deserialize_with = "null_default")]
        kind: String,
        #[serde(default, deserialize_with = "null_default")]
        size: u64,
        #[serde(default, deserialize_with = "null_default")]
        uploaded_at: String,
        #[serde(default, deserialize_with = "null_default")]
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
            let os = r.properties.os.clone();
            let base = match (os.as_str(), r.properties.release.as_str()) {
                ("", "") => "—".to_string(),
                (o, rel) => format!("{o} {rel}").trim().to_string(),
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
                flavor: if os.is_empty() {
                    "Other".to_string()
                } else {
                    os
                },
                base,
                arch: if arch.is_empty() {
                    "—".to_string()
                } else {
                    arch
                },
                kind: if r.kind.is_empty() {
                    "container".to_string()
                } else {
                    r.kind
                },
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
            if !(1000..65000).contains(&uid) {
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
            return Err(Error::Incus(format!(
                "image list {target}: {}",
                o.stderr.trim()
            )));
        }
        parse_images(&o.stdout)
    }

    /// Set one instance config key (`incus config set <name> <key> <value>`).
    pub fn set_config(&self, name: &str, key: &str, value: &str) -> Result<()> {
        let o = self.incus_run(&["config", "set", name, key, value])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "config set {key}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Unset one instance config key (`incus config unset`).
    pub fn unset_config(&self, name: &str, key: &str) -> Result<()> {
        let o = self.incus_run(&["config", "unset", name, key])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "config unset {key}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Add a device to an instance (`incus config device add <name> <dev> <type> key=value…`).
    pub fn add_device(&self, name: &str, dev: &str, keys: &BTreeMap<String, String>) -> Result<()> {
        let dtype = keys.get("type").cloned().unwrap_or_else(|| "disk".into());
        let mut args: Vec<String> = vec![
            "config".into(),
            "device".into(),
            "add".into(),
            name.into(),
            dev.into(),
            dtype,
        ];
        for (k, v) in keys {
            if k != "type" {
                args.push(format!("{k}={v}"));
            }
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = self.incus_run(&argv)?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "device add {dev}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Remove an instance-local device (`incus config device remove`).
    pub fn remove_device(&self, name: &str, dev: &str) -> Result<()> {
        let o = self.incus_run(&["config", "device", "remove", name, dev])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "device remove {dev}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Apply a profile to an instance (`incus profile add <instance> <profile>`).
    pub fn add_profile(&self, name: &str, profile: &str) -> Result<()> {
        let o = self.incus_run(&["profile", "add", name, profile])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "profile add {profile}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Remove a profile from an instance (`incus profile remove`).
    pub fn remove_profile(&self, name: &str, profile: &str) -> Result<()> {
        let o = self.incus_run(&["profile", "remove", name, profile])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "profile remove {profile}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Apply a converge plan (from `reconcile::converge_plan`) to a live instance, step by step.
    pub fn converge(&self, name: &str, plan: &[ConvergeOp], reporter: &dyn Reporter) -> Result<()> {
        for op in plan {
            match op {
                ConvergeOp::SetConfig { key, value } => {
                    reporter.step(&format!("set {key}={value}"));
                    self.set_config(name, key, value)?;
                }
                ConvergeOp::UnsetConfig { key } => {
                    reporter.step(&format!("unset {key}"));
                    self.unset_config(name, key)?;
                }
                ConvergeOp::AddDevice { name: dev, keys } => {
                    reporter.step(&format!("add device {dev}"));
                    self.add_device(name, dev, keys)?;
                }
                ConvergeOp::RemoveDevice { name: dev } => {
                    reporter.step(&format!("remove device {dev}"));
                    self.remove_device(name, dev)?;
                }
                ConvergeOp::AddProfile { name: p } => {
                    reporter.step(&format!("apply profile {p}"));
                    self.add_profile(name, p)?;
                }
                ConvergeOp::RemoveProfile { name: p } => {
                    reporter.step(&format!("remove profile {p}"));
                    self.remove_profile(name, p)?;
                }
            }
        }
        Ok(())
    }

    /// List an instance's snapshots.
    pub fn snapshots(&self, instance: &str) -> Result<Vec<Snapshot>> {
        let o = self.incus_run(&["list", instance, "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "list {instance}: {}",
                o.stderr.trim()
            )));
        }
        parse_snapshots(&o.stdout, instance)
    }

    /// Take a snapshot of an instance.
    pub fn snapshot_create(&self, instance: &str, name: &str) -> Result<()> {
        let o = self.incus_run(&["snapshot", "create", instance, name])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "snapshot create {name}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Restore an instance to a snapshot.
    pub fn snapshot_restore(&self, instance: &str, name: &str) -> Result<()> {
        let o = self.incus_run(&["snapshot", "restore", instance, name])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "snapshot restore {name}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Delete a snapshot.
    pub fn snapshot_delete(&self, instance: &str, name: &str) -> Result<()> {
        let o = self.incus_run(&["snapshot", "delete", instance, name])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "snapshot delete {name}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Read a live instance's Incus surface back from the server (config/devices/profiles).
    pub fn instance(&self, name: &str) -> Result<InstanceConfig> {
        let o = self.incus_run(&["list", name, "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("list {name}: {}", o.stderr.trim())));
        }
        parse_instance(&o.stdout, name)
    }

    /// Reconcile a TOML-owned Incus profile into the project: create it if missing, then converge
    /// its config + devices toward the declared intent.
    pub fn reconcile_profile(
        &self,
        desired: &crate::config::IncusProfile,
        reporter: &dyn Reporter,
    ) -> Result<()> {
        let live = self.incus_profiles()?;
        let existing = live.iter().find(|p| p.name == desired.name);
        if existing.is_none() {
            reporter.step(&format!("Creating Incus profile '{}'", desired.name));
            let o = self.incus_run(&["profile", "create", &desired.name])?;
            if !o.ok() {
                return Err(Error::Incus(format!(
                    "profile create {}: {}",
                    desired.name,
                    o.stderr.trim()
                )));
            }
        }
        let plan = crate::reconcile::profile_converge_plan(desired, existing);
        for op in &plan {
            match op {
                ConvergeOp::SetConfig { key, value } => {
                    reporter.step(&format!("{}: set {key}={value}", desired.name));
                    let o = self.incus_run(&["profile", "set", &desired.name, key, value])?;
                    if !o.ok() {
                        return Err(Error::Incus(format!(
                            "profile set {key}: {}",
                            o.stderr.trim()
                        )));
                    }
                }
                ConvergeOp::UnsetConfig { key } => {
                    reporter.step(&format!("{}: unset {key}", desired.name));
                    let _ = self.incus_run(&["profile", "unset", &desired.name, key]);
                }
                ConvergeOp::AddDevice { name: dev, keys } => {
                    reporter.step(&format!("{}: add device {dev}", desired.name));
                    let dtype = keys.get("type").cloned().unwrap_or_else(|| "disk".into());
                    let mut args: Vec<String> = vec![
                        "profile".into(),
                        "device".into(),
                        "add".into(),
                        desired.name.clone(),
                        dev.clone(),
                        dtype,
                    ];
                    for (k, v) in keys {
                        if k != "type" {
                            args.push(format!("{k}={v}"));
                        }
                    }
                    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
                    let o = self.incus_run(&argv)?;
                    if !o.ok() {
                        return Err(Error::Incus(format!(
                            "profile device add {dev}: {}",
                            o.stderr.trim()
                        )));
                    }
                }
                ConvergeOp::RemoveDevice { name: dev } => {
                    reporter.step(&format!("{}: remove device {dev}", desired.name));
                    let _ = self.incus_run(&["profile", "device", "remove", &desired.name, dev]);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Incus projects (features / limits / restrictions).
    pub fn projects(&self) -> Result<Vec<ProjectRecord>> {
        let o = self.incus_run(&["project", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("project list: {}", o.stderr.trim())));
        }
        parse_projects(&o.stdout)
    }

    /// Storage pools in the project, each with its custom volumes.
    pub fn storage(&self) -> Result<Vec<StoragePool>> {
        let o = self.incus_run(&["storage", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("storage list: {}", o.stderr.trim())));
        }
        let mut pools = parse_storage_pools(&o.stdout)?;
        for pool in pools.iter_mut() {
            if let Ok(v) =
                self.incus_run(&["storage", "volume", "list", &pool.name, "--format", "json"])
            {
                if v.ok() {
                    pool.volumes = parse_storage_volumes(&v.stdout).unwrap_or_default();
                }
            }
        }
        Ok(pools)
    }

    /// Incus profiles (config+devices composition bundles) in the project.
    pub fn incus_profiles(&self) -> Result<Vec<IncusProfileRecord>> {
        let o = self.incus_run(&["profile", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("profile list: {}", o.stderr.trim())));
        }
        parse_incus_profiles(&o.stdout)
    }

    /// Managed Incus networks (bridges) in the VM.
    pub fn networks(&self) -> Result<Vec<NetworkRecord>> {
        let o = self.incus_run(&["network", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("network list: {}", o.stderr.trim())));
        }
        parse_networks(&o.stdout)
    }

    /// Network ACLs (named allow/deny rulesets — the egress-policy layer).
    pub fn network_acls(&self) -> Result<Vec<NetworkAcl>> {
        let o = self.incus_run(&["network", "acl", "list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "network acl list: {}",
                o.stderr.trim()
            )));
        }
        parse_network_acls(&o.stdout)
    }

    /// Create a network ACL (`incus network acl create <name>`). Idempotent: an
    /// "already exists" failure is treated as success.
    pub fn network_acl_create(&self, name: &str) -> Result<()> {
        let o = self.incus_run(&["network", "acl", "create", name])?;
        let already = format!("{} {}", o.stderr, o.stdout)
            .to_lowercase()
            .contains("already exists");
        if !o.ok() && !already {
            return Err(Error::Incus(format!(
                "network acl create {name}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// The `key=value` args for a rule (action/destination/destination_port/protocol), shared by
    /// add and remove. Empty fields are omitted.
    fn acl_rule_args(rule: &AclRule) -> Vec<String> {
        let mut args = vec![format!("action={}", rule.action)];
        if !rule.destination.is_empty() {
            args.push(format!("destination={}", rule.destination));
        }
        if !rule.port.is_empty() {
            args.push(format!("destination_port={}", rule.port));
        }
        if !rule.protocol.is_empty() {
            args.push(format!("protocol={}", rule.protocol));
        }
        args
    }

    /// Add a rule to an ACL (`incus network acl rule add <acl> <direction> key=value…`).
    pub fn network_acl_rule_add(&self, acl: &str, direction: &str, rule: &AclRule) -> Result<()> {
        let mut args: Vec<String> = vec![
            "network".into(),
            "acl".into(),
            "rule".into(),
            "add".into(),
            acl.into(),
            direction.into(),
        ];
        args.extend(Self::acl_rule_args(rule));
        if !rule.description.is_empty() {
            args.push("--description".into());
            args.push(rule.description.clone());
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = self.incus_run(&argv)?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "network acl rule add {acl}/{direction}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Remove a matching rule from an ACL (`incus network acl rule remove <acl> <direction> …`).
    pub fn network_acl_rule_remove(
        &self,
        acl: &str,
        direction: &str,
        rule: &AclRule,
    ) -> Result<()> {
        let mut args: Vec<String> = vec![
            "network".into(),
            "acl".into(),
            "rule".into(),
            "remove".into(),
            acl.into(),
            direction.into(),
        ];
        args.extend(Self::acl_rule_args(rule));
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = self.incus_run(&argv)?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "network acl rule remove {acl}/{direction}: {}",
                o.stderr.trim()
            )));
        }
        Ok(())
    }

    /// Apply an egress-ACL plan (from `enforce::egress_acl_plan`) to a single named ACL.
    pub fn apply_egress(&self, acl: &str, plan: &[AclOp], reporter: &dyn Reporter) -> Result<()> {
        for op in plan {
            match op {
                AclOp::Create => {
                    reporter.step(&format!("Creating network ACL '{acl}'"));
                    self.network_acl_create(acl)?;
                }
                AclOp::AddRule { direction, rule } => {
                    reporter.step(&format!("{acl}: allow {} {}", rule.destination, rule.port));
                    self.network_acl_rule_add(acl, direction, rule)?;
                }
                AclOp::RemoveRule { direction, rule } => {
                    reporter.step(&format!("{acl}: remove {} {}", rule.destination, rule.port));
                    self.network_acl_rule_remove(acl, direction, rule)?;
                }
            }
        }
        Ok(())
    }

    /// Bind an egress ACL to a sandbox's nic with a default-drop egress posture, idempotently.
    /// The nic is usually inherited from a profile, so we `config device override` it (copies the
    /// inherited device instance-local with our keys); if it is already instance-local, `override`
    /// reports it exists and we fall back to `config device set` for each key.
    pub fn bind_egress_acl(&self, sandbox: &str, nic: &str, acl: &str) -> Result<()> {
        let acls = format!("security.acls={acl}");
        let drop = "security.acls.default.egress.action=drop".to_string();
        let o = self.incus_run(&["config", "device", "override", sandbox, nic, &acls, &drop])?;
        if o.ok() {
            return Ok(());
        }
        let exists = format!("{} {}", o.stderr, o.stdout)
            .to_lowercase()
            .contains("already exists");
        if !exists {
            return Err(Error::Incus(format!(
                "binding ACL to {nic}: {}",
                o.stderr.trim()
            )));
        }
        for (k, v) in [
            ("security.acls", acl),
            ("security.acls.default.egress.action", "drop"),
        ] {
            let s = self.incus_run(&["config", "device", "set", sandbox, nic, k, v])?;
            if !s.ok() {
                return Err(Error::Incus(format!(
                    "setting {k} on {nic}: {}",
                    s.stderr.trim()
                )));
            }
        }
        Ok(())
    }

    /// Per-sandbox network attachments and addresses (services excluded).
    pub fn sandbox_networks(&self) -> Result<Vec<SandboxNetwork>> {
        let o = self.incus_run(&["list", "--format", "json"])?;
        if !o.ok() {
            return Err(Error::Incus(format!("list: {}", o.stderr.trim())));
        }
        parse_sandbox_networks(&o.stdout)
    }

    /// Create a Linux user inside a sandbox — one per agent, or the human operator. An agent is
    /// 1:1 with its Linux user. `human` users are best-effort added to the sudo/wheel group.
    pub fn add_user(&self, sandbox: &str, name: &str, human: bool) -> Result<()> {
        let o = self.incus_run(&["exec", sandbox, "--", "sh", "-c", &useradd_script(name)])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "creating user {name}: {}",
                o.stderr.trim()
            )));
        }
        if human {
            let sudo =
                format!("(usermod -aG sudo {name} || addgroup {name} wheel) 2>/dev/null || true");
            let _ = self.incus_run(&["exec", sandbox, "--", "sh", "-c", &sudo]);
        }
        Ok(())
    }

    /// Remove a Linux user (and its home) from a sandbox. Errors if the user still exists after.
    pub fn remove_user(&self, sandbox: &str, name: &str) -> Result<()> {
        let script = format!(
            "userdel -r {n} 2>/dev/null || deluser --remove-home {n} 2>/dev/null || deluser {n} 2>/dev/null || true; ! id {n} >/dev/null 2>&1",
            n = name
        );
        let o = self.incus_run(&["exec", sandbox, "--", "sh", "-c", &script])?;
        if !o.ok() {
            return Err(Error::Incus(format!(
                "removing user {name} (still present?)"
            )));
        }
        Ok(())
    }

    /// Build a custom image via the publish-from-container flow: launch a throwaway builder from
    /// `base`, run `setup` inside it, then `incus publish` it under `alias`. The builder is removed
    /// on success or failure. Progress streams via `reporter`.
    pub fn build_image(
        &self,
        base: &str,
        alias: &str,
        setup: &str,
        description: &str,
        reporter: &dyn Reporter,
    ) -> Result<()> {
        let tmp = builder_name(alias);
        let _ = self.incus_run(&["delete", "--force", &tmp]); // clear any leftover builder

        reporter.step(&format!("Launching builder from {base}"));
        if self.incus_streamed(&["launch", base, &tmp])? != 0 {
            return Err(Error::Incus(format!(
                "launching builder from '{base}' failed"
            )));
        }

        if !setup.trim().is_empty() {
            reporter.step("Running setup inside builder");
            let code = self.incus_streamed(&["exec", &tmp, "--", "sh", "-c", setup])?;
            if code != 0 {
                let _ = self.incus_run(&["delete", "--force", &tmp]);
                return Err(Error::Incus(format!("setup script failed (exit {code})")));
            }
        }

        reporter.step("Stopping builder");
        let _ = self.incus_run(&["stop", &tmp]); // best effort — publish needs it stopped

        reporter.step(&format!("Publishing image '{alias}'"));
        let descopt = format!("description={description}");
        let mut args: Vec<&str> = vec!["publish", &tmp, "--alias", alias, "--reuse"];
        if !description.is_empty() {
            args.push(&descopt);
        }
        let code = self.incus_streamed(&args)?;
        if code != 0 {
            let _ = self.incus_run(&["delete", "--force", &tmp]);
            return Err(Error::Incus(format!(
                "publishing image '{alias}' failed (exit {code})"
            )));
        }

        reporter.step("Removing builder");
        let _ = self.incus_run(&["delete", "--force", &tmp]);
        reporter.step(&format!("Image '{alias}' built"));
        Ok(())
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
        let args = launch_args(spec);
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let code = self.incus_streamed(&argv)?;
        if code != 0 {
            return Err(Error::Incus(format!("`incus launch` exited with {code}")));
        }
        for u in &spec.users {
            reporter.step(&format!("Creating {} user '{}'", role_word(u.role), u.name));
            self.add_user(&spec.name, &u.name, matches!(u.role, UserRole::Human))?;
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
    use crate::process::{out, FakeRunner};
    use crate::progress::SilentReporter;

    #[test]
    fn builder_name_sanitizes_alias() {
        assert_eq!(builder_name("dev-ubuntu-24.04"), "build-dev-ubuntu-24-04");
        assert_eq!(builder_name("My Image!"), "build-my-image");
    }

    #[test]
    fn build_image_launches_sets_up_publishes_and_cleans_up() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.build_image(
            "images:debian/12",
            "my-img",
            "apt-get install -y git",
            "desc",
            &SilentReporter,
        )
        .unwrap();
        assert!(r.called_with("launch"));
        assert!(r.called_with("images:debian/12"));
        assert!(r.called_with("publish"));
        assert!(r.called_with("my-img"));
        assert!(r.called_with("--reuse"));
        assert!(r.called_with("delete")); // builder removed
    }

    #[test]
    fn add_user_creates_linux_user() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.add_user("web-agent-01", "agent-claude", false).unwrap();
        assert!(r.called_with("exec"));
        assert!(r.called_with("agent-claude"));
        assert!(r.called_with("useradd"));
    }

    #[test]
    fn add_user_human_attempts_sudo() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.add_user("web-agent-01", "colin", true).unwrap();
        assert!(r.called_with("usermod")); // best-effort sudo/wheel for the human
    }

    #[test]
    fn remove_user_deletes_linux_user() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.remove_user("web-agent-01", "agent-claude").unwrap();
        assert!(r.called_with("exec"));
        assert!(r.called_with("userdel"));
        assert!(r.called_with("agent-claude"));
    }

    #[test]
    fn build_image_errors_when_publish_fails() {
        // launch/exec ok (exit 0); publish fails (exit 1).
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"publish") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        let c = CliIncus::new("llmsc", &r);
        assert!(c
            .build_image("images:debian/12", "x", "", "", &SilentReporter)
            .is_err());
    }

    fn sb(name: &str) -> Sandbox {
        Sandbox {
            name: name.into(),
            image: "images:debian/13".into(),
            nesting: true,
            users: vec![],
            ..Default::default()
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
    fn launch_args_render_the_incus_surface() {
        let mut spec = sb("web-agent-01"); // nesting: true
        spec.ephemeral = true;
        spec.description = Some("dev box".into());
        spec.profiles = vec!["sandbox".into(), "net-egress-filtered".into()];
        spec.config
            .insert("cloud-init.user-data".into(), "#cloud-config".into());
        let mut work = std::collections::BTreeMap::new();
        work.insert("type".into(), "disk".into());
        work.insert("source".into(), "/home/colin/proj".into());
        work.insert("path".into(), "/work".into());
        spec.devices.insert("work".into(), work);

        let a = launch_args(&spec);
        let joined = a.join(" ");
        assert_eq!(a[0], "launch");
        assert_eq!(a[1], "images:debian/13");
        assert_eq!(a[2], "web-agent-01");
        assert!(joined.contains("--ephemeral"));
        assert!(joined.contains("--description dev box"));
        assert!(joined.contains("-p sandbox"));
        assert!(joined.contains("-p net-egress-filtered"));
        assert!(joined.contains("-c security.privileged=false")); // invariant always present
        assert!(joined.contains("-c security.nesting=true")); // from nesting
        assert!(joined.contains("-c cloud-init.user-data=#cloud-config"));
        assert!(joined.contains("-d work,type=disk,path=/work,source=/home/colin/proj"));
    }

    #[test]
    fn launch_via_cli_uses_rendered_args() {
        // `info` (the exists check) non-zero so launch proceeds; everything else ok.
        let r = FakeRunner::new(|_, args| {
            if args.contains(&"info") {
                out(1, "")
            } else {
                out(0, "")
            }
        });
        let c = CliIncus::new("llmsc", &r);
        c.launch(&sb("web-agent-01"), &SilentReporter).unwrap();
        assert!(r.called_with("launch"));
        assert!(r.called_with("security.privileged=false"));
        assert!(r.called_with("security.nesting=true"));
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
    fn parse_images_tolerates_null_collections() {
        // Remote `incus image list images:` emits null (not []) for empty aliases/used_by.
        let json = r#"[
          {"fingerprint":"abc123def4567890","aliases":null,"used_by":null,
           "properties":{"os":"Debian","release":"12","architecture":"amd64"},
           "size":92000000,"uploaded_at":"2026-06-01T00:00:00Z"}
        ]"#;
        let imgs = parse_images(json).unwrap();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].name, "abc123def456"); // fingerprint fallback (null aliases)
        assert_eq!(imgs[0].base, "Debian 12");
        assert_eq!(imgs[0].used_by, 0);
    }

    #[test]
    fn parse_snapshots_reads_and_shortens_names() {
        let json = r#"[
          {"name":"web-agent-01","snapshots":[
            {"name":"web-agent-01/before-deploy","created_at":"2026-06-04T10:00:00Z","stateful":false},
            {"name":"snap0","created_at":"2026-06-03T09:00:00Z","stateful":true}]},
          {"name":"other","snapshots":[]}
        ]"#;
        let s = parse_snapshots(json, "web-agent-01").unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "before-deploy"); // "<instance>/" prefix stripped
        assert_eq!(s[0].created, "2026-06-04");
        assert!(s[1].stateful);
        assert!(matches!(
            parse_snapshots(json, "missing"),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn snapshot_ops_call_incus() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.snapshot_create("web-agent-01", "before-deploy").unwrap();
        c.snapshot_restore("web-agent-01", "before-deploy").unwrap();
        c.snapshot_delete("web-agent-01", "before-deploy").unwrap();
        for needle in ["snapshot", "create", "restore", "delete", "before-deploy"] {
            assert!(r.called_with(needle));
        }
    }

    #[test]
    fn parse_instance_reads_the_surface_and_filters_volatile() {
        let json = r#"[
          {"name":"web-agent-01","status":"Running","description":"dev box","ephemeral":true,
           "profiles":["default","sandbox"],
           "config":{"security.nesting":"true","image.description":"Alpine 3.21",
                     "volatile.eth0.hwaddr":"00:11:22:33:44:55"},
           "expanded_devices":{
             "eth0":{"type":"nic","network":"incusbr0"},
             "root":{"type":"disk","path":"/","pool":"default"},
             "work":{"type":"disk","source":"/home/colin/proj","path":"/work","shift":"true"}}},
          {"name":"other","status":"Stopped","config":{},"expanded_devices":{}}
        ]"#;
        let i = parse_instance(json, "web-agent-01").unwrap();
        assert_eq!(i.name, "web-agent-01");
        assert_eq!(i.status, InstanceStatus::Running);
        assert!(i.ephemeral);
        assert_eq!(i.profiles, vec!["default", "sandbox"]);
        assert_eq!(
            i.config.get("security.nesting").map(String::as_str),
            Some("true")
        );
        assert!(!i.config.contains_key("volatile.eth0.hwaddr")); // volatile filtered out
        assert_eq!(i.devices["work"]["source"], "/home/colin/proj");
        assert!(matches!(
            parse_instance(json, "missing"),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn instance_mutations_call_the_right_incus_commands() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.set_config("web-agent-01", "limits.processes", "512")
            .unwrap();
        c.unset_config("web-agent-01", "limits.processes").unwrap();
        let mut keys = std::collections::BTreeMap::new();
        keys.insert("type".into(), "disk".into());
        keys.insert("source".into(), "/h/p".into());
        keys.insert("path".into(), "/work".into());
        c.add_device("web-agent-01", "work", &keys).unwrap();
        c.remove_device("web-agent-01", "work").unwrap();
        c.add_profile("web-agent-01", "net-isolated").unwrap();
        c.remove_profile("web-agent-01", "net-isolated").unwrap();
        for needle in [
            "set",
            "unset",
            "device",
            "add",
            "remove",
            "profile",
            "limits.processes",
            "net-isolated",
            "source=/h/p",
        ] {
            assert!(
                r.called_with(needle),
                "expected an incus call containing {needle:?}"
            );
        }
    }

    #[test]
    fn parse_projects_reads_config_and_usage() {
        let json = r#"[
          {"name":"default","description":"Default Incus project",
           "config":{"features.images":"true","features.profiles":"true"},
           "used_by":["/1.0/instances/web-agent-01","/1.0/profiles/default"]}
        ]"#;
        let p = parse_projects(json).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "default");
        assert_eq!(p[0].used_by, 2);
        assert_eq!(p[0].config["features.images"], "true");
    }

    #[test]
    fn parse_storage_reads_pools_and_custom_volumes() {
        let pools = r#"[
          {"name":"default","driver":"dir","description":"","config":{"source":"/var/lib/incus/x"},
           "used_by":["/1.0/instances/web-agent-01"]}
        ]"#;
        let p = parse_storage_pools(pools).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].driver, "dir");
        assert_eq!(p[0].used_by, 1);
        assert_eq!(p[0].config["source"], "/var/lib/incus/x");

        let vols = r#"[
          {"name":"web-agent-01","type":"container","config":{},"used_by":["x"]},
          {"name":"shared-data","type":"custom","config":{"size":"10GiB"},"used_by":[]}
        ]"#;
        let v = parse_storage_volumes(vols).unwrap();
        assert_eq!(v.len(), 1, "only custom volumes are surfaced");
        assert_eq!(v[0].name, "shared-data");
        assert_eq!(v[0].config["size"], "10GiB");
    }

    #[test]
    fn parse_incus_profiles_reads_config_devices_and_usage() {
        let json = r#"[
          {"name":"default","description":"Default Incus profile",
           "config":{},"devices":{"eth0":{"type":"nic","network":"incusbr0"},"root":{"type":"disk","path":"/","pool":"default"}},
           "used_by":["/1.0/instances/web-agent-01","/1.0/instances/ci-runner"]},
          {"name":"nesting","description":"L3","config":{"security.nesting":"true"},"devices":{},"used_by":[]}
        ]"#;
        let p = parse_incus_profiles(json).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "default");
        assert_eq!(p[0].used_by, 2);
        assert_eq!(p[0].devices["eth0"]["network"], "incusbr0");
        assert_eq!(p[1].config["security.nesting"], "true");
    }

    #[test]
    fn parse_network_acls_reads_rules() {
        let json = r#"[
          {"name":"egress-allowlist","description":"web/pkg allowlist",
           "ingress":[],
           "egress":[
             {"action":"allow","destination":"github.com","protocol":"tcp","destination_port":"443","description":"git"},
             {"action":"reject","destination":"","protocol":"","destination_port":"","description":"default-deny"}],
           "used_by":["/1.0/instances/web-agent-01"]}
        ]"#;
        let a = parse_network_acls(json).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, "egress-allowlist");
        assert_eq!(a[0].used_by, 1);
        assert_eq!(a[0].egress.len(), 2);
        assert_eq!(a[0].egress[0].action, "allow");
        assert_eq!(a[0].egress[0].destination, "github.com");
        assert_eq!(a[0].egress[0].port, "443");
    }

    #[test]
    fn apply_egress_creates_acl_and_diffs_rules() {
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        let allow = AclRule {
            action: "allow".into(),
            source: String::new(),
            destination: "10.21.32.0/24".into(),
            port: "4000".into(),
            protocol: "tcp".into(),
            description: "LLM proxy".into(),
        };
        let stale = AclRule {
            action: "allow".into(),
            source: String::new(),
            destination: "203.0.113.0/24".into(),
            port: "22".into(),
            protocol: "tcp".into(),
            description: String::new(),
        };
        let plan = vec![
            AclOp::Create,
            AclOp::AddRule {
                direction: "egress".into(),
                rule: allow,
            },
            AclOp::RemoveRule {
                direction: "egress".into(),
                rule: stale,
            },
        ];
        c.apply_egress("llmsc-egress-web-agent-01", &plan, &SilentReporter)
            .unwrap();
        assert!(r.called_with("create"));
        assert!(r.called_with("llmsc-egress-web-agent-01"));
        assert!(r.called_with("add"));
        assert!(r.called_with("destination_port=4000"));
        assert!(r.called_with("remove"));
        assert!(r.called_with("destination=203.0.113.0/24"));
    }

    #[test]
    fn bind_egress_acl_overrides_then_falls_back_to_set() {
        // override succeeds (profile-inherited nic).
        let r = FakeRunner::new(|_, _| out(0, ""));
        let c = CliIncus::new("llmsc", &r);
        c.bind_egress_acl("web-agent-01", "eth0", "llmsc-egress-web-agent-01")
            .unwrap();
        assert!(r.called_with("override"));
        assert!(r.called_with("security.acls=llmsc-egress-web-agent-01"));

        // override reports the device already exists → fall back to per-key set.
        let r2 = FakeRunner::new(|_, args| {
            if args.contains(&"override") {
                out(1, "Error: Device already exists")
            } else {
                out(0, "")
            }
        });
        let c2 = CliIncus::new("llmsc", &r2);
        c2.bind_egress_acl("web-agent-01", "eth0", "llmsc-egress-web-agent-01")
            .unwrap();
        assert!(r2.called_with("set"));
        assert!(r2.called_with("security.acls.default.egress.action"));
    }

    #[test]
    fn network_acl_create_is_idempotent_on_already_exists() {
        let r = FakeRunner::new(|_, _| out(1, "Error: The network ACL already exists"));
        let c = CliIncus::new("llmsc", &r);
        // "already exists" is swallowed; a real failure would propagate.
        c.network_acl_create("llmsc-egress-x").unwrap();
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
                    profile: Some("researcher".into()),
                    guardrails: None,
                },
                User {
                    name: "operator".into(),
                    role: UserRole::Human,
                    profile: None,
                    guardrails: None,
                },
            ],
            ..Default::default()
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
