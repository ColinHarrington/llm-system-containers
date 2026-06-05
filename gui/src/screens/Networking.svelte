<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui } from "../lib/store.svelte";
  import { networking, listNetworkAcls } from "../lib/core";
  import type { NetworkAclInfo, NetworkingData } from "../lib/types";

  let data = $state<NetworkingData>({ networks: [], sandboxes: [] });
  let acls = $state<NetworkAclInfo[]>([]);
  let hoveredNet = $state<string | null>(null);

  $effect(() => {
    ui.dataVersion;
    void (async () => { data = await networking(); })();
  });
  $effect(() => {
    ui.dataVersion;
    void listNetworkAcls().then((a) => (acls = a)).catch(() => (acls = []));
  });

  const sbHot = (nets: string[]) => hoveredNet != null && nets.includes(hoveredNet);
</script>

<div class="content">
  <div class="flex gap8 mb16 wrap">
    <span class="chip"><Icon name="net" size={14} /> {data.networks.length} networks · {data.sandboxes.length} sandboxes</span>
    <span class="chip mono right">incus network list</span>
  </div>

  <!-- Networks -->
  <div class="grid g-3 mb16">
    {#each data.networks as n (n.name)}
      <div class="card pad net" class:hot={hoveredNet === n.name}
        role="button" tabindex="0"
        onmouseenter={() => (hoveredNet = n.name)} onmouseleave={() => (hoveredNet = null)}>
        <div class="flex gap8 mb8">
          <span class="dot ok"></span>
          <span class="mono small strong" style="color:var(--text)">{n.name}</span>
          <span class="tag">{n.kind}</span>
          {#if n.nat}
            <span class="pill ok right" title="Outbound NAT to host/internet"><Icon name="globe" size={11} /> NAT</span>
          {:else}
            <span class="pill right" title="No outbound NAT"><Icon name="ban" size={11} /> no NAT</span>
          {/if}
        </div>
        <div class="kv"><span class="k">IPv4</span><span class="v mono small">{n.ipv4}</span></div>
        <div class="kv"><span class="k">Sandboxes</span><span class="v mono small">{n.usedBy}</span></div>
      </div>
    {/each}
    {#if data.networks.length === 0}
      <div class="card"><div class="empty"><div class="icon"><Icon name="net" size={24} /></div>No managed networks — bring the VM up to see its bridges.</div></div>
    {/if}
  </div>

  <!-- Attachments -->
  <div class="card mb16">
    <div class="card-head"><h3>Attachments</h3><span class="sub">which sandbox attaches to which network · live addresses</span></div>
    {#if data.sandboxes.length === 0}
      <div class="empty"><div class="icon"><Icon name="box" size={24} /></div>No sandboxes.</div>
    {:else}
      <table class="tbl">
        <thead><tr><th>Sandbox (L2)</th><th>Networks</th><th>IPv4</th><th>Status</th></tr></thead>
        <tbody>
          {#each data.sandboxes as s (s.name)}
            <tr class:hot={sbHot(s.networks)}>
              <td><span class="mono small strong" style="color:var(--text)">{s.name}</span></td>
              <td><div class="flex gap6 wrap">{#each s.networks as net}<span class="tag mono">{net}</span>{/each}{#if s.networks.length === 0}<span class="muted small">—</span>{/if}</div></td>
              <td class="mono small">{s.ipv4}</td>
              <td>
                {#if s.status === "running"}<span class="pill ok"><span class="dot ok"></span> running</span>
                {:else}<span class="pill"><span class="dot muted"></span> stopped</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Network ACLs — the real egress-policy layer -->
  <section class="card mt16">
    <div class="card-head"><h3>Egress ACLs</h3><span class="sub">named allow/deny rulesets · <span class="mono">incus network acl</span></span></div>
    {#if acls.length === 0}
      <div class="empty"><div class="icon"><Icon name="shield" size={24} /></div>No ACLs defined yet.</div>
    {:else}
      <div class="pad acls">
        {#each acls as a (a.name)}
          <div class="acl">
            <div class="flex gap8 mb6"><span class="mono small strong" style="color:var(--text)">{a.name}</span>
              <span class="muted xsmall">{a.description}</span>
              <span class="tag right">{a.usedBy} used · {a.egress.length} egress</span></div>
            <div class="rules">
              {#each a.egress as r}
                <div class="rule">
                  <span class="act {r.action}">{r.action}</span>
                  <span class="mono small">{r.destination || "*"}{r.port ? `:${r.port}` : ""}{r.protocol ? ` (${r.protocol})` : ""}</span>
                  {#if r.description}<span class="muted xsmall right">{r.description}</span>{/if}
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Honest note: ACLs are read here, but applying/enforcing them is the remaining work. -->
  <div class="banner warn mt16">
    <Icon name="shield" size={18} />
    <span>
      ACLs above are read from Incus, but llmsc does not yet <strong>apply</strong> them to sandbox
      nics or wire mitmproxy/Zeek + Tetragon enforcement (M4). Today sandboxes share the VM bridge;
      auto-attaching egress ACLs + per-UID policy come with that work.
    </span>
  </div>
</div>

<style>
  .chip { display: inline-flex; align-items: center; gap: 6px; font-size: 11px; color: var(--text-2); border: 1px solid var(--border); border-radius: 8px; padding: 4px 9px; }
  .chip.mono { color: var(--text-3); }
  .net { transition: border-color .15s; cursor: default; }
  .net.hot { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  tr.hot td { background: var(--accent-soft); }
  .acls { display: flex; flex-direction: column; gap: 14px; }
  .acl { border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--card-2); padding: 10px 12px; }
  .rules { display: flex; flex-direction: column; gap: 3px; }
  .rule { display: flex; align-items: center; gap: 8px; font-size: 11.5px; }
  .act { font-size: 10px; font-weight: 600; text-transform: uppercase; border-radius: 4px; padding: 1px 6px; }
  .act.allow { color: var(--ok); background: var(--ok-soft); }
  .act.reject, .act.drop { color: #d9485a; background: #f871711a; }
</style>
