//! Declarative on-disk configuration (TOML) — the single source of *intent*.
//!
//! Both the CLIs and the GUI read/write this. See `planning/tech-stack.md`.

use crate::error::Error;
use crate::service::{Placement, Service};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn is_false(b: &bool) -> bool {
    !*b
}

/// Quote a single-line YAML scalar (Incus config values are always strings).
fn yaml_inline(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Emit a `key: value` line at the given indent; multiline values use a `|` block scalar
/// (the natural YAML form for e.g. `cloud-init.user-data`).
fn push_yaml_kv(out: &mut String, indent: usize, key: &str, value: &str) {
    let pad = " ".repeat(indent);
    if value.contains('\n') {
        out.push_str(&format!("{pad}{key}: |\n"));
        let inner = " ".repeat(indent + 2);
        for line in value.lines() {
            out.push_str(&format!("{inner}{line}\n"));
        }
    } else {
        out.push_str(&format!("{pad}{key}: {}\n", yaml_inline(value)));
    }
}

/// Top-level llmsc configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The human operator's Linux username — the default user created in every sandbox. Set once
    /// (defaults to the host username), overridable per sandbox. One human per sandbox.
    #[serde(default = "default_operator_username")]
    pub operator: String,
    /// The L1 VM that hosts everything.
    pub vm: VmConfig,
    /// Declared L2 sandboxes (desired state).
    #[serde(default, rename = "sandbox", skip_serializing_if = "Vec::is_empty")]
    pub sandboxes: Vec<Sandbox>,
    /// Enabled services.
    #[serde(default, rename = "service", skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<Service>,
    /// TOML-owned Incus profiles (config+device composition bundles) reconciled into the project.
    #[serde(
        default,
        rename = "incus_profile",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub incus_profiles: Vec<IncusProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            operator: default_operator_username(),
            vm: VmConfig::default(),
            sandboxes: Vec::new(),
            services: Vec::new(),
            incus_profiles: Vec::new(),
        }
    }
}

/// A TOML-owned Incus profile: a reusable bundle of `config` + `devices` (the same two maps an
/// instance carries) composed onto sandboxes. See `planning/research/incus-instance-inputs.md` §9.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncusProfile {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub devices: BTreeMap<String, BTreeMap<String, String>>,
}

/// Recommended starter Incus profiles (safe, no external deps). The user adopts/reconciles these
/// into the project; richer kinds (`net-*`/`fs-*`) depend on networks/storage that must exist.
pub fn starter_incus_profiles() -> Vec<IncusProfile> {
    vec![
        IncusProfile {
            name: "sandbox".into(),
            description: Some("LLMSC unprivileged sandbox base".into()),
            config: BTreeMap::from([("security.privileged".into(), "false".into())]),
            devices: BTreeMap::new(),
        },
        IncusProfile {
            name: "nesting".into(),
            description: Some("Nested rootless app containers (L3)".into()),
            config: BTreeMap::from([("security.nesting".into(), "true".into())]),
            devices: BTreeMap::new(),
        },
    ]
}

/// The host username — used as the default operator name. Falls back to "operator".
pub fn default_operator_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "operator".to_string())
}

/// The L1 VM (`llmsc-vm`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmConfig {
    pub name: String,
    pub cpus: u32,
    pub memory_gib: u32,
    pub disk_gib: u32,
    #[serde(default)]
    pub driver: VmDriverKind,
}

/// VM backend driver. MVP ships Lima on both platforms; others are future.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VmDriverKind {
    #[default]
    Lima,
    Parallels,
    Libvirt,
    Proxmox,
}

