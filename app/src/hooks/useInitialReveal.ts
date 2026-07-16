import { useEffect, useRef } from 'react'

/**
 * Returns `true` only on the FIRST render where `ready` becomes true, then `false`
 * forever after — so a grid can stagger its cards in on the initial data load
 * without re-animating on every subsequent search / filter / sort / poll
 * re-render (which would be the over-animation trap). Pair with the `.stagger-in`
 * utility + a per-card `--stagger-delay`.
 */
export function useInitialReveal(ready: boolean): boolean {
  const seen = useRef(false)
  const first = ready && !seen.current
  useEffect(() => {
    if (ready) seen.current = true
  }, [ready])
  return first
}

export default useInitialReveal
