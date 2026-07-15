/**
 * Offline mock fixtures for the oracle browse views — decoded-shaped
 * {@link OracleSummary} / {@link OracleDetail} objects so the list + detail
 * pages are visually reviewable with NO chain / RPC (headless render, design
 * review). This does NOT pollute the real data path: the pages call these ONLY
 * when {@link isMockMode} is true (`VITE_MOCK=1` build-time, or a `?mock` query
 * param at runtime); otherwise they go through `fetchOracles`/`fetchOracleDetail`
 * over the live {@link Connection}.
 */
export { isMockMode, isE2eMode } from './mode'
export { mockMarketAmms } from './amm'
export { mockOracles, mockOracleDetail } from './fixtures'
