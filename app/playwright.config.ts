import { defineConfig, devices } from '@playwright/test'

/**
 * Browser E2E for the Kassandra dApp against a real local validator.
 *
 * `globalSetup` boots surfpool (+ deploys the program, inits the protocol, mints
 * KASS/USDC, funds the wallet keypair, and seeds oracles) and writes the funded
 * keypair to `e2e/.wallet.json`; the specs inject it so the real-signing e2e
 * wallet (`VITE_E2E=1`) signs + sends on the local validator. `webServer` starts
 * the Vite dev server pointed at surfpool (:8899) in e2e mode.
 *
 * Run via `scripts/e2e-playwright.sh` (ensures the .so + SDK + browser are built).
 */
export default defineConfig({
  testDir: './e2e',
  // The forked challenge-market project has its own config (playwright.fork.config.ts) —
  // it boots a mainnet-forking surfpool on a different port; keep it out of this run.
  testIgnore: '**/fork/**',
  timeout: 120_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  globalSetup: './e2e/global-setup.ts',
  use: {
    baseURL: 'http://localhost:5173',
    headless: true,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm exec vite --port 5173 --strictPort',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      VITE_RPC_URL: 'http://127.0.0.1:8899',
      VITE_E2E: '1',
    },
  },
})
