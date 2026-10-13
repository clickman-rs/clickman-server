import { afterEach, expect, test, vi } from 'vitest';
import type { CachebayInstance } from 'cachebay';
import { createStandaloneClient } from './client';

const clients: CachebayInstance[] = [];
const lifetimes: AbortController[] = [];
const query = 'query Viewer { viewer { canCreateProjects } }';
function client() {
  const lifetime = new AbortController();
  lifetimes.push(lifetime);
  const cache = createStandaloneClient('test-access-token', '/clickman/graphql', lifetime.signal);
  clients.push(cache);
  return { cache, lifetime };
}
afterEach(() => {
  for (const lifetime of lifetimes.splice(0)) lifetime.abort();
  for (const cache of clients.splice(0)) cache.dispose();
  vi.unstubAllGlobals();
});

test('standalone authenticates its named GraphQL operation without host cookies', async () => {
  const fetcher = vi.fn().mockResolvedValue(
    new Response(
      JSON.stringify({
        data: { __typename: 'Query', viewer: { __typename: 'Viewer', canCreateProjects: true } },
      }),
      { headers: { 'content-type': 'application/json' } },
    ),
  );
  vi.stubGlobal('fetch', fetcher);
  const { cache } = client();
  const response = await cache.executeQuery<{ viewer: { canCreateProjects: boolean } }>({
    query,
    variables: {},
  });
  expect(response.error).toBeNull();
  expect(response.data?.viewer.canCreateProjects).toBe(true);
  const [endpoint, init] = fetcher.mock.calls[0] as [string, RequestInit];
  expect(endpoint).toBe('/clickman/graphql');
  expect(init.credentials).toBe('omit');
  expect(init.headers).toMatchObject({ authorization: 'Bearer test-access-token' });
  expect(JSON.parse(init.body as string)).toMatchObject({ operationName: 'Viewer', variables: {} });
});

test('an authentication rejection is observable as an operation error', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 401 })));
  const { cache } = client();
  const response = await cache.executeQuery({ query });
  expect(response.data).toBeNull();
  expect(response.error?.networkError?.message).toContain('This access token was rejected');
});

test('GraphQL errors are preserved instead of becoming an empty success', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          errors: [{ message: 'Project access denied', extensions: { code: 'FORBIDDEN' } }],
        }),
      ),
    ),
  );
  const { cache } = client();
  const response = await cache.executeQuery({ query });
  expect(response.data).toBeNull();
  expect(response.error?.graphqlErrors?.[0]?.message).toBe('Project access denied');
});

test('ending the session cancels an outstanding HTTP request', async () => {
  let started!: () => void;
  const ready = new Promise<void>((resolve) => {
    started = resolve;
  });
  vi.stubGlobal(
    'fetch',
    vi.fn(
      (_url: string, init: RequestInit) =>
        new Promise<Response>((_resolve, reject) => {
          const signal = init.signal!;
          signal.addEventListener('abort', () => reject(signal.reason), { once: true });
          started();
        }),
    ),
  );
  const { cache, lifetime } = client();
  const pending = cache.executeQuery({ query });
  await ready;
  lifetime.abort();
  const response = await pending;
  expect(response.data).toBeNull();
  expect(response.error?.networkError?.name).toBe('AbortError');
});
