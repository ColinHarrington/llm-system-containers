<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { listProfiles } from "../lib/core";
  import type { ProfileInfo } from "../lib/types";

  let profiles = $state<ProfileInfo[]>([]);
  $effect(() => {
    void (async () => { profiles = await listProfiles(); })();
  });
</script>

<div class="content">
  <div class="banner info mb16">
    <Icon name="shield" size={18} />
    <span>
      A profile is a reusable, named bundle of permission boundaries assigned to an agent (one Linux
      user). They are <strong>presets, not the enforcement</strong> — the kernel/infra backstops
      (Tetragon, Incus ACLs, LiteLLM) are what hold. Compiling profiles down to those is later work.
    </span>
  </div>

  <div class="grid g-2">
    {#each profiles as p (p.name)}
      <div class="card pad">
        <div class="flex gap10 mb8">
          <div class="pico"><Icon name="shield" size={16} /></div>
          <div><div class="strong mono" style="color:var(--text)">{p.name}</div>
            <div class="muted xsmall">{p.summary}</div></div>
          {#if p.controlPlane !== "none"}
            <span class="pill warn right" title="Can take platform actions"><Icon name="steer" size={11} /> control-plane</span>
          {/if}
        </div>
        <div class="axes">
          <div class="kv"><span class="k">Filesystem</span><span class="v small">{p.filesystem}</span></div>
          <div class="kv"><span class="k">Network egress</span><span class="v small">{p.network}</span></div>
          <div class="kv"><span class="k">Nested L3</span><span class="v">
            {#if p.l3}<span class="pill ok"><span class="dot ok"></span> allowed</span>
            {:else}<span class="pill"><span class="dot muted"></span> off</span>{/if}
          </span></div>
          <div class="kv"><span class="k">LLM budget</span><span class="v small mono">{p.llmBudget}</span></div>
          <div class="kv"><span class="k">Control-plane</span><span class="v small">{p.controlPlane}</span></div>
        </div>
      </div>
    {/each}
  </div>

  <p class="xsmall muted mt12">
    Assign a profile when adding an agent to a sandbox. Custom/user-defined profiles and
    enforcement compilation are follow-ons (see <span class="mono">planning/agent-profiles.md</span>).
  </p>
</div>

<style>
  .pico { width: 32px; height: 32px; border-radius: 8px; background: var(--accent-dim); color: var(--accent-text); display: grid; place-items: center; flex: none; }
  .axes { display: flex; flex-direction: column; }
</style>
