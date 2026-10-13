<script lang="ts">
  import { page } from '$app/state';
  import { base } from '$app/paths';
</script>

<main class="error-page">
  <h1>{page.status}</h1>
  <p>{page.error?.message ?? 'Unable to open this page.'}</p>
  <a href={`${base}/projects`}>Back to Clickman</a>
</main>

<style>
  .error-page {
    padding: 3rem;
  }
</style>
