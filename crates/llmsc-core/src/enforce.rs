//! Compile an egress policy into an Incus network ACL + nic binding — the **per-container**
//! enforcement ring (`planning/security-model.md`). Pure (no I/O); the applier lives in
//! [`crate::incus`]. Per-UID egress is a later Tetragon ring that compiles from each agent's
//! [`crate::config::Guardrails::network`].
//!
//! **Scope:** Incus ACL rules are L3/L4 only (CIDR + port + protocol). Domain/HTTP allowlists
//! (e.g. "github.com only") are mitmproxy's job (a later ring), so a named set like `web` is
//! coarse here (any host on the given port). The `llm` set resolves to the bridge subnet in v1
//! (east-west reaches the LiteLLM service container) — a precise service-IP/ACL-subject lookup
//! is a TODO.

use crate::config::{EgressPosture, Sandbox};
use crate::incus::{AclOp, AclRule, NetworkAcl};
use std::collections::BTreeMap;

/// Context for resolving named destination sets to concrete L3/L4 rules.
#[derive(Debug, Clone)]
pub struct EnforceCtx {
    /// The Incus bridge the sandbox's nic attaches to (e.g. `incusbr0`).
    pub bridge: String,
    /// The bridge IPv4 network in CIDR form (e.g. `10.0.0.0/24`) — the coarse v1 destination for
    /// the `llm` named set. Host bits are normalized away by [`cidr_network`].
    pub bridge_subnet: String,
}

/// The deterministic ACL name for a sandbox's egress policy.
pub fn egress_acl_name(sandbox: &str) -> String {
    format!("llmsc-egress-{sandbox}")
}

fn rule(action: &str, destination: &str, port: &str, protocol: &str, description: &str) -> AclRule {
    AclRule {
        action: action.to_string(),
        source: String::new(),
        destination: destination.to_string(),
        port: port.to_string(),
        protocol: protocol.to_string(),
        description: description.to_string(),
    }
}

/// Zero the host bits of an IPv4 CIDR (`10.1.2.3/24` → `10.1.2.0/24`) so it is a valid network
/// destination. Returns the input unchanged if it is not a parseable IPv4 CIDR (e.g. IPv6).
pub fn cidr_network(cidr: &str) -> String {
    let Some((addr, prefix)) = cidr.split_once('/') else {
        return cidr.to_string();
    };
    let (Ok(octets), Ok(bits)) = (
        addr.split('.')
            .map(|o| o.parse::<u8>())
            .collect::<Result<Vec<_>, _>>(),
        prefix.parse::<u32>(),
    ) else {
        return cidr.to_string();
    };
    if octets.len() != 4 || bits > 32 {
        return cidr.to_string();
    }
    let ip = octets
        .iter()
        .fold(0u32, |acc, &o| (acc << 8) | u32::from(o));
    let mask = if bits == 0 {
        0
    } else {
        u32::MAX << (32 - bits)
    };
    let net = ip & mask;
    format!(
        "{}.{}.{}.{}/{bits}",
        (net >> 24) & 0xff,
        (net >> 16) & 0xff,
        (net >> 8) & 0xff,
        net & 0xff
    )
}

/// Resolve one allow entry — a named set (`llm`, `package-registries`, `web`) or a raw
/// `CIDR:port[/proto]` (IPv4; `proto` defaults to `tcp`) — into zero or more egress allow rules.
fn resolve_allow(entry: &str, ctx: &EnforceCtx) -> Vec<AclRule> {
    match entry {
        "llm" => vec![rule(
            "allow",
            &cidr_network(&ctx.bridge_subnet),
            "4000",
            "tcp",
            "LLM proxy (coarse: bridge subnet:4000)",
        )],
        "package-registries" => vec![rule(
            "allow",
            "0.0.0.0/0",
            "443",
            "tcp",
            "package registries (coarse: any:443)",
        )],
        "web" => vec![
            rule("allow", "0.0.0.0/0", "443", "tcp", "web (coarse: any:443)"),
            rule("allow", "0.0.0.0/0", "80", "tcp", "web (coarse: any:80)"),
        ],
        raw => parse_raw(raw).into_iter().collect(),
    }
}

