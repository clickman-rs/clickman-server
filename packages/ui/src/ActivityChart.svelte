<script lang="ts">
  let { days }: { days: { day: string; dau: number }[] } = $props();
  const width = 1000;
  const height = 200;
  let max = $derived(Math.max(1, ...days.map(d => d.dau)));
  let points = $derived(days.map((d, i) => `${i * width / Math.max(1, days.length - 1)},${height - 10 - d.dau / max * (height - 30)}`).join(' '));
</script>
<div class="surface p-6">
  <div class="mb-8 flex justify-between"><div><h2>Daily active users</h2><p class="mt-1 text-sm text-muted">Last 90 days · UTC</p></div><span class="text-xs text-muted">{max.toLocaleString()} peak</span></div>
  <svg viewBox="0 0 {width} {height}" role="img" aria-label="Daily active users over the last 90 days" class="h-48 w-full overflow-visible" preserveAspectRatio="none">
    <defs><linearGradient id="activity-fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="#146e59" stop-opacity=".16" /><stop offset="100%" stop-color="#146e59" stop-opacity=".01" /></linearGradient></defs>
    {#each [0, .5, 1] as line}<line x1="0" x2={width} y1={10 + line * (height - 20)} y2={10 + line * (height - 20)} stroke="#e5ece9" stroke-dasharray="4 5" />{/each}
    {#if days.length}<polygon points="0,{height} {points} {width},{height}" fill="url(#activity-fill)" /><polyline points={points} fill="none" stroke="#146e59" stroke-width="2.5" vector-effect="non-scaling-stroke" />{/if}
  </svg>
  <div class="mt-4 flex justify-between text-xs text-muted"><span>{days[0]?.day ?? ''}</span><span>{days.at(-1)?.day ?? ''}</span></div>
</div>
