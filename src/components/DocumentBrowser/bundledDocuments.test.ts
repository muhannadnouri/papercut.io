import { describe, expect, it } from 'vitest'
import type { DocumentInfo } from '../../types/search'
import {
  buildBundledDocumentTree,
  bundledDocumentFolderNames,
} from './bundledDocuments'
import { splitDocumentGroupsBySource } from './documentGroups'

describe('buildBundledDocumentTree', () => {
  it('preserves nested bundled folders and descendant counts', () => {
    const tree = buildBundledDocumentTree([
      document('Published', '/documents/johnSmith/publishings/book.html'),
      document('Draft', '/documents/johnSmith/notes/draft.html'),
      document('Root page', '/documents/root.html'),
    ], 'en')

    expect(tree.documentCount).toBe(3)
    expect(tree.documents.map((item) => item.title)).toEqual(['Root page'])
    expect(tree.folders[0]).toMatchObject({
      id: '/documents/johnSmith',
      name: 'johnSmith',
      documentCount: 2,
    })
    expect(tree.folders[0].folders.map((folder) => folder.name)).toEqual([
      'notes',
      'publishings',
    ])
  })

  it('keeps duplicate filenames distinct by their canonical URLs', () => {
    const tree = buildBundledDocumentTree([
      document('First copy', '/documents/first/shared.html'),
      document('Second copy', '/documents/second/shared.html'),
    ], 'en')

    expect(tree.folders.map((folder) => folder.documents[0].url)).toEqual([
      '/documents/first/shared.html',
      '/documents/second/shared.html',
    ])
  })
})

describe('bundledDocumentFolderNames', () => {
  it('decodes each directory and ignores filenames, queries, and hashes', () => {
    expect(bundledDocumentFolderNames(
      '/documents/John%20Smith/Research%20Notes/page.html?view=1#section',
    )).toEqual(['John Smith', 'Research Notes'])
    expect(bundledDocumentFolderNames('/documents/root.html')).toEqual([])
  })
})

describe('splitDocumentGroupsBySource', () => {
  it('keeps bundled, uploaded, and audiobook documents on separate render paths', () => {
    const uploaded = document('Upload', '/uploads/123.html', 'upload')
    const bundled = document('Bundled', '/documents/source/page.html', 'bundled')
    const audiobook = document('Audiobook', '/user-uploads/book.html', 'audiobook-upload')
    const split = splitDocumentGroupsBySource([{
      author: 'Mixed',
      docs: [uploaded, bundled, audiobook],
    }])

    expect(split.uploadDocs).toEqual([uploaded])
    expect(split.bundledDocs).toEqual([bundled])
    expect(split.otherGroups[0].docs).toEqual([audiobook])
    expect(split.nonBundledGroups[0].docs).toEqual([uploaded, audiobook])
  })
})

function document(
  title: string,
  url: string,
  source: DocumentInfo['source'] = 'bundled',
): DocumentInfo {
  return { title, url, format: 'html', source }
}
