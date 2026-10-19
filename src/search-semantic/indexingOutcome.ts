export function isIndexingCancelled(error: unknown): boolean {
  return error === 'Semantic indexing cancelled'
}
