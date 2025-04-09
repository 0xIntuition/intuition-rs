
import { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
	schema: 'http://localhost:8080/v1/graphql',
	documents: ['src/**/*.ts'],
	ignoreNoDocuments: true,
	generates: {
		'./src/graphql/': {
			preset: 'client',
			config: {
				documentMode: 'string'
			}
		}
	}
};

export default config;
