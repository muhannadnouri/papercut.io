# EPUB Upload, Reader, Search, And Audiobook Plan

This document records the EPUB implementation path and remaining follow-up work
while keeping the path open for later PDF support. The first EPUB ship is a
normalized import path: parse EPUB natively, store a sanitized generated reading
HTML copy, index chapter/block sections into SQLite FTS, and let the existing
reader/TTS surface open that stored reading copy. A richer EPUB-specific viewer
can follow without rewriting search or audiobook generation.

## Product Goal

The implemented MVP lets users:

1. Import a local `.epub` file from the Library import menu.
2. See the imported book under User Uploads with title and storage metadata.
3. Open and read the book offline.
4. Search EPUB text through the same search UI as bundled and HTML documents.
5. Save and play an audiobook from the EPUB with existing native TTS controls.
6. Delete the uploaded EPUB and its search/source files from local app data.

## Design Decision

Implement EPUB as a new format adapter in the existing upload pipeline before
adding a custom reader experience. The adapter should emit the same boring shared
shape used by search and TTS:

```text
ParsedDocument {
  title,
  format,
  view_html,
  sections: [{ ordinal, heading?, text, locator? }]
}
```

For the first EPUB release, `view_html` is a sanitized, generated reading copy
assembled from the EPUB spine. The original EPUB archive is not retained by the
current MVP; a future richer viewer can add that storage deliberately. Search/TTS
should not depend on rendering the original archive in React.

## Implemented MVP

The MVP path is implemented with generated reading HTML, SQLite FTS indexing, Library import, existing TTS save/playback support, rewritten and target-validated internal EPUB links, retained safe local raster images, app-owned DOM reader link scrolling, and fixture coverage for TOC links, cross-chapter links, EPUB 2 footnotes/backlinks, image manifest assets, generated section extraction, sanitizer regressions, missing-fragment fallback, and empty-spine rejection.

The EPUB parser is split into focused ZIP/XML parsing, path, asset, DOM rewrite, and render helpers. It uses crate-backed base64/percent decoding plus DOM-based fragment rewriting. Current EPUB image retention covers supported local raster images referenced by retained reader content; manifest covers that are not referenced by the spine are a future reader-polish item.

## Remaining Follow-Ups

1. Add schema versioning before durable metadata changes such as locators, source kind, original archive retention, or view-source layout changes.
2. Add per-section locator metadata so uploaded search results can jump to chapter/page locations.
3. Add more EPUB parser fixtures for malformed OPF/container cases, spine edge cases, oversized image skipping, and metadata fallback.
4. Detect EPUB 2/3 cover metadata and render a safe retained raster cover near the top of generated reading HTML, still respecting existing image caps and SVG skipping.
5. Add duplicate detection based on source hash so repeated imports can update or skip existing records.
6. Add a reindex action for uploaded documents if parser or sanitizer behavior changes after import.
7. Add import progress reporting for very large EPUB/PDF files.
8. Add richer EPUB reader features such as TOC, location restore, pagination, EPUB-specific appearance controls, or a foliate-js/epub.js-backed viewer if generated reading HTML is not enough. App-wide Light/System/Dark theme already applies to the generated HTML reader.
9. For very large books, move from one fully-rendered generated HTML document toward chapter/page-level rendering with locator-aware Find and TTS ranges. Current TTS caches are mutation-aware, but the next highlight after a large DOM mutation can still rebuild the active reader text index.
10. Add a runtime PDF import module later that extracts page text and stores page records in the same SQLite schema.
11. Decide whether Pagefind remains the bundled-document engine long term or whether all documents should eventually share SQLite FTS.

## Historical Task Notes

The sections below record the implementation shape and acceptance checks used for the MVP. They remain useful when evaluating regressions or planning PDF/richer-reader follow-up work.

### 1. Generalize Upload Parser Types

- Add a generic parsed-document module under `src-tauri/src/document_uploads/`
  such as `parsed.rs`.
- Move `ParsedSection` out of `html/mod.rs`.
- Replace `ParsedHtmlDocument` with `ParsedDocument`.
- Add `format: DocumentFormat` or a constrained string value (`html`, `epub`,
  later `pdf`).
- Add optional `locator` metadata for future open-to-chapter/page behavior.
- Keep existing HTML import behavior byte-for-byte compatible where possible.

Acceptance checks:

- HTML import still stores, lists, searches, opens, saves audio, and deletes.
- `uploaded_documents.format` is no longer hardcoded to `html` in the store.

