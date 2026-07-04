import { defineConfig, devices } from '@playwright/test'

/**
 * Browser E2E for the INDEXER integration.
 *
 * `globalSetup` boots surfpool (:8960), seeds an oracle with real transactions,
 * then runs the actual `kassandra-indexer` binary against surfpool + an ephemeral
 * Postgres. `webServer` starts a Vite dev server (:5175) with `VITE_INDEXER_URL`
 * pointed at the indexer, so the app renders its on-chain ActivityFeed from real
 * indexed data.
 *
 * Run via `scripts/e2e-playwright-indexer.sh` (needs the indexer binary built +
 * postgres available). Separate config/ports so it never collides with the
 * default (:8899/:5173) or forked (:8940/:5174) projects.
 */
export default defineConfig({
  testDir: './e2e/indexer',
  timeout: 120_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  globalSetup: './e2e/indexer/global-setup.ts',
  use: {
    baseURL: 'http://localhost:5175',
    headless: true,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm exec vite --port 5175 --strictPort',
    url: 'http://localhost:5175',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      VITE_RPC_URL: 'http://127.0.0.1:8960',
      VITE_INDEXER_URL: 'http://127.0.0.1:3111',
      VITE_E2E: '1',
    },
  },
})
