import type { CachebayInstance } from 'cachebay';
import {
  AnalyticsDocument,
  SegmentsDocument,
  SaveFunnelDocument,
  SaveSegmentDocument,
  type AnalyticsQuery,
  type AnalyticsQueryVariables,
  type SegmentsQuery,
  type SegmentsQueryVariables,
  type SaveFunnelMutation,
  type SaveFunnelMutationVariables,
  type SaveSegmentMutation,
  type SaveSegmentMutationVariables,
} from '../graphql/generated';
import { dataOrThrow } from '../graphql/errors';
import type { FunnelDefinition, SegmentDefinition } from './types';
export * from './types';
export { createClickmanClient, type ClientOptions } from './transport';
export * from '../graphql/generated';
export { dataOrThrow, errorMessage } from '../graphql/errors';

/** Headless API for dashboard extensions and browser agent tools. The caller
 * supplies its existing Cachebay client and an immutable project scope. */
export function createClickmanApi(
  client: CachebayInstance,
  projectId: string,
  lifetime: AbortSignal,
) {
  const active = () => lifetime.throwIfAborted();
  return {
    projectId,
    async project() {
      active();
      const result = await client.executeQuery<AnalyticsQuery, AnalyticsQueryVariables>({
        query: AnalyticsDocument,
        variables: { id: projectId },
        cachePolicy: 'network-only',
      });
      active();
      const data = dataOrThrow(result).project;
      if (data.id !== projectId) throw new Error('The server returned a different project.');
      return data;
    },
    async segments() {
      active();
      const result = await client.executeQuery<SegmentsQuery, SegmentsQueryVariables>({
        query: SegmentsDocument,
        variables: { id: projectId },
        cachePolicy: 'network-only',
      });
      active();
      const data = dataOrThrow(result).project;
      if (data.id !== projectId) throw new Error('The server returned a different project.');
      return data.segments;
    },
    async saveFunnel(definition: FunnelDefinition) {
      active();
      const result = await client.executeMutation<SaveFunnelMutation, SaveFunnelMutationVariables>({
        query: SaveFunnelDocument,
        variables: { projectId, definition },
      });
      active();
      return dataOrThrow(result).saveFunnel;
    },
    async saveSegment(
      definition: SegmentDefinition,
      options: { enabled?: boolean; fireExisting?: boolean } = {},
    ) {
      active();
      const result = await client.executeMutation<
        SaveSegmentMutation,
        SaveSegmentMutationVariables
      >({
        query: SaveSegmentDocument,
        variables: {
          projectId,
          definition,
          enabled: options.enabled ?? true,
          fireExisting: options.fireExisting ?? false,
        },
      });
      active();
      return dataOrThrow(result).saveSegment;
    },
  };
}
export type ClickmanApi = ReturnType<typeof createClickmanApi>;
