export interface PagefindSubResult {
  url: string
  title?: string
  excerpt?: string
}

export interface SearchResult {
  id: string
  url: string
  meta: { title: string }
  excerpt: string
  pageIndex?: number | null
  content?: string
  sub_results?: PagefindSubResult[]
  customExcerpt?: string
  matchCount?: number
  matchScope?: 'section' | 'document'
}

export interface SearchOpenTarget {
  hash?: string
  text?: string
  pageIndex?: number
}

export interface PagefindInstance {
  search: (query: string) => Promise<{ results: { id: string; data: () => Promise<SearchResult> }[] }>
  destroy?: () => void
}

export interface DocumentInfo {
  title: string
  url: string
  uploadId?: string
  originalFileName?: string | null
  format?: string
  source?: 'bundled' | 'upload' | 'audiobook-upload'
  importedAtMs?: number
  bytes?: number
  sections?: number
  coverMediaType?: string | null
  textStatus?: 'processing' | 'ready' | 'recognition-available' | 'recognition-required'
}
