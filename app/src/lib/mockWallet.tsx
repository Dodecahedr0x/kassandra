/**
 * The render-only mock wallet provider (mock mode ONLY). Swapped in for the real
 * `WalletProvider` in `AppProviders` under `?mock` so `useWallet()` reports a
 * scripted connected wallet, letting the WF2 write-form states be headless
 * reviewed without an automatable browser wallet. See `lib/mockWrite` for the
 * connection + `sendTransaction` scripting.
 */
import { useMemo, type ReactNode } from 'react'
import { WalletContext, type WalletContextState } from '@solana/wallet-adapter-react'
import { mockPublicKey, mockSendTransaction, mockWalletConnected } from './mockWrite'

/**
 * Provide a fixed {@link WalletContextState} so `useWallet()` reports a
 * connected mock wallet (when `?wallet=connected`) with a scripted
 * `sendTransaction`.
 */
export function MockWalletProvider({ children }: { children: ReactNode }) {
  const connected = mockWalletConnected()
  const value = useMemo<WalletContextState>(
    () =>
      ({
        autoConnect: false,
        wallets: [],
        wallet: null,
        publicKey: connected ? mockPublicKey : null,
        connecting: false,
        connected,
        disconnecting: false,
        select: () => {},
        connect: async () => {},
        disconnect: async () => {},
        sendTransaction: mockSendTransaction,
        signTransaction: undefined,
        signAllTransactions: undefined,
        signMessage: undefined,
        signIn: undefined,
      }) as WalletContextState,
    [connected],
  )
  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>
}

export default MockWalletProvider
