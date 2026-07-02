/**
 * WF2 — the React seam over the pure write-action state machine.
 *
 * {@link useWriteAction} wires wallet-adapter (`useWallet`) + the RPC
 * `Connection` into {@link runWriteAction}: it exposes the current
 * {@link WriteStatus}, a `run(build)` that drives one wallet-signed write, the
 * `connection`/`publicKey`/`connected` the forms need to assemble their WF1
 * `build*Ixs` args, and a `reset`.
 *
 * The wallet-backed {@link TxSender} is `(ixs) => sendTransaction(new
 * Transaction().add(...ixs), connection)` — wallet-adapter fills in the fee
 * payer + recent blockhash and signs.
 *
 * Under mock mode (`?mock`) the connection is swapped for a
 * {@link mockWriteConnection} and the ix-build is skipped, so the render harness
 * can drive every status without touching an RPC (see `lib/mockWallet`).
 */
import { useCallback, useMemo, useState } from 'react'
import { Transaction, type Connection, type TransactionInstruction } from '@solana/web3.js'
import { useWallet } from '@solana/wallet-adapter-react'
import { useConnection } from '../lib/cluster'
import { isMockMode } from '../data/mockOracles'
import { mockWriteConnection } from '../lib/mockWrite'
import type { TxSender } from '../data/send'
import { isBusy, runWriteAction, type WriteStatus } from '../data/writeAction'

export interface WriteAction {
  /** The current lifecycle status of the last-started write. */
  status: WriteStatus
  /** True while building/signing/confirming. */
  busy: boolean
  /** The connected wallet's base58 address, or `null` when disconnected. */
  address: string | null
  /** Whether a wallet is connected (the forms gate on this). */
  connected: boolean
  /** The RPC connection the forms pass to their `build*Ixs` call. */
  connection: Connection
  /** Drive one wallet-signed write from an ix-builder; no-op if already busy. */
  run: (build: () => Promise<TransactionInstruction[]>) => Promise<void>
  /** Reset back to `idle` (e.g. after a success line is dismissed). */
  reset: () => void
}

/**
 * @param onSuccess called with the confirmed signature (the form refetches the
 *   oracle so the new proposer/fact/vote appears).
 */
export function useWriteAction(onSuccess?: (signature: string) => void): WriteAction {
  const { connection: liveConnection } = useConnection()
  const { publicKey, connected, sendTransaction } = useWallet()
  const [status, setStatus] = useState<WriteStatus>({ kind: 'idle' })

  const mock = isMockMode()
  // A stable mock connection for the render harness (real one otherwise).
  const connection = useMemo(
    () => (mock ? mockWriteConnection() : liveConnection),
    [mock, liveConnection],
  )

  const walletSender = useMemo<TxSender | null>(() => {
    if (!connected || !publicKey) return null
    return async (ixs) => {
      const tx = new Transaction()
      for (const ix of ixs) tx.add(ix)
      return sendTransaction(tx, connection)
    }
  }, [connected, publicKey, sendTransaction, connection])

  const run = useCallback(
    async (build: () => Promise<TransactionInstruction[]>) => {
      if (isBusy(status)) return
      if (!walletSender) {
        setStatus({ kind: 'error', message: 'Connect a wallet to participate.' })
        return
      }
      await runWriteAction({
        // Under mock mode skip the real ix-build (fake mints aren't valid keys);
        // the mock wallet + connection drive the observable status transitions.
        build: mock ? async () => [] : build,
        connection,
        walletSender,
        setStatus,
        onSuccess,
      })
    },
    [status, walletSender, connection, mock, onSuccess],
  )

  const reset = useCallback(() => setStatus({ kind: 'idle' }), [])

  return {
    status,
    busy: isBusy(status),
    address: publicKey ? publicKey.toBase58() : null,
    connected,
    connection,
    run,
    reset,
  }
}
