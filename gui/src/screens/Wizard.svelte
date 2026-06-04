<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { navigate } from "../lib/store.svelte";
  import { listServices, createPlatform, operatorDefault, SERVICE_META } from "../lib/core";
  import type { ServiceEntry } from "../lib/types";

  const steps = [
    { st: "Resources", sd: "CPU · memory · disk" },
    { st: "Services", sd: "Pick & place" },
    { st: "Networking", sd: "Egress & inspection" },
    { st: "Review & create", sd: "Confirm setup" },
  ];
  let step = $state(0);

  let operator = $state("");
  let cpus = $state(8);
  let memoryGib = $state(16);
  let diskGib = $state(120);
  let services = $state<ServiceEntry[]>([]);
  let egress = $state<"Allowlist" | "Inspect-all" | "Locked down">("Allowlist");
  let routeLlm = $state(true);
  let inspect = $state(true);
  let tetragon = $state(true);
  let subnet = $state("10.118.0.0/24");
  let creating = $state(false);
  let done = $state(false);

  $effect(() => {
    void (async () => {
      services = await listServices();
      if (!operator) operator = await operatorDefault();
    })();
  });

  const enabled = $derived(services.filter((s) => s.enabled).map((s) => s.name));
  const defaultDenyEgress = $derived(egress !== "Inspect-all");
  const meta = (name: string) => SERVICE_META[name] ?? { initials: name.slice(0, 2), color: "#8a90a3", placement: "service" };

  function goto(i: number) { step = Math.max(0, Math.min(steps.length - 1, i)); }
  function next() { if (step < steps.length - 1) goto(step + 1); }
  function back() { goto(step - 1); }

  async function create() {
    creating = true;
    try {
      await createPlatform({ operator: operator.trim(), cpus, memoryGib, diskGib, services: enabled, defaultDenyEgress });
      done = true;
    } finally { creating = false; }
  }
</script>

