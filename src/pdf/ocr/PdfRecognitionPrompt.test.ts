import { describe, expect, it } from 'vitest'
import {
  isPdfRecognitionStatusForDocument,
  pdfRecognitionActionPageCount,
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
    const mixedIssues = { failedPages: [2], lowConfidencePages: [1, 3] }

    expect(pdfRecognitionIssueAction(mixedIssues)).toBe('retry')
    expect(pdfRecognitionActionPageCount(mixedIssues)).toBe(1)
    expect(pdfRecognitionIssueAction({ failedPages: [], lowConfidencePages: [1] })).toBe('accept')
    expect(pdfRecognitionActionPageCount({ failedPages: [], lowConfidencePages: [1] })).toBe(1)
    expect(pdfRecognitionIssueAction({ failedPages: [], lowConfidencePages: [] })).toBeNull()
    expect(pdfRecognitionActionPageCount()).toBe(0)
  })
})
