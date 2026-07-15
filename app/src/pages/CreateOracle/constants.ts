export const selectClass =
  'w-full rounded-tag border border-pebble bg-pure-card px-3 py-2 font-inter text-[14px] ' +
  'text-sepia focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sepia/40 ' +
  'focus-visible:ring-offset-2 focus-visible:ring-offset-parchment'

export const textareaClass =
  'w-full rounded-tag border border-pebble bg-pure-card px-3 py-2 font-inter text-[14px] ' +
  'text-sepia placeholder:text-driftwood focus-visible:outline-none focus-visible:ring-2 ' +
  'focus-visible:ring-sepia/40 focus-visible:ring-offset-2 focus-visible:ring-offset-parchment ' +
  'aria-[invalid=true]:border-ember-orange/60'

// Valid-base58 KASS/USDC placeholders for the offline `?mock` render (no protocol
// on-chain) — chosen so the client-side address validation passes and the
// submitting/success states are drivable via `?mock&wallet=connected`.
export const MOCK_KASS = 'So11111111111111111111111111111111111111112'
export const MOCK_USDC = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'
