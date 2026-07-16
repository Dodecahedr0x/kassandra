/**
 * Offline unit tests for `uniformSplit` — the even base-unit distribution behind
 * the group's default "deposit a uniform share of the total to each market". Pure,
 * no React / chain.
 */
import { describe, expect, it } from 'vitest'

import { uniformSplit } from '../src/market/data/actions/bulkLiquidity'

describe('uniformSplit', () => {
  it('divides evenly when it divides cleanly', () => {
    expect(uniformSplit(9n, 3)).toEqual([3n, 3n, 3n])
    expect(uniformSplit(1_000_000_000n, 2)).toEqual([500_000_000n, 500_000_000n])
  })

  it('spreads the remainder one base unit at a time across the leading shares', () => {
    expect(uniformSplit(10n, 3)).toEqual([4n, 3n, 3n]) // remainder 1 → first share
    expect(uniformSplit(10n, 4)).toEqual([3n, 3n, 2n, 2n]) // remainder 2 → first two
    expect(uniformSplit(7n, 3)).toEqual([3n, 2n, 2n])
  })

  it('always sums back to the exact total (no dust)', () => {
    for (const [total, n] of [
      [1_000_000_001n, 3],
      [123_456_789n, 7],
      [5n, 4],
    ] as const) {
      const shares = uniformSplit(total, n)
      expect(shares).toHaveLength(n)
      expect(shares.reduce((a, b) => a + b, 0n)).toBe(total)
    }
  })

  it('handles n<=0 and a zero total', () => {
    expect(uniformSplit(100n, 0)).toEqual([])
    expect(uniformSplit(0n, 3)).toEqual([0n, 0n, 0n])
  })

  it('rejects a negative total', () => {
    expect(() => uniformSplit(-1n, 2)).toThrow()
  })
})
