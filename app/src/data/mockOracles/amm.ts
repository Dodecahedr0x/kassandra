// --- mock v0.4 Amm pools for the challenged market (raw bytes at the offsets) -
// These are REAL Amm byte blobs (disc + fields at the pinned offsets from
// `ammV04.ts`), decoded back through `decodeAmmV04` so the panel exercises the
// genuine decoder path offline. Slots: created@1000, start_delay 150, last@2150
// ⇒ 1000 accumulating slots. Aggregator = twap * slots (twap PRICE_SCALE-scaled,
// 1e12). Reserves: 9-dec base (KASS), 6-dec quote (USDC).
//   pass: twap ≈ 1.000 · reserves 1000 KASS / 1000 USDC ⇒ spot 1.000
//   fail: twap ≈ 1.090 · reserves 1000 KASS / 1090 USDC ⇒ spot 1.090
// margin 1/10 ⇒ disqualify when fail > pass*1.1; progress = (fail-pass)*10/pass
// = (0.09)*10/1 = 0.90 ⇒ NEAR the margin (the single ember accent lights up),
// not yet over (would need fail > 1.10).
import {
  AMM_ACCOUNT_DISCRIMINATOR,
  AMM_AGGREGATOR_OFFSET,
  AMM_BASE_AMOUNT_OFFSET,
  AMM_BASE_DECIMALS_OFFSET,
  AMM_BASE_MINT_OFFSET,
  AMM_CREATED_AT_SLOT_OFFSET,
  AMM_LAST_UPDATED_SLOT_OFFSET,
  AMM_MIN_LEN,
  AMM_QUOTE_AMOUNT_OFFSET,
  AMM_QUOTE_DECIMALS_OFFSET,
  AMM_QUOTE_MINT_OFFSET,
  AMM_START_DELAY_SLOTS_OFFSET,
  decodeAmmV04,
  type MarketAmms,
} from '../ammV04'

const PRICE_SCALE_MOCK = 1_000_000_000_000n
const AMM_SLOTS = 1000n

function seedBytes(seed: number): Uint8Array {
  return Uint8Array.from({ length: 32 }, (_, i) => (seed * 17 + i * 5) & 0xff)
}

/** Encode a v0.4 `Amm` byte blob at the pinned offsets (for offline preview). */
function encodeMockAmm(opts: {
  twap: bigint // PRICE_SCALE-scaled time-weighted price
  baseAmount: bigint
  quoteAmount: bigint
  baseSeed: number
  quoteSeed: number
}): Uint8Array {
  const buf = new Uint8Array(AMM_MIN_LEN + 8) // + seq_num tail, like the real account
  buf.set(AMM_ACCOUNT_DISCRIMINATOR, 0)
  const dv = new DataView(buf.buffer)
  dv.setBigUint64(AMM_CREATED_AT_SLOT_OFFSET, 1000n, true)
  buf.set(seedBytes(opts.baseSeed), AMM_BASE_MINT_OFFSET)
  buf.set(seedBytes(opts.quoteSeed), AMM_QUOTE_MINT_OFFSET)
  buf[AMM_BASE_DECIMALS_OFFSET] = 9
  buf[AMM_QUOTE_DECIMALS_OFFSET] = 6
  dv.setBigUint64(AMM_BASE_AMOUNT_OFFSET, opts.baseAmount, true)
  dv.setBigUint64(AMM_QUOTE_AMOUNT_OFFSET, opts.quoteAmount, true)
  dv.setBigUint64(AMM_LAST_UPDATED_SLOT_OFFSET, 1000n + 150n + AMM_SLOTS, true)
  // aggregator (u128) = twap * accumulating slots
  const agg = opts.twap * AMM_SLOTS
  dv.setBigUint64(AMM_AGGREGATOR_OFFSET, agg & 0xffffffffffffffffn, true)
  dv.setBigUint64(AMM_AGGREGATOR_OFFSET + 8, agg >> 64n, true)
  dv.setBigUint64(AMM_START_DELAY_SLOTS_OFFSET, 150n, true)
  return buf
}

const MOCK_PASS_AMM = encodeMockAmm({
  twap: PRICE_SCALE_MOCK, // 1.000
  baseAmount: 1_000_000_000_000n, // 1000 KASS (9 dec)
  quoteAmount: 1_000_000_000n, // 1000 USDC (6 dec)
  baseSeed: 91,
  quoteSeed: 92,
})
const MOCK_FAIL_AMM = encodeMockAmm({
  twap: (PRICE_SCALE_MOCK * 1090n) / 1000n, // 1.090
  baseAmount: 1_000_000_000_000n, // 1000 KASS
  quoteAmount: 1_090_000_000n, // 1090 USDC
  baseSeed: 93,
  quoteSeed: 94,
})

/** The decoded pass/fail pools for the mock challenged market. */
export function mockMarketAmms(): MarketAmms {
  return { pass: decodeAmmV04(MOCK_PASS_AMM), fail: decodeAmmV04(MOCK_FAIL_AMM) }
}
