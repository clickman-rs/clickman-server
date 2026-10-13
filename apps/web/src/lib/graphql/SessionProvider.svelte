<script lang="ts">
  import { untrack, onDestroy, type Snippet } from 'svelte';
  import { base } from '$app/paths';
  import { createStandaloneClient } from './client';
  import { setClient } from './context';

  let { token, children }: { token: string; children: Snippet } = $props();
  const requests = new AbortController();
  const client = createStandaloneClient(
    untrack(() => token),
    import.meta.env.VITE_CLICKMAN_GRAPHQL_ENDPOINT ?? `${base}/graphql`,
    requests.signal,
  );
  setClient(client);
  onDestroy(() => {
    requests.abort();
    client.dispose();
  });
</script>

{@render children()}
