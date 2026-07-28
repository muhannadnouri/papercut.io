import type { DocumentInfo } from '../../types/search'

export interface BundledDocumentFolder {
  id: string
  name: string
  folders: BundledDocumentFolder[]
  documents: DocumentInfo[]
  documentCount: number
}

export interface BundledDocumentTree {
  folders: BundledDocumentFolder[]
  documents: DocumentInfo[]
  documentCount: number
}

interface MutableFolder {
  id: string
  name: string
  folders: Map<string, MutableFolder>
  documents: DocumentInfo[]
}

/**
 * Build the read-only folder hierarchy encoded in bundled `/documents/...` URLs.
 *
 * Bundled documents are static Vite/Pagefind assets rather than uploaded-library
 * records, so their canonical URL remains both their identity and folder source.
 */
export function buildBundledDocumentTree(
  documents: DocumentInfo[],
  locale: string,
): BundledDocumentTree {
  const root: MutableFolder = {
    id: '/documents',
    name: '',
    folders: new Map(),
    documents: [],
  }

  for (const document of documents) {
    let parent = root
    const path: string[] = []

    for (const segment of bundledDocumentFolderNames(document.url)) {
      path.push(segment)
      const id = `/documents/${path.map(encodeURIComponent).join('/')}`
      let folder = parent.folders.get(segment)
      if (!folder) {
        folder = { id, name: segment, folders: new Map(), documents: [] }
        parent.folders.set(segment, folder)
      }
      parent = folder
    }

    parent.documents.push(document)
  }

  const collator = new Intl.Collator(locale, { numeric: true, sensitivity: 'base' })
  return finalizeFolder(root, collator)
}

/** Return decoded directory names while leaving the final filename out. */
export function bundledDocumentFolderNames(url: string): string[] {
  const marker = '/documents/'
  const markerIndex = url.indexOf(marker)
  if (markerIndex === -1) return []

  const relativePath = url
    .slice(markerIndex + marker.length)
    .split(/[?#]/, 1)[0]
  const segments = relativePath.split('/').filter(Boolean)
  return segments.slice(0, -1).map(decodePathSegment)
}

function finalizeFolder(
  folder: MutableFolder,
  collator: Intl.Collator,
): BundledDocumentTree {
  const folders = Array.from(folder.folders.values())
    .map((child) => {
      const finalized = finalizeFolder(child, collator)
      return { id: child.id, name: child.name, ...finalized }
    })
    .sort((a, b) => collator.compare(a.name, b.name))
  const documents = folder.documents
    .slice()
    .sort((a, b) => collator.compare(a.title, b.title))

  return {
    folders,
    documents,
    documentCount: documents.length +
      folders.reduce((total, child) => total + child.documentCount, 0),
  }
}

function decodePathSegment(segment: string): string {
  try {
    return decodeURIComponent(segment)
  } catch {
    return segment
  }
}
