import { describe, expect, it } from 'vitest'
import { updateScopedSelection } from './documentSelection'

describe('library transfer document selection', () => {
  it('changes only the filtered scope and preserves hidden selections', () => {
    expect(updateScopedSelection(['hidden', 'match-a'], ['match-a', 'match-b'], true))
      .toEqual(['hidden', 'match-a', 'match-b'])
    expect(updateScopedSelection(['hidden', 'match-a', 'match-b'], ['match-a', 'match-b'], false))
      .toEqual(['hidden'])
  })
})
