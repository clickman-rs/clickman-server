<script lang="ts">
  let { label, value, detail }: { label: string; value: string; detail: string } = $props();
</script>

<div class="cm-surface cm:p-5">
  <div class="cm:text-sm cm:text-muted">{label}</div>
  <div class="cm:mt-3 cm:text-3xl cm:font-semibold cm:tracking-tight cm:tabular-nums">{value}</div>
  <div class="cm:mt-2 cm:text-xs cm:text-muted">{detail}</div>
</div>