/// An L2 system container (LLMSC). Modeled on the Incus instance (`InstancesPost`): the
/// declarative intent that renders to an Incus instance. We only ever create **unprivileged
/// containers** — `type: container` and `security.privileged: false` are tool-fixed invariants
/// (see `planning/research/incus-instance-inputs.md`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sandbox {
    pub name: String,
    /// Image alias / source ref (e.g. `images:alpine/3.21`).
    pub image: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Allow nested rootless app containers (L3) — sugar for `config["security.nesting"]=true`.
    #[serde(default)]
    pub nesting: bool,
    /// Delete the instance when it stops.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ephemeral: bool,
    /// Incus profiles to apply (ordered; later overrides earlier). Empty → Incus default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    /// Instance-local Incus `config` keys (e.g. `limits.cpu`, `raw.idmap`, `cloud-init.*`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, String>,
    /// Instance-local Incus `devices` (name → {type, …keys}): workspace mounts, nics, …
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub devices: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default, rename = "user", skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<User>,
    /// Container-level network egress policy. `None` = unmanaged (no ACL applied — existing
    /// behavior). `Some` = a structured policy that compiles to an Incus network ACL bound to
    /// the nic (see [`crate::enforce`]). This is the per-container ring; per-UID egress is a
    /// later Tetragon ring that compiles from each agent's [`Guardrails::network`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress: Option<EgressPolicy>,
}

/// A container-level network egress policy — the legible intent that compiles to an Incus
/// network ACL (L3/L4) and, for HTTP(S) domain allowlists, a mitmproxy config (L7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressPolicy {
    #[serde(default)]
    pub posture: EgressPosture,
    /// Allowed L3/L4 destinations (only meaningful for `Allowlist`): named sets (`llm`,
    /// `package-registries`, `web`) or raw `CIDR:port[/proto]` (e.g. `10.0.0.0/8:443/tcp`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    /// Allowed HTTP(S) **domains** (the L7 allowlist enforced by mitmproxy, e.g. `github.com`).
    /// When non-empty, the sandbox is pointed at the mitmproxy egress proxy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<String>,
}

/// The posture of an egress policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EgressPosture {
    /// Drop all egress (default — least privilege).
    #[default]
    DenyAll,
    /// Drop all egress except the entries in [`EgressPolicy::allow`].
    Allowlist,
    /// No restriction — no ACL is created/bound.
    Open,
}

impl EgressPolicy {
    /// The default policy for a newly-created managed sandbox: agents may reach the LLM proxy and
    /// nothing else (the headline default-deny posture from `planning/security-model.md`).
    pub fn default_managed() -> Self {
        Self {
            posture: EgressPosture::Allowlist,
            allow: vec!["llm".to_string()],
            domains: Vec::new(),
        }
    }
}

impl Sandbox {
    /// Add a user, or replace the existing one with the same name.
    pub fn set_user(&mut self, user: User) {
        match self.users.iter_mut().find(|u| u.name == user.name) {
            Some(u) => *u = user,
            None => self.users.push(user),
        }
    }

    /// Render this sandbox's declarative intent as the Incus instance YAML (`InstancePut` shape)
    /// — the exact artifact `incus create <image> <name> < config.yaml` consumes. TOML stays the
    /// source of intent; this is the rendered boundary artifact (see the research note §8).
    pub fn to_instance_yaml(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# incus create {} {}\n", self.image, self.name));
        if let Some(d) = self.description.as_deref().filter(|d| !d.is_empty()) {
            out.push_str(&format!("description: {}\n", yaml_inline(d)));
        }
        if self.ephemeral {
            out.push_str("ephemeral: true\n");
        }
        out.push_str("profiles:\n");
        if self.profiles.is_empty() {
            out.push_str("- default\n");
        } else {
            for p in &self.profiles {
                out.push_str(&format!("- {p}\n"));
            }
        }
        let cfg = self.effective_config();
        if !cfg.is_empty() {
            out.push_str("config:\n");
            for (k, v) in &cfg {
                push_yaml_kv(&mut out, 2, k, v);
            }
        }
        if !self.devices.is_empty() {
            out.push_str("devices:\n");
            for (name, keys) in &self.devices {
                out.push_str(&format!("  {name}:\n"));
                if let Some(t) = keys.get("type") {
                    push_yaml_kv(&mut out, 4, "type", t);
                }
                for (k, v) in keys {
                    if k != "type" {
                        push_yaml_kv(&mut out, 4, k, v);
                    }
                }
            }
        }
        out
    }

