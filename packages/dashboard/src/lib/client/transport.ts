import { createCachebay, CombinedError } from 'cachebay';
import { parse, print, getOperationAST, type ExecutionResult, type GraphQLError } from 'graphql';

export interface ClientOptions {
  endpoint: string;
  lifetime: AbortSignal;
  token?: string;
  credentials?: RequestCredentials;
  headers?: Record<string, string>;
}

/** The host owns lifetime/disposal. Recreate this client when the authenticated
 * user or organization changes; the SaaS host also recreates it per project. */
export function createClickmanClient(options: ClientOptions) {
  return createCachebay({
    cachePolicy: 'cache-and-network',
    suspensionTimeout: 0,
    transport: {
      http: async ({ query, variables }) => {
        const signal = AbortSignal.any([options.lifetime, AbortSignal.timeout(30_000)]);
        try {
          signal.throwIfAborted();
          const document = typeof query === 'string' ? parse(query) : query;
          const response = await fetch(options.endpoint, {
            method: 'POST',
            credentials: options.credentials ?? 'omit',
            headers: {
              'content-type': 'application/json',
              ...options.headers,
              ...(options.token ? { authorization: `Bearer ${options.token}` } : {}),
            },
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
                ? options.token
                  ? 'This access token was rejected. Sign out and use a valid token.'
                  : 'Authentication was rejected. Sign in again.'
                : response.status === 403
                  ? 'You no longer have access to this organization or action.'
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
