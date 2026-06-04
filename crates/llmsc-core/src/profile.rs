//! Agent profiles — reusable, named bundles of permission boundaries assigned to an agent.
//!
//! A profile is the human-legible layer over the low-level [security model](../planning):
//! filesystem / syscall / network ACLs, L3 nesting, LLM budget, and control-plane capability.
//! **Profiles are presets, not the enforcement** — the kernel/infra backstops (Tetragon, Incus
//! ACLs, LiteLLM) are what actually hold. Today this catalog is the definition layer; compiling a
//! profile down to those backstops is later work. See `planning/agent-profiles.md`.

/// A shipped, opinionated, least-privilege profile archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: &'static str,
    pub summary: &'static str,
    /// Filesystem access posture.
    pub filesystem: &'static str,
    /// Network egress posture.
    pub network: &'static str,
    /// Whether nested rootless app containers (L3) are allowed.
    pub l3: bool,
    /// LLM virtual-key budget tier.
    pub llm_budget: &'static str,
    /// Control-plane capability (platform actions) — `"none"` for most archetypes.
    pub control_plane: &'static str,
}

/// The shipped profile archetypes (`planning/agent-profiles.md`).
pub fn catalog() -> &'static [ProfileDef] {
    &[
        ProfileDef {
            name: "researcher",
            summary: "Read, research, gather context",
            filesystem: "RO repo + docs, RW scratch",
            network: "Web/docs allowlist via mitmproxy",
            l3: false,
            llm_budget: "generous",
            control_plane: "none",
        },
        ProfileDef {
            name: "tester",
            summary: "Run and write tests",
            filesystem: "RW repo",
            network: "Limited (package registries)",
            l3: true,
            llm_budget: "medium",
            control_plane: "none",
        },
        ProfileDef {
            name: "builder",
            summary: "Compile, build images",
            filesystem: "RW repo + artifacts",
            network: "Registry/package allowlist",
            l3: true,
            llm_budget: "medium",
            control_plane: "none",
        },
        ProfileDef {
            name: "validation",
            summary: "Run checks; never writes — strictest",
            filesystem: "Read-only everything",
            network: "None except LLM",
            l3: false,
            llm_budget: "small",
            control_plane: "none",
        },
        ProfileDef {
            name: "orchestrator",
            summary: "Drive other agents (software-factory)",
            filesystem: "Minimal (own scratch)",
            network: "None raw (internal coordination only)",
            l3: false,
            llm_budget: "broad",
            control_plane: "launch/stop sandboxes, coordinate agents",
        },
    ]
}

/// Look up a profile archetype by name.
pub fn lookup(name: &str) -> Option<&'static ProfileDef> {
    catalog().iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_the_five_archetypes() {
        let names: Vec<_> = catalog().iter().map(|p| p.name).collect();
        assert_eq!(names, ["researcher", "tester", "builder", "validation", "orchestrator"]);
    }

    #[test]
    fn validation_is_strictest_and_orchestrator_has_control_plane() {
        assert_eq!(lookup("validation").unwrap().network, "None except LLM");
        assert!(!lookup("validation").unwrap().l3);
        assert_ne!(lookup("orchestrator").unwrap().control_plane, "none");
        assert!(lookup("nope").is_none());
    }
}
