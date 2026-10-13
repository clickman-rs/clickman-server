import type { TypedDocumentNode as DocumentNode } from '@graphql-typed-document-node/core';
export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
export type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
export type MakeOptional<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]?: Maybe<T[SubKey]> };
export type MakeMaybe<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]: Maybe<T[SubKey]> };
export type MakeEmpty<T extends { [key: string]: unknown }, K extends keyof T> = { [_ in K]?: never };
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: { input: string; output: string; }
  String: { input: string; output: string; }
  Boolean: { input: boolean; output: boolean; }
  Int: { input: number; output: number; }
  Float: { input: number; output: number; }
  /**
   * Implement the DateTime<Utc> scalar
   *
   * The input/output is a string in RFC3339 format.
   */
  DateTime: { input: string; output: string; }
  /** A scalar that can represent any JSON value. */
  JSON: { input: unknown; output: unknown; }
  /**
   * A UUID is a unique 128-bit number, stored as 16 octets. UUIDs are parsed as
   * Strings within GraphQL. UUIDs are used to assign unique identifiers to
   * entities without requiring a central allocating authority.
   *
   * # References
   *
   * * [Wikipedia: Universally Unique Identifier](http://en.wikipedia.org/wiki/Universally_unique_identifier)
   * * [RFC4122: A Universally Unique Identifier (UUID) URN Namespace](http://tools.ietf.org/html/rfc4122)
   */
  UUID: { input: string; output: string; }
};

export type AccessKey = {
  __typename?: 'AccessKey';
  id: Scalars['ID']['output'];
  name: Scalars['String']['output'];
  revokedAt?: Maybe<Scalars['DateTime']['output']>;
  role: ProjectRole;
};

export type Destination = {
  __typename?: 'Destination';
  config: Scalars['JSON']['output'];
  id: Scalars['ID']['output'];
  kind: Scalars['String']['output'];
  name: Scalars['String']['output'];
};

export type Funnel = {
  __typename?: 'Funnel';
  definition: Scalars['JSON']['output'];
  id: Scalars['ID']['output'];
  key: Scalars['String']['output'];
  name: Scalars['String']['output'];
};

export type IssuedKey = {
  __typename?: 'IssuedKey';
  id: Scalars['ID']['output'];
  token: Scalars['String']['output'];
};

export type Mutation = {
  __typename?: 'Mutation';
  createProject: Project;
  deleteDestination: Scalars['Boolean']['output'];
  deleteFunnel: Scalars['Boolean']['output'];
  eraseActor: Scalars['Boolean']['output'];
  issueAccessKey: IssuedKey;
  issueWriteKey: IssuedKey;
  refreshAnalytics: Scalars['Boolean']['output'];
  revokeAccessKey: Scalars['Boolean']['output'];
  revokeWriteKey: Scalars['Boolean']['output'];
  saveFunnel: Funnel;
  saveMixpanelDestination: Destination;
  updateSettings: Project;
};


export type MutationCreateProjectArgs = {
  name: Scalars['String']['input'];
  slug: Scalars['String']['input'];
};


