import { describe, expect, it } from 'vitest'
import type { DocumentInfo } from '../../types/search'
import { buildLibraryTree } from './libraryTree'

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
})
