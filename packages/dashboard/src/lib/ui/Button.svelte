<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  let {
    children,
    variant = 'primary',
    class: className = '',
    ...rest
  }: HTMLButtonAttributes & {
    children?: Snippet;
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  } = $props();
  const styles = {
    primary: 'cm:bg-brand cm:text-white cm:hover:bg-brand/90',
    secondary: 'cm:border cm:border-line cm:bg-white cm:text-ink cm:hover:bg-canvas',
    ghost: 'cm:text-muted cm:hover:bg-brand-light cm:hover:text-brand',
    danger: 'cm:border cm:border-red-200 cm:text-red-700 cm:hover:bg-red-50',
  };
</script>

<button
  type="button"
  class="cm:inline-flex cm:items-center cm:justify-center cm:gap-2 cm:rounded-lg cm:px-4 cm:py-2.5 cm:text-sm cm:font-medium cm:transition-colors cm:disabled:opacity-50 {styles[
    variant
  ]} {className}"
  {...rest}
>
  {#if children}{@render children()}{/if}
</button>
