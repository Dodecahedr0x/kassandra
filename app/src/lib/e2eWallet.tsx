/**
 * The REAL-SIGNING e2e wallet provider (e2e mode ONLY — `VITE_E2E=1`).
 *
 * Unlike {@link MockWalletProvider} (render-only: a fake key + scripted
 * `sendTransaction` that never touches RPC), this provider drives a REAL funded
 * `Keypair` against the live cluster connection: it signs the transaction the
 * UI builds and submits it via the app's own `Connection`. The keypair's secret
 * is injected into the page by the Playwright harness (which generated + funded
 * it on the local validator) as `window.__E2E_WALLET_SECRET__` (a 64-byte
 * array), so the browser e2e exercises the exact WF2 write path a real wallet
 * would — sign + send + confirm — with no automatable browser extension.
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { WalletContext, type WalletContextState } from '@solana/wallet-adapter-react'
import { Keypair } from '@solana/web3.js'

/** The global the Playwright harness sets (via `addInitScript`) before app JS runs. */
declare global {
  interface Window {
    __E2E_WALLET_SECRET__?: number[]
  }
}

/**
 * Reconstruct the injected funded keypair (async in this web3.js flavor) and
 * expose it as a connected wallet whose `sendTransaction` signs for real and
 * submits over the app's live `Connection` — mirroring `keypairSender`.
 */
export function E2eWalletProvider({ children }: { children: ReactNode }) {
  const [keypair, setKeypair] = useState<Keypair | null>(null)

  useEffect(() => {
    // The funded keypair comes from either the Playwright-injected global or,
    // for interactive `WALLET=funded make dev`, a build-time env var the
    // orchestrator sets (avoids needing an HTML-injection plugin).
    const envSecret = (import.meta.env.VITE_E2E_WALLET_SECRET as string | undefined)?.trim()
    const secret =
      (typeof window !== 'undefined' ? window.__E2E_WALLET_SECRET__ : undefined) ??
      (envSecret ? (JSON.parse(envSecret) as number[]) : undefined)
    if (!secret || secret.length === 0) return
    let cancelled = false
    void Keypair.fromSecretKey(new Uint8Array(secret)).then((kp) => {
      if (!cancelled) setKeypair(kp)
    })
    return () => {
      cancelled = true
    }
  }, [])

  const value = useMemo<WalletContextState>(() => {
    const connected = keypair !== null
    return {
      autoConnect: false,
      wallets: [],
      wallet: null,
      publicKey: (keypair ? keypair.publicKey : null) as WalletContextState['publicKey'],
      connecting: false,
      connected,
      disconnecting: false,
      select: () => {},
      connect: async () => {},
      disconnect: async () => {},
      // The UI calls `sendTransaction(new Transaction().add(...ixs), connection)`
      // and relies on the wallet to fill the fee payer + blockhash and sign —
      // exactly what `keypairSender` does, but on the wallet-supplied tx.
      sendTransaction: (async (tx: any, connection: any) => {
        if (!keypair) throw new Error('e2e wallet not ready')
        tx.feePayer = keypair.publicKey
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash
        await tx.sign(keypair)
        return connection.sendRawTransaction(await tx.serialize(), { skipPreflight: false })
      }) as unknown as WalletContextState['sendTransaction'],
      signTransaction: (async (tx: any) => {
        if (!keypair) throw new Error('e2e wallet not ready')
        await tx.sign(keypair)
        return tx
      }) as unknown as WalletContextState['signTransaction'],
      signAllTransactions: (async (txs: any[]) => {
        if (!keypair) throw new Error('e2e wallet not ready')
        for (const tx of txs) await tx.sign(keypair)
        return txs
      }) as unknown as WalletContextState['signAllTransactions'],
      signMessage: undefined,
      signIn: undefined,
    } as WalletContextState
  }, [keypair])

  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>
}

export default E2eWalletProvider