    /// The effective Incus `config` map for this sandbox: the invariants
    /// (`security.privileged=false`), `security.nesting` from [`nesting`](Self::nesting), then the
    /// instance-local [`config`](Self::config) overrides on top.
    pub fn effective_config(&self) -> BTreeMap<String, String> {
        let mut c = BTreeMap::new();
        c.insert("security.privileged".to_string(), "false".to_string());
        if self.nesting {
            c.insert("security.nesting".to_string(), "true".to_string());
        }
        for (k, v) in &self.config {
            c.insert(k.clone(), v.clone());
        }
        c
    }
}

/// A Linux user inside a sandbox (one per agent, plus a human operator).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub role: UserRole,
    /// The agent profile this user's guardrails were *seeded from* (provenance). None for the
    /// human operator. The profile is a seed, not a live link — guardrails diverge once refined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The agent's own guardrails — concrete, per-agent, editable. Seeded from `profile` at
    /// creation, then refined independently. None for the human operator (unrestricted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrails: Option<Guardrails>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    #[default]
    Agent,
    Human,
}

/// An agent's guardrails — the legible permission bundle (the axes of an agent profile), held
/// per-agent so it can be refined after being seeded from a profile. Presets, not enforcement:
/// compiling these to Tetragon / Incus ACLs / LiteLLM is later work (see `planning/agent-profiles.md`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guardrails {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub filesystem: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub network: String,
    #[serde(default)]
    pub l3: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub llm_budget: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub control_plane: String,
}

impl Guardrails {
    /// Seed guardrails from an agent-profile archetype (its axes). None if the profile is unknown.
    pub fn from_profile(name: &str) -> Option<Self> {
        crate::profile::lookup(name).map(|p| Guardrails {
            filesystem: p.filesystem.to_string(),
            network: p.network.to_string(),
            l3: p.l3,
            llm_budget: p.llm_budget.to_string(),
            control_plane: p.control_plane.to_string(),
        })
    }
}

