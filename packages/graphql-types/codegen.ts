import type { CodegenConfig } from '@graphql-codegen/cli';
const config: CodegenConfig = {
  schema: ['../../schema/schema.graphql', 'cachebay.graphql'],
  documents: '../../apps/web/src/lib/operations.graphql',
  generates: { 'src/generated.ts': { plugins: ['typescript', 'typescript-operations', 'typed-document-node'], config: { useTypeImports: true, enumsAsTypes: true, scalars: { UUID: 'string', DateTime: 'string', JSON: 'unknown' } } } },
};
export default config;
