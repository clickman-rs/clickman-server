<script lang="ts">
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '@clickman/ui';
  import { SaveFunnelDocument, DeleteFunnelDocument, type SaveFunnelMutation, type SaveFunnelMutationVariables, type DeleteFunnelMutation, type DeleteFunnelMutationVariables } from '@clickman/graphql-types';
  import { number, percentage, type FunnelReport } from './lib/reports';
  let { projectId, canAdmin, definitions, reports, onchanged }: { projectId: string; canAdmin: boolean; definitions: { id: string; key: string; name: string; definition: unknown }[]; reports: FunnelReport[]; onchanged: () => Promise<void> } = $props();
  const save = createMutation<SaveFunnelMutation, SaveFunnelMutationVariables>({ query: SaveFunnelDocument });
  const remove = createMutation<DeleteFunnelMutation, DeleteFunnelMutationVariables>({ query: DeleteFunnelDocument });
  let adding = $state(false); let name = $state(''); let key = $state(''); let window = $state('7d'); let range = $state('90d'); let breakdown = $state(''); let error = $state('');
  let steps = $state([{ event: '', where: '' }, { event: '', where: '' }]);
  async function submit(event: SubmitEvent) {
    event.preventDefault(); error = '';
    try {
      const definition = { key, name, window, range, breakdown: breakdown || null, steps: steps.map(s => ({ event: s.event.includes(',') ? s.event.split(',').map(e => e.trim()) : s.event.trim(), ...(s.where.trim() ? { where: JSON.parse(s.where) } : {}) })) };
      const result = await save.execute({ projectId, definition });
      if (result.error) throw result.error;
      adding = false; await onchanged();
    } catch (e) { error = e instanceof Error ? e.message : String(e); }
  }
  async function deleteFunnel(key: string) { error = ''; const result = await remove.execute({ projectId, key }); if (result.error) error = result.error.message; else await onchanged(); }
</script>
<div class="mb-5 flex items-center justify-between"><div><h2>Funnels</h2><p class="mt-1 text-sm text-muted">Follow an ordered path from first action to conversion.</p></div>{#if canAdmin}<Button variant="secondary" onclick={() => adding = !adding}>+ New funnel</Button>{/if}</div>
{#if error}<div role="alert" class="error mb-5">{error}</div>{/if}
{#if adding}
  <form onsubmit={submit} class="surface mb-6 grid gap-4 p-6 sm:grid-cols-2">
    <label>Name<input bind:value={name} required placeholder="Purchase" /></label><label>Key<input bind:value={key} required pattern="[a-z0-9_]+" placeholder="purchase" /></label>
    <label>Conversion window<input bind:value={window} required pattern="[1-9][0-9]*[hd]" /></label><label>Entry range<input bind:value={range} required pattern="[1-9][0-9]*[hd]" /></label>
    <label class="sm:col-span-2">Breakdown property (optional)<input bind:value={breakdown} placeholder="context.os.name" /></label>
    <div class="sm:col-span-2"><h3 class="mb-3">Steps</h3><div class="grid gap-3">{#each steps as step, i}<div class="rounded-lg bg-canvas p-4"><div class="mb-3 flex items-center justify-between"><span class="section-label">Step {i + 1}</span>{#if steps.length > 2}<button type="button" class="text-xs text-muted" onclick={() => steps = steps.filter((_, index) => index !== i)}>Remove</button>{/if}</div><label>Event name<input bind:value={step.event} required placeholder="purchase_started" /></label><label class="mt-3">Property filters (optional)<input bind:value={step.where} placeholder={'{"store":"app_store"}'} /></label></div>{/each}</div></div>
    <div class="flex flex-wrap gap-3 sm:col-span-2">{#if steps.length < 10}<Button variant="secondary" onclick={() => steps = [...steps, { event: '', where: '' }]}>+ Add step</Button>{/if}<Button type="submit" disabled={save.isFetching}>Save funnel</Button></div>
  </form>
{/if}
{#if !definitions.length}<div class="surface empty">No funnels yet.{#if canAdmin} Create one to measure conversion.{/if}</div>{/if}
<div class="grid gap-5">
  {#each definitions as definition}
    {@const report = reports.find(r => r.key === definition.key)}
    <section class="surface p-6"><div class="mb-6 flex justify-between"><div><h2>{definition.name}</h2><p class="mt-1 text-xs text-muted">{String((definition.definition as { range?: string }).range ?? '90d')} entry range · {String((definition.definition as { window?: string }).window ?? '7d')} to convert</p></div>{#if canAdmin}<Button variant="ghost" disabled={remove.isFetching} onclick={() => deleteFunnel(definition.key)}>Delete</Button>{/if}</div>
      {#if report}<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">{#each report.steps as step, i}<div class="rounded-xl border border-line p-4"><p class="section-label">Step {i + 1}</p><p class="mt-3 break-words text-sm font-medium">{step.events.join(' / ')}</p><p class="mt-4 text-2xl font-semibold tabular-nums">{number(step.actors)}</p><p class="mt-2 text-xs text-muted">{percentage(step.actors, report.steps[0].actors)} of entrants{#if step.median_seconds !== null} · {step.median_seconds}s median{/if}</p></div>{/each}</div>
        {#if report.breakdown.length}<div class="mt-6 overflow-x-auto"><h3 class="mb-3">By {report.breakdown_key}</h3><table><thead><tr><th>Value</th>{#each report.steps as _, i}<th class="text-right">Step {i + 1}</th>{/each}</tr></thead><tbody>{#each report.breakdown as row}<tr><td>{String(row.value ?? 'Missing')}</td>{#each row.actors as count}<td class="text-right">{number(count)}</td>{/each}</tr>{/each}</tbody></table></div>{/if}
      {:else}<p class="text-sm text-muted">This funnel will be computed on the next report refresh.</p>{/if}
    </section>
  {/each}
</div>
