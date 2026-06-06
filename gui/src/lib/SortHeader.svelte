<script lang="ts">
  // A sortable <th>. Click (or Enter/Space on the button) toggles sort on this column.
  // `sort` is the shared { key, dir } state; `col` is this header's key.
  let {
    label,
    col,
    sort,
    onsort,
    align = "left",
  }: {
    label: string;
    col: string;
    sort: { key: string; dir: 1 | -1 };
    onsort: (key: string) => void;
    align?: "left" | "right";
  } = $props();

  const active = $derived(sort.key === col);
</script>

<th
  aria-sort={active ? (sort.dir === 1 ? "ascending" : "descending") : "none"}
  style={align === "right" ? "text-align:right" : ""}
>
  <button class="sort-th" class:active onclick={() => onsort(col)} style={align === "right" ? "flex-direction:row-reverse" : ""}>
    <span>{label}</span>
    <span class="arrow" class:on={active}>{active && sort.dir === -1 ? "▼" : "▲"}</span>
  </button>
</th>

<style>
  .sort-th {
    display: inline-flex; align-items: center; gap: 5px;
    border: none; background: transparent; cursor: pointer; padding: 0;
    font: inherit; color: inherit; text-transform: inherit; letter-spacing: inherit;
  }
  .sort-th:hover { color: var(--text); }
  .arrow { font-size: 8px; opacity: 0; transition: opacity 0.12s; }
  .sort-th:hover .arrow { opacity: 0.4; }
  .arrow.on { opacity: 0.9; color: var(--accent-text); }
</style>
