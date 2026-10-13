<script lang="ts">
  import { untrack } from 'svelte';
  import { Button } from '../ui/index';
  import EventFilterFields from '../ui/EventFilterFields.svelte';
  import { duration, filterDraft, filterDefinition } from '../ui/forms';
  import { errorMessage } from '../graphql/errors';
  import type { SegmentDefinition } from '../client/types';

  let {
    definition,
    fireExisting = false,
    busy,
    funnels,
    onsave,
    oncancel,
  }: {
    definition?: SegmentDefinition;
    fireExisting?: boolean;
    busy: boolean;
    funnels: { key: string; name: string }[];
    onsave: (definition: SegmentDefinition, fireExisting: boolean) => Promise<void>;
    oncancel: () => void;
  } = $props();
  const initial = untrack(() => definition);
  const rule = initial?.rule;
  let name = $state(initial?.name ?? '');
  let key = $state(initial?.key ?? '');
  let kind = $state<SegmentDefinition['rule']['kind']>(rule?.kind ?? 'event');
  let entry = $state(
    filterDraft(
      rule?.kind === 'event'
        ? rule.filter
        : rule?.kind === 'inactive_after'
          ? rule.entry
          : undefined,
    ),
  );
  let absent = $state(
    filterDraft(rule?.kind === 'inactive_after' ? rule.absent : { event: 'purchase' }),
  );
  let range = $state(rule && 'range' in rule ? rule.range : '7d');
  let delay = $state(
    rule?.kind === 'funnel'
      ? (rule.wait ?? '0h')
      : rule?.kind === 'inactive_after'
        ? rule.after
        : '24h',
  );
  let funnelKey = $state(rule?.kind === 'funnel' ? rule.funnel_key : '');
  let atStep = $state<number | undefined>(rule?.kind === 'funnel' ? rule.at_step : 1);
  let excludeLater = $state(rule?.kind === 'funnel' ? (rule.exclude_later ?? false) : false);
  let backfill = $state(untrack(() => fireExisting));
  let error = $state('');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    error = '';
    try {
      let rule: SegmentDefinition['rule'];
      if (kind === 'funnel') {
        if (!funnelKey || !atStep || !Number.isInteger(atStep))
          throw new Error('Choose a funnel and its step.');
        rule = {
          kind,
          funnel_key: funnelKey,
          at_step: atStep,
          exclude_later: excludeLater,
          wait: duration(delay, true),
        };
      } else if (kind === 'inactive_after') {
        rule = {
          kind,
          entry: filterDefinition(entry),
          absent: filterDefinition(absent),
          after: duration(delay),
          range: duration(range),
        };
      } else {
        rule = { kind, filter: filterDefinition(entry), range: duration(range) };
      }
      await onsave({ key, name, rule }, backfill);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }
</script>

<form onsubmit={submit} class="cm-surface cm:mb-6 cm:p-6">
  {#if error}<p role="alert" class="cm-error cm:mb-4">{error}</p>{/if}
  <fieldset disabled={busy} class="cm:grid cm:gap-4 cm:sm:grid-cols-2">
    <label>Name<input bind:value={name} required maxlength="200" /></label>
    <label
      >Key<input
        bind:value={key}
        disabled={!!initial}
        required
        pattern="[a-z0-9_]+"
        maxlength="100"
      /></label
    >
    <label
      >Condition<select bind:value={kind}>
        <option value="event">Performed an event</option>
        <option value="funnel">Reached a funnel stage</option>
        <option value="inactive_after">Did not complete a follow-up</option>
      </select></label
    >
    {#if kind === 'funnel'}
      <label
        >Funnel<select bind:value={funnelKey} required
          ><option value="">Select a funnel</option>
          {#each funnels as funnel}<option value={funnel.key}>{funnel.name}</option>{/each}</select
        ></label
      >
      <label
        >Stage (1-based)<input type="number" min="1" max="10" bind:value={atStep} required /></label
      >
      <label
        >Wait after reaching stage<input
          bind:value={delay}
          required
          pattern="(0h|[1-9][0-9]*[hd])"
        /></label
      >
      <label class="cm:flex cm:items-center cm:gap-2"
        ><input type="checkbox" bind:checked={excludeLater} />Exclude people who reached a later
        stage</label
      >
    {:else}
      <label>Lookback range<input bind:value={range} required pattern="[1-9][0-9]*[hd]" /></label>
      <div class="cm:sm:col-span-2"><EventFilterFields bind:filter={entry} /></div>
      {#if kind === 'inactive_after'}
        <div class="cm:sm:col-span-2">
          <EventFilterFields
            bind:filter={absent}
            eventLabel="Missing follow-up event"
            filtersLabel="Missing follow-up property filters (JSON, optional)"
          />
        </div>
        <label
          >Wait after first event<input
            bind:value={delay}
            required
            pattern="[1-9][0-9]*[hd]"
          /></label
        >
      {/if}
    {/if}
    <label class="cm:flex cm:items-center cm:gap-2 cm:sm:col-span-2"
      ><input type="checkbox" bind:checked={backfill} />Emit entry events for existing matches after
      this rule is saved</label
    >
    <p class="cm:text-xs cm:text-muted cm:sm:col-span-2">
      Without backfill, historical matches enter the audience silently. Webhooks fire on entry,
      including a later re-entry. Timers are checked by the worker.
    </p>
    <div class="cm:flex cm:gap-2">
      <Button type="submit">{busy ? 'Saving…' : 'Save segment'}</Button><Button
        variant="ghost"
        onclick={oncancel}>Cancel</Button
      >
    </div>
  </fieldset>
</form>