/// Parse a raw `CIDR:port[/proto]` allow entry (IPv4). `None` if there is no destination.
fn parse_raw(s: &str) -> Option<AclRule> {
    let (dest, rest) = match s.rsplit_once(':') {
        Some((d, r)) => (d, Some(r)),
        None => (s, None),
    };
    if dest.trim().is_empty() {
        return None;
    }
    let (port, proto) = match rest {
        Some(r) => match r.split_once('/') {
            Some((p, pr)) => (p.to_string(), pr.to_string()),
            None => (r.to_string(), "tcp".to_string()),
        },
        None => (String::new(), "tcp".to_string()),
    };
    Some(rule("allow", &cidr_network(dest), &port, &proto, "custom"))
}

/// Compile a sandbox's egress policy into its Incus network ACL. `None` when the sandbox is
/// unmanaged or `Open` (no ACL is created or bound). For `DenyAll` the ACL has no allow rules —
/// the nic's default-drop ([`egress_nic_device`]) does the dropping.
pub fn egress_acl(sandbox: &Sandbox, ctx: &EnforceCtx) -> Option<NetworkAcl> {
    let policy = sandbox.egress.as_ref()?;
    let egress = match policy.posture {
        EgressPosture::Open => return None,
        EgressPosture::DenyAll => Vec::new(),
        EgressPosture::Allowlist => policy
            .allow
            .iter()
            .flat_map(|e| resolve_allow(e, ctx))
            .collect(),
    };
    Some(NetworkAcl {
        name: egress_acl_name(&sandbox.name),
        description: format!("llmsc-managed egress for {}", sandbox.name),
        used_by: 0,
        ingress: Vec::new(),
        egress,
    })
}

/// The instance-local `eth0` device override that binds the ACL with a default-drop egress
/// posture. Added to `sandbox.devices` so it converges through `reconcile::converge_plan`.
pub fn egress_nic_device(acl_name: &str, bridge: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("type".to_string(), "nic".to_string()),
        ("network".to_string(), bridge.to_string()),
        ("security.acls".to_string(), acl_name.to_string()),
        (
            "security.acls.default.egress.action".to_string(),
            "drop".to_string(),
        ),
    ])
}

/// A rule's identity for idempotent diffing — action + destination + port + protocol (the
/// description is cosmetic and may be normalized by Incus on read-back, so it is excluded).
fn rule_key(r: &AclRule) -> (String, String, String, String) {
    (
        r.action.clone(),
        r.destination.clone(),
        r.port.clone(),
        r.protocol.clone(),
    )
}

