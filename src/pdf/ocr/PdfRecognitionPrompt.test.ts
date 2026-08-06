import { describe, expect, it } from 'vitest'
import {
  isPdfRecognitionStatusForDocument,
  pdfRecognitionIssueAction,
} from './pdfRecognitionPromptState'

describe('isPdfRecognitionStatusForDocument', () => {
  it('matches OCR feedback only to its owning document', () => {
    const status = {
      status: 'recognizing',
      format: 'pdf-ocr',
      documentUrl: 'uploaded://pdf/one',
    } as const

    expect(isPdfRecognitionStatusForDocument(status, 'uploaded://pdf/one')).toBe(true)
    expect(isPdfRecognitionStatusForDocument(status, 'uploaded://pdf/two')).toBe(false)
    expect(isPdfRecognitionStatusForDocument({ status: 'idle' }, 'uploaded://pdf/one')).toBe(false)
  })

  it('retries failed pages but accepts usable low-confidence text', () => {
    expect(pdfRecognitionIssueAction({ failedPages: [2], lowConfidencePages: [1] })).toBe('retry')
    expect(pdfRecognitionIssueAction({ failedPages: [], lowConfidencePages: [1] })).toBe('accept')
    expect(pdfRecognitionIssueAction({ failedPages: [], lowConfidencePages: [] })).toBeNull()
  })
})
