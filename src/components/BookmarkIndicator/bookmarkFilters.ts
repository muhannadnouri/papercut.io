import type { AuthorGroup } from '../../hooks/useDocumentFilters'

/** Keep bookmarked documents and discard groups left empty by the filter. */
export function filterBookmarkedGroups(
  groups: AuthorGroup[],
  bookmarkedUrls: ReadonlySet<string>,
  enabled: boolean,
): AuthorGroup[] {
  if (!enabled) return groups
  return groups
    .map((group) => ({
      ...group,
      docs: group.docs.filter((doc) => bookmarkedUrls.has(doc.url)),
    }))
    .filter((group) => group.docs.length > 0)
}
