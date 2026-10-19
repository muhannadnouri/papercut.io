import type { SearchOpenTarget, SearchTermMatch } from '../types/search'

export function indexedSearchOpenTarget(
  target: Pick<SearchTermMatch, 'sectionIndex' | 'pageIndex'> & {
    text?: string | null
    occurrenceIndex?: number
  },
  text = target.text ?? undefined,
): SearchOpenTarget {
  return {
    text,
    sectionIndex: target.sectionIndex ?? undefined,
    pageIndex: target.pageIndex ?? undefined,
    ...(target.occurrenceIndex === undefined
      ? {}
      : { occurrenceIndex: target.occurrenceIndex }),
  }
}
