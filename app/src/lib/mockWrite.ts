/**
 * Render-only wallet + connection mocks for WF2 (mock mode ONLY — never in the
 * live path). A real browser wallet can't be automated headlessly, so to
 * exercise the write-form STATES (connected-idle / signing / confirming /
 * success / error) the {@link MockWalletProvider} provides a fixed `publicKey` +
 * a scripted `sendTransaction`, and {@link useWriteAction} runs against a
 * {@link mockWriteConnection} so no RPC is touched.
 *
 * Everything here is gated behind {@link isMockMode} at the provider swap in
 * `AppProviders`, exactly like `mockOracles` — it does not pollute production.
 *
 * Query params (only read under `?mock`):
 *   - `wallet=connected`  → the mock wallet reports connected with a fake key.
 *   - `tx=success` (default) | `error` | `reject` | `failconfirm` | `slow`
 *       drive the `sendTransaction`/confirm outcome for the render harness.
 */
import type { WalletContextState } from '@solana/wallet-adapter-react'
import type { Connection, Transaction } from '@solana/web3.js'

/** A stable, obviously-fake base58 key for the mock connected wallet. */
export const MOCK_PUBKEY = 'MockWa11etAdaL0ve1ace111111111111111111111111'

/** A minimal `PublicKey` stand-in — the forms only ever call `.toBase58()`/`.toString()`. */
export const mockPublicKey = {
  toBase58: () => MOCK_PUBKEY,
  toString: () => MOCK_PUBKEY,
} as unknown as WalletContextState['publicKey']

function param(name: string): string | null {
  if (typeof window === 'undefined') return null
  return new URLSearchParams(window.location.search).get(name)
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** The scripted `tx` outcome for the render harness (default `success`). */
function txMode(): string {
  return param('tx') ?? 'success'
}

/** Whether the mock wallet should report connected (`?wallet=connected`). */
export function mockWalletConnected(): boolean {
  return param('wallet') === 'connected'
}

/**
 * A mock `Connection` for {@link useWriteAction} under mock mode: the ATA always
 * reports absent (so the create-ATA path is exercised) and the signature
 * confirms immediately unless `?tx=failconfirm`.
 */
export function mockWriteConnection(): Connection {
  return {
    getAccountInfo: async () => null,
    getSignatureStatuses: async () => ({
      value: [
        txMode() === 'failconfirm'
          ? { err: { InstructionError: [0, { Custom: 1 }] }, confirmationStatus: 'confirmed' }
          : { err: null, confirmationStatus: 'confirmed' },
      ],
    }),
    getTokenAccountBalance: async () => ({ value: { amount: '250000000000', decimals: 9 } }),
  } as unknown as Connection
}

/** A wallet error carrying a `code`, mirroring what Phantom/Solflare throw. */
class MockWalletError extends Error {
  code?: number
  logs?: string[]
  constructor(name: string, message: string, extra?: { code?: number; logs?: string[] }) {
    super(message)
    this.name = name
    this.code = extra?.code
    this.logs = extra?.logs
  }
}

/** The scripted `sendTransaction` — resolves a fake signature or throws per `?tx=`. */
export const mockSendTransaction = (async (_tx: Transaction) => {
  await sleep(txMode() === 'slow' ? 900 : 120) // let "Signing…" be observable
  switch (txMode()) {
    case 'reject':
      throw new MockWalletError('WalletSignTransactionError', 'User rejected the request.', {
        code: 4001,
      })
    case 'error':
      throw new MockWalletError('SendTransactionError', 'Simulation failed', {
        logs: [
          'Program KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY invoke [1]',
          'Program log: Error: WrongPhase',
          'Program KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY failed: custom program error: 0x1',
        ],
      })
    default:
      return 'mockSig1111111111111111111111111111111111111111111111111111111111'
  }
}) as unknown as WalletContextState['sendTransaction']
