<script lang="ts">
  import { untrack } from 'svelte';
  import { createQuery, createMutation } from 'cachebay/svelte';
  import { Button } from '@clickman/ui';
  import { ProjectAdminDocument, IssueWriteKeyDocument, RevokeWriteKeyDocument, IssueAccessKeyDocument, RevokeAccessKeyDocument, UpdateSettingsDocument, EraseActorDocument, SaveDestinationDocument, DeleteDestinationDocument, type ProjectAdminQuery, type ProjectAdminQueryVariables, type IssueWriteKeyMutation, type IssueWriteKeyMutationVariables, type RevokeWriteKeyMutation, type RevokeWriteKeyMutationVariables, type IssueAccessKeyMutation, type IssueAccessKeyMutationVariables, type RevokeAccessKeyMutation, type RevokeAccessKeyMutationVariables, type UpdateSettingsMutation, type UpdateSettingsMutationVariables, type EraseActorMutation, type EraseActorMutationVariables, type SaveDestinationMutation, type SaveDestinationMutationVariables, type DeleteDestinationMutation, type DeleteDestinationMutationVariables, type ProjectRole } from '@clickman/graphql-types';
  import type { Settings } from './lib/reports';
  let { projectId, settings, onchanged }: { projectId: string; settings: Settings; onchanged: () => Promise<void> } = $props();
  const query = createQuery<ProjectAdminQuery, ProjectAdminQueryVariables>({ query: ProjectAdminDocument, variables: () => ({ id: projectId }) });
  const issueWrite = createMutation<IssueWriteKeyMutation, IssueWriteKeyMutationVariables>({ query: IssueWriteKeyDocument });
  const revokeWrite = createMutation<RevokeWriteKeyMutation, RevokeWriteKeyMutationVariables>({ query: RevokeWriteKeyDocument });
  const issueAccess = createMutation<IssueAccessKeyMutation, IssueAccessKeyMutationVariables>({ query: IssueAccessKeyDocument });
  const revokeAccess = createMutation<RevokeAccessKeyMutation, RevokeAccessKeyMutationVariables>({ query: RevokeAccessKeyDocument });
  const update = createMutation<UpdateSettingsMutation, UpdateSettingsMutationVariables>({ query: UpdateSettingsDocument });
  const erase = createMutation<EraseActorMutation, EraseActorMutationVariables>({ query: EraseActorDocument });
  const saveDestination = createMutation<SaveDestinationMutation, SaveDestinationMutationVariables>({ query: SaveDestinationDocument });
  const deleteDestination = createMutation<DeleteDestinationMutation, DeleteDestinationMutationVariables>({ query: DeleteDestinationDocument });
  let source = $state(''); let accessName = $state(''); let role = $state<ProjectRole>('READER'); let issued = $state(''); let copied = $state(false); let error = $state(''); let saved = $state('');
  let maxAge = $state(untrack(() => settings.max_event_age_days));
  let dedup = $state(untrack(() => settings.dedup_window_days));
  let retention = $state<number | undefined>(untrack(() => settings.retention_days ?? undefined));
  let lag = $state(untrack(() => settings.rotation_lag_seconds));
  let rate = $state(untrack(() => settings.rate_limit.per_second));
  let burst = $state(untrack(() => settings.rate_limit.burst));
  let fragments = $state(untrack(() => settings.filter_fragments.join(', ')));
  let externalId = $state(''); let confirmErase = $state(false);
  const credentialPrefix = untrack(() => `CLICKMAN_${projectId.replaceAll('-', '').toUpperCase()}_`);
  let destinationName = $state('mixpanel'); let mixpanelId = $state(''); let usernameEnv = $state(`${credentialPrefix}MIXPANEL_USERNAME`); let secretEnv = $state(`${credentialPrefix}MIXPANEL_SECRET`); let region = $state('us');
  async function createWrite(event: SubmitEvent) {
    event.preventDefault(); error = ''; copied = false;
    const result = await issueWrite.execute({ projectId, source });
    if (result.error) error = result.error.message;
    else { issued = result.data!.issueWriteKey.token; source = ''; await query.refetch(); }
  }
  async function createAccess(event: SubmitEvent) {
    event.preventDefault(); error = ''; copied = false;
    const result = await issueAccess.execute({ projectId, name: accessName, role });
    if (result.error) error = result.error.message;
    else { issued = result.data!.issueAccessKey.token; accessName = ''; await query.refetch(); }
  }
  async function revoke(kind: 'write' | 'access', id: string) {
    const result = await (kind === 'write' ? revokeWrite : revokeAccess).execute({ projectId, id });
    if (result.error) error = result.error.message; else await query.refetch();
  }
  async function saveSettings(event: SubmitEvent) {
    event.preventDefault(); error = ''; saved = '';
    const result = await update.execute({ projectId, settings: { filter_fragments: fragments.split(',').map(v => v.trim()).filter(Boolean), max_event_age_days: maxAge, dedup_window_days: dedup, retention_days: retention ?? null, rotation_lag_seconds: lag, rate_limit: { per_second: rate, burst } } });
    if (result.error) error = result.error.message; else { saved = 'Project settings saved.'; await onchanged(); }
  }
  async function eraseActor(event: SubmitEvent) {
    event.preventDefault(); error = ''; saved = '';
    const result = await erase.execute({ projectId, externalId });
    if (result.error) error = result.error.message;
    else { saved = result.data!.eraseActor ? 'User data erased. Reports will refresh on the next processing cycle.' : 'No data found for this user.'; externalId = ''; confirmErase = false; await onchanged(); }
  }
  async function addDestination(event: SubmitEvent) {
    event.preventDefault(); error = ''; saved = '';
    const result = await saveDestination.execute({ projectId, name: destinationName, config: { project_id: mixpanelId, username_env: usernameEnv, secret_env: secretEnv, region } });
    if (result.error) error = result.error.message; else { saved = 'Destination saved.'; await query.refetch(); }
  }
  async function removeDestination(id: string) { const result = await deleteDestination.execute({ projectId, id }); if (result.error) error = result.error.message; else await query.refetch(); }
  async function copy() { try { await navigator.clipboard.writeText(issued); copied = true; } catch { error = 'Clipboard is unavailable. Select and copy the key below.'; } }
