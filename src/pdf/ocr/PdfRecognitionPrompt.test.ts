import { describe, expect, it } from 'vitest'
import { isPdfRecognitionStatusForDocument } from './pdfRecognitionPromptState'

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
})