export type MutationDeleteDestinationArgs = {
  id: Scalars['UUID']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationDeleteFunnelArgs = {
  key: Scalars['String']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationEraseActorArgs = {
  externalId: Scalars['String']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationIssueAccessKeyArgs = {
  name: Scalars['String']['input'];
  projectId: Scalars['UUID']['input'];
  role: ProjectRole;
};


export type MutationIssueWriteKeyArgs = {
  projectId: Scalars['UUID']['input'];
  source: Scalars['String']['input'];
};


export type MutationRefreshAnalyticsArgs = {
  projectId: Scalars['UUID']['input'];
};


export type MutationRevokeAccessKeyArgs = {
  id: Scalars['UUID']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationRevokeWriteKeyArgs = {
  id: Scalars['UUID']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationSaveFunnelArgs = {
  definition: Scalars['JSON']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationSaveMixpanelDestinationArgs = {
  config: Scalars['JSON']['input'];
  name: Scalars['String']['input'];
  projectId: Scalars['UUID']['input'];
};


export type MutationUpdateSettingsArgs = {
  projectId: Scalars['UUID']['input'];
  settings: Scalars['JSON']['input'];
};

/** Information about pagination in a connection */
export type PageInfo = {
  __typename?: 'PageInfo';
  /** When paginating forwards, the cursor to continue. */
  endCursor?: Maybe<Scalars['String']['output']>;
  /** When paginating forwards, are there more items? */
  hasNextPage: Scalars['Boolean']['output'];
  /** When paginating backwards, are there more items? */
  hasPreviousPage: Scalars['Boolean']['output'];
  /** When paginating backwards, the cursor to continue. */
  startCursor?: Maybe<Scalars['String']['output']>;
};

export type Project = {
  __typename?: 'Project';
  accessKeys: Array<AccessKey>;
  canAdmin: Scalars['Boolean']['output'];
  createdAt: Scalars['DateTime']['output'];
  destinations: Array<Destination>;
  funnels: Array<Funnel>;
  id: Scalars['ID']['output'];
  name: Scalars['String']['output'];
  reports: Array<Report>;
  settings: Scalars['JSON']['output'];
  slug: Scalars['String']['output'];
  writeKeys: Array<WriteKey>;
};

export type ProjectConnection = {
  __typename?: 'ProjectConnection';
  /** A list of edges. */
  edges: Array<ProjectEdge>;
  /** A list of nodes. */
  nodes: Array<Project>;
  /** Information to aid in pagination. */
  pageInfo: PageInfo;
};

/** An edge in a connection. */
export type ProjectEdge = {
  __typename?: 'ProjectEdge';
  /** A cursor for use in pagination */
  cursor: Scalars['String']['output'];
  /** The item at the end of the edge */
  node: Project;
};

export type ProjectRole =
  | 'ADMIN'
  | 'READER';

export type Query = {
  __typename?: 'Query';
  project: Project;
  projects: ProjectConnection;
  viewer: Viewer;
};


export type QueryProjectArgs = {
  id: Scalars['UUID']['input'];
};

export type Report = {
  __typename?: 'Report';
  computedAt: Scalars['DateTime']['output'];
  id: Scalars['ID']['output'];
  key: Scalars['String']['output'];
  result: Scalars['JSON']['output'];
};

export type Viewer = {
  __typename?: 'Viewer';
  canCreateProjects: Scalars['Boolean']['output'];
};

export type WriteKey = {
  __typename?: 'WriteKey';
  id: Scalars['ID']['output'];
  revokedAt?: Maybe<Scalars['DateTime']['output']>;
  source: Scalars['String']['output'];
};

export type SaveFunnelMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  definition: Scalars['JSON']['input'];
}>;


export type SaveFunnelMutation = { __typename?: 'Mutation', saveFunnel: { __typename?: 'Funnel', id: string, key: string, name: string, definition: unknown } };

export type DeleteFunnelMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  key: Scalars['String']['input'];
}>;


export type DeleteFunnelMutation = { __typename?: 'Mutation', deleteFunnel: boolean };

export type AnalyticsQueryVariables = Exact<{
  id: Scalars['UUID']['input'];
}>;


export type AnalyticsQuery = { __typename?: 'Query', project: { __typename?: 'Project', id: string, slug: string, name: string, canAdmin: boolean, settings: unknown, reports: Array<{ __typename?: 'Report', id: string, key: string, result: unknown, computedAt: string }>, funnels: Array<{ __typename?: 'Funnel', id: string, key: string, name: string, definition: unknown }> } };

export type RefreshAnalyticsMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
}>;


export type RefreshAnalyticsMutation = { __typename?: 'Mutation', refreshAnalytics: boolean };

export type DashboardAccessQueryVariables = Exact<{ [key: string]: never; }>;


export type DashboardAccessQuery = { __typename?: 'Query', viewer: { __typename?: 'Viewer', canCreateProjects: boolean } };

export type ProjectsQueryVariables = Exact<{ [key: string]: never; }>;


export type ProjectsQuery = { __typename?: 'Query', projects: { __typename?: 'ProjectConnection', edges: Array<{ __typename?: 'ProjectEdge', cursor: string, node: { __typename?: 'Project', id: string, slug: string, name: string, canAdmin: boolean } }>, pageInfo: { __typename?: 'PageInfo', startCursor?: string | null, endCursor?: string | null, hasPreviousPage: boolean, hasNextPage: boolean } } };

export type CreateProjectMutationVariables = Exact<{
  slug: Scalars['String']['input'];
  name: Scalars['String']['input'];
}>;


export type CreateProjectMutation = { __typename?: 'Mutation', createProject: { __typename?: 'Project', id: string, slug: string, name: string, canAdmin: boolean } };

export type ProjectAdminQueryVariables = Exact<{
  id: Scalars['UUID']['input'];
}>;


export type ProjectAdminQuery = { __typename?: 'Query', project: { __typename?: 'Project', id: string, writeKeys: Array<{ __typename?: 'WriteKey', id: string, source: string, revokedAt?: string | null }>, accessKeys: Array<{ __typename?: 'AccessKey', id: string, name: string, role: ProjectRole, revokedAt?: string | null }>, destinations: Array<{ __typename?: 'Destination', id: string, name: string, kind: string, config: unknown }> } };

export type IssueWriteKeyMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  source: Scalars['String']['input'];
}>;


export type IssueWriteKeyMutation = { __typename?: 'Mutation', issueWriteKey: { __typename?: 'IssuedKey', id: string, token: string } };

export type RevokeWriteKeyMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  id: Scalars['UUID']['input'];
}>;


export type RevokeWriteKeyMutation = { __typename?: 'Mutation', revokeWriteKey: boolean };

export type IssueAccessKeyMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  name: Scalars['String']['input'];
  role: ProjectRole;
}>;


