import { describe, expect, it } from 'vitest'
import { pdfLoadErrorMessage } from './pdfJs'

describe('pdfLoadErrorMessage', () => {
  it.each([
    ['PasswordException', 'Password-protected PDFs are not supported.'],
    ['InvalidPDFException', 'This PDF is damaged or invalid.'],
    ['AbortException', 'PDF loading was interrupted.'],
    ['ResponseException', 'The PDF could not be loaded.'],
  ])('maps %s without exposing parser internals', (name, message) => {
    expect(pdfLoadErrorMessage(namedError(name, 'internal parser details'))).toBe(message)
  })

  it('distinguishes a missing PDF from other response failures', () => {
    const error = namedError('ResponseException', 'unexpected response') as Error & {
      missing: boolean
    }
    error.missing = true
    expect(pdfLoadErrorMessage(error)).toBe('The PDF file is missing or unavailable.')
  })

  it('preserves useful errors outside the known PDF.js failure contract', () => {
    expect(pdfLoadErrorMessage(new Error('PDF exceeds the 2000-page import limit')))
      .toBe('PDF exceeds the 2000-page import limit')
  })
})

function namedError(name: string, message: string): Error {
  const error = new Error(message)
  error.name = name
  return error
}
