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