export type IssueAccessKeyMutation = { __typename?: 'Mutation', issueAccessKey: { __typename?: 'IssuedKey', id: string, token: string } };

export type RevokeAccessKeyMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  id: Scalars['UUID']['input'];
}>;


export type RevokeAccessKeyMutation = { __typename?: 'Mutation', revokeAccessKey: boolean };

export type UpdateSettingsMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  settings: Scalars['JSON']['input'];
}>;


export type UpdateSettingsMutation = { __typename?: 'Mutation', updateSettings: { __typename?: 'Project', id: string, settings: unknown } };

export type EraseActorMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  externalId: Scalars['String']['input'];
}>;


export type EraseActorMutation = { __typename?: 'Mutation', eraseActor: boolean };

export type SaveDestinationMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  name: Scalars['String']['input'];
  config: Scalars['JSON']['input'];
}>;


export type SaveDestinationMutation = { __typename?: 'Mutation', saveMixpanelDestination: { __typename?: 'Destination', id: string, name: string, kind: string, config: unknown } };

export type DeleteDestinationMutationVariables = Exact<{
  projectId: Scalars['UUID']['input'];
  id: Scalars['UUID']['input'];
}>;


export type DeleteDestinationMutation = { __typename?: 'Mutation', deleteDestination: boolean };


