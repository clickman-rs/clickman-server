import { createCachebay, CombinedError, type HttpContext, type OperationResult } from 'cachebay';

export function createCache(token: string) {
  return createCachebay({
    cachePolicy: 'cache-and-network',
    transport: { http: async (ctx: HttpContext): Promise<OperationResult> => {
      try {
        const response = await fetch(import.meta.env.VITE_CLICKMAN_GRAPHQL_ENDPOINT ?? './graphql', { method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` }, body: JSON.stringify({ query: ctx.query, variables: ctx.variables }) });
        if (!response.ok) throw new Error(response.status === 401 ? 'This access token was rejected. Sign out and use a valid token.' : `The server returned HTTP ${response.status}.`);
        const result = await response.json();
        return { data: result.data ?? null, error: result.errors?.length ? new CombinedError({ graphqlErrors: result.errors, response }) : null };
      } catch (error) { return { data: null, error: new CombinedError({ networkError: error instanceof Error ? error : new Error(String(error)) }) }; }
    } },
  });
}
