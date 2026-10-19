import { describe, expect, it, vi } from 'vitest'
import { syncPdfViewerLayout } from './pdfViewerLayout'

describe('syncPdfViewerLayout', () => {
  it('recomputes responsive fit modes without resetting numeric zoom', () => {
    const update = vi.fn()
    const assigned: string[] = []
    let scale = 'page-width'
    const viewer = {
      get currentScaleValue() {
        return scale
      },
      set currentScaleValue(value: string) {
        assigned.push(value)
        scale = value
      },
      update,
    }

    syncPdfViewerLayout(viewer, false)
    expect(assigned).toEqual([])

    syncPdfViewerLayout(viewer, true)
    expect(assigned).toEqual(['page-width'])

    scale = 'page-fit'
    syncPdfViewerLayout(viewer, false)
    expect(assigned).toEqual(['page-width', 'page-fit'])

    scale = '1.25'
    syncPdfViewerLayout(viewer, true)
    expect(assigned).toEqual(['page-width', 'page-fit'])
    expect(update).toHaveBeenCalledTimes(4)
  })
})
