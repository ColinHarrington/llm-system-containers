<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { networking } from "../lib/core";
  import type { NetId, NetSandbox, NetUid } from "../lib/types";

  let sandboxes = $state<NetSandbox[]>([]);
  let sectionEl = $state<HTMLElement | null>(null);
  let edges = $state<{ key: string; d: string; color: string; dashed: boolean; sb?: string; net?: string }[]>([]);
  let crossPath = $state("");
  let resizeTick = $state(0);
  let hoveredSb = $state<string | null>(null);
  let hoveredNet = $state<string | null>(null);

  const NETS: Record<NetId, { color: string; label: string }> = {
    "svc-net": { color: "#94a3b8", label: "svc-net" },
    "egress-net": { color: "#f59e0b", label: "egress-net" },
    "isolated": { color: "#f87171", label: "isolated" },
  };

  $effect(() => {
    void (async () => { sandboxes = await networking(); })();
  });

  function rectIn(el: Element, c: DOMRect) {
    const r = el.getBoundingClientRect();
    return { x: r.left - c.left, y: r.top - c.top, w: r.width, h: r.height, cy: r.top - c.top + r.height / 2 };
  }
  function curveRight(a: ReturnType<typeof rectIn>, b: ReturnType<typeof rectIn>) {
    const x1 = a.x + a.w, y1 = a.cy, x2 = b.x, y2 = b.cy, mx = (x1 + x2) / 2;
    return `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`;
  }

  // Recompute connector wires whenever data/layout changes.
  $effect(() => {
    sandboxes; resizeTick; // deps
    const sec = sectionEl;
    if (!sec || sandboxes.length === 0) return;
    const id = requestAnimationFrame(() => {
      const c = sec.getBoundingClientRect();
      const q = (sel: string) => sec.querySelector(sel);
      const out: typeof edges = [];
      for (const sb of sandboxes) {
        const sbEl = q(`#net-sb-${sb.name}`);
        if (!sbEl) continue;
        for (const n of sb.nets) {
          const netEl = q(`#net-${n}`);
          if (!netEl) continue;
          out.push({ key: `${sb.name}-${n}`, d: curveRight(rectIn(sbEl, c), rectIn(netEl, c)), color: NETS[n].color, dashed: n === "egress-net", sb: sb.name, net: n });
        }
      }
      const egEl = q("#net-egress-net"), upEl = q("#net-uplink"), inEl = q("#net-internet"), isoEl = q("#net-isolated");
      if (egEl && upEl) {
        const eg = rectIn(egEl, c), up = rectIn(upEl, c);
        out.push({ key: "eg-up", d: curveRight(eg, up), color: "#f59e0b", dashed: true, net: "egress-net" });
        if (inEl) {
          const inr = rectIn(inEl, c);
          out.push({ key: "up-in", d: `M ${up.x + up.w / 2} ${up.y + up.h} L ${inr.x + inr.w / 2} ${inr.y}`, color: "#f59e0b", dashed: true, net: "egress-net" });
        }
      }
      if (isoEl) {
        const iso = rectIn(isoEl, c);
        const x1 = iso.x + iso.w, y = iso.cy, x2 = x1 + 34;
        out.push({ key: "iso-stub", d: `M ${x1} ${y} L ${x2} ${y}`, color: "#f87171", dashed: false, net: "isolated" });
        crossPath = `M ${x2 - 5} ${y - 5} L ${x2 + 5} ${y + 5} M ${x2 + 5} ${y - 5} L ${x2 - 5} ${y + 5}`;
      }
      edges = out;
    });
    return () => cancelAnimationFrame(id);
  });

  $effect(() => {
    const onResize = () => (resizeTick = resizeTick + 1);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  function edgeOpacity(e: { sb?: string; net?: string }): number {
    if (hoveredSb) return e.sb === hoveredSb ? 0.95 : 0.12;
    if (hoveredNet) return e.net === hoveredNet ? 0.95 : 0.12;
    return 0.55;
  }
  const sbHot = (name: string) =>
    hoveredSb === name || (hoveredNet != null && (sandboxes.find((s) => s.name === name)?.nets.includes(hoveredNet as NetId) ?? false));
  const netHot = (n: NetId) =>
    hoveredNet === n || (hoveredSb != null && (sandboxes.find((s) => s.name === hoveredSb)?.nets.includes(n) ?? false));

  function policyClass(egress: string): string {
    if (/no-egress/.test(egress)) return "p-rose";
    if (/deny-all/.test(egress)) return "p-rose";
    if (/broad/.test(egress)) return "p-sky";
    if (/allowlist/.test(egress)) return "p-amber";
    return "";
  }
  const netPill = (n: NetId) => NETS[n];
</script>

<div class="content">
  <div class="flex gap8 mb16 wrap">
    <span class="chip"><Icon name="shield" size={14} /> Default-deny egress</span>
    <span class="chip"><Icon name="cpu" size={14} /> Enforced by Tetragon · Incus ACLs</span>
    <button class="btn sm mono right">llmsctl net</button>
  </div>

  <div class="legend mb16">
    <span class="flex gap6"><span class="line" style="background:#94a3b8"></span>internal services (svc-net)</span>
    <span class="flex gap6"><span class="line" style="background:#f59e0b"></span>controlled egress (inspected)</span>
    <span class="flex gap6"><span class="line" style="background:#f87171"></span>blocked / no-egress</span>
    <span class="flex gap6"><Icon name="eye" size={14} /> mitmproxy + Zeek inspection</span>
  </div>

  <!-- Topology diagram -->
  <section class="diagram" bind:this={sectionEl}>
    <div class="flex gap10 mb12" style="padding:0 4px">
      <span class="dot ok pulse"></span>
      <div>
        <div class="flex gap8"><span class="strong" style="color:var(--text)">llmsc-vm</span><span class="tag">L1 · VM</span></div>
        <div class="muted xsmall mono mt4">running · Incus-managed bridges & ACLs</div>
      </div>
      <div class="right small t2"><span class="strong" style="color:var(--text)">3</span> networks · <span class="strong" style="color:var(--text)">{sandboxes.length}</span> sandboxes</div>
    </div>

    <svg class="wires" aria-hidden="true">
      {#each edges as e (e.key)}
        <path d={e.d} fill="none" stroke={e.color} stroke-width={edgeOpacity(e) > 0.7 ? 3 : 2}
          stroke-linecap="round" class:flow={e.dashed} style="opacity:{edgeOpacity(e)}" />
      {/each}
      {#if crossPath}<path d={crossPath} stroke="#f87171" stroke-width="2.2" stroke-linecap="round" fill="none" />{/if}
    </svg>

    <div class="cols">
      <!-- LEFT: sandboxes -->
      <div class="col">
        <div class="collabel">Sandboxes · L2</div>
        {#each sandboxes as sb (sb.name)}
          <div id="net-sb-{sb.name}" class="node" class:hot={sbHot(sb.name)}
            role="button" tabindex="0"
            onmouseenter={() => (hoveredSb = sb.name)} onmouseleave={() => (hoveredSb = null)}>
            <div class="flex gap6 mb4">
              <span class="dot ok"></span>
              <span class="mono small strong" style="color:var(--text)">{sb.name}</span>
              <span class="tag">L2</span>
            </div>
            <div class="muted xsmall mono mb8">{sb.image}</div>
            <div class="flex gap6 wrap mb8">
              {#each sb.nets as n}
                <span class="netpill" style="color:{netPill(n).color};border-color:{netPill(n).color}55;background:{netPill(n).color}14">{netPill(n).label}</span>
              {/each}
            </div>
            <div class="flex">
              {#if sb.inspected}
                <span class="insp"><Icon name="eye" size={12} /> inspected</span>
              {:else}
                <span class="muted xsmall">not inspected</span>
              {/if}
              <span class="right muted xsmall">{sb.uids.length} UIDs</span>
            </div>
          </div>
        {/each}
      </div>

      <!-- CENTER: networks -->
      <div class="col">
        <div class="collabel center">VM networks · Incus bridges</div>

        <div id="net-svc-net" class="lane" class:hot={netHot("svc-net")} style="--lane:#94a3b8"
          role="button" tabindex="0" onmouseenter={() => (hoveredNet = "svc-net")} onmouseleave={() => (hoveredNet = null)}>
          <div class="flex gap6 mb8">
            <span class="ldot" style="background:#94a3b8"></span>
            <span class="mono small" style="color:var(--text)">svc-net</span>
            <span class="tag mono">internal · 10.71.10.0/24</span>
            <span class="right nochip rose"><Icon name="ban" size={11} /> no internet</span>
          </div>
          <p class="lanetext">Sandboxes reach internal services only. LLM access is brokered here by LiteLLM (virtual keys) — agents never reach providers directly.</p>
          <div class="flex gap6 wrap">
            {#each ["LiteLLM", "Phoenix", "Grafana", "SeaweedFS"] as s}<span class="svcnode">{s}</span>{/each}
          </div>
        </div>

        <div id="net-egress-net" class="lane amber" class:hot={netHot("egress-net")} style="--lane:#f59e0b"
          role="button" tabindex="0" onmouseenter={() => (hoveredNet = "egress-net")} onmouseleave={() => (hoveredNet = null)}>
          <div class="flex gap6 mb8">
            <span class="ldot" style="background:#f59e0b"></span>
            <span class="mono small" style="color:var(--text)">egress-net</span>
            <span class="tag mono">controlled · 10.71.20.0/24</span>
            <span class="right nochip amber"><Icon name="lock" size={11} /> default-deny + allowlist</span>
          </div>
          <p class="lanetext">All outbound forced through the inspection chain. HTTP(S) terminated & policy-checked by mitmproxy; every connection logged by Zeek.</p>
          <div class="flex gap6 wrap">
            <span class="svcnode amber"><Icon name="shield" size={12} /> mitmproxy</span>
            <span class="svcnode amber"><Icon name="eye" size={12} /> Zeek</span>
          </div>
        </div>

        <div id="net-isolated" class="lane rose" class:hot={netHot("isolated")} style="--lane:#f87171"
          role="button" tabindex="0" onmouseenter={() => (hoveredNet = "isolated")} onmouseleave={() => (hoveredNet = null)}>
          <div class="flex gap6 mb8">
            <span class="ldot" style="background:#f87171"></span>
            <span class="mono small" style="color:var(--text)">isolated</span>
            <span class="tag mono">no-egress · air-gapped</span>
            <span class="right nochip rose"><Icon name="ban" size={11} /> no outbound</span>
          </div>
          <p class="lanetext">For sensitive sandboxes. The only path off-box is LLM inference via LiteLLM on svc-net — nothing else, no general internet.</p>
        </div>
      </div>

      <!-- RIGHT: uplink / internet -->
      <div class="col">
        <div class="collabel center">Host uplink</div>
        <div id="net-uplink" class="node">
          <div class="flex gap6 mb4"><Icon name="globe" size={15} /><span class="mono small" style="color:var(--text)">vm-uplink</span></div>
          <div class="muted xsmall">NAT bridge to the host's network → internet. Reachable from egress-net only, post-inspection.</div>
        </div>
        <div id="net-internet" class="node muted-node">
          <div class="flex gap6 mb4"><Icon name="globe" size={15} /><span class="small" style="color:var(--text)">Internet</span></div>
          <div class="muted xsmall">Only allowlisted destinations are reachable. LLM providers are reached for the agent by LiteLLM.</div>
        </div>
        <div class="node">
          <div class="xsmall muted mb8" style="text-transform:uppercase;letter-spacing:.06em">Allowlist · egress-net</div>
          <ul class="allow">
            <li class="ok"><Icon name="check" size={12} /> github.com</li>
            <li class="ok"><Icon name="check" size={12} /> *.npmjs.org</li>
            <li class="ok"><Icon name="check" size={12} /> pypi.org</li>
            <li class="ok"><Icon name="check" size={12} /> ghcr.io</li>
            <li class="no"><Icon name="ban" size={12} /> * (default-deny)</li>
          </ul>
        </div>
      </div>
    </div>

    <p class="xsmall muted mt16" style="padding:0 4px">Hover a sandbox or a network to highlight its attachments. Grey = svc-net, amber = inspected egress, red = blocked.</p>
  </section>

  <!-- Attachment table -->
  <section class="card mt16">
    <div class="card-head"><h3>Attachments & policy</h3><span class="sub">sandbox ↔ networks ↔ egress ↔ inspection · per-container and per-UID</span></div>
    <table class="tbl">
      <thead><tr><th>Sandbox (L2)</th><th>Networks</th><th>Per-UID egress policy</th><th>Inspected</th><th>LLM path</th><th style="text-align:right">Profile</th></tr></thead>
      <tbody>
        {#each sandboxes as sb (sb.name)}
          <tr class:hot={hoveredSb === sb.name}>
            <td><div class="flex gap6"><span class="dot ok" style="width:6px;height:6px"></span><span class="mono small strong" style="color:var(--text)">{sb.name}</span></div>
              <div class="muted xsmall mono mt4" style="padding-left:14px">{sb.image}</div></td>
            <td><div class="flex gap6 wrap">{#each sb.nets as n}<span class="netpill" style="color:{netPill(n).color};border-color:{netPill(n).color}55;background:{netPill(n).color}14">{netPill(n).label}</span>{/each}</div></td>
            <td>{#each sb.uids as u}<div class="uidrow"><span class="mono xsmall uidname" class:human={u.kind === "human"}>{u.uid}</span><span class="pol {policyClass(u.egress)}">{u.egress}</span></div>{/each}</td>
            <td class="small">{#if sb.inspected}<span class="insp"><Icon name="eye" size={13} /> mitmproxy + Zeek</span>{:else}<span class="muted flex gap6"><Icon name="ban" size={13} /> n/a</span>{/if}</td>
            <td class="small"><span class="llmpath"><Icon name="llm" size={13} /> {sb.llm} <span class="muted">· vkey</span></span></td>
            <td style="text-align:right"><span class="tag mono">{sb.profile}</span></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>

  <!-- Policy themes -->
  <div class="grid mt16" style="grid-template-columns:repeat(auto-fit,minmax(250px,1fr))">
    <div class="card pad"><div class="flex gap8 mb8"><Icon name="lock" size={16} /><span class="strong">Default-deny egress</span></div>
      <p class="small t2">No sandbox reaches the internet by default. Outbound is opt-in via an allowlist on egress-net; everything else is dropped.</p></div>
    <div class="card pad"><div class="flex gap8 mb8"><Icon name="llm" size={16} /><span class="strong">LLM only via virtual keys</span></div>
      <p class="small t2">Agents call LiteLLM on svc-net with virtual keys. Real provider credentials live only in the proxy — agents never hold them or reach providers directly.</p></div>
    <div class="card pad"><div class="flex gap8 mb8"><Icon name="eye" size={16} /><span class="strong">Inspect what flows</span></div>
      <p class="small t2">mitmproxy can block/modify HTTP(S) at the app layer; Zeek passively logs every connection at the packet layer for audit.</p></div>
    <div class="card pad"><div class="flex gap8 mb8"><Icon name="cpu" size={16} /><span class="strong">Kernel-level enforcement</span></div>
      <p class="small t2">Incus network ACLs and Tetragon (eBPF) decide whether a connection is allowed — per container, per UID, non-bypassable from userspace.</p></div>
  </div>
</div>

<style>
  .chip { display: inline-flex; align-items: center; gap: 6px; font-size: 11px; color: var(--text-2); border: 1px solid var(--border); border-radius: 8px; padding: 4px 9px; }
  .legend { display: flex; flex-wrap: wrap; gap: 8px 20px; font-size: 11px; color: var(--text-3); }
  .legend .line { width: 22px; height: 2px; border-radius: 2px; display: inline-block; }
  .diagram { position: relative; border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--card-2); padding: 16px; overflow: hidden; }
  .wires { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; z-index: 1; }
  .flow { stroke-dasharray: 4 6; animation: dash 1.1s linear infinite; }
  @keyframes dash { to { stroke-dashoffset: -16; } }
  .cols { position: relative; z-index: 2; display: grid; grid-template-columns: 260px 1fr 220px; gap: 24px; margin-top: 12px; }
  .col { display: flex; flex-direction: column; gap: 12px; }
  .collabel { font-size: 10px; text-transform: uppercase; letter-spacing: .06em; color: var(--text-3); padding: 0 4px; }
  .collabel.center { text-align: center; }
  .node { border: 1px solid var(--border); border-radius: var(--radius); background: var(--card); padding: 12px; transition: border-color .15s, box-shadow .15s; }
  .node.hot { box-shadow: 0 0 0 1.5px #38bdf8; border-color: #38bdf8; }
  .muted-node { background: var(--card-2); }
  .lane { border: 1px solid var(--border); border-radius: var(--radius); background: var(--card); padding: 12px; transition: box-shadow .15s; }
  .lane.amber { border-color: #f59e0b55; background: #f59e0b0d; }
  .lane.rose { border-color: #f8717155; background: #f871710d; }
  .lane.hot { box-shadow: 0 0 0 1.5px var(--lane); }
  .ldot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; flex: none; }
  .lanetext { font-size: 11px; color: var(--text-3); margin: 0 0 8px; line-height: 1.45; }
  .nochip { font-size: 10px; display: inline-flex; align-items: center; gap: 4px; border-radius: 6px; padding: 1px 6px; border: 1px solid; }
  .nochip.rose { color: #d9485a; border-color: #f8717155; background: #f871711a; }
  .nochip.amber { color: #b5790f; border-color: #f59e0b55; background: #f59e0b1a; }
  .netpill { font-size: 9.5px; font-family: var(--mono); border-radius: 5px; padding: 1px 5px; border: 1px solid; }
  .svcnode { font-size: 11px; color: var(--text-2); border: 1px solid var(--border); border-radius: 8px; padding: 3px 8px; display: inline-flex; align-items: center; gap: 6px; background: var(--card-2); }
  .svcnode::before { content: ""; width: 6px; height: 6px; border-radius: 50%; background: var(--ok); }
  .svcnode.amber { color: #b5790f; border-color: #f59e0b55; }
  .svcnode.amber::before { background: #f59e0b; }
  .insp { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: #b5790f; }
  .allow { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; font-family: var(--mono); font-size: 11px; }
  .allow li { display: flex; align-items: center; gap: 6px; }
  .allow li.ok { color: var(--ok); }
  .allow li.no { color: #d9485a; }
  .uidrow { display: flex; align-items: center; gap: 8px; padding: 2px 0; }
  .uidname { width: 110px; flex: none; color: var(--text-2); }
  .uidname.human { color: var(--accent-strong); }
  .pol { font-size: 10px; border: 1px solid var(--border); border-radius: 5px; padding: 1px 6px; color: var(--text-2); }
  .pol.p-rose { color: #d9485a; border-color: #f8717155; background: #f871710d; }
  .pol.p-sky { color: #2f8fd0; border-color: #38bdf855; background: #38bdf80d; }
  .pol.p-amber { color: #b5790f; border-color: #f59e0b55; background: #f59e0b0d; }
  .llmpath { display: inline-flex; align-items: center; gap: 4px; color: #2f8fd0; }
  tr.hot { background: #38bdf814; }
</style>
