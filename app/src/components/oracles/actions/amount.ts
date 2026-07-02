/**
 * Parse a KASS amount typed as a whole number of base units (raw, unscaled —
 * matching how the detail view shows bond/stake). Returns the `bigint` value or
 * an inline error message for the form.
 */
export function parseAmount(raw: string): { value?: bigint; error?: string } {
  const t = raw.trim()
  if (t === '') return { error: 'Enter a KASS amount.' }
  if (!/^\d+$/.test(t)) return { error: 'Amount must be a whole number of KASS base units.' }
  const value = BigInt(t)
  if (value <= 0n) return { error: 'Amount must be greater than zero.' }
  return { value }
}
