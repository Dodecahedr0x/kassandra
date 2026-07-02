import type { Cluster } from './cluster'

/**
 * A Solana Explorer URL for a confirmed signature — or `null` on localnet
 * (nothing public to link to). Devnet gets `?cluster=devnet`; mainnet needs no
 * cluster param.
 */
export function explorerTxUrl(cluster: Cluster, signature: string): string | null {
  const base = `https://explorer.solana.com/tx/${signature}`
  switch (cluster) {
    case 'localnet':
      return null
    case 'devnet':
      return `${base}?cluster=devnet`
    case 'mainnet-beta':
      return base
  }
}

/** `Abc1…Xy9z` short form of a signature/address for confirmation lines. */
export function shortSig(signature: string, head = 4, tail = 4): string {
  if (signature.length <= head + tail + 1) return signature
  return `${signature.slice(0, head)}…${signature.slice(-tail)}`
}
