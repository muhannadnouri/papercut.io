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
    expect(shouldAutoDismissDocumentImport({
      ...failed,
      batchResult: { ...failed.batchResult!, failures: [], imported: [{ importImageWarnings: { missing: 2 } } as unknown as UploadedDocument] },
    })).toBe(false)
    expect(shouldAutoDismissDocumentImport({ status: 'error' })).toBe(false)
  })

  it('keeps recognition issues visible for review and retry', () => {
    expect(shouldAutoDismissDocumentImport({ status: 'recognized' })).toBe(true)
    expect(shouldAutoDismissDocumentImport({
      status: 'recognized',
      recognitionIssues: { failedPages: [2], unrecognizedPages: [3], lowConfidencePages: [4] },
    })).toBe(false)
  })

  it('retains incomplete source-formatting reports without warning on complete or legacy imports', () => {
    const imported: UploadedDocument[] = [{
      sourceFormatting: { policy: 'source-emphasis-v1', warnings: ['unread-stylesheet'] },
    } as UploadedDocument]
    const status: DocumentImportStatus = {
      status: 'imported',
      batchResult: {
        selected: 1, processed: 1, imported, alreadyInLibrary: [], failures: [], cancelled: false,
      },
    }

    expect(shouldAutoDismissDocumentImport(status)).toBe(false)
    expect(shouldAutoDismissDocumentImport({ ...status, status: 'cancelled' })).toBe(false)
    imported[0].sourceFormatting!.warnings = []
    expect(shouldAutoDismissDocumentImport(status)).toBe(true)
    delete imported[0].sourceFormatting
    expect(shouldAutoDismissDocumentImport(status)).toBe(true)
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
