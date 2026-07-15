/** Pad to 2 digits — used by the hand-rolled datetime-local formatter. */
function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n)
}

/** Format a `Date` as a `datetime-local` value (`YYYY-MM-DDTHH:mm`), local time. */
export function toDatetimeLocal(d: Date): string {
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` +
    `T${pad(d.getHours())}:${pad(d.getMinutes())}`
  )
}

/** A `datetime-local` string → unix SECONDS (local-time interpreted). NaN if unparseable. */
export function datetimeLocalToUnix(value: string): number {
  const ms = new Date(value).getTime()
  return Number.isNaN(ms) ? NaN : Math.floor(ms / 1000)
}
