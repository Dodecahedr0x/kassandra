/**
 * Enable mock mode. `VITE_MOCK=1` at build time forces it; otherwise a `?mock`
 * query param flips a live build into fixtures for offline preview.
 */
export function isMockMode(): boolean {
  if (import.meta.env.VITE_MOCK === '1') return true
  if (typeof window !== 'undefined') {
    return new URLSearchParams(window.location.search).has('mock')
  }
  return false
}

/**
 * E2E mode (`VITE_E2E=1` or `?e2e`): swap in the REAL-SIGNING e2e wallet
 * (`lib/e2eWallet`) driven by a Playwright-injected funded keypair, against the
 * LIVE cluster connection. Distinct from mock mode — nothing is faked; the write
 * path signs + sends + confirms on the local validator.
 */
export function isE2eMode(): boolean {
  if (import.meta.env.VITE_E2E === '1') return true
  if (typeof window !== 'undefined') {
    return new URLSearchParams(window.location.search).has('e2e')
  }
  return false
}
