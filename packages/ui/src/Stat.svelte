<script lang="ts">
  let { label, value, detail }: { label: string; value: string; detail: string } = $props();
</script>
<div class="surface p-5">
  <div class="text-sm text-muted">{label}</div>
  <div class="mt-3 text-3xl font-semibold tracking-tight tabular-nums">{value}</div>
  <div class="mt-2 text-xs text-muted">{detail}</div>
</div>
