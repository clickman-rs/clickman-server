<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  let { children, variant = 'primary', class: className = '', ...rest }: HTMLButtonAttributes & { children?: Snippet; variant?: 'primary' | 'secondary' | 'ghost' | 'danger' } = $props();
  const styles = { primary: 'bg-brand text-white hover:bg-brand/90', secondary: 'border border-line bg-white text-ink hover:bg-canvas', ghost: 'text-muted hover:bg-brand-light hover:text-brand', danger: 'border border-red-200 text-red-700 hover:bg-red-50' };
</script>
<button type="button" class="inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors disabled:opacity-50 {styles[variant]} {className}" {...rest}>
  {#if children}{@render children()}{/if}
</button>
