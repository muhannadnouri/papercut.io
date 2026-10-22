/** Native review identities and scalar offsets are opaque: the editor never supplies replacement text or HTML. */
export interface ReviewExpected {
  schemaVersion: number
  policy: string
  variantId: string
  generation: string
  revision: number
}

export interface ScalarRange { start: number; end: number }
export type MarkKind = 'bold' | 'italic'
export interface Mark { range: ScalarRange; kind: MarkKind }
export interface ReviewBlockKey { ordinal: number; sourceIdentity: string }
export type ReviewObligation = 'emphasis' | 'typography' | 'link' | 'note' | 'attachment' | 'sourceFormatting'

export interface ReviewBlock {
  key: ReviewBlockKey
  structureIdentity: string
  text: string
  textHash: string
  locked: ScalarRange[]
  fixed: Mark[]
  editable: Mark[]
  obligations: ReviewObligation[]
  approved: boolean
}

export interface ReviewDocument {
  expected: ReviewExpected
  normalizationPolicy: string
  sourceHash: string
  publishedHash: string
  blocks: ReviewBlock[]
}

export interface ReviewDraft {
  document: ReviewDocument
  sourcePreviewHtml: string
  previewHtml: string
  assetPaths: Record<string, string>
  blockPreviews: Array<{ sourceHtml: string; blockers: string[] }>
  publicationError: string | null
}

export type ReviewEdit = { action: 'replaceMarks'; marks: Mark[] } | { action: 'reset' } | { action: 'approve' }
export interface ReviewEditRequest { expected: ReviewExpected; block: ReviewBlockKey; edit: ReviewEdit }