<div class="content">
  <div class="banner info mb16">
    <Icon name="shield" size={18} />
    <span>This sets up <strong>llmsc-vm</strong> — the host-native VM (L1) that runs Incus and hosts your sandboxes. Equivalent to <span class="mono">llmsctl init</span>.</span>
  </div>

  {#if done}
    <div class="card pad center" style="max-width:520px">
      <div class="empty"><div class="icon"><Icon name="check" size={26} /></div>
        <h2 style="margin:0 0 4px;color:var(--text)">Environment ready</h2>
        <p class="muted">llmsc-vm is configured and up — head to the dashboard.</p>
        <button class="btn primary mt12" onclick={() => navigate("dashboard")}>Go to dashboard</button>
      </div>
    </div>
  {:else}
    <div class="wizard-wrap">
      <div class="steps">
        {#each steps as s, i (s.st)}
          <div class="step" class:active={i === step} class:done={i < step}
            onclick={() => goto(i)} onkeydown={(e) => (e.key === "Enter" || e.key === " ") && goto(i)}
            role="button" tabindex="0">
            <div class="num">{i < step ? "✓" : i + 1}</div>
            <div><div class="st">{s.st}</div><div class="sd">{s.sd}</div></div>
          </div>
        {/each}
      </div>

      <div>
        {#if step === 0}
          <div class="card pad">
            <h3 style="margin:0 0 4px">How much power should the VM get?</h3>
            <p class="hint mb16">These are reserved from your host for <span class="mono">llmsc-vm</span>. You can change them later.</p>
            <div class="field mb20">
              <label for="w-operator">Your username <span class="hint">(the human operator — default Linux user in every sandbox)</span></label>
              <input id="w-operator" class="input mono" bind:value={operator} placeholder="operator" />
            </div>
            <div class="field mb20">
              <div class="flex"><label for="w-cpu">CPU cores</label><span class="right strong mono">{cpus} cores</span></div>
              <input id="w-cpu" type="range" min="1" max="12" bind:value={cpus} />
            </div>
            <div class="field mb20">
              <div class="flex"><label for="w-mem">Memory</label><span class="right strong mono">{memoryGib} GB</span></div>
              <input id="w-mem" type="range" min="2" max="24" bind:value={memoryGib} />
            </div>
            <div class="field">
              <div class="flex"><label for="w-disk">Disk</label><span class="right strong mono">{diskGib} GB</span></div>
              <input id="w-disk" type="range" min="20" max="250" step="10" bind:value={diskGib} />
            </div>
          </div>
        {:else if step === 1}
          <div class="card">
            <div class="card-head"><h3>Choose your services</h3><span class="sub">Each runs in the VM (L1) or its own isolated sandbox (L2)</span></div>
            <div>
              {#each services as s (s.name)}
                <div class="svc-row">
                  <div class="svc-ico" style="background:{meta(s.name).color}">{meta(s.name).initials}</div>
                  <div style="flex:1">
                    <div class="strong">{s.name} <span class="tag">{s.priority}</span></div>
                    <div class="hint">{s.description}</div>
                  </div>
                  <label class="switch"><input type="checkbox" bind:checked={s.enabled} /><span class="track"></span></label>
                </div>
              {/each}
            </div>
          </div>
          <div class="hint mt12">Tip: isolate anything that holds credentials or inspects traffic (LiteLLM, mitmproxy) in its own L2 sandbox.</div>
        {:else if step === 2}
          <div class="card pad">
            <h3 style="margin:0 0 4px">Networking</h3>
            <p class="hint mb16">How sandboxes reach the internet and how traffic is watched.</p>
            <div class="field mb20">
              <label for="w-egress">Egress policy</label>
              <div class="desc">Controls what agents can reach from inside a sandbox.</div>
              <select id="w-egress" class="input" bind:value={egress}>
                <option value="Allowlist">Allowlist (recommended) — only approved hosts + services</option>
                <option value="Inspect-all">Inspect-all — open egress, fully captured</option>
                <option value="Locked down">Locked down — services only, no public internet</option>
              </select>
            </div>
            <div class="flex gap12 mb16" style="align-items:flex-start">
              <label class="switch"><input type="checkbox" bind:checked={routeLlm} /><span class="track"></span></label>
              <div><div class="strong small">Route LLM traffic through LiteLLM only</div><div class="hint">Agents reach model APIs solely via their virtual key.</div></div>
            </div>
            <div class="flex gap12 mb16" style="align-items:flex-start">
              <label class="switch"><input type="checkbox" bind:checked={inspect} /><span class="track"></span></label>
              <div><div class="strong small">Capture & inspect egress (mitmproxy + Zeek)</div><div class="hint">Record sandbox network traffic for review.</div></div>
            </div>
            <div class="flex gap12" style="align-items:flex-start">
              <label class="switch"><input type="checkbox" bind:checked={tetragon} /><span class="track"></span></label>
              <div><div class="strong small">Tetragon eBPF kernel enforcement</div><div class="hint">Kernel-level backstop for process & network policy, per UID.</div></div>
            </div>
            <div class="divider"></div>
            <div class="field"><label for="w-subnet">VM subnet</label><div class="desc">Internal bridge network for the VM and its sandboxes.</div>
              <input id="w-subnet" class="input mono" bind:value={subnet} /></div>
          </div>
        {:else}
          <div class="card">
            <div class="card-head"><h3>Review & create</h3><span class="sub">Confirm before we launch llmsc-vm</span></div>
            <div class="pad">
              <div class="grid g-2" style="gap:24px">
                <div>
                  <div class="nav-label" style="padding-left:0">Resources</div>
                  <div class="kv"><span class="k">CPU</span><span class="v mono">{cpus} cores</span></div>
                  <div class="kv"><span class="k">Memory</span><span class="v mono">{memoryGib} GB</span></div>
                  <div class="kv"><span class="k">Disk</span><span class="v mono">{diskGib} GB</span></div>
                  <div class="kv"><span class="k">VM driver</span><span class="v">Lima</span></div>
                </div>
                <div>
                  <div class="nav-label" style="padding-left:0">Networking</div>
                  <div class="kv"><span class="k">Egress</span><span class="v">{egress}</span></div>
                  <div class="kv"><span class="k">LLM routing</span><span class="v">{routeLlm ? "LiteLLM only" : "direct"}</span></div>
                  <div class="kv"><span class="k">Inspection</span><span class="v">{inspect ? "mitmproxy + Zeek" : "off"}</span></div>
                  <div class="kv"><span class="k">Enforcement</span><span class="v">{tetragon ? "Tetragon eBPF" : "off"}</span></div>
                </div>
              </div>
              <div class="divider"></div>
              <div class="nav-label" style="padding-left:0">Services</div>
              <div class="flex gap8 wrap">
                {#each enabled as name}<span class="pill ok"><span class="dot ok"></span> {name}</span>{/each}
                {#if enabled.length === 0}<span class="muted small">none selected</span>{/if}
              </div>
              <div class="banner info mt16"><Icon name="shield" size={18} />
                <span>All sandboxes will run <strong>unprivileged</strong> with nested rootless containers enabled. No privileged DinD anywhere.</span></div>
            </div>
          </div>
        {/if}

        <div class="flex gap10 mt20">
          <button class="btn" onclick={back} disabled={step === 0 || creating}>‹ Back</button>
          <span class="right"></span>
          <button class="btn ghost" onclick={() => navigate("dashboard")}>Cancel</button>
          {#if step < steps.length - 1}
            <button class="btn primary" onclick={next}><span>Continue</span><Icon name="arrow" size={15} /></button>
          {:else}
            <button class="btn primary" onclick={create} disabled={creating}>
              <Icon name="check" size={15} /><span>{creating ? "Creating…" : "Create environment"}</span>
            </button>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>
