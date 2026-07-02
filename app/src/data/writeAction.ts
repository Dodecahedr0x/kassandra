/**
 * WF2 — the write-action state machine (pure, NO React).
 *
 * Orchestrates a single wallet-signed write: build the instructions (WF1
 * `build*Ixs`), sign them via a wallet-backed {@link TxSender}, then confirm the
 * signature — surfacing every step as a {@link WriteStatus} the form renders:
 *
 *   idle → building → signing → confirming → success{signature}
 *                                          ↘ error{message, logs?}
 *
 * The `signing → confirming` split is observed by wrapping the sender: the
 * wallet promise resolving (the user approved + the tx was submitted) flips the
 * status to `confirming` before {@link sendAndConfirm} polls the signature.
 *
 * Errors are mapped to a human message: a {@link ValidationError} keeps its
 * field message, a wallet user-rejection collapses to "Transaction rejected in
 * wallet", and a {@link SendError} keeps its message + any program logs.
 *
 * This module is React-free so the transitions + error mapping are unit-tested
 * offline with a mock sender/connection (`test/writeAction.unit.test.ts`).
 */
import type { Connection, TransactionInstruction } from '@solana/web3.js'
import { ValidationError } from './actions'
import { SendError, sendAndConfirm, type TxSender } from './send'

/** The lifecycle of one wallet-signed write, as rendered by the forms. */
export type WriteStatus =
  | { kind: 'idle' }
  /** Assembling the instructions (WF1 `build*Ixs`, incl. the ATA-existence check). */
  | { kind: 'building' }
  /** Awaiting the wallet — the user is approving the transaction. */
  | { kind: 'signing' }
  /** Submitted; polling the signature to confirmation. */
  | { kind: 'confirming' }
  /** Confirmed — carries the base58 signature. */
  | { kind: 'success'; signature: string }
  /** Failed — a readable message plus any program logs pulled off the error. */
  | { kind: 'error'; message: string; logs?: string[] }

/** True while a write is in-flight (building/signing/confirming). */
export function isBusy(status: WriteStatus): boolean {
  return status.kind === 'building' || status.kind === 'signing' || status.kind === 'confirming'
}

/**
 * Best-effort detection of a wallet *user rejection* (Phantom/Solflare surface
 * `code: 4001` and/or a "User rejected the request." message; wallet-adapter
 * wraps them as `WalletSignTransactionError` / `WalletSendTransactionError`).
 * Looks through a {@link SendError}'s `cause`.
 */
export function isUserRejection(err: unknown): boolean {
  const cause = err instanceof SendError ? (err.cause ?? err) : err
  const e = cause as { code?: unknown; name?: unknown; message?: unknown } | null
  if (!e) return false
  if (e.code === 4001) return true
  const name = typeof e.name === 'string' ? e.name : ''
  const msg = (typeof e.message === 'string' ? e.message : String(cause ?? '')).toLowerCase()
  if (/reject|declined|denied|cancel/.test(msg)) return true
  // A bare wallet sign/send error with no logs is almost always a user dismissal.
  return name === 'WalletSignTransactionError' || name === 'WalletSendTransactionError'
}

/** Map any thrown error into the `error` status payload the form renders. */
export function mapWriteError(err: unknown): { message: string; logs?: string[] } {
  if (err instanceof ValidationError) return { message: err.message }
  if (isUserRejection(err)) return { message: 'Transaction rejected in wallet.' }
  if (err instanceof SendError) return { message: err.message, logs: err.logs }
  return { message: err instanceof Error ? err.message : String(err) }
}

export interface RunWriteActionOpts {
  /** Assemble the instructions (a WF1 `build*Ixs` call closing over its args). */
  build: () => Promise<TransactionInstruction[]>
  /** The RPC connection (ATA-existence check inside `build` + signature confirm). */
  connection: Connection
  /** The wallet-backed sender: sign + submit, resolve to the signature. */
  walletSender: TxSender
  /** Drives the form's status region through every transition. */
  setStatus: (status: WriteStatus) => void
  /** Invoked with the signature once confirmed (the form refetches the oracle). */
  onSuccess?: (signature: string) => void
}

/**
 * Run one wallet-signed write end to end, pushing each {@link WriteStatus}
 * through `setStatus`. Never throws — a failure lands as an `error` status.
 */
export async function runWriteAction(opts: RunWriteActionOpts): Promise<WriteStatus> {
  const { build, connection, walletSender, setStatus, onSuccess } = opts
  try {
    setStatus({ kind: 'building' })
    const ixs = await build()
    setStatus({ kind: 'signing' })
    // Wrap the sender so the wallet promise resolving flips us to `confirming`.
    const wrapped: TxSender = async (i) => {
      const signature = await walletSender(i)
      setStatus({ kind: 'confirming' })
      return signature
    }
    const { signature } = await sendAndConfirm(connection, wrapped, ixs)
    const success: WriteStatus = { kind: 'success', signature }
    setStatus(success)
    onSuccess?.(signature)
    return success
  } catch (err) {
    const failure: WriteStatus = { kind: 'error', ...mapWriteError(err) }
    setStatus(failure)
    return failure
  }
}
