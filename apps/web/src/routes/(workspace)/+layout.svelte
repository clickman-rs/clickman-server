<script lang="ts">
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import { page } from '$app/state';
  import { ClickmanButton } from '@clickman/dashboard';
  import { tokenState, signOut } from '$lib/auth/session';
  import SessionProvider from '$lib/graphql/SessionProvider.svelte';
  let { children } = $props();
  let error = $state('');
  let preserveDestination = $state(true);
  function logout() {
    try {
      signOut();
      preserveDestination = false;
    } catch {
      error = 'Unable to clear this browser tab’s session. Allow browser storage and try again.';
    }
  }
  $effect(() => {
    if (!$tokenState) {
      const next = preserveDestination
        ? `?next=${encodeURIComponent(page.url.pathname + page.url.search)}`
        : '';
      void goto(`${base}/login${next}`, { replaceState: true }).catch(() => {
        error = 'Unable to open sign-in. Reload this page to try again.';
      });
    }
  });
</script>

{#if error}<p role="alert" class="cm-error">{error}</p>{/if}
{#if $tokenState}{#key $tokenState}<SessionProvider token={$tokenState}>
      <header>
        <div class="header-content">
          <a class="brand" href={`${base}/projects`}><span>c.</span>clickman</a>
          <div>
            <span class="tagline">Your database. Your data.</span><ClickmanButton
              variant="ghost"
              onclick={logout}>Sign out</ClickmanButton
            >
          </div>
        </div>
      </header>
      <main>{@render children()}</main>
    </SessionProvider>{/key}{/if}

<style>
  header {
    border-bottom: 1px solid var(--cm-color-line);
    background: white;
    padding: 0 1.5rem;
  }
  .header-content {
    max-width: 80rem;
    height: 5rem;
    margin: auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .header-content > div {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .brand {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    font-size: 1.25rem;
    font-weight: 600;
    text-decoration: none;
    color: inherit;
  }
  .brand span {
    display: grid;
    place-items: center;
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 0.75rem;
    background: var(--cm-color-brand);
    color: white;
  }
  .tagline {
    color: var(--cm-color-muted);
    font-size: 0.75rem;
  }
  main {
    max-width: 80rem;
    margin: auto;
    padding: 2rem 1.5rem;
  }
  @media (max-width: 640px) {
    .tagline {
      display: none;
    }
  }
</style>
