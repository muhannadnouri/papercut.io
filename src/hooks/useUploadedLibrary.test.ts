import { describe, expect, it } from 'vitest'
import { shouldAutoDismissDocumentImport, type DocumentImportStatus } from './useUploadedLibrary'

describe('shouldAutoDismissDocumentImport', () => {
  it('keeps failures visible while dismissing successful and cancelled imports', () => {
    const failed: DocumentImportStatus = {
      status: 'imported',
      batchResult: {
        selected: 1,
        processed: 1,
        imported: [],
        failures: [{ fileName: 'bad.epub', error: 'Invalid EPUB' }],
        cancelled: false,
      },
    }

    expect(shouldAutoDismissDocumentImport({ status: 'imported' })).toBe(true)
    expect(shouldAutoDismissDocumentImport({ status: 'cancelled' })).toBe(true)
    expect(shouldAutoDismissDocumentImport(failed)).toBe(false)
    expect(shouldAutoDismissDocumentImport({ status: 'error' })).toBe(false)
  })
})
