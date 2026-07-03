import { defineConfig, devices } from '@playwright/test'

/**
 * Browser E2E for the FORKED challenge-market cluster.
 *
 * `globalSetup` boots surfpool FORKING MAINNET (executable MetaDAO programs) in
 * clock block-production mode on :8940, drives an oracle to the Challenge phase,
 * funds the challenger keypair, and writes it to `e2e/fork/.wallet.json`.
 * `webServer` starts a SECOND Vite dev server (:5174) pointed at the forked
 * validator so the real-signing e2e wallet composes + trades the market in-browser.
 *
 * Run via `scripts/e2e-playwright-fork.sh` (needs network for the mainnet fork).
 * Separate from the default config so the two surfpool instances never collide.
 */
export default defineConfig({
  testDir: './e2e/fork',
  timeout: 720_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  globalSetup: './e2e/fork/global-setup.ts',
  use: {
    baseURL: 'http://localhost:5174',
    headless: true,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm exec vite --port 5174 --strictPort',
    url: 'http://localhost:5174',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      VITE_RPC_URL: 'http://127.0.0.1:8940',
      VITE_E2E: '1',
    },
  },
})
