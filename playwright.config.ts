import { defineConfig } from '@playwright/test';

// Renderer checks use synthetic IPC only. Native Tauri acceptance runs separately.
export default defineConfig({
  testDir: './tests/renderer',
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  workers: 2,
  reporter: 'list',
  use: { baseURL: 'http://127.0.0.1:1420', trace: 'retain-on-failure' },
  projects: [
    { name: 'chromium-renderer', use: { browserName: 'chromium' } },
    { name: 'webkit-renderer', use: { browserName: 'webkit' } },
  ],
  webServer: {
    command: 'npm run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
