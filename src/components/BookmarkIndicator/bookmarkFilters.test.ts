import { describe, expect, it } from 'vitest'
import { filterBookmarkedGroups } from './bookmarkFilters'

const groups = [
  { author: 'A', docs: [{ title: 'One', url: '/one' }, { title: 'Two', url: '/two' }] },
  { author: 'B', docs: [{ title: 'Three', url: '/three' }] },
]

describe('filterBookmarkedGroups', () => {
  it('keeps matching documents and removes empty groups only when enabled', () => {
    expect(filterBookmarkedGroups(groups, new Set(['/two']), false)).toBe(groups)
    expect(filterBookmarkedGroups(groups, new Set(['/two']), true)).toEqual([
      { author: 'A', docs: [{ title: 'Two', url: '/two' }] },
    ])
  })
})
