/** Apply a bulk action only to the current result scope, preserving hidden selections. */
export function updateScopedSelection(
  selectedIds: string[],
  scopedIds: string[],
  selected: boolean,
): string[] {
  const scope = new Set(scopedIds)
  return selected
    ? [...new Set([...selectedIds, ...scopedIds])]
    : selectedIds.filter((id) => !scope.has(id))
}

/** Apply the text and selected-only controls in one pass so their intersection stays predictable. */
export function filterTransferDocuments<T extends {
  id: string
  title: string
  originalFileName?: string | null
}>(
  documents: T[],
  query: string,
  locale: string,
  selectedIds: string[],
  selectedOnly: boolean,
): T[] {
  const normalizedQuery = query.trim().toLocaleLowerCase(locale)
  const selected = new Set(selectedIds)
  return documents.filter((document) => (
    (!selectedOnly || selected.has(document.id))
    && (!normalizedQuery || [document.title, document.originalFileName]
      .some((value) => value?.toLocaleLowerCase(locale).includes(normalizedQuery)))
  ))
}