</script>

<div class="grid gap-6">
  {#if error || query.error}<div role="alert" class="error">{error || query.error?.message}</div>{/if}
  {#if saved}<div role="status" class="rounded-lg bg-brand-light p-4 text-sm text-brand">{saved}</div>{/if}
  {#if issued}<section class="rounded-xl border border-brand/30 bg-brand-light p-5"><div class="mb-3 flex justify-between"><h3>Copy your new key</h3><button class="text-sm text-brand" onclick={() => issued = ''}>Dismiss</button></div><p class="mb-3 text-sm text-muted">This key is shown only once.</p><div class="flex gap-3"><input aria-label="New key" readonly value={issued} class="font-mono" /><Button variant="secondary" onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button></div></section>{/if}
  <section class="surface p-6"><h2>Event sources</h2><p class="mt-1 text-sm text-muted">Give each application its own write key.</p>
    <form onsubmit={createWrite} class="my-5 flex items-end gap-3"><label class="flex-1">Source name<input bind:value={source} placeholder="ios" required maxlength="200" /></label><Button type="submit" disabled={issueWrite.isFetching}>Create write key</Button></form>
    {#if query.data?.project.writeKeys.length}<table><thead><tr><th>Source</th><th>Status</th><th></th></tr></thead><tbody>{#each query.data.project.writeKeys as key}<tr><td class="font-medium">{key.source}</td><td class="text-muted">{key.revokedAt ? 'Revoked' : 'Active'}</td><td class="text-right">{#if !key.revokedAt}<Button variant="ghost" disabled={revokeWrite.isFetching} onclick={() => revoke('write', key.id)}>Revoke</Button>{/if}</td></tr>{/each}</tbody></table>{/if}
  </section>
  <section class="surface p-6"><h2>Project access</h2><p class="mt-1 text-sm text-muted">Reader keys can view reports. Admin keys can manage this project.</p>
    <form onsubmit={createAccess} class="my-5 grid items-end gap-3 sm:grid-cols-[1fr_10rem_auto]"><label>Key name<input bind:value={accessName} placeholder="Product team" required /></label><label>Role<select bind:value={role}><option value="READER">Reader</option><option value="ADMIN">Admin</option></select></label><Button type="submit" disabled={issueAccess.isFetching}>Create access key</Button></form>
    {#if query.data?.project.accessKeys.length}<table><thead><tr><th>Name</th><th>Role</th><th>Status</th><th></th></tr></thead><tbody>{#each query.data.project.accessKeys as key}<tr><td class="font-medium">{key.name}</td><td class="text-muted">{key.role.toLowerCase()}</td><td class="text-muted">{key.revokedAt ? 'Revoked' : 'Active'}</td><td class="text-right">{#if !key.revokedAt}<Button variant="ghost" disabled={revokeAccess.isFetching} onclick={() => revoke('access', key.id)}>Revoke</Button>{/if}</td></tr>{/each}</tbody></table>{/if}
  </section>
  <section class="surface p-6"><h2>Processing and retention</h2>
    <form onsubmit={saveSettings} class="mt-5 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
      <label>Maximum event age (days)<input type="number" bind:value={maxAge} min="1" max="3650" required /></label><label>Deduplication window (days)<input type="number" bind:value={dedup} min="1" max="365" required /></label><label>Retention (days)<input type="number" bind:value={retention} min="1" max="36500" placeholder="Keep forever" /></label>
      <label>Processing delay (seconds)<input type="number" bind:value={lag} min="0" max="86400" required /></label><label>Batches per second<input type="number" bind:value={rate} step="any" min="0.01" required /></label><label>Burst batches<input type="number" bind:value={burst} min="1" max="1000000" required /></label>
      <label class="sm:col-span-2 lg:col-span-3">Sensitive property fragments<input bind:value={fragments} /><span class="text-xs font-normal text-muted">Comma-separated fragments. Matching properties are filtered before storage.</span></label>
      <p class="text-xs text-muted sm:col-span-2 lg:col-span-3">Retention removes complete months. Events awaiting delivery remain until delivery finishes.</p>
      <div><Button type="submit" disabled={update.isFetching}>Save settings</Button></div>
    </form>
  </section>
  <section class="surface p-6"><h2>Destinations</h2><p class="mt-1 text-sm text-muted">Forward sanitized events to Mixpanel. Credentials are read from the server environment.</p>
    {#each query.data?.project.destinations ?? [] as destination}<div class="mt-4 flex items-center justify-between rounded-lg bg-canvas p-4"><div><h3>{destination.name}</h3><p class="mt-1 text-xs text-muted">{destination.kind}</p></div><Button variant="ghost" disabled={deleteDestination.isFetching} onclick={() => removeDestination(destination.id)}>Remove</Button></div>{/each}
    <form onsubmit={addDestination} class="mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-3"><label>Destination name<input bind:value={destinationName} required /></label><label>Mixpanel project ID<input bind:value={mixpanelId} required pattern="[0-9]+" /></label><label>Region<select bind:value={region}><option value="us">US</option><option value="eu">EU</option><option value="in">India</option></select></label><label>Username environment variable<input bind:value={usernameEnv} required /></label><label>Secret environment variable<input bind:value={secretEnv} required /></label><div class="flex items-end"><Button type="submit" disabled={saveDestination.isFetching}>Save destination</Button></div></form>
  </section>
  <section class="surface border-red-200 p-6"><h2>Erase user data</h2><p class="mt-1 text-sm text-muted">Remove this user's events and activity from this project. Data already forwarded must be erased at the destination separately.</p>
    <form onsubmit={eraseActor} class="mt-5 grid gap-4"><label>External user ID<input bind:value={externalId} placeholder="user_42" required /></label><label class="flex items-center gap-2 font-normal"><input type="checkbox" bind:checked={confirmErase} />I want to permanently erase this user's data.</label><div><Button type="submit" variant="danger" disabled={!confirmErase || erase.isFetching}>Erase data</Button></div></form>
  </section>
</div>
