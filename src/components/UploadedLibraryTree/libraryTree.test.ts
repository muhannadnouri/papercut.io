import { describe, expect, it } from 'vitest'
import type { DocumentInfo } from '../../types/search'
import { buildLibraryTree, countDescendantFolders } from './libraryTree'

describe('buildLibraryTree', () => {
  it('includes uploaded HTML and PDF documents', () => {
    const documents: DocumentInfo[] = [
      {
        title: 'HTML document',
        url: '/uploads/a1.html',
        format: 'html',
        source: 'upload',
      },
      {
        title: 'PDF document',
        url: '/uploads/b2.pdf',
        format: 'pdf',
        source: 'upload',
      },
    ]

    const tree = buildLibraryTree(documents, {
      folders: [],
      documentLocations: [],
    })

    expect(tree.nodes.map((node) => node.title)).toEqual([
      'HTML document',
      'PDF document',
    ])
    expect([...tree.nodeByKey.keys()]).toEqual([
      'document:a1',
      'document:b2',
    ])
  })

  it('keeps only matching document ancestor folders when empty folders are hidden', () => {
    const documents: DocumentInfo[] = [{
      title: 'Matching document',
      url: '/uploads/a1.html',
      format: 'html',
      source: 'upload',
    }]
    const tree = buildLibraryTree(documents, {
      folders: [
        { id: 'parent', name: 'Parent', depth: 0, sortOrder: 0, createdAtMs: 0, updatedAtMs: 0 },
        { id: 'match', parentId: 'parent', name: 'Match', depth: 1, sortOrder: 0, createdAtMs: 0, updatedAtMs: 0 },
        { id: 'empty', parentId: 'parent', name: 'Empty', depth: 1, sortOrder: 1, createdAtMs: 0, updatedAtMs: 0 },
      ],
      documentLocations: [{ documentId: 'a1', folderId: 'match', sortOrder: 0 }],
    }, { hideEmptyFolders: true })

    expect(tree.nodes).toHaveLength(1)
    expect(tree.nodes[0]).toMatchObject({
      kind: 'folder',
      title: 'Parent',
      documentCount: 1,
      children: [{
        kind: 'folder',
        title: 'Match',
        documentCount: 1,
      }],
    })
    expect(tree.nodeByKey.has('folder:empty')).toBe(false)
    expect(countDescendantFolders(tree.nodes[0])).toBe(1)
  })
})
