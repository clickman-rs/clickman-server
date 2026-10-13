<script lang="ts">
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { createAction } from '../ui/action.svelte';
  import { dataOrThrow } from '../graphql/errors';
  import {
    EraseActorDocument,
    type EraseActorMutation,
    type EraseActorMutationVariables,
  } from '../graphql/generated';
  let { projectId, onchanged }: { projectId: string; onchanged: () => Promise<void> } = $props();
  const action = createAction();
  const erase = createMutation<EraseActorMutation, EraseActorMutationVariables>({
    query: EraseActorDocument,
  });
  let externalId = $state('');
  let confirmErase = $state(false);
  let saved = $state('');
  function eraseActor(event: SubmitEvent) {
    event.preventDefault();
    if (!confirmErase) return;
    saved = '';
    void action.run(async () => {
      const result = dataOrThrow<EraseActorMutation>(
        await erase.execute({ projectId, externalId }),
      );
      externalId = '';
      confirmErase = false;
      await onchanged();
      saved = result.eraseActor
        ? 'User data erased. Reports will refresh on the next processing cycle.'
        : 'No data found for this user.';
    });
  }
</script>

{#if action.error}<p role="alert" class="cm-error">{action.error}</p>{/if}
{#if saved}<p role="status" class="cm:rounded-lg cm:bg-brand-light cm:p-4 cm:text-sm cm:text-brand">
    {saved}
  </p>{/if}
<fieldset disabled={action.busy}>
  <section class="cm-surface cm:border-red-200 cm:p-6">
    <h2>Erase user data</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">
      Remove this user's events and activity from this project. Data already forwarded must be
      erased at the destination separately.
    </p>
    <form onsubmit={eraseActor} class="cm:mt-5 cm:grid cm:gap-4">
      <label>External user ID<input bind:value={externalId} placeholder="user_42" required /></label
      ><label class="cm:flex cm:items-center cm:gap-2 cm:font-normal"
        ><input type="checkbox" bind:checked={confirmErase} />I want to permanently erase this
        user's data.</label
      >
      <div>
        <Button type="submit" variant="danger" disabled={!confirmErase || action.busy}
          >Erase data</Button
        >
      </div>
    </form>
  </section>
</fieldset>