/// Diff a compiled egress ACL against the live one (read back via `incus::parse_network_acls`)
/// into the ops that converge it. Only `egress` is managed; ingress is left untouched. Pure.
pub fn egress_acl_plan(desired: &NetworkAcl, live: Option<&NetworkAcl>) -> Vec<AclOp> {
    let mut plan = Vec::new();
    if live.is_none() {
        plan.push(AclOp::Create);
    }
    let empty = Vec::new();
    let live_egress = live.map(|a| &a.egress).unwrap_or(&empty);
    let live_keys: Vec<_> = live_egress.iter().map(rule_key).collect();
    let want_keys: Vec<_> = desired.egress.iter().map(rule_key).collect();

    for r in &desired.egress {
        if !live_keys.contains(&rule_key(r)) {
            plan.push(AclOp::AddRule {
                direction: "egress".to_string(),
                rule: r.clone(),
            });
        }
    }
    for r in live_egress {
        if !want_keys.contains(&rule_key(r)) {
            plan.push(AclOp::RemoveRule {
                direction: "egress".to_string(),
                rule: r.clone(),
            });
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EgressPolicy, EgressPosture};

    fn ctx() -> EnforceCtx {
        EnforceCtx {
            bridge: "incusbr0".to_string(),
            bridge_subnet: "10.21.32.1/24".to_string(),
        }
    }

    fn sb_with(policy: Option<EgressPolicy>) -> Sandbox {
        Sandbox {
            name: "web-agent-01".to_string(),
            image: "images:alpine/3.21".to_string(),
            egress: policy,
            ..Default::default()
        }
    }

    #[test]
    fn cidr_network_zeroes_host_bits() {
        assert_eq!(cidr_network("10.21.32.1/24"), "10.21.32.0/24");
        assert_eq!(cidr_network("0.0.0.0/0"), "0.0.0.0/0");
        assert_eq!(cidr_network("192.168.5.130/25"), "192.168.5.128/25");
        // Non-IPv4 / unparseable inputs pass through untouched.
        assert_eq!(cidr_network("fd00::/8"), "fd00::/8");
        assert_eq!(cidr_network("nonsense"), "nonsense");
    }

    #[test]
    fn unmanaged_and_open_compile_to_no_acl() {
        assert!(egress_acl(&sb_with(None), &ctx()).is_none());
        assert!(egress_acl(
            &sb_with(Some(EgressPolicy {
                posture: EgressPosture::Open,
                allow: vec![]
            })),
            &ctx()
        )
        .is_none());
    }

    #[test]
    fn deny_all_compiles_to_empty_egress() {
        let acl = egress_acl(
            &sb_with(Some(EgressPolicy {
                posture: EgressPosture::DenyAll,
                allow: vec![],
            })),
            &ctx(),
        )
        .unwrap();
        assert_eq!(acl.name, "llmsc-egress-web-agent-01");
        assert!(acl.egress.is_empty()); // nic default-drop does the work
    }

    #[test]
    fn allowlist_resolves_named_sets() {
        let acl = egress_acl(&sb_with(Some(EgressPolicy::default_managed())), &ctx()).unwrap();
        // default_managed = allowlist [llm] → one allow to the normalized bridge subnet:4000.
        assert_eq!(acl.egress.len(), 1);
        let r = &acl.egress[0];
        assert_eq!(r.action, "allow");
        assert_eq!(r.destination, "10.21.32.0/24");
        assert_eq!(r.port, "4000");
        assert_eq!(r.protocol, "tcp");
    }

    #[test]
    fn allowlist_resolves_web_and_raw() {
        let acl = egress_acl(
            &sb_with(Some(EgressPolicy {
                posture: EgressPosture::Allowlist,
                allow: vec!["web".to_string(), "192.168.0.0/16:8080".to_string()],
            })),
            &ctx(),
        )
        .unwrap();
        // web → :443 + :80, raw → :8080/tcp.
        assert_eq!(acl.egress.len(), 3);
        let ports: Vec<_> = acl.egress.iter().map(|r| r.port.as_str()).collect();
        assert!(ports.contains(&"443") && ports.contains(&"80") && ports.contains(&"8080"));
        let raw = acl.egress.iter().find(|r| r.port == "8080").unwrap();
        assert_eq!(raw.destination, "192.168.0.0/16");
        assert_eq!(raw.protocol, "tcp");
    }

    #[test]
    fn plan_creates_when_missing_then_is_idempotent() {
        let acl = egress_acl(&sb_with(Some(EgressPolicy::default_managed())), &ctx()).unwrap();

        // Missing live → Create + AddRule for the one allow.
        let plan = egress_acl_plan(&acl, None);
        assert!(plan.contains(&AclOp::Create));
        assert_eq!(
            plan.iter()
                .filter(|op| matches!(op, AclOp::AddRule { .. }))
                .count(),
            1
        );

        // Live == desired → no-op (rules match by identity).
        let plan = egress_acl_plan(&acl, Some(&acl));
        assert!(plan.is_empty());
    }

    #[test]
    fn plan_adds_and_removes_drifted_rules() {
        let desired = egress_acl(&sb_with(Some(EgressPolicy::default_managed())), &ctx()).unwrap();
        // Live ACL exists but has a stale rule and not the desired one.
        let live = NetworkAcl {
            name: desired.name.clone(),
            description: String::new(),
            used_by: 1,
            ingress: vec![],
            egress: vec![rule("allow", "203.0.113.0/24", "22", "tcp", "stale")],
        };
        let plan = egress_acl_plan(&desired, Some(&live));
        assert!(!plan.contains(&AclOp::Create)); // already exists
        assert!(plan
            .iter()
            .any(|op| matches!(op, AclOp::AddRule { rule, .. } if rule.port == "4000")));
        assert!(plan
            .iter()
            .any(|op| matches!(op, AclOp::RemoveRule { rule, .. } if rule.port == "22")));
    }

    #[test]
    fn nic_device_binds_acl_with_default_drop() {
        let d = egress_nic_device("llmsc-egress-x", "incusbr0");
        assert_eq!(d.get("type").map(String::as_str), Some("nic"));
        assert_eq!(
            d.get("security.acls").map(String::as_str),
            Some("llmsc-egress-x")
        );
        assert_eq!(
            d.get("security.acls.default.egress.action")
                .map(String::as_str),
            Some("drop")
        );
    }
}
