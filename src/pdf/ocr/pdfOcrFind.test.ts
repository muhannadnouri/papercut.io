import { describe, expect, it, vi } from 'vitest'
import { createPdfOcrFindAdapter } from './pdfOcrFind'

describe('createPdfOcrFindAdapter', () => {
  it('maps compact page counts to next and previous OCR occurrences', async () => {
    const navigate = vi.fn()
    const onResult = vi.fn()
    const adapter = createPdfOcrFindAdapter(
      async () => ({
        matchCount: 3,
        pages: [
          { pageIndex: 2, matchCount: 2 },
          { pageIndex: 5, matchCount: 1 },
        ],
      }),
      navigate,
      vi.fn(),
      onResult,
    )

    adapter.api.search('needle')
    await vi.waitFor(() => expect(navigate).toHaveBeenCalledTimes(1))
    expect(navigate).toHaveBeenLastCalledWith(
      { pageIndex: 2, occurrenceIndex: 0 },
      'needle',
    )

    adapter.api.next()
    adapter.api.next()
    expect(navigate).toHaveBeenLastCalledWith(
      { pageIndex: 5, occurrenceIndex: 0 },
      'needle',
    )

    adapter.api.previous()
    expect(navigate).toHaveBeenLastCalledWith(
      { pageIndex: 2, occurrenceIndex: 1 },
      'needle',
    )
    expect(onResult).toHaveBeenLastCalledWith({ currentIndex: 1, matchCount: 3 })
  })
})
