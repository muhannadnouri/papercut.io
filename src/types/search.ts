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
  content?: string
  sub_results?: PagefindSubResult[]
  customExcerpt?: string
  matchCount?: number
  matchScope?: 'section' | 'document'
}

export interface SearchOpenTarget {
  hash?: string
  text?: string
}

export interface PagefindInstance {
  search: (query: string) => Promise<{ results: { id: string; data: () => Promise<SearchResult> }[] }>
  destroy?: () => void
}

export interface DocumentInfo {
  title: string
  url: string
  format?: string
  source?: 'bundled' | 'upload' | 'audiobook-upload'
  coverMediaType?: string | null
}
