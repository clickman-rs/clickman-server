<script lang="ts">
  import { createQuery, createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import type { createAction } from '../ui/action.svelte';
  import { pollVisible } from '../graphql/poll.svelte';
  import {
    WebhooksDocument,
    SaveWebhookDocument,
    DeleteWebhookDocument,
    ReplayWebhookDocument,
    type WebhooksQuery,
    type WebhooksQueryVariables,
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
    segments,
    canAdmin,
    action,
    createWebhook,
  }: {
    projectId: string;
    segments: { id: string; name: string }[];
    canAdmin: boolean;
    action: ReturnType<typeof createAction>;
    createWebhook?: WebhookCreator;
  } = $props();
  let notice = $state('');
  const hooks = createQuery<WebhooksQuery, WebhooksQueryVariables>({
    query: WebhooksDocument,
    variables: () => ({ id: projectId }),
    enabled: () => canAdmin,
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
  let hookSegment = $state('');
  let hookName = $state('');
  let hookKind = $state('webhook');
  let hookUrl = $state('');
  let signing = $state('');

  const webhooks = $derived(
    canAdmin && hooks.data?.project.id === projectId ? hooks.data.project.webhooks : [],
  );
  const logs = $derived(
    canAdmin && hooks.data?.project.id === projectId ? hooks.data.project.webhookLogs : [],
  );
  pollVisible(
    hooks,
    () => canAdmin,
    (cause) => (notice = errorMessage(cause)),
  );
  export async function reload() {
    await hooks.refetch();
    if (hooks.error) throw hooks.error;
    notice = '';
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

{#if notice}<p role="alert" class="cm-error cm:mt-4">{notice}</p>{/if}
<section class="cm-surface cm:mt-6 cm:p-6">
  <h2>Webhooks and Zapier</h2>
  <p class="cm:mt-1 cm:text-sm cm:text-muted">
    Connect a receiver or Zapier Catch Hook to a segment entry.
  </p>
  <form onsubmit={submitHook} class="cm:mt-5">
    <fieldset disabled={action.busy || !segments.length} class="cm:grid cm:gap-4 cm:sm:grid-cols-2">
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
          ><option value="webhook">Webhook</option><option value="zapier">Zapier Catch Hook</option
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
        Receivers get a stable X-Clickman-Event-Id and an HMAC signature. Delivery is at least once;
        use the event ID to deduplicate.{#if !createWebhook}
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
    {#if !logs.length}<p class="cm:mt-3 cm:text-sm cm:text-muted">No delivery attempts yet.</p>{/if}
  </div>
</section>
