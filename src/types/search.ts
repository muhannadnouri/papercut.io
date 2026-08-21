export interface PagefindSubResult {
  url: string
  title?: string
  excerpt?: string
}

export interface SearchPassage {
  excerpt: string
  sectionTitle?: string | null
  sectionIndex: number
  pageIndex?: number | null
}

export interface SearchMatchLocation {
  binIndex: number
  sectionIndex: number
  pageIndex?: number | null
  matchCount: number
  text?: string | null
}

export interface SearchTermMatch {
  term: string
  matchingSections: number
  sectionIndex?: number | null
  pageIndex?: number | null
  text?: string | null
}

export interface SearchResult {
  id: string
  url: string
  meta: { title: string }
  excerpt: string
  sectionIndex?: number
  pageIndex?: number | null
  content?: string
  sub_results?: PagefindSubResult[]
  customExcerpt?: string
  matchCount?: number
  matchScope?: 'section' | 'document'
  matchingSections?: number
  passages?: SearchPassage[]
  matchLocations?: SearchMatchLocation[]
  termMatches?: SearchTermMatch[]
  source?: 'upload' | 'starter'
}

export interface SearchOpenTarget {
  hash?: string
  text?: string
  sectionIndex?: number
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
