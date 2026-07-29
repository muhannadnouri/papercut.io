import type { DocumentInfo } from '../../types/search'

export function isBookDocument(doc: DocumentInfo): boolean {
  return doc.format === 'epub' || doc.format === 'pdf'
}
