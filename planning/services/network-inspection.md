# Service — Network Inspection

**Priority:** Core

## Purpose

Inspect, capture, and optionally block/modify network traffic from sandbox containers —
both for **security** (control agent egress) and **observability/audit** (see exactly what
agents are calling). Can act as a proxy or VPN for capturing traffic.

## Two complementary tools

### mitmproxy — application layer (HTTP/HTTPS)

[mitmproxy](https://github.com/mitmproxy/mitmproxy):

- Intercepts, logs, and can **block or modify** HTTP/HTTPS requests.
- Python scripting API for custom rules (e.g. allowlists, redaction, policy enforcement).
- Good fit for watching and controlling what agents call over HTTP(S).

### Zeek — packet layer (passive)

[Zeek](https://github.com/zeek/zeek):

- Passive network traffic analyzer (not a proxy).
- Generates structured logs of **all** connections for audit/forensics.
- Sees beyond HTTP — full picture at the packet level.

### Together

- **mitmproxy** catches and can *block* HTTP(S) at the application layer.
- **Zeek** observes *everything* at the packet layer for audit.

## VPN / tunnel option

A WireGuard-based service could route all container egress through an inspectable tunnel for
VPN-style capture. Noted as an option; not committed.

## Relationship to security model

Network inspection complements the kernel-level network controls (Tetragon + Incus
policies) in the [security model](../security-model.md): Tetragon/Incus decide *whether* a
connection is allowed; mitmproxy/Zeek inspect and record *what* flows over allowed
connections.

## Open items

- mitmproxy TLS interception setup inside containers (cert distribution to agent users).
- Whether egress is forced through the proxy by default (default-deny direct egress).
- Zeek log volume/retention vs. memory-efficiency goal.
- WireGuard tunnel: in scope or out.
