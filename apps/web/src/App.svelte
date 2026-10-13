<script lang="ts">
  import { Button } from '@clickman/ui';
  import Dashboard from './Dashboard.svelte';
  let token = $state(sessionStorage.getItem('clickman.token') ?? '');
  let input = $state('');
  function login(event: SubmitEvent) { event.preventDefault(); token = input.trim(); if (token) sessionStorage.setItem('clickman.token', token); input = ''; }
  function logout() { sessionStorage.removeItem('clickman.token'); token = ''; }
</script>

{#if token}
  {#key token}<Dashboard {token} onlogout={logout} />{/key}
{:else}
  <main class="flex min-h-screen items-center justify-center p-6">
    <div class="w-full max-w-md">
      <div class="mb-10 flex items-center gap-3"><span class="flex h-10 w-10 items-center justify-center rounded-xl bg-brand font-semibold text-white">c.</span><span class="text-xl font-semibold tracking-tight">clickman</span></div>
      <h1>Know your product.</h1><p class="mt-3 text-muted">Product analytics in your own database.</p>
      <form onsubmit={login} class="surface mt-8 grid gap-5 p-6">
        <label>Access token<input type="password" bind:value={input} required autocomplete="off" placeholder="Paste your access token" /></label>
        <p class="text-xs leading-relaxed text-muted">Use an instance administrator token or a project access token.</p>
        <Button type="submit" disabled={!input.trim()}>Open dashboard <span aria-hidden="true">→</span></Button>
      </form>
      <p class="mt-6 text-center text-xs text-muted">Your token stays in this browser tab's session.</p>
    </div>
  </main>
{/if}
