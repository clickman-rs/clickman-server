<script lang="ts">
  import { createQuery } from 'cachebay/svelte';
  import {
    ProjectAdminDocument,
    type ProjectAdminQuery,
    type ProjectAdminQueryVariables,
  } from '../graphql/generated';
  import { errorMessage } from '../graphql/errors';
  import type { Settings } from '../reports';
  import Keys from './Keys.svelte';
  import Processing from './Processing.svelte';
  import Destinations from './Destinations.svelte';
  import Erasure from './Erasure.svelte';
  let {
    projectId,
    settings,
    onchanged,
  }: { projectId: string; settings: Settings; onchanged: () => Promise<void> } = $props();
  const query = createQuery<ProjectAdminQuery, ProjectAdminQueryVariables>({
    query: ProjectAdminDocument,
    variables: () => ({ id: projectId }),
  });
  const data = $derived(query.data?.project.id === projectId ? query.data.project : undefined);
  async function refresh() {
    await query.refetch();
    if (query.error) throw query.error;
  }
</script>

<div class="cm:grid cm:gap-6">
  {#if query.error}<p role="alert" class="cm-error">{errorMessage(query.error)}</p>{/if}
  {#if data}
    <Keys {projectId} {data} onrefresh={refresh} />
    <Processing {projectId} {settings} {onchanged} />
    <Destinations {projectId} destinations={data.destinations} onrefresh={refresh} />
    <Erasure {projectId} {onchanged} />
  {:else if !query.error}<p role="status" class="cm-empty">Loading project settings…</p>{/if}
</div>
