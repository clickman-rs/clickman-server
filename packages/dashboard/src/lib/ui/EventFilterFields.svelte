<script lang="ts">
  import { Button } from './index';
  import type { FilterDraft } from './forms';
  let {
    filter = $bindable(),
    eventLabel = 'Event',
    filtersLabel = 'Property filters (JSON, optional)',
  }: { filter: FilterDraft; eventLabel?: string; filtersLabel?: string } = $props();
</script>

<div class="cm:grid cm:gap-3">
  {#each filter.events as _, index}
    <div class="cm:flex cm:items-end cm:gap-3">
      <label class="cm:flex-1">
        {index === 0 ? eventLabel : `${eventLabel} alternative ${index + 1}`}
        <input
          bind:value={filter.events[index]}
          required
          maxlength="200"
          placeholder="checkout_started"
        />
      </label>
      {#if filter.events.length > 1}
        <Button
          variant="ghost"
          onclick={() => (filter.events = filter.events.filter((_, i) => i !== index))}
          aria-label={`Remove ${eventLabel.toLowerCase()} ${index + 1}`}>Remove</Button
        >
      {/if}
    </div>
  {/each}
  <div>
    <Button variant="ghost" onclick={() => (filter.events = [...filter.events, ''])}
      >+ Alternative event</Button
    >
  </div>
  <label>{filtersLabel}<input bind:value={filter.where} placeholder={'{"plan":"paid"}'} /></label>
</div>
