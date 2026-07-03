/**
 * Read + patch on-chain accounts from the FORKED surfpool (port 8940) in the
 * Playwright test process — asserting each browser write by its persistent
 * on-chain effect, and fabricating the few balances/timestamps the forked
 * compose→settle flow needs (extra conditional tokens to swap; a past
 * `Market.twap_end` so the settle gate opens without waiting the TWAP window).
 */
import { decodeMarket } from '@kassandra/sdk'
import { tokenAccountAmount, tokenAccountBytes } from '../../../sdk/test/surfpool/harness.ts'

const RPC = 'http://127.0.0.1:8940'
const PROGRAM = 'KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY'
const TOKEN_PROGRAM = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA'

async function rpc<T>(method: string, params: unknown[]): Promise<T> {
  const res = await fetch(RPC, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  })
  return ((await res.json()) as { result: T }).result
}

export async function getAccountData(address: string): Promise<Uint8Array | null> {
  const json = await rpc<{ value?: { data?: [string, string] } | null }>('getAccountInfo', [
    address,
    { encoding: 'base64', commitment: 'confirmed' },
  ])
  const value = json?.value
  if (!value || !value.data) return null
  return Uint8Array.from(Buffer.from(value.data[0], 'base64'))
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

export async function poll<T>(
  read: () => Promise<T>,
  pred: (v: T) => boolean,
  timeoutMs = 30_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs
  let last: T | undefined
  while (Date.now() < deadline) {
    try {
      last = await read()
      if (pred(last)) return last
    } catch {
      // The account may not exist yet (a compose step still in flight) — keep polling.
    }
    await sleep(300)
  }
  throw new Error(
    `on-chain poll timed out; last = ${JSON.stringify(last, (_k, v) => (typeof v === 'bigint' ? v.toString() : v))}`,
  )
}

export async function marketAt(address: string): Promise<ReturnType<typeof decodeMarket>> {
  const data = await getAccountData(address)
  if (!data) throw new Error(`market ${address} not found`)
  return decodeMarket(data)
}

export async function tokenBalance(address: string): Promise<bigint> {
  const data = await getAccountData(address)
  if (!data) return 0n
  return tokenAccountAmount(data)
}

async function setAccountRaw(address: string, data: Uint8Array, owner: string): Promise<void> {
  await rpc('surfnet_setAccount', [
    address,
    { lamports: 5_000_000, owner, executable: false, data: Buffer.from(data).toString('hex') },
  ])
}

/** Fabricate a token account at `address` holding `amount` of `mint` for `owner`. */
export async function fabricateTokenAccount(
  address: string,
  mintBytesB58: Uint8Array,
  ownerBytes: Uint8Array,
  amount: bigint,
): Promise<void> {
  await setAccountRaw(address, tokenAccountBytes(mintBytesB58, ownerBytes, amount), TOKEN_PROGRAM)
}

/** Decode a v0.4 AMM's spot TWAP (created_at@9, last_updated@131, aggregator@171). */
export async function ammTwap(amm: string): Promise<bigint> {
  const data = await getAccountData(amm)
  if (!data) return 0n
  const dv = new DataView(data.buffer, data.byteOffset, data.length)
  const u128 = (off: number): bigint =>
    dv.getBigUint64(off, true) | (dv.getBigUint64(off + 8, true) << 64n)
  const createdAt = dv.getBigUint64(9, true)
  const lastUpdated = dv.getBigUint64(131, true)
  const aggregator = u128(171)
  const startDelay = dv.getBigUint64(219, true)
  const slots = lastUpdated - (createdAt + startDelay)
  return slots > 0n && aggregator > 0n ? aggregator / slots : 0n
}

/** Rewind `Market.twap_end` (i64 @ 392) to the past so settle opens immediately. */
export async function backdateMarketTwapEnd(market: string): Promise<void> {
  const data = await getAccountData(market)
  if (!data) throw new Error('market not found')
  const d = Uint8Array.from(data)
  const past = BigInt(Math.floor(Date.now() / 1000) - 3600)
  new DataView(d.buffer).setBigInt64(392, past, true)
  await setAccountRaw(market, d, PROGRAM)
}

/** Rewind the oracle's `phase_ends_at` (i64 @ 144) so the Challenge window has elapsed. */
export async function backdateOraclePhaseEnd(oracle: string): Promise<void> {
  const data = await getAccountData(oracle)
  if (!data) throw new Error('oracle not found')
  const d = Uint8Array.from(data)
  const past = BigInt(Math.floor(Date.now() / 1000) - 3600)
  new DataView(d.buffer).setBigInt64(144, past, true)
  await setAccountRaw(oracle, d, PROGRAM)
}

/** The oracle's phase discriminant byte (@161): 6=Challenge, 7=Resolved, 8=InvalidDeadend. */
export async function oraclePhaseByte(oracle: string): Promise<number | null> {
  const data = await getAccountData(oracle)
  return data ? data[161] : null
}
