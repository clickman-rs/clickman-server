import type { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
  schema: ['../../schema/schema.graphql', 'src/lib/graphql/cachebay.graphql'],
  documents: 'src/lib/**/*.operations.graphql',
  generates: {
    'src/lib/graphql/generated.ts': {
      plugins: ['typescript', 'typescript-operations', 'typed-document-node'],
      config: {
        useTypeImports: true,
        enumsAsTypes: true,
        scalars: { UUID: 'string', DateTime: 'string', JSON: 'unknown' },
      },
    },
  },
};
export default config;
