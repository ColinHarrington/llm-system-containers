//! Declarative on-disk configuration (TOML) — the single source of *intent*.
//!
//! Both the CLIs and the GUI read/write this. See `planning/tech-stack.md`.

use serde::{Deserialize, Serialize};

/// Top-level llmsc configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Config {
    /// The L1 VM that hosts everything.
    pub vm: VmConfig,
    /// Declared L2 sandboxes (desired state).
    #[serde(default, rename = "sandbox", skip_serializing_if = "Vec::is_empty")]
    pub sandboxes: Vec<Sandbox>,
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
                    },
                    User {
                        name: "operator".into(),
                        role: UserRole::Human,
                    },
                ],
            }],
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
        )
            .prop_map(|(name, role)| User { name, role })
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
        (vm, proptest::collection::vec(arb_sandbox(), 0..3))
            .prop_map(|(vm, sandboxes)| Config { vm, sandboxes })
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
