//! Service model + catalog.
//!
//! Services (LLM proxy, observability, storage, …) are shared infrastructure. Each runs either
//! in the **L1 VM** or in its own **L2 container** — a placement/isolation choice, not a layer.
//! See `planning/services/README.md`.

use serde::{Deserialize, Serialize};

/// Where a service runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    /// In its own L2 container (default — better isolation, routable interface).
    #[default]
    Container,
    /// Directly in the L1 VM (lighter, less isolation).
    Vm,
}

/// An enabled service (recorded in config).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    #[serde(default)]
    pub placement: Placement,
}

/// A known service the platform can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub default_placement: Placement,
    /// MVP / Core / Optional / Future (mirrors planning/services/README.md).
    pub priority: &'static str,
}

/// The known service catalog.
pub fn catalog() -> &'static [CatalogEntry] {
    &[
        CatalogEntry {
            name: "litellm",
            description: "LLM proxy — agents use virtual keys; real credentials never exposed",
            default_placement: Placement::Container,
            priority: "MVP",
        },
        CatalogEntry {
            name: "phoenix",
            description: "LLM/agent observability — traces, token usage",
            default_placement: Placement::Container,
            priority: "MVP",
        },
        CatalogEntry {
            name: "grafana",
            description: "Dashboards over metrics + logs",
            default_placement: Placement::Container,
            priority: "MVP",
        },
        CatalogEntry {
            name: "seaweedfs",
            description: "Durable, versioned, mountable shared storage",
            default_placement: Placement::Container,
            priority: "Core",
        },
        CatalogEntry {
            name: "mitmproxy",
            description: "Network inspection / egress proxy",
            default_placement: Placement::Container,
            priority: "Core",
        },
        CatalogEntry {
            name: "forgejo",
            description: "Internal git platform",
            default_placement: Placement::Container,
            priority: "Optional",
        },
    ]
}

/// Look up a catalog entry by name.
pub fn lookup(name: &str) -> Option<&'static CatalogEntry> {
    catalog().iter().find(|e| e.name == name)
}

/// Prefix for service container names inside the VM (e.g. `svc-litellm`).
///
/// Services are shared infrastructure, **never sandboxes**. This convention is the single source
/// of truth for telling the two apart, so sandbox listings can exclude service containers.
pub const CONTAINER_PREFIX: &str = "svc-";

/// The L2 container name for a service (e.g. `litellm` → `svc-litellm`).
pub fn container_name(service: &str) -> String {
    format!("{CONTAINER_PREFIX}{service}")
}

/// Whether an Incus instance name belongs to a service container (so: not a sandbox).
pub fn is_service_container(instance: &str) -> bool {
    instance.starts_with(CONTAINER_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_mvp_services() {
        assert!(lookup("litellm").is_some());
        assert!(lookup("phoenix").is_some());
        assert_eq!(lookup("litellm").unwrap().priority, "MVP");
    }

    #[test]
    fn unknown_service_is_none() {
        assert!(lookup("nope").is_none());
    }

    #[test]
    fn default_placement_is_container() {
        assert_eq!(Placement::default(), Placement::Container);
    }

    #[test]
    fn service_container_naming_roundtrips() {
        assert_eq!(container_name("litellm"), "svc-litellm");
        assert!(is_service_container("svc-litellm"));
        assert!(is_service_container(&container_name("phoenix")));
    }

    #[test]
    fn sandboxes_are_not_service_containers() {
        assert!(!is_service_container("web-agent-01"));
        assert!(!is_service_container("ci-runner"));
    }
}
