<script lang="ts">
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import { ClickmanWorkspace, type ClickmanRoute } from '@clickman/dashboard';
  import { useClient } from '$lib/graphql/context';
  import type { PageProps } from './$types';
  let { data }: PageProps = $props();
  const client = useClient();
  function navigate(route: ClickmanRoute) {
    return goto(
      `${base}/projects${route.projectId ? `/${encodeURIComponent(route.projectId)}/${route.section}` : ''}`,
      { replaceState: data.route.projectId === null },
    );
  }
</script>

<ClickmanWorkspace {client} route={data.route} onnavigate={navigate} />
