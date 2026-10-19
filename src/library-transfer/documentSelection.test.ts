import { describe, expect, it } from 'vitest'
import { filterTransferDocuments, updateScopedSelection } from './documentSelection'

describe('library transfer document selection', () => {
  it('changes only the filtered scope and preserves hidden selections', () => {
    expect(updateScopedSelection(['hidden', 'match-a'], ['match-a', 'match-b'], true))
      .toEqual(['hidden', 'match-a', 'match-b'])
    expect(updateScopedSelection(['hidden', 'match-a', 'match-b'], ['match-a', 'match-b'], false))
      .toEqual(['hidden'])
  })

  it('intersects text matches with the selected-only view', () => {
    const documents = [
      { id: 'a', title: 'Alpha Report' },
      { id: 'b', title: 'Beta Report' },
      { id: 'c', title: 'Notes' },
    ]

    expect(filterTransferDocuments(documents, 'report', 'en', ['b', 'c'], true))
      .toEqual([documents[1]])
  })
})
