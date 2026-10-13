import { createContext } from 'svelte';
import type { CachebayInstance } from 'cachebay';

export const [useClient, setClient] = createContext<CachebayInstance>();