impl Config {
    /// Parse a TOML document into a [`Config`].
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Serialize this [`Config`] back to a TOML document.
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Read and parse a config file.
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("reading {}: {e}", path.display())))?;
        Self::from_toml(&text)
            .map_err(|e| Error::Config(format!("parsing {}: {e}", path.display())))
    }

    /// Write this config to `path` (creating parent directories).
    pub fn save(&self, path: &Path) -> crate::error::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| Error::Config(format!("creating {}: {e}", parent.display())))?;
            }
        }
        let text = self
            .to_toml()
            .map_err(|e| Error::Config(format!("serializing: {e}")))?;
        std::fs::write(path, text)
            .map_err(|e| Error::Config(format!("writing {}: {e}", path.display())))
    }

    /// A declared sandbox by name.
    pub fn sandbox(&self, name: &str) -> Option<&Sandbox> {
        self.sandboxes.iter().find(|s| s.name == name)
    }

    /// A declared sandbox by name, mutably (to converge config/devices/profiles edits).
    pub fn sandbox_mut(&mut self, name: &str) -> Option<&mut Sandbox> {
        self.sandboxes.iter_mut().find(|s| s.name == name)
    }

    /// Record a sandbox (insert if absent; update image/nesting if present). Returns a mut ref.
    pub fn upsert_sandbox(&mut self, name: &str, image: &str, nesting: bool) -> &mut Sandbox {
        if let Some(i) = self.sandboxes.iter().position(|s| s.name == name) {
            self.sandboxes[i].image = image.to_string();
            self.sandboxes[i].nesting = nesting;
            return &mut self.sandboxes[i];
        }
        self.sandboxes.push(Sandbox {
            name: name.to_string(),
            image: image.to_string(),
            nesting,
            users: Vec::new(),
            ..Default::default()
        });
        self.sandboxes.last_mut().unwrap()
    }

    /// A TOML-owned Incus profile by name.
    pub fn incus_profile(&self, name: &str) -> Option<&IncusProfile> {
        self.incus_profiles.iter().find(|p| p.name == name)
    }

    /// Insert/replace a TOML-owned Incus profile by name.
    pub fn put_incus_profile(&mut self, profile: IncusProfile) {
        match self
            .incus_profiles
            .iter()
            .position(|p| p.name == profile.name)
        {
            Some(i) => self.incus_profiles[i] = profile,
            None => self.incus_profiles.push(profile),
        }
    }

    /// Insert a sandbox, or replace the existing one with the same name (declarative intent).
    pub fn put_sandbox(&mut self, sandbox: Sandbox) {
        match self.sandboxes.iter().position(|s| s.name == sandbox.name) {
            Some(i) => self.sandboxes[i] = sandbox,
            None => self.sandboxes.push(sandbox),
        }
    }

    /// Add or replace a user in a declared sandbox. Returns false if the sandbox isn't declared.
    pub fn set_sandbox_user(&mut self, sandbox: &str, user: User) -> bool {
        match self.sandboxes.iter_mut().find(|s| s.name == sandbox) {
            Some(s) => {
                s.set_user(user);
                true
            }
            None => false,
        }
    }

    /// Set an agent's guardrails in a declared sandbox. Returns false if the user isn't found.
    pub fn set_user_guardrails(&mut self, sandbox: &str, user: &str, g: Guardrails) -> bool {
        match self.sandboxes.iter_mut().find(|s| s.name == sandbox) {
            Some(s) => match s.users.iter_mut().find(|u| u.name == user) {
                Some(u) => {
                    u.guardrails = Some(g);
                    true
                }
                None => false,
            },
            None => false,
        }
    }

    /// Set a declared sandbox's egress policy. Returns true if the sandbox exists.
    pub fn set_sandbox_egress(&mut self, sandbox: &str, policy: EgressPolicy) -> bool {
        match self.sandboxes.iter_mut().find(|s| s.name == sandbox) {
            Some(s) => {
                s.egress = Some(policy);
                true
            }
            None => false,
        }
    }

    /// Remove a user from a declared sandbox. Returns true if it was present.
    pub fn remove_sandbox_user(&mut self, sandbox: &str, name: &str) -> bool {
        match self.sandboxes.iter_mut().find(|s| s.name == sandbox) {
            Some(s) => {
                let before = s.users.len();
                s.users.retain(|u| u.name != name);
                s.users.len() != before
            }
            None => false,
        }
    }

    /// Remove a declared sandbox. Returns true if it was present.
    pub fn remove_sandbox(&mut self, name: &str) -> bool {
        let before = self.sandboxes.len();
        self.sandboxes.retain(|s| s.name != name);
        self.sandboxes.len() != before
    }

    /// Is the named service enabled?
    pub fn service_enabled(&self, name: &str) -> bool {
        self.services.iter().any(|s| s.name == name)
    }

    /// Enable a service; returns true if newly added (false if already enabled).
    pub fn enable_service(&mut self, name: &str, placement: Placement) -> bool {
        if self.service_enabled(name) {
            return false;
        }
        self.services.push(Service {
            name: name.to_string(),
            placement,
        });
        true
    }

    /// Disable a service; returns true if it was present.
    pub fn disable_service(&mut self, name: &str) -> bool {
        let before = self.services.len();
        self.services.retain(|s| s.name != name);
        self.services.len() != before
    }

    /// The effective config: project (`./llmsc.toml`) if present, else user
    /// (`$XDG_CONFIG_HOME/llmsc/config.toml`) if present, else defaults.
    pub fn load_effective() -> crate::error::Result<Self> {
        let project = project_config_path();
        if project.exists() {
            return Self::load(&project);
        }
        let user = user_config_path();
        if user.exists() {
            return Self::load(&user);
        }
        Ok(Self::default())
    }
}

/// Per-project config path (`./llmsc.toml`).
pub fn project_config_path() -> PathBuf {
    PathBuf::from("llmsc.toml")
}

