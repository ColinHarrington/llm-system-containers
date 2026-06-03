# Networking — VM networks &amp; sandbox attachments (concept)

A view of **the VM's networks and which networks are attached to which sandboxes**, plus
egress and inspection policy. Matches the dark concept aesthetic of `mockups/nested-view`
(Tailwind CDN, inline SVG sprite, vanilla JS, no build step — opens by double-click).

```
llmsc-vm (L1)  ── outer block
  ├─ svc-net      internal services (LiteLLM / Phoenix / Grafana / SeaweedFS), no internet
  ├─ egress-net   controlled outbound · default-deny + allowlist · mitmproxy + Zeek
  └─ isolated     air-gapped / no-egress (LLM only via LiteLLM on svc-net)
       └─ vm-uplink → Internet  (reachable from egress-net only, post-inspection)
```

## What it shows

- **Topology diagram** — the L1 VM as the containing block, three named Incus bridge
  networks as center lanes, the four sandboxes (L2) attached on the left, and the host
  uplink / internet on the right. **Inline-SVG connector wires** are drawn at runtime
  between sandbox cards and the networks they're attached to:
  - solid grey = `svc-net`, animated dashed amber = inspected `egress-net`, red dead-end
    (with an X) = `isolated` / no-egress.
  - `egress-net` wires continue through `vm-uplink` to `Internet`.
- **Services on their networks** — LiteLLM, Phoenix, Grafana, SeaweedFS sit on `svc-net`;
  mitmproxy + Zeek sit on `egress-net` (placement = L1 or own L2, an isolation choice).
- **Attachment table** — each sandbox &harr; its networks &harr; **per-UID egress policy**
  &harr; whether traffic is **inspected** (mitmproxy / Zeek) &harr; LLM path (always LiteLLM
  via virtual key) &harr; profile. Per-UID rows make explicit that an `agent-*` UID and the
  human `operator` UID can carry different egress rules in the same sandbox.
- **Policy theme cards** — default-deny egress; LLM only via LiteLLM virtual keys (agents
  never reach providers directly); inspect what flows (mitmproxy blocks/modifies HTTP(S),
  Zeek passively logs all connections); kernel-level enforcement (Incus ACLs + Tetragon
  eBPF decide *whether* a connection is allowed, per container and per UID).

## Interaction

- Hover a **sandbox** to highlight its wires + light up the networks it's on and its table
  row. Hover a **network lane** to highlight every sandbox attached to it.
- Egress wires animate (marching ants) to read as live, inspected traffic.

## Realistic data (consistent with other mockups)

- Sandboxes: `web-agent-01`, `ci-runner` (both on svc-net + egress-net, inspected),
  `data-pipeline` (locked-down, svc-net only), `research-01` (air-gapped on `isolated`).
- Egress allowlist sample: github.com, *.npmjs.org, pypi.org, ghcr.io — everything else
  default-deny. LLM providers are deliberately **not** allowlisted; LiteLLM reaches them.

## Deliberately not shown

- **L3 app containers** (nested Docker/Podman) — out of scope for the networking view.

## Vocabulary

VM/L1 · sandbox/LLMSC/L2 · unprivileged · services · `llmsc`/`llmsctl` · virtual keys ·
default-deny · per-UID · Incus ACLs · Tetragon (eBPF) · mitmproxy (app-layer) · Zeek (passive).
