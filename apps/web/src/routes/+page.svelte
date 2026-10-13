<script lang="ts">
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import { tokenState } from '$lib/auth/session';
  let error = $state('');
  $effect(() => {
    void goto(`${base}/${$tokenState ? 'projects' : 'login'}`, { replaceState: true }).catch(() => {
      error = 'Unable to open Clickman. Reload this page to try again.';
    });
  });
</script>

{#if error}<p role="alert">{error}</p>{:else}<p role="status">Opening dashboard…</p>{/if}
