import { createClickmanClient } from '@clickman/dashboard/client';

export function createStandaloneClient(token: string, endpoint: string, lifetime: AbortSignal) {
  return createClickmanClient({ token, endpoint, lifetime });
}