### 2. Add Schema Versioning Before New Metadata

- Add an explicit database metadata/version table.
- Keep current schema valid for existing user installs.
- Add migrations before adding `locator`, `source_kind`, `view_source`, or
  other durable fields.
- Keep delete atomic across metadata, sections, FTS rows, and source directory.

Acceptance checks:

- Fresh database creates latest schema.
- Existing HTML-only database migrates without re-import.
- Failed migration returns a clear import/search error instead of partial rows.

### 3. Split Stored Source Concepts

- Keep a viewable sanitized HTML file for every import format, named consistently
  (`reader.html` or `source.html`).
- Store the original EPUB archive only if needed for future viewer fidelity,
  debugging, or export. Do not render unsanitized archive content directly.
- Keep one stable document URL for app routing; store format separately so viewer
  choice is not forced by a file extension.
- Update frontend source loader names from `loadHtmlDocument` toward
  `loadDocumentSource` or `loadViewHtml`.

Acceptance checks:

- Existing uploaded HTML URLs continue to open.
- EPUB can open through generated reading HTML even if `EpubViewer` remains a
  simple wrapper or disabled for first release.

### 4. Add EPUB Parser Module

- Add `src-tauri/src/document_uploads/epub/`.
- Validate ZIP structure and reject encrypted/unsupported archives early.
- Read `META-INF/container.xml`, locate the OPF package document, then read
  metadata, manifest, and spine order.
- Resolve relative paths from OPF to XHTML spine items.
- Sanitize each XHTML spine document with a maintained sanitizer such as
  `ammonia`.
- Extract title from OPF metadata, then fallback to first heading, then filename.
- Extract ordered readable sections from headings, paragraphs, list items,
  blockquotes, and useful wrapper text.
- Assemble a generated reading HTML document with chapter anchors and minimal
  app-owned CSS. Prefix imported anchors by chapter so TOC, cross-chapter, footnote, and
  backlink links stay local to the generated reader document. Use a DOM walk
  for the post-sanitizer rewrite pass instead of a handwritten tag scanner.
  Validate fragment targets against collected chapter anchors; if a fragment is
  missing but the target chapter exists, fall back to the chapter wrapper anchor.
- Drop scripts, remote resources, inline event handlers, iframes, objects, and
  unsafe URLs. Retain local PNG, JPEG, GIF, and WebP manifest images as data
  URLs only within parser caps; skip SVG and oversized assets.
- Skip or clearly ignore non-text spine items for the first pass.

Library guidance:

- Prefer permissive crates compatible with the app's MIT license and Rust
  version floor.
- Use focused crates where they reduce handwritten parsing risk, and prefer
  actively maintained crates when choosing new parser dependencies. Current EPUB
  import uses `zip`, `roxmltree`, `ammonia`, `kuchikiki`, `base64`, and
  `percent-encoding`; `kuchikiki` is a post-sanitizer DOM walker, not the
  sanitizer/security boundary, and should remain replaceable if a maintained
  HTML mutation crate fits better later.
- Do not use a GPL EPUB parser crate unless the licensing decision is deliberate.
- Keep parser code unit-testable without Tauri app handles.

Acceptance checks:

- Reflowable EPUB imports and produces non-empty sections.
- Chapter order follows the OPF spine, not ZIP file order.
- Generated reading HTML contains no active scriptable content.
- Search snippets include EPUB text.
- Generated reader output feeds the same section extraction path used by search and TTS.
- TOC links, cross-chapter links, EPUB 2 footnotes/backlinks, sanitizer regressions, missing-fragment fallback, and local manifest images are covered by fixture tests.

### 5. Wire EPUB Import UI And Commands

- Route HTML and EPUB selections through the generic `document_uploads_import_batch` command.
- Keep the frontend API format-neutral through `importDocumentBatch`.
- Add **Import > Files** in `DocumentsPanel` for one or more HTML/EPUB files.
- Use the same progress, cancellation, and partial-failure state for both formats.
- Refresh the uploaded document list and open the document when exactly one selected file imports successfully.
- Keep audiobook bundle import separate from generic document import.

Acceptance checks:

- Cancelled picker shows cancelled status, not error.
- Import error messages name EPUB-specific failures clearly.
- Delete removes EPUB source directory and search rows.

### 6. Make Viewer Resolution Format-Aware

- Add `format` to `DocumentInfo` and pass it into `DocumentViewer`.
- Resolve viewer by document metadata first, URL fallback second.
- For MVP, let EPUB use the shared sanitized HTML viewer against generated
  reading HTML. The viewer renders into an app-owned DOM surface so links, Find,
  and TTS ranges share one scroll model.
