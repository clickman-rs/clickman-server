<script lang="ts">
  import { untrack } from 'svelte';
  import { Button } from '../ui/index';
  import EventFilterFields from '../ui/EventFilterFields.svelte';
  import { duration, filterDraft, filterDefinition } from '../ui/forms';
  import { errorMessage } from '../graphql/errors';
  import type { FunnelDefinition } from '../client/types';
  let {
    definition,
    busy,
    onsave,
    oncancel,
  }: {
    definition?: FunnelDefinition;
    busy: boolean;
    onsave: (definition: FunnelDefinition) => Promise<void>;
    oncancel: () => void;
  } = $props();
  const initial = untrack(() => definition);
  let name = $state(initial?.name ?? '');
  let key = $state(initial?.key ?? '');
  let conversionWindow = $state(initial?.window ?? '7d');
  let range = $state(initial?.range ?? '90d');
  let breakdown = $state(initial?.breakdown ?? '');
  let steps = $state(initial?.steps.map(filterDraft) ?? [filterDraft(), filterDraft()]);
  let error = $state('');
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    error = '';
    try {
      await onsave({
        key,
        name,
        window: duration(conversionWindow),
        range: duration(range),
        breakdown: breakdown || null,
        steps: steps.map(filterDefinition) as FunnelDefinition['steps'],
      });
    } catch (cause) {
      error = errorMessage(cause);
    }
  }
</script>

<form onsubmit={submit} class="cm-surface cm:mb-6 cm:p-6">
  {#if error}<p role="alert" class="cm-error cm:mb-4">{error}</p>{/if}
  <fieldset disabled={busy} class="cm:grid cm:gap-4 cm:sm:grid-cols-2">
    <label>Name<input bind:value={name} required maxlength="200" placeholder="Purchase" /></label>
    <label
      >Key<input
        bind:value={key}
        disabled={!!initial}
        required
        pattern="[a-z0-9_]+"
        maxlength="100"
        placeholder="purchase"
      /></label
    >
    <label
      >Conversion window<input
        bind:value={conversionWindow}
        required
        pattern="[1-9][0-9]*[hd]"
      /></label
    >
    <label>Entry range<input bind:value={range} required pattern="[1-9][0-9]*[hd]" /></label>
    <label class="cm:sm:col-span-2"
      >Breakdown property (optional)<input
        bind:value={breakdown}
        maxlength="128"
        placeholder="context.os.name"
      /></label
    >
    <div class="cm:sm:col-span-2">
      <h3 class="cm:mb-3">Steps</h3>
      <div class="cm:grid cm:gap-3">
        {#each steps as _, index}
          <div class="cm:rounded-lg cm:bg-canvas cm:p-4">
            <div class="cm:mb-3 cm:flex cm:items-center cm:justify-between">
              <span class="cm-section-label">Step {index + 1}</span>
              {#if steps.length > 2}<Button
                  variant="ghost"
                  aria-label={`Remove step ${index + 1}`}
                  onclick={() => (steps = steps.filter((_, i) => i !== index))}>Remove</Button
                >{/if}
            </div>
            <EventFilterFields
              bind:filter={steps[index]}
              eventLabel="Event name"
              filtersLabel="Property filters (optional)"
            />
          </div>
        {/each}
      </div>
    </div>
    <p class="cm:text-xs cm:text-muted cm:sm:col-span-2">
      People enter through the first step. Each later step must follow within the conversion window.
      Add alternatives when either event should count for a step.
    </p>
    <div class="cm:flex cm:flex-wrap cm:gap-3 cm:sm:col-span-2">
      {#if steps.length < 10}<Button
          variant="secondary"
          onclick={() => (steps = [...steps, filterDraft()])}>+ Add step</Button
        >{/if}
      <Button type="submit">{busy ? 'Saving…' : 'Save funnel'}</Button>
      <Button variant="ghost" onclick={oncancel}>Cancel</Button>
    </div>
  </fieldset>
</form>
