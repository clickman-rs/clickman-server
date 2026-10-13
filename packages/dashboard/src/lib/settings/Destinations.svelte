<script lang="ts">
  import { untrack } from 'svelte';
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { createAction } from '../ui/action.svelte';
  import { dataOrThrow } from '../graphql/errors';
  import {
    SaveDestinationDocument,
    DeleteDestinationDocument,
    type SaveDestinationMutation,
    type SaveDestinationMutationVariables,
    type DeleteDestinationMutation,
    type DeleteDestinationMutationVariables,
    type ProjectAdminQuery,
  } from '../graphql/generated';
  let {
    projectId,
    destinations,
    onrefresh,
  }: {
    projectId: string;
    destinations: ProjectAdminQuery['project']['destinations'];
    onrefresh: () => Promise<void>;
  } = $props();
  const action = createAction();
  const saveDestination = createMutation<SaveDestinationMutation, SaveDestinationMutationVariables>(
    { query: SaveDestinationDocument },
  );
  const deleteDestination = createMutation<
    DeleteDestinationMutation,
    DeleteDestinationMutationVariables
  >({ query: DeleteDestinationDocument });
  const credentialPrefix = untrack(
    () => `CLICKMAN_${projectId.replaceAll('-', '').toUpperCase()}_`,
  );
  let destinationName = $state('mixpanel');
  let mixpanelId = $state('');
  let usernameEnv = $state(`${credentialPrefix}MIXPANEL_USERNAME`);
  let secretEnv = $state(`${credentialPrefix}MIXPANEL_SECRET`);
  let region = $state('us');
  let saved = $state('');
  function addDestination(event: SubmitEvent) {
    event.preventDefault();
    saved = '';
    void action.run(async () => {
      dataOrThrow(
        await saveDestination.execute({
          projectId,
          name: destinationName,
          config: {
            project_id: mixpanelId,
            username_env: usernameEnv,
            secret_env: secretEnv,
            region,
          },
        }),
      );
      await onrefresh();
      saved = 'Destination saved.';
    });
  }
  function removeDestination(id: string) {
    saved = '';
    void action.run(async () => {
      dataOrThrow(await deleteDestination.execute({ projectId, id }));
      await onrefresh();
    });
  }
</script>

{#if action.error}<p role="alert" class="cm-error">{action.error}</p>{/if}
{#if saved}<p role="status" class="cm:rounded-lg cm:bg-brand-light cm:p-4 cm:text-sm cm:text-brand">
    {saved}
  </p>{/if}
<fieldset disabled={action.busy}>
  <section class="cm-surface cm:p-6">
    <h2>Destinations</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">
      Forward sanitized events to Mixpanel. Credentials are read from the server environment.
    </p>
    {#each destinations as destination}<div
        class="cm:mt-4 cm:flex cm:items-center cm:justify-between cm:rounded-lg cm:bg-canvas cm:p-4"
      >
        <div>
          <h3>{destination.name}</h3>
          <p class="cm:mt-1 cm:text-xs cm:text-muted">{destination.kind}</p>
        </div>
        <Button
          variant="ghost"
          disabled={action.busy}
          onclick={() => removeDestination(destination.id)}>Remove</Button
        >
      </div>{/each}
    <form
      onsubmit={addDestination}
      class="cm:mt-5 cm:grid cm:gap-4 cm:sm:grid-cols-2 cm:lg:grid-cols-3"
    >
      <label>Destination name<input bind:value={destinationName} required /></label><label
        >Mixpanel project ID<input bind:value={mixpanelId} required pattern="[0-9]+" /></label
      ><label
        >Region<select bind:value={region}
          ><option value="us">US</option><option value="eu">EU</option><option value="in"
            >India</option
          ></select
        ></label
      ><label>Username environment variable<input bind:value={usernameEnv} required /></label><label
        >Secret environment variable<input bind:value={secretEnv} required /></label
      >
      <div class="cm:flex cm:items-end">
        <Button type="submit" disabled={action.busy}>Save destination</Button>
      </div>
    </form>
  </section>
</fieldset>
