import { expect, test } from 'vitest';
import { CombinedError } from 'cachebay';
import { GraphQLError } from 'graphql';
import { dataOrThrow } from './errors';

test('partial GraphQL data with errors cannot be treated as a completed write', () => {
  const error = new CombinedError({ graphqlErrors: [new GraphQLError('Write was rejected')] });
  expect(() => dataOrThrow({ data: { id: 'partial-result' }, error })).toThrow(error);
});

test('a result without data or an error is not a successful operation', () => {
  expect(() => dataOrThrow({ data: null, error: null })).toThrow('The server returned no data');
});
