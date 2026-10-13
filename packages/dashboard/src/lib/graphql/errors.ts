import { CombinedError, type OperationResult } from 'cachebay';

export function errorMessage(error: unknown): string {
  if (error instanceof CombinedError)
    return error.networkError?.message ?? error.graphqlErrors?.[0]?.message ?? error.message;
  return error instanceof Error ? error.message : 'Unable to complete this operation.';
}

export function dataOrThrow<T>(result: OperationResult<T>): T {
  if (result.error) throw result.error;
  if (result.data == null) throw new Error('The server returned no data. Please try again.');
  return result.data;
}
