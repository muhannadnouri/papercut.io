import type { AuthorGroup } from '../../hooks/useDocumentFilters'

/**
 * Split grouped documents into the two render paths used by the document browser.
 *
 * Uploaded documents are rendered through UploadedLibraryTree so folder organization
 * can be shared by Library and Search. Everything else stays grouped by author/source
 * for the simpler DocumentList path.
 */
export function splitDocumentGroupsByUpload(groupedDocs: AuthorGroup[]) {
  const uploadDocs = groupedDocs.flatMap((group) => group.docs.filter((doc) => doc.source === 'upload'))
  const nonUploadGroups = groupedDocs
    .map((group) => ({ ...group, docs: group.docs.filter((doc) => doc.source !== 'upload') }))
    .filter((group) => group.docs.length > 0)

  return { uploadDocs, nonUploadGroups }
}
