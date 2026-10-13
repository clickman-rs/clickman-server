<script lang="ts">
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import {
    SaveFunnelDocument,
    DeleteFunnelDocument,
    type SaveFunnelMutation,
    type SaveFunnelMutationVariables,
    type DeleteFunnelMutation,
    type DeleteFunnelMutationVariables,
  } from '../graphql/generated';
  import { number, percentage, type FunnelReport } from '../reports';
  import { createAction } from '../ui/action.svelte';
  import { dataOrThrow } from '../graphql/errors';
  import FunnelEditor from './FunnelEditor.svelte';
  import type { FunnelDefinition } from '../client/types';
  let {
    projectId,
    canAdmin,
    definitions,
    reports,
    onchanged,
  }: {
    projectId: string;
    canAdmin: boolean;
    definitions: { id: string; key: string; name: string; definition: unknown }[];
    reports: FunnelReport[];
    onchanged: () => Promise<void>;
  } = $props();
  const save = createMutation<SaveFunnelMutation, SaveFunnelMutationVariables>({
    query: SaveFunnelDocument,
  });
  const remove = createMutation<DeleteFunnelMutation, DeleteFunnelMutationVariables>({
    query: DeleteFunnelDocument,
  });
  const action = createAction();
  type Definition = (typeof definitions)[number];
  let draft = $state<{ funnel?: Definition } | null>(null);
  async function submit(definition: FunnelDefinition) {
    if (!canAdmin) return;
    await action.run(async () => {
      dataOrThrow<SaveFunnelMutation>(await save.execute({ projectId, definition }));
      await onchanged();
      draft = null;
    });
  }
  async function deleteFunnel(key: string) {
    if (!canAdmin) return;
    await action.run(async () => {
      dataOrThrow<DeleteFunnelMutation>(await remove.execute({ projectId, key }));
      await onchanged();
      if (draft?.funnel?.key === key) draft = null;
    });
  }
</script>

<div class="cm:mb-5 cm:flex cm:items-center cm:justify-between">
  <div>
    <h2>Funnels</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">
      Follow an ordered path from first action to conversion.
    </p>
  </div>
  {#if canAdmin}<Button
      variant="secondary"
      disabled={action.busy}
      onclick={() => (draft = draft && !draft.funnel ? null : {})}>+ New funnel</Button
    >{/if}
</div>
{#if action.error}<div role="alert" class="cm-error cm:mb-5">{action.error}</div>{/if}
{#if draft && canAdmin}
  {#key draft.funnel?.id ?? 'new'}
    <FunnelEditor
      definition={draft.funnel?.definition as FunnelDefinition | undefined}
      busy={action.busy}
      onsave={submit}
      oncancel={() => (draft = null)}
    />
  {/key}
{/if}
{#if !definitions.length}<div class="cm-surface cm-empty">
    No funnels yet.{#if canAdmin}
      Create one to measure conversion.{/if}
  </div>{/if}
<div class="cm:grid cm:gap-5">
  {#each definitions as definition}
    {@const report = reports.find((r) => r.key === definition.key)}
    <section class="cm-surface cm:p-6" aria-label={definition.name}>
      <div class="cm:mb-6 cm:flex cm:justify-between">
        <div>
          <h2>{definition.name}</h2>
          <p class="cm:mt-1 cm:text-xs cm:text-muted">
            {String((definition.definition as { range?: string }).range ?? '90d')} entry range · {String(
              (definition.definition as { window?: string }).window ?? '7d',
            )} to convert
          </p>
        </div>
        {#if canAdmin}<div class="cm:flex cm:items-start cm:gap-3">
            <Button
              variant="ghost"
              disabled={action.busy}
              onclick={() => (draft = { funnel: definition })}>Edit</Button
            ><Button
              variant="ghost"
              disabled={action.busy}
              onclick={() => deleteFunnel(definition.key)}>Delete</Button
            >
          </div>{/if}
      </div>
      {#if report}<div class="cm:grid cm:gap-4 cm:sm:grid-cols-2 cm:lg:grid-cols-4">
          {#each report.steps as step, i}<div class="cm:rounded-xl cm:border cm:border-line cm:p-4">
              <p class="cm-section-label">Step {i + 1}</p>
              <p class="cm:mt-3 cm:break-words cm:text-sm cm:font-medium">
                {step.events.join(' / ')}
              </p>
              <p class="cm:mt-4 cm:text-2xl cm:font-semibold cm:tabular-nums">
                {number(step.actors)}
              </p>
              <p class="cm:mt-2 cm:text-xs cm:text-muted">
                {percentage(step.actors, report.steps[0].actors)} of entrants{#if step.median_seconds !== null}
                  · {step.median_seconds}s median{/if}
              </p>
            </div>{/each}
        </div>
        {#if report.breakdown.length}<div class="cm:mt-6 cm:overflow-x-auto">
            <h3 class="cm:mb-3">By {report.breakdown_key}</h3>
            <table>
              <thead
                ><tr
                  ><th>Value</th>{#each report.steps as _, i}<th class="cm:text-right"
                      >Step {i + 1}</th
                    >{/each}</tr
                ></thead
              ><tbody
                >{#each report.breakdown as row}<tr
                    ><td>{String(row.value ?? 'Missing')}</td>{#each row.actors as count}<td
                        class="cm:text-right">{number(count)}</td
                      >{/each}</tr
                  >{/each}</tbody
              >
            </table>
          </div>{/if}
      {:else}<p class="cm:text-sm cm:text-muted">
          This funnel will be computed on the next report refresh.
        </p>{/if}
    </section>
  {/each}
</div>
