<script lang="ts">
  import type { ClickmanWorkspaceProps } from './types';
  import Provider from './graphql/Provider.svelte';
  import WorkspaceContent from './WorkspaceContent.svelte';

  let { client, route, onnavigate }: ClickmanWorkspaceProps = $props();
</script>

<div class="clickman">
  {#key client}
    <Provider {client}><WorkspaceContent {route} {onnavigate} /></Provider>
  {/key}
</div>
