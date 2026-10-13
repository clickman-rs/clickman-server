<script lang="ts">
  import { createQuery, createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { createAction } from '../ui/action.svelte';
  import { pollVisible } from '../graphql/poll.svelte';
  import {
    SegmentsDocument,
    WebhooksDocument,
    SaveSegmentDocument,
    DeleteSegmentDocument,
    SaveWebhookDocument,
    DeleteWebhookDocument,
    ReplayWebhookDocument,
    type SegmentsQuery,
    type SegmentsQueryVariables,
    type WebhooksQuery,
    type WebhooksQueryVariables,
    type SaveSegmentMutation,
    type SaveSegmentMutationVariables,
    type DeleteSegmentMutation,
    type DeleteSegmentMutationVariables,
    type SaveWebhookMutation,
    type SaveWebhookMutationVariables,
    type DeleteWebhookMutation,
    type DeleteWebhookMutationVariables,
    type ReplayWebhookMutation,
    type ReplayWebhookMutationVariables,
  } from '../graphql/generated';
  import { dataOrThrow, errorMessage } from '../graphql/errors';
  import type { WebhookCreator } from '../types';

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
  const query = createQuery<SegmentsQuery, SegmentsQueryVariables>({
    query: SegmentsDocument,
    variables: () => ({ id: projectId }),
  });
  const hooks = createQuery<WebhooksQuery, WebhooksQueryVariables>({
    query: WebhooksDocument,
    variables: () => ({ id: projectId }),
    enabled: () => canAdmin,
  });
  const save = createMutation<SaveSegmentMutation, SaveSegmentMutationVariables>({
    query: SaveSegmentDocument,
  });
  const remove = createMutation<DeleteSegmentMutation, DeleteSegmentMutationVariables>({
    query: DeleteSegmentDocument,
  });
  const saveHook = createMutation<SaveWebhookMutation, SaveWebhookMutationVariables>({
    query: SaveWebhookDocument,
  });
  const removeHook = createMutation<DeleteWebhookMutation, DeleteWebhookMutationVariables>({
    query: DeleteWebhookDocument,
  });
  const replay = createMutation<ReplayWebhookMutation, ReplayWebhookMutationVariables>({
    query: ReplayWebhookDocument,
  });
  const action = createAction();
  let notice = $state('');
  let adding = $state(false);
  let editing = $state<string | null>(null);
  let name = $state('');
  let key = $state('');
  let kind = $state('event');
  let event = $state('');
  let absent = $state('purchase');
  let filters = $state('');
  let range = $state('7d');
  let delay = $state('24h');
  let funnelKey = $state('');
  let atStep = $state<number | undefined>(1);
  let excludeLater = $state(false);
  let fireExisting = $state(false);
  let hookSegment = $state('');
  let hookName = $state('');
  let hookKind = $state('webhook');
  let hookUrl = $state('');
  let signing = $state('');
  const segments = $derived(
    query.data?.project.id === projectId ? query.data.project.segments : [],
  );
  const webhooks = $derived(
    canAdmin && hooks.data?.project.id === projectId ? hooks.data.project.webhooks : [],
  );
  const logs = $derived(
    canAdmin && hooks.data?.project.id === projectId ? hooks.data.project.webhookLogs : [],
  );
  pollVisible(
    query,
    () => true,
    (cause) => (notice = errorMessage(cause)),
  );
  pollVisible(
    hooks,
    () => canAdmin,
    (cause) => (notice = errorMessage(cause)),
  );
  async function reload() {
    await Promise.all([query.refetch(), ...(canAdmin ? [hooks.refetch()] : [])]);
    if (query.error) throw query.error;
    if (canAdmin && hooks.error) throw hooks.error;
  }
  function edit(segment: (typeof segments)[number]) {
    const rule = (segment.definition as { rule: Record<string, unknown> }).rule;
    name = segment.name;
    key = segment.key;
    editing = segment.id;
    fireExisting = segment.fireExisting;
    kind = String(rule.kind);
    range = String(rule.range ?? '7d');
    delay = String(rule.after ?? rule.wait ?? '24h');
    const filter = (rule.filter ?? rule.entry ?? { event: '' }) as {
      event: string;
      where?: unknown;
    };
    event = filter.event;
    filters = filter.where ? JSON.stringify(filter.where) : '';
    absent = String((rule.absent as { event?: string } | undefined)?.event ?? 'purchase');
    funnelKey = String(rule.funnel_key ?? '');
    atStep = Number(rule.at_step ?? 1);
    excludeLater = Boolean(rule.exclude_later);
    adding = true;
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canAdmin) return;
    await action.run(async () => {
      const filter = {
        event: event.trim(),
        ...(filters.trim() ? { where: JSON.parse(filters) } : {}),
      };
      const rule =
        kind === 'event'
          ? { kind, filter, range }
          : kind === 'funnel'
            ? {
                kind,
                funnel_key: funnelKey,
                at_step: atStep,
                exclude_later: excludeLater,
                wait: delay,
              }
            : { kind, entry: filter, absent: { event: absent.trim() }, after: delay, range };
      dataOrThrow(
        await save.execute({
          projectId,
          definition: { key, name, rule },
          enabled: editing ? (segments.find((s) => s.id === editing)?.enabled ?? false) : true,
          fireExisting,
        }),
      );
      await reload();
      adding = false;
      editing = null;
    });
  }
  async function toggle(segment: (typeof segments)[number]) {
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
    });
  }
  async function submitHook(e: SubmitEvent) {
    e.preventDefault();
    if (!canAdmin) return;
    await action.run(async () => {
      if (createWebhook)
        await createWebhook({
          projectId,
          segmentId: hookSegment,
          name: hookName,
          kind: hookKind,
          url: hookUrl,
          signingSecret: signing,
        });
      else
        dataOrThrow(
          await saveHook.execute({
            projectId,
            input: {
              segmentId: hookSegment,
              name: hookName,
              kind: hookKind,
              enabled: true,
              config: { url: hookUrl, signing_ref: signing },
            },
          }),
        );
      signing = '';
      hookName = '';
      hookUrl = '';
      await reload();
    });
  }
  async function deleteHook(id: string) {
    if (!canAdmin) return;
    await action.run(async () => {
      dataOrThrow(await removeHook.execute({ projectId, id }));
      await reload();
    });
  }
  async function retry(id: string) {
    if (!canAdmin) return;
    await action.run(async () => {
      const result = dataOrThrow<ReplayWebhookMutation>(await replay.execute({ projectId, id }));
      if (!result.replayWebhook)
        throw new Error('Enable the segment and webhook before replaying.');
      await reload();
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
      onclick={() => {
        editing = null;
        key = '';
        name = '';
        adding = !adding;
      }}>+ New segment</Button
    >{/if}
</div>
{#if action.error || notice || query.error}<div role="alert" class="cm-error cm:mb-5">
    {action.error || notice || errorMessage(query.error)}
  </div>{/if}
{#if adding && canAdmin}
  <form onsubmit={submit} class="cm-surface cm:mb-6 cm:p-6">
    <fieldset disabled={action.busy} class="cm:grid cm:gap-4 cm:sm:grid-cols-2">
      <label>Name<input bind:value={name} required maxlength="200" /></label><label
        >Key<input
          bind:value={key}
          disabled={!!editing}
          required
          pattern="[a-z0-9_]+"
          maxlength="100"
        /></label
      >
      <label
        >Condition<select bind:value={kind}
          ><option value="event">Performed an event</option><option value="funnel"
            >Reached a funnel stage</option
          ><option value="inactive_after">Did not complete a follow-up</option></select
        ></label
      >
      {#if kind === 'funnel'}
        <label
          >Funnel<select bind:value={funnelKey} required
            ><option value="">Select a funnel</option>{#each funnels as funnel}<option
                value={funnel.key}>{funnel.name}</option
              >{/each}</select
          ></label
        >
        <label
          >Stage (1-based)<input
            type="number"
            min="1"
            max="10"
            bind:value={atStep}
            required
          /></label
        >
        <label
          >Wait after reaching stage<input
            bind:value={delay}
            required
            pattern="[0-9]+[hd]"
          /></label
        >
        <label class="cm:flex cm:items-center cm:gap-2"
          ><input type="checkbox" bind:checked={excludeLater} />Exclude people who reached a later
          stage</label
        >
      {:else}
        <label>Event<input bind:value={event} required maxlength="200" /></label><label
          >Lookback range<input bind:value={range} required pattern="[1-9][0-9]*[hd]" /></label
        >
        <label
          >Property filters (JSON, optional)<input
            bind:value={filters}
            placeholder={'{"plan":"pro"}'}
          /></label
        >
        {#if kind === 'inactive_after'}<label
            >Missing follow-up event<input bind:value={absent} required maxlength="200" /></label
          ><label
            >Wait after first event<input
              bind:value={delay}
              required
              pattern="[1-9][0-9]*[hd]"
            /></label
          >{/if}
      {/if}
      <label class="cm:flex cm:items-center cm:gap-2 cm:sm:col-span-2"
        ><input type="checkbox" bind:checked={fireExisting} />Emit entry events for existing matches
        after this rule is saved</label
      >
      <p class="cm:text-xs cm:text-muted cm:sm:col-span-2">
        Without backfill, historical matches enter the audience silently. Webhooks fire on entry,
        including a later re-entry. Timers are checked by the worker.
      </p>
      <div class="cm:flex cm:gap-2">
        <Button type="submit">Save segment</Button><Button
          variant="ghost"
          onclick={() => (adding = false)}>Cancel</Button
        >
      </div>
    </fieldset>
  </form>
{/if}
{#if segments.length}
  <div class="cm-surface cm:overflow-x-auto">
    <table>
      <thead
        ><tr
          ><th>Segment</th><th>Members</th><th>Status</th><th>Evaluated</th>{#if canAdmin}<th
              >Actions</th
            >{/if}</tr
        ></thead
      ><tbody>
        {#each segments as segment (segment.id)}<tr
            ><td class="cm:font-medium"
              >{segment.name}<span class="cm:ml-2 cm:text-xs cm:text-muted">{segment.key}</span></td
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
{#if canAdmin}
  <section class="cm-surface cm:mt-6 cm:p-6">
    <h2>Webhooks and Zapier</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">
      Connect a receiver or Zapier Catch Hook to a segment entry.
    </p>
    <form onsubmit={submitHook} class="cm:mt-5">
      <fieldset
        disabled={action.busy || !segments.length}
        class="cm:grid cm:gap-4 cm:sm:grid-cols-2"
      >
        <label
          >Segment<select bind:value={hookSegment} required
            ><option value="">Select a segment</option>{#each segments as segment}<option
                value={segment.id}>{segment.name}</option
              >{/each}</select
          ></label
        >
        <label>Webhook name<input bind:value={hookName} required maxlength="100" /></label>
        <label
          >Receiver<select bind:value={hookKind}
            ><option value="webhook">Webhook</option><option value="zapier"
              >Zapier Catch Hook</option
            ></select
          ></label
        >
        <label
          >Endpoint URL<input
            type="url"
            bind:value={hookUrl}
            required
            placeholder="https://…"
          /></label
        >
        <label class="cm:sm:col-span-2"
          >{createWebhook ? 'Signing secret' : 'Signing secret environment reference'}<input
            type={createWebhook ? 'password' : 'text'}
            bind:value={signing}
            required
            minlength={createWebhook ? 16 : 1}
            autocomplete="off"
            placeholder={createWebhook
              ? 'At least 16 bytes'
              : `CLICKMAN_${projectId.replaceAll('-', '').toUpperCase()}_WEBHOOK_SIGNING`}
          /></label
        >
        <p class="cm:text-xs cm:text-muted cm:sm:col-span-2">
          Receivers get a stable X-Clickman-Event-Id and an HMAC signature. Delivery is at least
          once; use the event ID to deduplicate.{#if !createWebhook}
            Set the referenced secret on your worker.{/if}
        </p>
        <div><Button type="submit">Add webhook</Button></div>
      </fieldset>
    </form>
    {#if hooks.error}<p role="alert" class="cm-error cm:mt-4">{errorMessage(hooks.error)}</p>{/if}
    {#each webhooks as hook}<div
        class="cm:mt-4 cm:flex cm:items-center cm:justify-between cm:border-t cm:border-line cm:pt-4"
      >
        <span>{hook.name} · {hook.kind} · {hook.enabled ? 'Active' : 'Paused'}</span><Button
          variant="danger"
          disabled={action.busy}
          onclick={() => deleteHook(hook.id)}>Remove</Button
        >
      </div>{/each}
  </section>
  <section class="cm-surface cm:mt-6 cm:p-6">
    <h2>Delivery log</h2>
    <div class="cm:mt-4 cm:overflow-x-auto">
      <table>
        <thead
          ><tr><th>Event</th><th>Status</th><th>Attempts</th><th>Response</th><th>Actions</th></tr
          ></thead
        ><tbody
          >{#each logs as log}<tr
              ><td class="cm:text-xs">{log.entryId}</td><td>{log.status}</td><td>{log.attempts}</td
              ><td
                >{log.lastStatus ?? '—'}{#if log.lastError}<span
                    class="cm:block cm:text-xs cm:text-muted">{log.lastError}</span
                  >{/if}</td
              ><td
                ><Button
                  variant="ghost"
                  disabled={action.busy || log.status === 'canceled'}
                  onclick={() => retry(log.id)}>Replay</Button
                ></td
              ></tr
            >{/each}</tbody
        >
      </table>
      {#if !logs.length}<p class="cm:mt-3 cm:text-sm cm:text-muted">
          No delivery attempts yet.
        </p>{/if}
    </div>
  </section>
{/if}
