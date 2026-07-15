import { ValidationError } from "../../actions";

// ── seed / TWAP math (mirror challenge-market-e2e buildPool) ─────────────────

/** PRICE_SCALE — the v0.4 AMM's fixed-point scale for the TWAP observation. */
export const PRICE_SCALE = 1_000_000_000_000n;
/** `twap_max_observation_change_per_update` — `(2^64−1) · 1e12` (no clamp; a
 * single crank folds the current price into the TWAP verbatim, exactly as the
 * E2E's `MAX_PRICE`). */
export const MAX_OBSERVATION_CHANGE = ((1n << 64n) - 1n) * PRICE_SCALE;
/** Default base reserve: 100 conditional-KASS (9 dp) — the E2E's `BASE_RESERVE`. */
export const DEFAULT_BASE_RESERVE = 100_000_000_000n;
/** Default quote reserve: 100 conditional-USDC (6 dp) → seeded price 1e12-scaled 1.0 (the E2E's `QUOTE_NEUTRAL`). */
export const DEFAULT_QUOTE_RESERVE = 100_000_000n;

/** The default deterministic question id (mirrors the E2E's `fill(0x07)`). */
export const DEFAULT_QUESTION_ID = new Uint8Array(32).fill(0x07);

/**
 * The v0.4 `twap_initial_observation` for a pool seeded with `baseReserve` base
 * and `quoteReserve` quote — `quoteReserve · PRICE_SCALE / baseReserve` (the
 * scaled spot price the pool opens at). Mirrors `buildPool`'s `initialObs`.
 */
export function twapInitialObservation(baseReserve: bigint, quoteReserve: bigint): bigint {
  if (baseReserve <= 0n) throw new ValidationError("baseReserve", "baseReserve must be greater than zero.");
  return (quoteReserve * PRICE_SCALE) / baseReserve;
}
