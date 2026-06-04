<script lang="ts">
  import { listServices, createPlatform } from "../lib/core";
  import type { ServiceEntry } from "../lib/types";

  const steps = ["Resources", "Services", "Networking", "Review"];
  let step = $state(0);

  let cpus = $state(4);
  let memoryGib = $state(8);
  let diskGib = $state(100);
  let services = $state<ServiceEntry[]>([]);
  let defaultDenyEgress = $state(true);
  let creating = $state(false);
  let done = $state(false);

  $effect(() => {
    listServices().then((s) => (services = s));
  });

  let enabled = $derived(services.filter((s) => s.enabled).map((s) => s.name));

  function next() { if (step < steps.length - 1) step++; }
  function back() { if (step > 0) step--; }

  async function create() {
    creating = true;
    try {
      await createPlatform({ cpus, memoryGib, diskGib, services: enabled, defaultDenyEgress });
      done = true;
    } finally {
      creating = false;
    }
  }
</script>

<div class="card wizard">
  <ol class="rail">
    {#each steps as s, i (s)}
      <li class:active={i === step && !done} class:done={i < step || done}>{i + 1}. {s}</li>
    {/each}
  </ol>

  {#if done}
    <div class="step">
      <h2>VM created</h2>
      <p class="muted">Your platform is configured — head to the Dashboard.</p>
    </div>
  {:else if step === 0}
    <div class="step">
      <h2>Resources</h2>
      <label>CPU cores <input type="number" min="1" bind:value={cpus} /></label>
      <label>Memory (GiB) <input type="number" min="1" bind:value={memoryGib} /></label>
      <label>Disk (GiB) <input type="number" min="10" bind:value={diskGib} /></label>
    </div>
  {:else if step === 1}
    <div class="step">
      <h2>Services</h2>
      {#each services as s (s.name)}
        <label class="svc">
          <input type="checkbox" bind:checked={s.enabled} />
          <strong>{s.name}</strong>
          <span class="muted">{s.description}</span>
        </label>
      {/each}
    </div>
  {:else if step === 2}
    <div class="step">
      <h2>Networking</h2>
      <label class="chk">
        <input type="checkbox" bind:checked={defaultDenyEgress} />
        Default-deny egress (agents reach only services)
      </label>
    </div>
  {:else}
    <div class="step">
      <h2>Review</h2>
      <ul class="review">
        <li>VM: {cpus} CPU · {memoryGib} GiB RAM · {diskGib} GiB disk</li>
        <li>Services: {enabled.join(", ") || "none"}</li>
        <li>Egress: {defaultDenyEgress ? "default-deny" : "open"}</li>
      </ul>
    </div>
  {/if}

  {#if !done}
    <div class="actions">
      <button class="btn" onclick={back} disabled={step === 0 || creating}>Back</button>
      {#if step < steps.length - 1}
        <button class="btn primary" onclick={next}>Next</button>
      {:else}
        <button class="btn primary" onclick={create} disabled={creating}>
          {creating ? "Creating…" : "Create VM"}
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wizard { display: flex; flex-direction: column; gap: 1.1rem; max-width: 640px; }
  .rail { display: flex; gap: 1rem; list-style: none; padding: 0; margin: 0; font-size: 0.85rem; }
  .rail li { color: var(--muted); }
  .rail li.active { color: var(--accent); font-weight: 600; }
  .rail li.done { color: var(--ok); }
  .step { display: flex; flex-direction: column; gap: 0.7rem; }
  .step label { display: flex; align-items: center; justify-content: space-between; gap: 1rem; font-size: 0.92rem; }
  .step input[type="number"] { width: 120px; padding: 0.4rem 0.6rem; border: 1px solid var(--border); border-radius: 8px; }
  .svc { justify-content: flex-start !important; gap: 0.5rem !important; }
  .chk { justify-content: flex-start !important; gap: 0.5rem !important; }
  .review { margin: 0; padding-left: 1.1rem; color: var(--text); line-height: 1.8; }
  .actions { display: flex; justify-content: space-between; }
</style>
