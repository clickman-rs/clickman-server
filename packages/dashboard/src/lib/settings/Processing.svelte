<script lang="ts">
  import { untrack } from 'svelte';
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { createAction } from '../ui/action.svelte';
  import { dataOrThrow } from '../graphql/errors';
  import {
    UpdateSettingsDocument,
    type UpdateSettingsMutation,
    type UpdateSettingsMutationVariables,
  } from '../graphql/generated';
  import type { Settings } from '../reports';
  let {
    projectId,
    settings,
    onchanged,
    maxRetentionDays,
  }: {
    projectId: string;
    settings: Settings;
    onchanged: () => Promise<void>;
    maxRetentionDays?: number;
  } = $props();
  const action = createAction();
  const update = createMutation<UpdateSettingsMutation, UpdateSettingsMutationVariables>({
    query: UpdateSettingsDocument,
  });
  let saved = $state('');
  let maxAge = $state(untrack(() => settings.max_event_age_days));
  let dedup = $state(untrack(() => settings.dedup_window_days));
  let retention = $state<number | undefined>(
    untrack(() =>
      maxRetentionDays === undefined
        ? (settings.retention_days ?? undefined)
        : Math.min(settings.retention_days ?? maxRetentionDays, maxRetentionDays),
    ),
  );
  let lag = $state(untrack(() => settings.rotation_lag_seconds));
  let rate = $state(untrack(() => settings.rate_limit.per_second));
  let burst = $state(untrack(() => settings.rate_limit.burst));
  let fragments = $state(untrack(() => settings.filter_fragments.join(', ')));
  let origins = $state(untrack(() => (settings.allowed_origins ?? []).join(', ')));
  function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    saved = '';
    void action.run(async () => {
      dataOrThrow(
        await update.execute({
          projectId,
          settings: {
            allowed_origins: origins
              .split(',')
              .map((v) => v.trim())
              .filter(Boolean),
            filter_fragments: fragments
              .split(',')
              .map((value) => value.trim())
              .filter(Boolean),
            max_event_age_days: maxAge,
            dedup_window_days: dedup,
            retention_days: retention ?? null,
            rotation_lag_seconds: lag,
            rate_limit: { per_second: rate, burst },
          },
        }),
      );
      await onchanged();
      saved = 'Project settings saved.';
    });
  }
</script>

{#if action.error}<p role="alert" class="cm-error">{action.error}</p>{/if}
{#if saved}<p role="status" class="cm:rounded-lg cm:bg-brand-light cm:p-4 cm:text-sm cm:text-brand">
    {saved}
  </p>{/if}
<fieldset disabled={action.busy}>
  <section class="cm-surface cm:p-6">
    <h2>Processing and retention</h2>
    <form
      onsubmit={saveSettings}
      class="cm:mt-5 cm:grid cm:gap-5 cm:sm:grid-cols-2 cm:lg:grid-cols-3"
    >
      <label
        >Maximum event age (days)<input
          type="number"
          bind:value={maxAge}
          min="1"
          max="3650"
          required
        /></label
      ><label
        >Deduplication window (days)<input
          type="number"
          bind:value={dedup}
          min="1"
          max="365"
          required
        /></label
      ><label
        >Retention (days)<input
          type="number"
          bind:value={retention}
          min="1"
          max={maxRetentionDays ?? 36500}
          placeholder={maxRetentionDays ? `Up to ${maxRetentionDays} days` : 'Keep forever'}
          required={maxRetentionDays !== undefined}
        />{#if maxRetentionDays}<span class="cm:text-xs cm:font-normal cm:text-muted"
            >Your plan includes up to {maxRetentionDays} days.</span
          >{/if}</label
      >
      <label
        >Processing delay (seconds)<input
          type="number"
          bind:value={lag}
          min="0"
          max="86400"
          required
        /></label
      ><label
        >Batches per second<input
          type="number"
          bind:value={rate}
          step="any"
          min="0.01"
          required
        /></label
      ><label
        >Burst batches<input
          type="number"
          bind:value={burst}
          min="1"
          max="1000000"
          required
        /></label
      >
      <label class="cm:sm:col-span-2 cm:lg:col-span-3"
        >Sensitive property fragments<input bind:value={fragments} /><span
          class="cm:text-xs cm:font-normal cm:text-muted"
          >Comma-separated fragments. Matching properties are filtered before storage.</span
        ></label
      >
      <label class="cm:sm:col-span-2 cm:lg:col-span-3"
        >Browser origins<input
          bind:value={origins}
          placeholder="https://example.com, https://staging.example.com"
        /><span class="cm:text-xs cm:font-normal cm:text-muted"
          >Optional comma-separated origins. Native SDKs continue to work without an Origin header.
          Public write keys still require rate limits.</span
        ></label
      >
      <p class="cm:text-xs cm:text-muted cm:sm:col-span-2 cm:lg:col-span-3">
        Retention removes complete months. Events awaiting delivery remain until delivery finishes.
      </p>
      <div><Button type="submit" disabled={action.busy}>Save settings</Button></div>
    </form>
  </section>
</fieldset>
