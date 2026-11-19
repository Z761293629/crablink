import { defineConfig, devices } from '@playwright/test';

// End-to-end tests against the real crablink-server binary serving web/dist, on an
// embedded Postgres (crates/server/examples/e2e.rs). Build the web app first: `pnpm build`.
const port = 8787;
export const baseURL = `http://localhost:${port}`;
export const admin = { email: 'admin@example.com', password: 'e2e admin password' };

export default defineConfig({
  testDir: 'e2e',
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL,
    locale: 'en-US',
    timezoneId: 'UTC',
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command:
      'cargo build -p crablink-server --bin crablink-server && cargo run -p crablink-server --example e2e',
    url: `${baseURL}/health`,
    // A cold Rust build takes minutes.
    timeout: 600_000,
    reuseExistingServer: !process.env.CI,
    // SIGTERM lets the harness stop its Postgres; the default SIGKILL would leak it.
    gracefulShutdown: { signal: 'SIGTERM', timeout: 10_000 },
    env: {
      CRABLINK_BASE_URL: baseURL,
      CRABLINK_LISTEN: `127.0.0.1:${port}`,
      CRABLINK_E2E_ADMIN_EMAIL: admin.email,
      CRABLINK_E2E_ADMIN_PASSWORD: admin.password,
    },
  },
});
