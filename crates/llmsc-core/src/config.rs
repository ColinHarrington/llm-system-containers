//! Declarative on-disk configuration (TOML) — the single source of *intent*.
//!
//! Both the CLIs and the GUI read/write this. See `planning/tech-stack.md`.

use crate::error::Error;
use crate::service::{Placement, Service};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            operator: default_operator_username(),
            vm: VmConfig::default(),
            sandboxes: Vec::new(),
            services: Vec::new(),
        }
    }
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

/// An L2 system container (LLMSC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sandbox {
    pub name: String,
    pub image: String,
    /// Allow nested rootless app containers (L3).
    #[serde(default)]
    pub nesting: bool,
    #[serde(default, rename = "user", skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<User>,
}

/// A Linux user inside a sandbox (one per agent, plus a human operator).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub role: UserRole,
    /// The agent profile assigned to this user (archetype name). None for the human operator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Agent,
    Human,
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
                    },
                    User {
                        name: "operator".into(),
                        role: UserRole::Human,
                        profile: None,
                    },
                ],
            }],
            services: vec![],
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
            .prop_map(|(name, role, profile)| User { name, role, profile })
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
        (arb_name(), vm, proptest::collection::vec(arb_sandbox(), 0..3)).prop_map(
            |(operator, vm, sandboxes)| Config {
                operator,
                vm,
                sandboxes,
                services: vec![],
            },
        )
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