/// User/global config path (`$XDG_CONFIG_HOME/llmsc/config.toml`, falling back to `~/.config`).
pub fn user_config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("llmsc").join("config.toml")
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            name: "llmsc".into(),
            cpus: 4,
            memory_gib: 8,
            disk_gib: 100,
            driver: VmDriverKind::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        Config {
            operator: "operator".into(),
            vm: VmConfig {
                name: "llmsc".into(),
                cpus: 4,
                memory_gib: 8,
                disk_gib: 100,
                driver: VmDriverKind::Lima,
            },
            sandboxes: vec![Sandbox {
                name: "web-agent-01".into(),
                image: "images:debian/13".into(),
                nesting: true,
                users: vec![
                    User {
                        name: "agent-claude".into(),
                        role: UserRole::Agent,
                        profile: None,
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
            }],
            services: vec![],
            incus_profiles: vec![],
        }
    }

    #[test]
    fn toml_round_trip() {
        let c = sample();
        let s = c.to_toml().expect("serialize");
        let back = Config::from_toml(&s).expect("parse");
        assert_eq!(c, back);
    }

    #[test]
    fn parses_minimal_document() {
        let doc = r#"
            [vm]
            name = "llmsc"
            cpus = 4
            memory_gib = 8
            disk_gib = 100
        "#;
        let c = Config::from_toml(doc).expect("parse");
        assert_eq!(c.vm.driver, VmDriverKind::Lima); // default
        assert!(c.sandboxes.is_empty());
    }

    #[test]
    fn parses_sandbox_with_users() {
        let doc = r#"
            [vm]
            name = "llmsc"
            cpus = 4
            memory_gib = 8
            disk_gib = 100

            [[sandbox]]
            name = "web-agent-01"
            image = "images:debian/13"
            nesting = true

            [[sandbox.user]]
            name = "agent-claude"
            role = "agent"

            [[sandbox.user]]
            name = "operator"
            role = "human"
        "#;
        let c = Config::from_toml(doc).expect("parse");
        assert_eq!(c.sandboxes.len(), 1);
        assert_eq!(c.sandboxes[0].users.len(), 2);
        assert_eq!(c.sandboxes[0].users[1].role, UserRole::Human);
    }

    #[test]
    fn toml_snapshot() {
        insta::assert_snapshot!(sample().to_toml().unwrap());
    }

    #[test]
    fn renders_instance_yaml() {
        let mut sb = Sandbox {
            name: "web-02".into(),
            image: "images:alpine/3.21".into(),
            description: Some("dev box".into()),
            nesting: true,
            ephemeral: true,
            profiles: vec!["sandbox".into(), "net-egress-filtered".into()],
            ..Default::default()
        };
        sb.config.insert(
            "cloud-init.user-data".into(),
            "#cloud-config\npackages:\n- git".into(),
        );
        let mut work = BTreeMap::new();
        work.insert("type".into(), "disk".into());
        work.insert("source".into(), "/h/p".into());
        work.insert("path".into(), "/work".into());
        sb.devices.insert("work".into(), work);

        let y = sb.to_instance_yaml();
        assert!(y.contains("# incus create images:alpine/3.21 web-02"));
        assert!(y.contains("description: \"dev box\""));
        assert!(y.contains("ephemeral: true"));
        assert!(y.contains("profiles:\n- sandbox\n- net-egress-filtered"));
        assert!(y.contains("  security.privileged: \"false\"")); // invariant
        assert!(y.contains("  security.nesting: \"true\""));
        assert!(y.contains("  cloud-init.user-data: |\n    #cloud-config\n    packages:")); // block scalar
        assert!(y.contains("  work:\n    type: \"disk\""));
        assert!(y.contains("    path: \"/work\""));
    }

    #[test]
    fn upsert_sandbox_and_users() {
        let mut c = Config::default();
        c.upsert_sandbox("web-agent-01", "images:debian/12", true);
        assert!(c.set_sandbox_user(
            "web-agent-01",
            User {
                name: "colin".into(),
                role: UserRole::Human,
                profile: None,
                guardrails: None
            },
        ));
        assert!(c.set_sandbox_user(
            "web-agent-01",
            User {
                name: "agent-claude".into(),
                role: UserRole::Agent,
                profile: Some("builder".into()),
                guardrails: None
            },
        ));
        let sb = c.sandbox("web-agent-01").unwrap();
        assert_eq!(sb.image, "images:debian/12");
        assert_eq!(sb.users.len(), 2);
        // Re-assigning a user replaces (no dup), and updates the profile.
        c.set_sandbox_user(
            "web-agent-01",
            User {
                name: "agent-claude".into(),
                role: UserRole::Agent,
                profile: Some("tester".into()),
                guardrails: None,
            },
        );
        let sb = c.sandbox("web-agent-01").unwrap();
        assert_eq!(sb.users.len(), 2);
        assert_eq!(sb.users[1].profile.as_deref(), Some("tester"));
        // Unknown sandbox -> no-op false.
        assert!(!c.set_sandbox_user(
            "nope",
            User {
                name: "x".into(),
                role: UserRole::Agent,
                profile: None,
                guardrails: None
            }
        ));
        // put_sandbox replaces by name (no dup).
        c.put_sandbox(Sandbox {
            name: "web-agent-01".into(),
            image: "images:alpine/3.21".into(),
            ..Default::default()
        });
        assert_eq!(c.sandboxes.len(), 1);
        assert_eq!(
            c.sandbox("web-agent-01").unwrap().image,
            "images:alpine/3.21"
        );
        assert!(c.sandbox("web-agent-01").unwrap().users.is_empty()); // replaced wholesale
                                                                      // Re-establish users for the remove test below (operator + one agent).
        c.set_sandbox_user(
            "web-agent-01",
            User {
                name: "colin".into(),
                role: UserRole::Human,
                profile: None,
                guardrails: None,
            },
        );
        c.set_sandbox_user(
            "web-agent-01",
            User {
                name: "agent-claude".into(),
                role: UserRole::Agent,
                profile: None,
                guardrails: None,
            },
        );
        // Remove an agent user.
        assert!(c.remove_sandbox_user("web-agent-01", "agent-claude"));
        assert_eq!(c.sandbox("web-agent-01").unwrap().users.len(), 1);
        assert!(!c.remove_sandbox_user("web-agent-01", "agent-claude")); // already gone
        assert!(c.remove_sandbox("web-agent-01"));
        assert!(c.sandbox("web-agent-01").is_none());
    }

    #[test]
    fn guardrails_seed_from_profile_then_refine() {
        // Seed from an agent-profile archetype.
        let g = Guardrails::from_profile("researcher").unwrap();
        assert!(!g.network.is_empty());
        assert!(!g.l3); // researcher: nesting off
        assert!(Guardrails::from_profile("nope").is_none());

        let mut c = Config::default();
        c.upsert_sandbox("sb", "images:alpine/3.21", false);
        c.set_sandbox_user(
            "sb",
            User {
                name: "agent-claude".into(),
                role: UserRole::Agent,
                profile: Some("researcher".into()),
                guardrails: Guardrails::from_profile("researcher"),
            },
        );
        // Refine the agent's own guardrails (diverges from the seed profile).
        assert!(c.set_user_guardrails(
            "sb",
            "agent-claude",
            Guardrails {
                network: "none".into(),
                ..Default::default()
            }
        ));
        let u = &c.sandbox("sb").unwrap().users[0];
        assert_eq!(u.guardrails.as_ref().unwrap().network, "none");
        assert!(!c.set_user_guardrails("sb", "missing", Guardrails::default()));
    }

    #[test]
    fn egress_policy_set_and_serde_roundtrip() {
        let mut c = Config::default();
        c.upsert_sandbox("sb", "images:alpine/3.21", false);
        // Unmanaged by default (absent from TOML).
        assert!(c.sandbox("sb").unwrap().egress.is_none());

        // Set a managed allowlist policy.
        assert!(c.set_sandbox_egress("sb", EgressPolicy::default_managed()));
        assert!(!c.set_sandbox_egress("nope", EgressPolicy::default_managed()));
        let p = c.sandbox("sb").unwrap().egress.clone().unwrap();
        assert_eq!(p.posture, EgressPosture::Allowlist);
        assert_eq!(p.allow, vec!["llm".to_string()]);

        // Round-trips through TOML with the kebab-case posture tag.
        let toml = c.to_toml().unwrap();
        assert!(toml.contains("posture = \"allowlist\""));
        let back = Config::from_toml(&toml).unwrap();
        assert_eq!(
            back.sandbox("sb").unwrap().egress,
            c.sandbox("sb").unwrap().egress
        );

        // An unmanaged sandbox emits no [sandbox.egress] block.
        c.set_sandbox_egress("sb", EgressPolicy::default());
        // deny-all is the enum default; allow is empty so it still serializes the posture.
        assert_eq!(
            c.sandbox("sb").unwrap().egress.as_ref().unwrap().posture,
            EgressPosture::DenyAll
        );
    }

    #[test]
    fn enable_disable_service() {
        let mut c = Config::default();
        assert!(!c.service_enabled("litellm"));
        assert!(c.enable_service("litellm", Placement::Container));
        assert!(c.service_enabled("litellm"));
        assert!(!c.enable_service("litellm", Placement::Container)); // already enabled
        assert!(c.disable_service("litellm"));
        assert!(!c.service_enabled("litellm"));
        assert!(!c.disable_service("litellm")); // not present
    }

    #[test]
    fn save_load_roundtrip_with_service() {
        let mut c = sample();
        c.enable_service("litellm", Placement::Container);
        let path =
            std::env::temp_dir().join(format!("llmsc-save-test-{}.toml", std::process::id()));
        c.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded, c);
    }

    #[test]
    fn load_reads_a_file() {
        let path = std::env::temp_dir().join(format!("llmsc-cfg-test-{}.toml", std::process::id()));
        std::fs::write(&path, sample().to_toml().unwrap()).unwrap();
        let loaded = Config::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded, sample());
    }

    use proptest::prelude::*;

    fn arb_driver() -> impl Strategy<Value = VmDriverKind> {
        prop_oneof![
            Just(VmDriverKind::Lima),
            Just(VmDriverKind::Parallels),
            Just(VmDriverKind::Libvirt),
            Just(VmDriverKind::Proxmox),
        ]
    }

    fn arb_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9-]{0,12}".prop_map(|s| s)
    }

    fn arb_user() -> impl Strategy<Value = User> {
        (
            arb_name(),
            prop_oneof![Just(UserRole::Agent), Just(UserRole::Human)],
            prop_oneof![Just(None), arb_name().prop_map(Some)],
        )
            .prop_map(|(name, role, profile)| User {
                name,
                role,
                profile,
                guardrails: None,
            })
    }

    fn arb_sandbox() -> impl Strategy<Value = Sandbox> {
        (
            arb_name(),
            arb_name(),
            any::<bool>(),
            proptest::collection::vec(arb_user(), 0..3),
        )
            .prop_map(|(name, image, nesting, users)| Sandbox {
                name,
                image,
                nesting,
                users,
                ..Default::default()
            })
    }

    fn arb_config() -> impl Strategy<Value = Config> {
        let vm = (
            arb_name(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            arb_driver(),
        )
            .prop_map(|(name, cpus, memory_gib, disk_gib, driver)| VmConfig {
                name,
                cpus,
                memory_gib,
                disk_gib,
                driver,
            });
        (
            arb_name(),
            vm,
            proptest::collection::vec(arb_sandbox(), 0..3),
        )
            .prop_map(|(operator, vm, sandboxes)| Config {
                operator,
                vm,
                sandboxes,
                services: vec![],
                incus_profiles: vec![],
            })
    }

    proptest! {
        #[test]
        fn round_trips_arbitrary(c in arb_config()) {
            let s = c.to_toml().unwrap();
            let back = Config::from_toml(&s).unwrap();
            prop_assert_eq!(c, back);
        }
    }
}