export const SaveFunnelDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveFunnel"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"definition"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"JSON"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveFunnel"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"definition"},"value":{"kind":"Variable","name":{"kind":"Name","value":"definition"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"key"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"definition"}}]}}]}}]} as unknown as DocumentNode<SaveFunnelMutation, SaveFunnelMutationVariables>;
export const DeleteFunnelDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"DeleteFunnel"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"key"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"deleteFunnel"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"key"},"value":{"kind":"Variable","name":{"kind":"Name","value":"key"}}}]}]}}]} as unknown as DocumentNode<DeleteFunnelMutation, DeleteFunnelMutationVariables>;
export const AnalyticsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"Analytics"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"id"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"project"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"id"},"value":{"kind":"Variable","name":{"kind":"Name","value":"id"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"slug"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"canAdmin"}},{"kind":"Field","name":{"kind":"Name","value":"settings"}},{"kind":"Field","name":{"kind":"Name","value":"reports"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"key"}},{"kind":"Field","name":{"kind":"Name","value":"result"}},{"kind":"Field","name":{"kind":"Name","value":"computedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"funnels"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"key"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"definition"}}]}}]}}]}}]} as unknown as DocumentNode<AnalyticsQuery, AnalyticsQueryVariables>;
export const RefreshAnalyticsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"RefreshAnalytics"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"refreshAnalytics"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}}]}]}}]} as unknown as DocumentNode<RefreshAnalyticsMutation, RefreshAnalyticsMutationVariables>;
export const DashboardAccessDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"DashboardAccess"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"viewer"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"canCreateProjects"}}]}}]}}]} as unknown as DocumentNode<DashboardAccessQuery, DashboardAccessQueryVariables>;
export const ProjectsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"Projects"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"projects"},"directives":[{"kind":"Directive","name":{"kind":"Name","value":"connection"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"mode"},"value":{"kind":"StringValue","value":"page","block":false}}]}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"edges"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"node"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"slug"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"canAdmin"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"pageInfo"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"startCursor"}},{"kind":"Field","name":{"kind":"Name","value":"endCursor"}},{"kind":"Field","name":{"kind":"Name","value":"hasPreviousPage"}},{"kind":"Field","name":{"kind":"Name","value":"hasNextPage"}}]}}]}}]}}]} as unknown as DocumentNode<ProjectsQuery, ProjectsQueryVariables>;
export const CreateProjectDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"CreateProject"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"slug"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"name"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"createProject"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"slug"},"value":{"kind":"Variable","name":{"kind":"Name","value":"slug"}}},{"kind":"Argument","name":{"kind":"Name","value":"name"},"value":{"kind":"Variable","name":{"kind":"Name","value":"name"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"slug"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"canAdmin"}}]}}]}}]} as unknown as DocumentNode<CreateProjectMutation, CreateProjectMutationVariables>;
export const ProjectAdminDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ProjectAdmin"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"id"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"project"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"id"},"value":{"kind":"Variable","name":{"kind":"Name","value":"id"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"writeKeys"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"source"}},{"kind":"Field","name":{"kind":"Name","value":"revokedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"accessKeys"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"role"}},{"kind":"Field","name":{"kind":"Name","value":"revokedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"destinations"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"config"}}]}}]}}]}}]} as unknown as DocumentNode<ProjectAdminQuery, ProjectAdminQueryVariables>;
export const IssueWriteKeyDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"IssueWriteKey"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"source"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"issueWriteKey"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"source"},"value":{"kind":"Variable","name":{"kind":"Name","value":"source"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"token"}}]}}]}}]} as unknown as DocumentNode<IssueWriteKeyMutation, IssueWriteKeyMutationVariables>;
export const RevokeWriteKeyDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"RevokeWriteKey"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"id"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"revokeWriteKey"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"id"},"value":{"kind":"Variable","name":{"kind":"Name","value":"id"}}}]}]}}]} as unknown as DocumentNode<RevokeWriteKeyMutation, RevokeWriteKeyMutationVariables>;
export const IssueAccessKeyDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"IssueAccessKey"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"name"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"role"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"ProjectRole"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"issueAccessKey"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"name"},"value":{"kind":"Variable","name":{"kind":"Name","value":"name"}}},{"kind":"Argument","name":{"kind":"Name","value":"role"},"value":{"kind":"Variable","name":{"kind":"Name","value":"role"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"token"}}]}}]}}]} as unknown as DocumentNode<IssueAccessKeyMutation, IssueAccessKeyMutationVariables>;
export const RevokeAccessKeyDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"RevokeAccessKey"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"id"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"revokeAccessKey"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"id"},"value":{"kind":"Variable","name":{"kind":"Name","value":"id"}}}]}]}}]} as unknown as DocumentNode<RevokeAccessKeyMutation, RevokeAccessKeyMutationVariables>;
export const UpdateSettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"UpdateSettings"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"settings"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"JSON"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"updateSettings"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"settings"},"value":{"kind":"Variable","name":{"kind":"Name","value":"settings"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"settings"}}]}}]}}]} as unknown as DocumentNode<UpdateSettingsMutation, UpdateSettingsMutationVariables>;
export const EraseActorDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"EraseActor"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"externalId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"eraseActor"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"externalId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"externalId"}}}]}]}}]} as unknown as DocumentNode<EraseActorMutation, EraseActorMutationVariables>;
export const SaveDestinationDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveDestination"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"name"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"config"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"JSON"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveMixpanelDestination"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"name"},"value":{"kind":"Variable","name":{"kind":"Name","value":"name"}}},{"kind":"Argument","name":{"kind":"Name","value":"config"},"value":{"kind":"Variable","name":{"kind":"Name","value":"config"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"config"}}]}}]}}]} as unknown as DocumentNode<SaveDestinationMutation, SaveDestinationMutationVariables>;
export const DeleteDestinationDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"DeleteDestination"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"id"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"UUID"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"deleteDestination"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"projectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"projectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"id"},"value":{"kind":"Variable","name":{"kind":"Name","value":"id"}}}]}]}}]} as unknown as DocumentNode<DeleteDestinationMutation, DeleteDestinationMutationVariables>;