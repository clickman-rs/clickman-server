<script lang="ts">
  import { createQuery } from 'cachebay/svelte';
  import {
    ProjectAdminDocument,
    type ProjectAdminQuery,
    type ProjectAdminQueryVariables,
  } from '../graphql/generated';
  import { errorMessage } from '../graphql/errors';
  import type { Settings } from '../reports';
  import type { DestinationCreator } from '../types';
  import Keys from './Keys.svelte';
  import Processing from './Processing.svelte';
  import Destinations from './Destinations.svelte';
  import Erasure from './Erasure.svelte';
  let {
    projectId,
    settings,
    onchanged,
    createDestination,
    allowAccessKeys,
    maxRetentionDays,
  }: {
    projectId: string;
    settings: Settings;
    onchanged: () => Promise<void>;
    createDestination?: DestinationCreator;
    allowAccessKeys?: boolean;
    maxRetentionDays?: number;
  } = $props();
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
    <Keys {projectId} {data} onrefresh={refresh} {allowAccessKeys} />
    <Processing {projectId} {settings} {onchanged} {maxRetentionDays} />
    <Destinations
      {projectId}
      destinations={data.destinations}
      onrefresh={refresh}
      {createDestination}
    />
    <Erasure {projectId} {onchanged} />
  {:else if !query.error}<p role="status" class="cm-empty">Loading project settings…</p>{/if}
</div>
