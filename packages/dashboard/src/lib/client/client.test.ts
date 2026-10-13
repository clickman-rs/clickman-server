import { afterEach, expect, test, vi } from 'vitest';
import { createClickmanClient, createClickmanApi } from './index';
import type { CachebayInstance } from 'cachebay';
const clients: CachebayInstance[] = [];
const lifetimes: AbortController[] = [];
afterEach(() => {
  for (const life of lifetimes.splice(0)) life.abort();
  for (const client of clients.splice(0)) client.dispose();
  vi.unstubAllGlobals();
});
function setup() {
  const life = new AbortController();
  lifetimes.push(life);
  const client = createClickmanClient({
    endpoint: '/api/organizations/3/graphql',
    credentials: 'same-origin',
    lifetime: life.signal,
  });
  clients.push(client);
  return { client, life, api: createClickmanApi(client, 'project-a', life.signal) };
}
test('headless tools use the host session transport and a fixed project scope', async () => {
  const fetcher = vi.fn().mockResolvedValue(
    new Response(
      JSON.stringify({
        data: {
          __typename: 'Query',
          project: {
            __typename: 'Project',
            id: 'project-a',
            slug: 'web',
            name: 'Web',
            canAdmin: false,
            settings: {},
            reports: [],
            funnels: [],
          },
        },
      }),
    ),
  );
  vi.stubGlobal('fetch', fetcher);
  const { api } = setup();
  expect((await api.project()).id).toBe('project-a');
  const [url, init] = fetcher.mock.calls[0] as [string, RequestInit];
  expect(url).toBe('/api/organizations/3/graphql');
  expect(init.credentials).toBe('same-origin');
  expect(init.headers).not.toHaveProperty('authorization');
  expect(JSON.parse(init.body as string)).toMatchObject({
    variables: { id: 'project-a' },
    operationName: 'Analytics',
  });
});
test('a project scope that ends during a read cannot return the old result', async () => {
  let started!: () => void;
  const ready = new Promise<void>((r) => (started = r));
  vi.stubGlobal(
    'fetch',
    vi.fn(
      (_url: string, init: RequestInit) =>
        new Promise<Response>((_resolve, reject) => {
          init.signal!.addEventListener('abort', () => reject(init.signal!.reason), { once: true });
          started();
        }),
    ),
  );
  const { api, life } = setup();
  const pending = api.project();
  await ready;
  life.abort();
  await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
});
test('write tools preserve denial and never write after scope disposal', async () => {
  const fetcher = vi.fn().mockResolvedValue(
    new Response(
      JSON.stringify({
        errors: [{ message: 'project administration denied', extensions: { code: 'FORBIDDEN' } }],
      }),
    ),
  );
  vi.stubGlobal('fetch', fetcher);
  const { api, life } = setup();
  const definition = {
    key: 'purchase',
    name: 'Purchase',
    steps: [{ event: 'view' }, { event: 'buy' }],
  } as const;
  await expect(
    api.saveFunnel({ ...definition, steps: [...definition.steps] }),
  ).rejects.toMatchObject({
    graphqlErrors: [expect.objectContaining({ message: 'project administration denied' })],
  });
  life.abort();
  await expect(
    api.saveSegment({
      key: 'visitors',
      name: 'Visitors',
      rule: { kind: 'event', filter: { event: 'view' }, range: '7d' },
    }),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(fetcher).toHaveBeenCalledTimes(1);
});
