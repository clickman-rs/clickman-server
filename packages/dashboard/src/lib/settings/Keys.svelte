<script lang="ts">
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { createAction } from '../ui/action.svelte';
  import { dataOrThrow } from '../graphql/errors';
  import IssuedKey from './IssuedKey.svelte';
  import {
    IssueWriteKeyDocument,
    IssueAccessKeyDocument,
    RevokeWriteKeyDocument,
    RevokeAccessKeyDocument,
    type IssueWriteKeyMutation,
    type IssueWriteKeyMutationVariables,
    type IssueAccessKeyMutation,
    type IssueAccessKeyMutationVariables,
    type RevokeWriteKeyMutation,
    type RevokeWriteKeyMutationVariables,
    type RevokeAccessKeyMutation,
    type RevokeAccessKeyMutationVariables,
    type ProjectAdminQuery,
    type ProjectRole,
  } from '../graphql/generated';
  let {
    projectId,
    data,
    onrefresh,
  }: { projectId: string; data: ProjectAdminQuery['project']; onrefresh: () => Promise<void> } =
    $props();
  const action = createAction();
  const issueWrite = createMutation<IssueWriteKeyMutation, IssueWriteKeyMutationVariables>({
    query: IssueWriteKeyDocument,
  });
  const issueAccess = createMutation<IssueAccessKeyMutation, IssueAccessKeyMutationVariables>({
    query: IssueAccessKeyDocument,
  });
  const revokeWrite = createMutation<RevokeWriteKeyMutation, RevokeWriteKeyMutationVariables>({
    query: RevokeWriteKeyDocument,
  });
  const revokeAccess = createMutation<RevokeAccessKeyMutation, RevokeAccessKeyMutationVariables>({
    query: RevokeAccessKeyDocument,
  });
  let source = $state('');
  let accessName = $state('');
  let role = $state<ProjectRole>('READER');
  let issued = $state('');
  function createWrite(event: SubmitEvent) {
    event.preventDefault();
    void action.run(async () => {
      const result = dataOrThrow<IssueWriteKeyMutation>(
        await issueWrite.execute({ projectId, source }),
      );
      issued = result.issueWriteKey.token;
      source = '';
      await onrefresh();
    });
  }
  function createAccess(event: SubmitEvent) {
    event.preventDefault();
    void action.run(async () => {
      const result = dataOrThrow<IssueAccessKeyMutation>(
        await issueAccess.execute({ projectId, name: accessName, role }),
      );
      issued = result.issueAccessKey.token;
      accessName = '';
      await onrefresh();
    });
  }
  function revoke(kind: 'write' | 'access', id: string) {
    void action.run(async () => {
      if (kind === 'write') dataOrThrow(await revokeWrite.execute({ projectId, id }));
      else dataOrThrow(await revokeAccess.execute({ projectId, id }));
      await onrefresh();
    });
  }
</script>

{#if action.error}<p role="alert" class="cm-error">{action.error}</p>{/if}
{#if issued}<IssuedKey
    value={issued}
    ondismiss={() => {
      issued = '';
    }}
  />{/if}
<fieldset disabled={action.busy} class="cm:grid cm:gap-6">
  <section class="cm-surface cm:p-6">
    <h2>Event sources</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">Give each application its own write key.</p>
    <form
      onsubmit={createWrite}
      class="cm:my-5 cm:grid cm:items-end cm:gap-3 cm:sm:grid-cols-[1fr_auto]"
    >
      <label class="cm:flex-1"
        >Source name<input bind:value={source} placeholder="ios" required maxlength="200" /></label
      ><Button type="submit" disabled={action.busy}>Create write key</Button>
    </form>
    {#if data.writeKeys.length}<table>
        <thead><tr><th>Source</th><th>Status</th><th></th></tr></thead><tbody
          >{#each data.writeKeys as key}<tr
              ><td class="cm:font-medium">{key.source}</td><td class="cm:text-muted"
                >{key.revokedAt ? 'Revoked' : 'Active'}</td
              ><td class="cm:text-right"
                >{#if !key.revokedAt}<Button
                    variant="ghost"
                    disabled={action.busy}
                    onclick={() => revoke('write', key.id)}>Revoke</Button
                  >{/if}</td
              ></tr
            >{/each}</tbody
        >
      </table>{/if}
  </section>
  <section class="cm-surface cm:p-6">
    <h2>Project access</h2>
    <p class="cm:mt-1 cm:text-sm cm:text-muted">
      Reader keys can view reports. Admin keys can manage this project.
    </p>
    <form
      onsubmit={createAccess}
      class="cm:my-5 cm:grid cm:items-end cm:gap-3 cm:sm:grid-cols-[1fr_10rem_auto]"
    >
      <label>Key name<input bind:value={accessName} placeholder="Product team" required /></label
      ><label
        >Role<select bind:value={role}
          ><option value="READER">Reader</option><option value="ADMIN">Admin</option></select
        ></label
      ><Button type="submit" disabled={action.busy}>Create access key</Button>
    </form>
    {#if data.accessKeys.length}<table>
        <thead><tr><th>Name</th><th>Role</th><th>Status</th><th></th></tr></thead><tbody
          >{#each data.accessKeys as key}<tr
              ><td class="cm:font-medium">{key.name}</td><td class="cm:text-muted"
                >{key.role.toLowerCase()}</td
              ><td class="cm:text-muted">{key.revokedAt ? 'Revoked' : 'Active'}</td><td
                class="cm:text-right"
                >{#if !key.revokedAt}<Button
                    variant="ghost"
                    disabled={action.busy}
                    onclick={() => revoke('access', key.id)}>Revoke</Button
                  >{/if}</td
              ></tr
            >{/each}</tbody
        >
      </table>{/if}
  </section>
</fieldset>
