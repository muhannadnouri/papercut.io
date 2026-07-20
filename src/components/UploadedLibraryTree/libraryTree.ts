import type { DocumentInfo } from '../../types/search'
import {
  type UploadedLibraryFolder,
  type UploadedLibraryOrganization,
  isUploadedDocumentUrl,
} from '../../uploads/DocumentUploads'

export type LibraryNode =
  | {
      key: string
      kind: 'folder'
      id: string
      title: string
      depth: number
      documentCount: number
      children: LibraryNode[]
    }
  | {
      key: string
      kind: 'document'
      id: string
      title: string
      url: string
      doc: DocumentInfo
      children: LibraryNode[]
    }

/**
 * Join uploaded documents, folder metadata, and persisted locations into the
 * shared hierarchy used by both the Library and Search filter views.
 */
export function buildLibraryTree(
  documents: DocumentInfo[],
  organization: UploadedLibraryOrganization,
  options: { hideEmptyFolders?: boolean; locale?: string } = {},
): {
  nodes: LibraryNode[]
  folders: UploadedLibraryFolder[]
  folderOptions: { id: string; label: string }[]
  nodeByKey: Map<string, LibraryNode>
} {
  const collator = new Intl.Collator(options.locale, { numeric: true, sensitivity: 'base' })
  const uploadDocs = documents
    .filter((doc) => doc.source === 'upload' && isUploadedDocumentUrl(doc.url))
    .map((doc) => ({ ...doc, uploadId: uploadIdFromUrl(doc.url) }))
    .filter((doc): doc is DocumentInfo & { uploadId: string } => Boolean(doc.uploadId))
  const foldersByParent = groupFoldersByParent(organization.folders)
  const locations = new Map(organization.documentLocations.map((location) => [location.documentId, location]))
  const docsByFolder = new Map<string, (DocumentInfo & { uploadId: string })[]>()
  for (const doc of uploadDocs) {
    const folderId = locations.get(doc.uploadId)?.folderId ?? null
    const key = folderId ?? ''
    const list = docsByFolder.get(key)
    if (list) list.push(doc)
    else docsByFolder.set(key, [doc])
  }

  const nodeByKey = new Map<string, LibraryNode>()
  const buildFolder = (folder: UploadedLibraryFolder): LibraryNode | null => {
    const children = [
      ...sortFolders(foldersByParent.get(folder.id) ?? [], collator)
        .map(buildFolder)
        .filter((node): node is LibraryNode => Boolean(node)),
      ...sortDocuments(docsByFolder.get(folder.id) ?? [], locations, collator).map(documentNode),
    ]
    if (options.hideEmptyFolders && countDocuments(children) === 0) return null
    const node: LibraryNode = {
      key: folderKey(folder.id),
      kind: 'folder',
      id: folder.id,
      title: folder.name,
      depth: folder.depth,
      documentCount: countDocuments(children),
      children,
    }
    nodeByKey.set(node.key, node)
    return node
  }
  const documentNode = (doc: DocumentInfo & { uploadId: string }): LibraryNode => {
    const node: LibraryNode = {
      key: documentKey(doc.uploadId),
      kind: 'document',
      id: doc.uploadId,
      title: doc.title,
      url: doc.url,
      doc,
      children: [],
    }
    nodeByKey.set(node.key, node)
    return node
  }

  const nodes = [
    ...sortFolders(foldersByParent.get('') ?? [], collator)
      .map(buildFolder)
      .filter((node): node is LibraryNode => Boolean(node)),
    ...sortDocuments(docsByFolder.get('') ?? [], locations, collator).map(documentNode),
  ]
  return {
    nodes,
    folders: organization.folders,
    folderOptions: buildFolderOptions(organization.folders, collator),
    nodeByKey,
  }
}

/** Collect descendant documents for folder-level search filter toggles. */
export function collectDocuments(node: LibraryNode): DocumentInfo[] {
  if (node.kind === 'document') return [node.doc]
  return node.children.flatMap(collectDocuments)
}

/** Group folders by parent id, using an empty string as the root bucket key. */
function groupFoldersByParent(folders: UploadedLibraryFolder[]): Map<string, UploadedLibraryFolder[]> {
  const groups = new Map<string, UploadedLibraryFolder[]>()
  for (const folder of folders) {
    const key = folder.parentId ?? ''
    const list = groups.get(key)
    if (list) list.push(folder)
    else groups.set(key, [folder])
  }
  return groups
}

/** Count only document descendants, not folders, for folder count badges. */
function countDocuments(nodes: LibraryNode[]): number {
  return nodes.reduce((total, node) => (
    total + (node.kind === 'document' ? 1 : countDocuments(node.children))
  ), 0)
}

/** Preserve manual folder order, with name as a stable tie-breaker. */
function sortFolders(folders: UploadedLibraryFolder[], collator: Intl.Collator): UploadedLibraryFolder[] {
  return folders.slice().sort((a, b) => a.sortOrder - b.sortOrder || collator.compare(a.name, b.name))
}

/** Preserve manual document order, with title as a stable tie-breaker. */
function sortDocuments(
  docs: (DocumentInfo & { uploadId: string })[],
  locations: Map<string, { sortOrder: number }>,
  collator: Intl.Collator,
): (DocumentInfo & { uploadId: string })[] {
  return docs
    .slice()
    .sort((a, b) => (
      (locations.get(a.uploadId)?.sortOrder ?? 0) - (locations.get(b.uploadId)?.sortOrder ?? 0) ||
      collator.compare(a.title, b.title)
    ))
}

/** Build move-target labels that include parent path context for duplicate names elsewhere. */
function buildFolderOptions(
  folders: UploadedLibraryFolder[],
  collator: Intl.Collator,
): { id: string; label: string }[] {
  const byId = new Map(folders.map((folder) => [folder.id, folder]))
  return sortFolders(folders, collator).map((folder) => ({
    id: folder.id,
    label: folderPath(folder, byId),
  }))
}

/** Walk parent pointers to display a folder path like `Parent / Child`. */
function folderPath(folder: UploadedLibraryFolder, byId: Map<string, UploadedLibraryFolder>): string {
  const parts = [folder.name]
  let parentId = folder.parentId
  while (parentId) {
    const parent = byId.get(parentId)
    if (!parent) break
    parts.unshift(parent.name)
    parentId = parent.parentId
  }
  return parts.join(' / ')
}

/** Extract the backend upload id from the app-local uploaded document URL. */
function uploadIdFromUrl(url: string): string | null {
  const match = url.match(/^\/uploads\/([a-fA-F0-9]+)\.html(?:[#?].*)?$/)
  return match?.[1] ?? null
}

/** Prefix ids so React Aria keys cannot collide between folders and documents. */
function folderKey(id: string): string {
  return 'folder:' + id
}

/** Prefix ids so React Aria keys cannot collide between documents and folders. */
function documentKey(id: string): string {
  return 'document:' + id
}
