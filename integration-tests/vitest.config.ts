import { defineConfig } from 'vitest/config'

// https://vitest.dev/config/
export default defineConfig({

	test: {
		testTimeout: 100000,
		globalSetup: ['./src/setup/global-setup.ts'],
	},
})
