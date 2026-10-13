<script lang="ts">
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import { page } from '$app/state';
  import { ClickmanButton } from '@clickman/dashboard';
  import { tokenState, signIn } from '$lib/auth/session';
  let input = $state('');
  let error = $state('');
  function destination() {
    const next = page.url.searchParams.get('next');
    return next?.startsWith(`${base}/projects`) ? next : `${base}/projects`;
  }
  function submit(event: SubmitEvent) {
    event.preventDefault();
    error = '';
    try {
      signIn(input.trim());
      input = '';
    } catch {
      error = 'Unable to save this browser tab’s session. Allow browser storage and try again.';
    }
  }
  $effect(() => {
    if ($tokenState)
      void goto(destination(), { replaceState: true }).catch(() => {
        error = 'Unable to open the dashboard. Reload this page to try again.';
      });
  });
</script>

<main class="login">
  <section>
    <div class="brand"><span>c.</span>clickman</div>
    <h1>Know your product.</h1>
    <p class="subtitle">Product analytics in your own database.</p>
    {#if error}<p role="alert" class="cm-error">{error}</p>{/if}
    {#if !$tokenState}<form onsubmit={submit} class="cm-surface">
        <label
          >Access token<input
            type="password"
            bind:value={input}
            required
            autocomplete="off"
            placeholder="Paste your access token"
          /></label
        >
        <p>Use an instance administrator token or a project access token.</p>
        <ClickmanButton type="submit" disabled={!input.trim()}
          >Open dashboard <span aria-hidden="true">→</span></ClickmanButton
        >
      </form>
      <p class="session-note">Your token stays in this browser tab's session.</p>
    {:else}<p role="status">Opening dashboard…</p>{/if}
  </section>
</main>

<style>
  .login {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 1.5rem;
  }
  section {
    width: min(100%, 28rem);
  }
  .brand {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 2.5rem;
  }
  .brand span {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 0.75rem;
    background: var(--cm-color-brand);
    color: white;
  }
  .subtitle {
    margin-top: 0.75rem;
    color: var(--cm-color-muted);
  }
  form {
    margin-top: 2rem;
    display: grid;
    gap: 1.25rem;
    padding: 1.5rem;
  }
  form p,
  .session-note {
    font-size: 0.75rem;
    color: var(--cm-color-muted);
  }
  .session-note {
    margin-top: 1.5rem;
    text-align: center;
  }
</style>
