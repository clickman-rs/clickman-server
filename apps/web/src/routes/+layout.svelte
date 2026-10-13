<script lang="ts">
  import '../app.css';
  import '@clickman/dashboard/styles.css';
  import { afterNavigate } from '$app/navigation';
  import { loadSession } from '$lib/auth/session';
  let { children } = $props();
  let ready = $state(false);
  let error = $state('');
  function load() {
    try {
      loadSession();
      ready = true;
      error = '';
    } catch {
      error = 'Unable to read this browser tab’s session. Allow browser storage and try again.';
    }
  }
  afterNavigate(({ type }) => {
    if (type === 'enter') load();
  });
</script>

<svelte:head><title>Clickman — Product analytics</title></svelte:head>
<div class="clickman">
  {#if error}<main class="session-error">
      <p role="alert">{error}</p>
      <button onclick={load}>Try again</button>
    </main>
  {:else if !ready}<p role="status" class="opening">Opening Clickman…</p>
  {:else}{@render children()}{/if}
</div>

<style>
  .opening {
    padding: 2rem;
  }
  .session-error {
    max-width: 32rem;
    margin: 4rem auto;
    padding: 2rem;
  }
</style>
