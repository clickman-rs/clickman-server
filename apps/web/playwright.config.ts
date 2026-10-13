import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  use: { baseURL: process.env.CLICKMAN_TEST_URL ?? 'http://127.0.0.1:44130', viewport: { width: 1440, height: 1000 }, ...(process.env.PLAYWRIGHT_CHROME ? { channel: 'chrome' } : {}) },
  reporter: 'list',
});
