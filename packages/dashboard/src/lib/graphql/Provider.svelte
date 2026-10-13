<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { setCachebay } from 'cachebay/svelte';
  import type { CachebayInstance } from 'cachebay';

  let { client, children }: { client: CachebayInstance; children: Snippet } = $props();
  setCachebay(untrack(() => client));
</script>

{@render children()}
