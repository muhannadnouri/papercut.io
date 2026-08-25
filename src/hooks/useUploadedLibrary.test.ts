import { describe, expect, it } from 'vitest'
import type { UploadedDocument } from '../uploads/DocumentUploads'
import {
  shouldAutoDismissDocumentImport,
  shouldRecognizeImportedScan,
  type DocumentImportStatus,
} from './useUploadedLibrary'

describe('shouldAutoDismissDocumentImport', () => {
  it('keeps failures visible while dismissing successful and cancelled imports', () => {
    const failed: DocumentImportStatus = {
      status: 'imported',
      batchResult: {
        selected: 1,
        processed: 1,
        imported: [],
        alreadyInLibrary: [],
        failures: [{ fileName: 'bad.epub', error: 'Invalid EPUB' }],
        cancelled: false,
      },
    }

    expect(shouldAutoDismissDocumentImport({ status: 'imported' })).toBe(true)
    expect(shouldAutoDismissDocumentImport({ status: 'cancelled' })).toBe(true)
    expect(shouldAutoDismissDocumentImport(failed)).toBe(false)
    expect(shouldAutoDismissDocumentImport({ status: 'error' })).toBe(false)
  })

  it('keeps recognition issues visible for review and retry', () => {
    expect(shouldAutoDismissDocumentImport({ status: 'recognized' })).toBe(true)
    expect(shouldAutoDismissDocumentImport({
      status: 'recognized',
      recognitionIssues: { failedPages: [2], unrecognizedPages: [3], lowConfidencePages: [4] },
    })).toBe(false)
  })
})

describe('shouldRecognizeImportedScan', () => {
  const document = {
    sourceKind: 'pdf',
    textStatus: 'recognition-required',
  } as UploadedDocument

  it('runs only packaged recognition languages for PDFs that need it', () => {
    expect(shouldRecognizeImportedScan(document, 'english')).toBe(true)
    expect(shouldRecognizeImportedScan(document, 'arabic')).toBe(true)
    expect(shouldRecognizeImportedScan(document, 'other')).toBe(false)
    expect(shouldRecognizeImportedScan({ ...document, textStatus: 'recognition-available' }, 'english')).toBe(true)
    expect(shouldRecognizeImportedScan({ ...document, textStatus: 'ready' }, 'english')).toBe(false)
  })
})
