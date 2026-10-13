import { untrack } from 'svelte';
import type { CreateQueryReturn } from 'cachebay/svelte';

export function pollVisible(
  query: CreateQueryReturn,
  enabled: () => boolean,
  onerror: (cause: unknown) => void,
): void {
  $effect(() => {
    if (!enabled()) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      if (stopped || document.visibilityState !== 'visible' || query.isFetching) return;
      try {
        await query.refetch();
      } catch (cause) {
        onerror(cause);
      }
    };
    const tick = async () => {
      await refresh();
      if (!stopped) timer = setTimeout(tick, 60_000);
    };
    untrack(() => {
      timer = setTimeout(tick, 60_000);
    });
    document.addEventListener('visibilitychange', refresh);
    window.addEventListener('focus', refresh);
    return () => {
      stopped = true;
      clearTimeout(timer);
      document.removeEventListener('visibilitychange', refresh);
      window.removeEventListener('focus', refresh);
    };
  });
}
