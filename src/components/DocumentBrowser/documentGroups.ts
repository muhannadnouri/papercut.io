import type { AuthorGroup } from '../../hooks/useDocumentFilters'

/**
 * Split grouped documents into the render paths used by the document browser.
 *
 * Uploaded and bundled documents use their respective folder trees. Imported
 * audiobook documents stay grouped for the simpler DocumentList path.
 */
export function splitDocumentGroupsBySource(groupedDocs: AuthorGroup[]) {
  const uploadDocs = groupedDocs.flatMap((group) => group.docs.filter((doc) => doc.source === 'upload'))
  const bundledDocs = groupedDocs.flatMap((group) => group.docs.filter((doc) => doc.source === 'bundled'))
  const nonBundledGroups = groupedDocs
    .map((group) => ({ ...group, docs: group.docs.filter((doc) => doc.source !== 'bundled') }))
    .filter((group) => group.docs.length > 0)
    .sort((a, b) => Number(b.docs.some((doc) => doc.source === 'upload')) -
      Number(a.docs.some((doc) => doc.source === 'upload')))
  const otherGroups = groupedDocs
    .map((group) => ({
      ...group,
      docs: group.docs.filter((doc) => doc.source !== 'upload' && doc.source !== 'bundled'),
    }))
    .filter((group) => group.docs.length > 0)

  return { uploadDocs, bundledDocs, nonBundledGroups, otherGroups }
}