- Keep `EpubViewer` available for a later richer renderer.
- If a custom EPUB viewer lands, make viewer capabilities explicit: find,
  scrolling, TTS highlight support, and locator navigation may differ by format.

Acceptance checks:

- HTML fallback remains unchanged.
- EPUB viewer choice does not require changing the search index URL.
- Find and TTS highlight work for the generated reading HTML MVP.

### 7. Make TTS Format-Adapter Friendly

- Rename HTML-specific audiobook helpers where they represent generic view HTML.
- Keep `chunkReadableSegments` as the shared TTS entry point.
- For EPUB MVP, derive chunks from generated reading HTML so current DOM-span
  highlighting remains valid.
- Future rich EPUB viewers may map chunks to EPUB CFI/locations, but that should
  be a second phase.

Acceptance checks:

- EPUB Save creates deterministic chunks.
- Existing model suggestion still works from chunk text.
- Saved EPUB audiobook reopens and plays from local WAV chunks.
- Highlight diagnostics report valid DOM ranges for generated reading HTML.

### 8. Improve Uploaded Search Locators

- Keep one search result card per uploaded document for the first pass.
- Store per-section locator metadata so future results can jump to chapter/page.
- Consider showing chapter title in `sub_results` for EPUB matches.
- Leave exact phrase unification as a separate search-quality task unless EPUB
  phrase behavior becomes visibly inconsistent.

Acceptance checks:

- EPUB results rank through SQLite FTS BM25.
- Snippets are sanitized before React rendering.
- Search remains explicit-submit only.

### 9. Add Tests

Rust unit tests:

Covered now:

- Manifest path resolution.
- TOC link rewriting.
- Cross-chapter link rewriting.
- EPUB 2 footnote and backlink rewriting.
- Missing-fragment fallback to chapter anchors.
- Local manifest image retention.
- Generated section extraction.
- DOM rewrite and sanitizer regression coverage for scripts, event handlers, unsafe links, and remote resources.
- Empty-spine text rejection.

Still useful:

- OPF/container parsing.
- Spine order.
- Missing metadata fallback.
- Empty/unreadable EPUB rejection.
- Delete/source cleanup helpers where practical.

Manual smoke tests:

- Import small public-domain EPUB.
- Search a known phrase.
- Open from search result and Library.
- Use in-document Find.
- Save audiobook.
- Play, pause, skip, and verify highlight.
- Delete upload and confirm search result disappears.

### 10. Defer Rich EPUB Reader And PDF Work

Richer EPUB reader:

- Detect EPUB 2/3 cover metadata and render safe retained raster covers in the generated reading HTML, even when the cover image is present only in the manifest and not referenced by a spine chapter.
- Evaluate foliate-js, `epub.js`, or Readium only after normalized import ships.
- Keep search/TTS source independent from the renderer.
- Add TOC, pagination, EPUB-specific appearance controls, and location restore as reader-quality work. App-wide Light/System/Dark theme already applies to the generated HTML reader.
- Keep Arabic typography script-aware. Bundled Arabic-focused fonts should remain explicit reader choices unless a future per-script font setting proves safe across mixed-language books.
- For very large books, bound reader work by rendering/indexing the active chapter or page instead of one generated DOM for the whole book. TTS chunk highlighting should then map through stored locators rather than scanning every rendered text node after a large mutation. New `.papercut-audiobook` exports preserve optional chunk source spans for the generated reader DOM, and older imports keep a cached live-DOM text-match fallback for legacy compatibility. Future EPUB/PDF work should still add format-aware chapter/page locators so imports and playback do not depend on one fully rendered document.

PDF:

- Reuse `ParsedDocument` and SQLite FTS sections.
- Store page-based locators instead of chapter locators.
- Use PDF-specific viewer and text extraction; do not force PDF visual rendering
  into generated HTML.

## External References

- W3C EPUB 3.3: `https://www.w3.org/TR/epub-33/`
- Ammonia sanitizer: `https://docs.rs/ammonia/latest/ammonia/`
- Kuchiki parser: `https://docs.rs/kuchiki/latest/kuchiki/`
- epub.js: `https://github.com/futurepress/epub.js`
- Readium CSS: `https://github.com/readium/css`
- Readium TS toolkit: `https://github.com/readium/ts-toolkit`
- PDF.js: `https://github.com/mozilla/pdf.js`
