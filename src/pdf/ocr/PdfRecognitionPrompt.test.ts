import { describe, expect, it } from 'vitest'
import {
  isPdfRecognitionStatusForDocument,
  pdfRecognitionActionPageCount,
  pdfRecognitionIndicatorState,
  pdfRecognitionIssueAction,
} from './pdfRecognitionPromptState'
import { PDF_OCR_INTERRUPTED } from './pdfRecognitionJob'

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

  it('retries technical failures but accepts usable partial recognition', () => {
    const mixedIssues = { failedPages: [2], unrecognizedPages: [4], lowConfidencePages: [1, 3] }

    expect(pdfRecognitionIssueAction(mixedIssues)).toBe('retry')
    expect(pdfRecognitionActionPageCount(mixedIssues)).toBe(1)
    expect(pdfRecognitionIssueAction({ failedPages: [], unrecognizedPages: [2], lowConfidencePages: [1] })).toBe('accept')
    expect(pdfRecognitionActionPageCount({ failedPages: [], unrecognizedPages: [2], lowConfidencePages: [1] })).toBe(2)
    expect(pdfRecognitionIssueAction({ failedPages: [], unrecognizedPages: [], lowConfidencePages: [] })).toBeNull()
    expect(pdfRecognitionActionPageCount()).toBe(0)
  })

  it('distinguishes OCR activity, attention, and failure without flagging optional cleanup', () => {
    const documentUrl = 'uploaded://pdf/one'

    expect(pdfRecognitionIndicatorState({
      status: 'recognizing',
      format: 'pdf-ocr',
      documentUrl,
    }, documentUrl, 'recognition-required')).toBe('running')
    expect(pdfRecognitionIndicatorState({
      status: 'recognized',
      format: 'pdf-ocr',
      documentUrl,
      recognitionIssues: { failedPages: [], unrecognizedPages: [], lowConfidencePages: [1] },
    }, documentUrl)).toBe('attention')
    expect(pdfRecognitionIndicatorState({
      status: 'error',
      format: 'pdf-ocr',
      documentUrl,
    }, documentUrl)).toBe('error')
    expect(pdfRecognitionIndicatorState({
      status: 'error',
      format: 'pdf-ocr',
      documentUrl,
      message: PDF_OCR_INTERRUPTED,
    }, documentUrl)).toBe('attention')
    expect(pdfRecognitionIndicatorState({ status: 'idle' }, documentUrl, 'recognition-required')).toBe('attention')
    expect(pdfRecognitionIndicatorState({ status: 'idle' }, documentUrl, 'recognition-available')).toBe('none')
  })
})
