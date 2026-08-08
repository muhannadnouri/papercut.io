import { describe, expect, it } from 'vitest'
import { parsePdfRecognitionJob } from './pdfRecognitionJob'

describe('parsePdfRecognitionJob', () => {
  it('accepts bounded recovery metadata and rejects malformed page lists', () => {
    expect(parsePdfRecognitionJob({
      documentUrl: '/uploads/abc123.pdf',
      language: 'ara',
      improveIssues: {
        failedPages: [2],
        unrecognizedPages: [],
        lowConfidencePages: [4],
      },
      sessionId: 'previous-session',
    })).toMatchObject({ documentUrl: '/uploads/abc123.pdf', language: 'ara' })

    expect(parsePdfRecognitionJob({
      documentUrl: '/uploads/abc123.pdf',
      language: 'eng',
      improveIssues: {
        failedPages: [0],
        unrecognizedPages: [],
        lowConfidencePages: [],
      },
      sessionId: 'previous-session',
    })).toBeNull()

    expect(parsePdfRecognitionJob({
      documentUrl: '/uploads/abc123.pdf',
      language: 'eng',
      improveIssues: {
        failedPages: [2_001],
        unrecognizedPages: [],
        lowConfidencePages: [],
      },
      sessionId: 'previous-session',
    })).toBeNull()

    expect(parsePdfRecognitionJob({
      documentUrl: 'https://example.com/document.pdf',
      language: 'eng',
      sessionId: 'previous-session',
    })).toBeNull()
  })
})
