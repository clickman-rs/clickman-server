<script lang="ts">
  import { Button } from '../ui/index';
  let { value, ondismiss }: { value: string; ondismiss: () => void } = $props();
  let copied = $state(false);
  let error = $state('');
  $effect(() => {
    value;
    copied = false;
    error = '';
  });
  async function copy() {
    const key = value;
    let timer: ReturnType<typeof setTimeout>;
    const deadline = new Promise<never>((_resolve, reject) => {
      timer = setTimeout(() => reject(new Error('Clipboard timed out.')), 3000);
    });
    try {
      await Promise.race([navigator.clipboard.writeText(key), deadline]);
      if (key === value) copied = true;
    } catch {
      error = 'Clipboard is unavailable. Select and copy the key below.';
    } finally {
      clearTimeout(timer!);
    }
  }
</script>

<section class="cm:rounded-xl cm:border cm:border-brand/30 cm:bg-brand-light cm:p-5">
  <div class="cm:mb-3 cm:flex cm:justify-between">
    <h3>Copy your new key</h3>
    <button type="button" class="cm:text-sm cm:text-brand" onclick={ondismiss}>Dismiss</button>
  </div>
  <p class="cm:mb-3 cm:text-sm cm:text-muted">This key is shown only once.</p>
  <div class="cm:flex cm:gap-3">
    <input aria-label="New key" readonly {value} class="cm:font-mono" /><Button
      variant="secondary"
      onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button
    >
  </div>
  {#if error}<p role="alert" class="cm:mt-3 cm:text-sm">{error}</p>{/if}
</section>
