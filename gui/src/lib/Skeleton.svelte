<script lang="ts">
  let { w = "100%", h = 14, r = 6, mb = 0 }: { w?: string; h?: number; r?: number; mb?: number } = $props();
</script>

<span class="sk" data-testid="skeleton" style="width:{w};height:{h}px;border-radius:{r}px;margin-bottom:{mb}px"></span>

<style>
  .sk {
    display: block;
    background: linear-gradient(90deg, var(--card-2) 25%, var(--card-3) 50%, var(--card-2) 75%);
    background-size: 200% 100%;
    animation: shimmer 1.3s ease-in-out infinite;
  }
  @keyframes shimmer { 0% { background-position: 200% 0; } 100% { background-position: -200% 0; } }
</style>
