import { createCachebay, CombinedError } from 'cachebay';
import { parse, print, getOperationAST, type ExecutionResult, type GraphQLError } from 'graphql';

export function createStandaloneClient(token: string, endpoint: string, lifetime: AbortSignal) {
  return createCachebay({
    cachePolicy: 'cache-and-network',
    suspensionTimeout: 0,
    transport: {
      http: async ({ query, variables }) => {
        const signal = AbortSignal.any([lifetime, AbortSignal.timeout(30_000)]);
        try {
          signal.throwIfAborted();
          const document = typeof query === 'string' ? parse(query) : query;
          const response = await fetch(endpoint, {
            method: 'POST',
            credentials: 'omit',
            headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
            body: JSON.stringify({
              query: typeof query === 'string' ? query : print(query),
              variables,
              operationName: getOperationAST(document)?.name?.value,
            }),
            signal,
          });
          if (!response.ok)
            throw new Error(
              response.status === 401
                ? 'This access token was rejected. Sign out and use a valid token.'
                : `The server returned HTTP ${response.status}.`,
            );
          const result: ExecutionResult = await response.json();
          signal.throwIfAborted();
          return {
            data: result.data ?? null,
            error: result.errors?.length
              ? new CombinedError({ graphqlErrors: [...result.errors] as GraphQLError[], response })
              : null,
          };
        } catch (cause) {
          return {
            data: null,
            error: new CombinedError({
              networkError: cause instanceof Error ? cause : new Error('Unable to reach Clickman.'),
            }),
          };
        }
      },
    },
  });
}
