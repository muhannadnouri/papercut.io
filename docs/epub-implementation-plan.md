# EPUB Upload, Reader, Search, And Audiobook Plan

This document records the EPUB implementation path and remaining follow-up work.
PDF now uses the same document/search contract through its own adapter and is
tracked separately in [pdf-ocr-scanning.md](pdf-ocr-scanning.md). The first EPUB ship is a
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
  sections: [{ ordinal, heading?, text }]
}
```

For the first EPUB release, `view_html` is a sanitized, generated reading copy
assembled from the EPUB spine. The original EPUB archive is not retained by the
current MVP; a future richer viewer can add that storage deliberately. Search/TTS
should not depend on rendering the original archive in React. The persisted
section ordinal now doubles as the reflowable reader locator, so a second locator
field would duplicate state without improving navigation.

## Implemented MVP

The MVP path is implemented with generated reading HTML, SQLite FTS indexing, Library import, existing TTS save/playback support, rewritten and target-validated internal EPUB links, retained local raster and external SVG images, app-owned DOM reader link scrolling, section-ordinal search targets, structured import-stage feedback, and fixture coverage for TOC links, cross-chapter links, EPUB 2 footnotes/backlinks, image manifest assets, generated section extraction, sanitizer regressions, missing-fragment fallback, and empty-spine rejection. Reader images are stored as separate content-hashed files, resolved through Tauri's scoped asset protocol, and marked for lazy loading and asynchronous decoding so the continuous reading page does not decode every illustration at startup. Search targets reuse the section ordinal already stored in SQLite and a generated safe DOM marker, avoiding a locator column migration while remaining compatible with older imports.

The EPUB parser is split into focused ZIP/XML parsing, path, asset, DOM rewrite, and render helpers. It uses crate-backed percent/base64 decoding plus DOM-based fragment rewriting. Current EPUB image retention covers supported local raster and external SVG images referenced by `img[src]` in retained reader content, plus supported inline SVG compositions converted to external images before sanitization, including image-only spine sections, under 5 MB per-image and 100 MB per-book limits. Older imports with inline raster data are upgraded non-destructively when opened. Newly imported EPUBs also resolve EPUB 3 `cover-image` properties and EPUB 2 `meta name="cover"` references, retain a declared raster cover under the existing 5 MB image cap, persist nullable cover media metadata, and generate a bounded gallery thumbnail. Existing imports remain valid without cover metadata; retained covers from earlier versions are thumbnailed lazily when first displayed.

## Image fidelity: staged repair

Stage 1 retains external SVG illustrations through the existing content-hashed
asset pipeline, including sanitization, scoped image URLs, and library transfer.
SVG bytes are rendered only as external HTML `img` resources, never inserted into
the reader DOM or embedded as an object/frame. This relies on the WebView's SVG
image processing mode to disable scripts and external resource loading; it does
not sanitize or flatten the SVG itself. Internal styles, text, and vector detail
are preserved; SVGs requiring external fonts, styles, or nested files still need
resource resolution in a later stage. Existing path validation, lazy loading, and 5 MB per-image / 100 MB
per-book limits still apply. Gallery thumbnails remain raster-only. Transfer
packages containing SVG assets require an updated recipient; older versions
reject their unsupported asset names.

Stage 2 converts inline SVG in well-formed XHTML into external SVG assets before
HTML sanitization. It preserves the viewport, aspect-ratio rules, composition,
internal CSS, inherited namespaces, root anchors, and available title/aria labels.
Local manifest-declared raster images referenced by `href` or `xlink:href` are
embedded as bounded data URLs inside the SVG; existing base64 raster data URLs
also work. Nested inline SVG stays within its parent composition. Nothing is
inserted as active SVG markup in the reader DOM, and no new dependency is needed.
An SVG `image` element with neither `href` nor `xlink:href` is skipped during
resource embedding, preserving the rest of the composition. A regression test
checks that vector content survives and later local raster references are still
embedded. Present but unresolved image references still reject conversion.

Inline and manifest assets share deduplication and the existing per-image and
per-book limits. The generated SVG must fit the per-image cap **after** base64
expansion. XHTML doctypes are accepted without fetching external DTDs; entity
declarations and malformed XML keep the old sanitized HTML fallback. Unresolved
image references, nested external SVG image files, and `xml:base` are not converted.
External CSS/font dependencies and publisher styles inherited from outside the
SVG remain unsupported.

Stage 3 reports unavailable images during import and in the reader. The stored
reading HTML keeps a locale-neutral reason on each omitted `img`, including an
unconverted inline SVG. Reasons distinguish missing or undeclared files,
unsupported formats, remote resources, invalid paths, per-image and aggregate
limits, failed SVG conversion, and otherwise unreadable images. Counts represent
image occurrences, not unique files. Image-only chapters with failures remain in
reading order when the book contains readable text or a retained image elsewhere.
A book containing only unavailable images still fails import.

The import result includes counts by reason. Library import shows document names
and explanations and leaves that result visible until dismissed. These counts
are derived from generated HTML without a schema migration; normal library
listings do not scan source files. Duplicate EPUB imports read the existing
source's diagnostics without reparsing the original archive. Markers also survive
source sanitization and library transfer.

Chapter sanitization owns the sequence: normalize XHTML empty elements once,
annotate image omissions, then sanitize HTML. Callers pass the chapter body
directly so diagnostics cannot be erased before annotation. Import dismissal and
the Library summary use named warning checks and totals for readability.

The reader replaces omitted or failed images with localized, accessible
placeholders that preserve alternative text and anchors. CSS-generated labels
avoid adding text nodes to search, bookmark, or narration offsets. Both already
failed images and later load/decode failures are handled, while pending lazy
images remain untouched. Explanations are translated in all eight app languages.

This reports the supported image pipeline, not full EPUB visual equivalence.
CSS backgrounds, object-based images, and missing dependencies inside an external
SVG are not audited. A successfully decoded SVG can still lack external fonts or
resources. Old imports can show generic placeholders for surviving source-less
images, but markup erased by an older parser cannot be diagnosed retroactively.
Runtime decode failures appear in the reader, not the earlier import summary.

The AI Agents in Depth sample now retains all 15 reading-order documents and all
115 images on a fresh parse: 112 external SVG illustrations, two PNG illustrations,
and one generated SVG containing the original JPEG cover. The cover's bytes,
viewport, and aspect-ratio rule are checked. The sample is not committed; its
optional regression check is:

```sh
cd src-tauri
cargo test --lib ai_agents_sample_retains_all_images_and_chapters -- --ignored
```

Local Linux WebKitGTK verification decoded all 115 images from the actual parser's
generated reading HTML, and the cover was visually checked. A separate SVG probe
decoded while its script, event handler, external stylesheet, and nested external
image made no requests. This was an isolated rendering check, not an end-to-end
Papercut import or a Windows/mobile WebView check; those platforms still need
release validation. Existing app data was left unchanged.

Stage 3 verification covers parser omission reasons and size budgets, sanitizer
round trips, warning-result dismissal behavior, and the sample's 115 retained
images with no import warnings. The runnable reader check at
`scripts/fixtures/reader/image-issues.html` uses the actual image observer and
translations. Run `npm run dev` and open that path on the Vite server; it reports
pass/fail for omitted, unresolved, cached, and live image failures, accessible
labels, locale changes, unchanged text offsets, and listener cleanup. It also
passed in Linux WebKitGTK. This is not a full Papercut import UI test.

Next: manually verify the import summary and reader together using an EPUB with
deliberately missing images, then perform platform smoke tests before release.
The [image diagnostics test book and acceptance steps](../scripts/fixtures/epub/README.md)
cover nine import omissions, one runtime decode failure, repeated references,
and an image-only chapter alongside two working SVGs.
Extend format coverage when another real book demonstrates a gap.

Reprocessing and retaining original archives are explicitly deferred for the
current single-user setup. Identical uploads return the existing entry, so
upgrading or re-uploading alone does not restore images from an older import.
Delete/reimport remains the manual option when losing that document's user state
is acceptable. Revisit in-place repair when preserving bookmarks or other user
state becomes necessary, or when supporting additional users makes manual repair
impractical.

## Remaining Follow-Ups

1. Keep durable metadata changes as explicit schema migrations; cover metadata introduced schema version 3 without rebuilding existing search or organization rows.
2. Add more EPUB parser fixtures for malformed OPF/container cases, spine edge cases, oversized image skipping, and metadata fallback.
3. Include retained covers in library-transfer packages now that gallery cover serving is stable.
4. Source-hash duplicate detection already returns existing records; in-place replacement remains deferred as described above.
5. Revisit reindex/reprocessing when preserving existing document state makes manual reimport insufficient.
6. Add determinate chapter/page progress only if import work exposes trustworthy units; current semantic stages deliberately avoid fake percentages.
7. Add richer EPUB reader features such as TOC, location restore, pagination, EPUB-specific appearance controls, or a foliate-js/epub.js-backed viewer if generated reading HTML is not enough. App-wide Light/System/Dark theme already applies to the generated HTML reader.
8. For very large books, move from one fully-rendered generated HTML document toward chapter/page-level rendering with locator-aware Find and TTS ranges. Current TTS caches are mutation-aware, but the next highlight after a large DOM mutation can still rebuild the active reader text index.
9. Keep the implemented PDF adapter on the shared SQLite document/search schema; remaining PDF/OCR work is tracked in [pdf-ocr-scanning.md](pdf-ocr-scanning.md).
10. Decide whether Pagefind remains the bundled-document engine long term or whether all documents should eventually share SQLite FTS.

## Historical Task Notes

The sections below record the implementation shape and acceptance checks used for the MVP. They remain useful when evaluating regressions or planning PDF/richer-reader follow-up work.

### 1. Generalize Upload Parser Types

- Add a generic parsed-document module under `src-tauri/src/document_uploads/`
  such as `parsed.rs`.
- Move `ParsedSection` out of `html/mod.rs`.
- Replace `ParsedHtmlDocument` with `ParsedDocument`.
- Add `format: DocumentFormat` or a constrained string value (`html`, `epub`,
  later `pdf`).
- Keep the ordinal stable so it can serve as the reflowable locator; do not add
  a second locator field unless a future viewer needs a different durable key.
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
  unsafe URLs. Retain referenced local PNG, JPEG, GIF, WebP, and SVG manifest images
  as content-hashed app-data assets within parser caps. Convert supported inline
  SVG compositions to external image assets with embedded local rasters before
  sanitization; skip CSS images, `srcset`, remote images, and oversized assets.
- Retain supported image-only HTML spine sections; ignore unsupported non-HTML
  spine resources for the first pass.

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
- Reuse the persisted section ordinal as the locator and map it to a generated,
  sanitizer-validated reader marker without adding a redundant SQLite column.
- Consider showing chapter title in `sub_results` for EPUB matches.
- Leave exact phrase unification as a separate search-quality task unless EPUB
  phrase behavior becomes visibly inconsistent.

Acceptance checks:

- EPUB results rank through SQLite FTS BM25.
- Snippets are sanitized before React rendering.
- Search remains explicit-submit only.
- Opening a result targets its indexed section before matching snippet text.

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

### 10. Defer Rich EPUB Reader; Share The PDF Contract

Richer EPUB reader:

- Retained EPUB 2/3 cover assets are served only to visible Library gallery cards through a narrow validated command. The command returns persisted display-sized thumbnails and serializes lazy thumbnail backfills for older imports so original high-resolution covers cannot create a burst of concurrent decodes. Gallery thumbnails are served separately from generated reader HTML. A cover page present in the EPUB spine is retained in reading order, including supported inline SVG wrappers.
- Evaluate foliate-js, `epub.js`, or Readium only after normalized import ships.
- Keep search/TTS source independent from the renderer.
- Add TOC, pagination, EPUB-specific appearance controls, and location restore as reader-quality work. App-wide Light/System/Dark theme already applies to the generated HTML reader.
- Keep Arabic typography script-aware. Bundled Arabic-focused fonts should remain explicit reader choices unless a future per-script font setting proves safe across mixed-language books.
- For very large EPUBs, bound reader work by rendering/indexing the active chapter instead of one generated DOM for the whole book. TTS chunk highlighting should then map through stored locators rather than scanning every rendered text node after a large mutation. New `.papercut-audiobook` exports preserve optional chunk source spans for the generated reader DOM, and older imports keep a cached live-DOM text-match fallback for legacy compatibility. PDF already uses page virtualization and page/text-item locators.

PDF:

- PDF reuses the shared SQLite document/FTS store with page records.
- PDF stores page/text-item locators instead of chapter locators.
- PDF uses PDF.js for viewer rendering and text extraction rather than forcing
  page visuals into generated HTML.

## External References

- W3C EPUB 3.3: `https://www.w3.org/TR/epub-33/`
- Ammonia sanitizer: `https://docs.rs/ammonia/latest/ammonia/`
- Kuchiki parser: `https://docs.rs/kuchiki/latest/kuchiki/`
- epub.js: `https://github.com/futurepress/epub.js`
- Readium CSS: `https://github.com/readium/css`
- Readium TS toolkit: `https://github.com/readium/ts-toolkit`
- PDF.js: `https://github.com/mozilla/pdf.js`
