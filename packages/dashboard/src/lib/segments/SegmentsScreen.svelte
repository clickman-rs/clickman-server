<script lang="ts">
  import { createQuery, createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { createAction } from '../ui/action.svelte';
  import { pollVisible } from '../graphql/poll.svelte';
  import {
    SegmentsDocument,
    SaveSegmentDocument,
    DeleteSegmentDocument,
    type SegmentsQuery,
    type SegmentsQueryVariables,
    type SaveSegmentMutation,
    type SaveSegmentMutationVariables,
    type DeleteSegmentMutation,
    type DeleteSegmentMutationVariables,
  } from '../graphql/generated';
  import { dataOrThrow, errorMessage } from '../graphql/errors';
  import type { WebhookCreator } from '../types';
  import type { SegmentDefinition } from '../client/types';
  import SegmentEditor from './SegmentEditor.svelte';
  import WebhooksPanel from './WebhooksPanel.svelte';
  let {
    projectId,
    canAdmin,
    funnels,
    createWebhook,
  }: {
    projectId: string;
    canAdmin: boolean;
    funnels: { key: string; name: string }[];
    createWebhook?: WebhookCreator;
  } = $props();
  type Segment = SegmentsQuery['project']['segments'][number];
  const query = createQuery<SegmentsQuery, SegmentsQueryVariables>({
    query: SegmentsDocument,
    variables: () => ({ id: projectId }),
  });
  const save = createMutation<SaveSegmentMutation, SaveSegmentMutationVariables>({
    query: SaveSegmentDocument,
  });
  const remove = createMutation<DeleteSegmentMutation, DeleteSegmentMutationVariables>({
    query: DeleteSegmentDocument,
  });
  const action = createAction();
  let notice = $state('');
  let draft = $state<{ segment?: Segment } | null>(null);
  let hooksPanel = $state<{ reload: () => Promise<void> } | undefined>();
  const segments = $derived(
    query.data?.project.id === projectId ? query.data.project.segments : [],
  );
  pollVisible(
    query,
    () => true,
    (cause) => (notice = errorMessage(cause)),
  );
  async function reload() {
    await Promise.all([query.refetch(), hooksPanel?.reload()]);
    if (query.error) throw query.error;
    notice = '';
  }
  function edit(segment: Segment) {
    draft = { segment };
  }
  async function submit(definition: SegmentDefinition, fireExisting: boolean) {
    if (!canAdmin) return;
    await action.run(async () => {
      // Read the current status again; editing a paused rule must not resume it.
      const enabled = draft?.segment
        ? (segments.find((s) => s.id === draft?.segment?.id)?.enabled ?? false)
        : true;
      dataOrThrow(await save.execute({ projectId, definition, enabled, fireExisting }));
      await reload();
      draft = null;
    });
  }
  async function toggle(segment: Segment) {
    if (!canAdmin) return;
    await action.run(async () => {
      dataOrThrow(
        await save.execute({
          projectId,
          definition: segment.definition,
          enabled: !segment.enabled,
          fireExisting: segment.fireExisting,
        }),
      );
      await reload();
    });
  }
  async function deleteSegment(id: string) {
    if (!canAdmin) return;
    await action.run(async () => {
      dataOrThrow(await remove.execute({ projectId, id }));
      await reload();
      if (draft?.segment?.id === id) draft = null;
    });
  }
</script>

<div class="cm:mb-5 cm:flex cm:flex-wrap cm:items-center cm:justify-between cm:gap-3">
  <div>
    <h2>Segments</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">
      Saved audiences, funnel stages and time-based follow-ups.
    </p>
  </div>
  {#if canAdmin}<Button
      variant="secondary"
      disabled={action.busy}
      onclick={() => (draft = draft ? null : {})}>+ New segment</Button
    >{/if}
</div>
{#if action.error || notice || query.error}<div role="alert" class="cm-error cm:mb-5">
    {action.error || notice || errorMessage(query.error)}
  </div>{/if}
{#if draft && canAdmin}
  {#key draft.segment?.id ?? 'new'}
    <SegmentEditor
      definition={draft.segment?.definition as SegmentDefinition | undefined}
      fireExisting={draft.segment?.fireExisting ?? false}
      busy={action.busy}
      {funnels}
      onsave={submit}
      oncancel={() => (draft = null)}
    />
  {/key}
{/if}
{#if segments.length}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (The named scroll region needs keyboard focus to expose off-screen columns.) -->
  <div class="cm-surface cm:overflow-x-auto" role="region" aria-label="Saved segments" tabindex="0">
    <table class="cm:min-w-[600px]">
      <thead
        ><tr
          ><th>Segment</th><th>Members</th><th>Status</th><th>Evaluated</th>{#if canAdmin}<th
              >Actions</th
            >{/if}</tr
        ></thead
      ><tbody>
        {#each segments as segment (segment.id)}<tr
            ><td class="cm:min-w-56 cm:font-medium"
              >{segment.name}<span class="cm:mt-1 cm:block cm:text-xs cm:text-muted"
                >{segment.key}</span
              ></td
            ><td>{segment.members.toLocaleString()}</td><td
              >{segment.enabled ? 'Active' : 'Paused'}</td
            ><td
              >{segment.evaluatedAt
                ? new Date(segment.evaluatedAt).toLocaleString()
                : 'Waiting for evaluation'}</td
            >{#if canAdmin}<td
                ><div class="cm:flex cm:flex-wrap cm:gap-2">
                  <Button variant="ghost" disabled={action.busy} onclick={() => edit(segment)}
                    >Edit</Button
                  ><Button variant="ghost" disabled={action.busy} onclick={() => toggle(segment)}
                    >{segment.enabled ? 'Pause' : 'Resume'}</Button
                  ><Button
                    variant="danger"
                    disabled={action.busy}
                    onclick={() => deleteSegment(segment.id)}>Delete</Button
                  >
                </div></td
              >{/if}</tr
          >{/each}
      </tbody>
    </table>
  </div>
{:else}<div class="cm-empty">
    {query.data
      ? 'No segments yet. Create an audience to follow people through a funnel.'
      : 'Loading segments…'}
  </div>{/if}

{#if canAdmin}<WebhooksPanel
    bind:this={hooksPanel}
    {projectId}
    {segments}
    {canAdmin}
    {action}
    {createWebhook}
  />{/if}
