import { describe, expect, it } from 'vitest'
import { parsePdfRecognitionJob } from './pdfRecognitionJob'

describe('parsePdfRecognitionJob', () => {
  it('accepts bounded recovery metadata and rejects malformed page lists', () => {
    expect(parsePdfRecognitionJob({
      documentUrl: 'uploaded://pdf/one',
      language: 'ara',
      improveIssues: {
        failedPages: [2],
        unrecognizedPages: [],
        lowConfidencePages: [4],
      },
      sessionId: 'previous-session',
    })).toMatchObject({ documentUrl: 'uploaded://pdf/one', language: 'ara' })

    expect(parsePdfRecognitionJob({
      documentUrl: 'uploaded://pdf/one',
      language: 'eng',
      improveIssues: {
        failedPages: [0],
        unrecognizedPages: [],
        lowConfidencePages: [],
      },
      sessionId: 'previous-session',
    })).toBeNull()
  })
})
