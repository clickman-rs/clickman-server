import { writable } from 'svelte/store';

export const tokenState = writable<string | null>(null);

export function loadSession(): void {
  tokenState.set(sessionStorage.getItem('clickman.token') || null);
}

export function signIn(token: string): void {
  sessionStorage.setItem('clickman.token', token);
  tokenState.set(token);
}

export function signOut(): void {
  sessionStorage.removeItem('clickman.token');
  tokenState.set(null);
}
