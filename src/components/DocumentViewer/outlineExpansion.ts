import type { OutlineEntry } from '../../viewers/types'

/** Branches above this depth start open (roots are depth zero). A lone title
 * opens one level in a long outline so the first view includes its chapters. */
export function initialOutlineExpansionDepth(entries: readonly OutlineEntry[]): number {
  let remaining = 20
  function exceedsLimit(siblings: readonly OutlineEntry[]): boolean {
    remaining -= siblings.length
    return remaining < 0 || siblings.some(entry => exceedsLimit(entry.children))
  }

  if (!exceedsLimit(entries)) {
    return Infinity
  }
  return entries.length === 1 ? 1 : 0
}
