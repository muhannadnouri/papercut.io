# PDF, OCR, And Document Scanning Plan

Status: Stage 8 in progress; Android restart recovery, native photo-picker, and iOS physical validation open
Last updated: 2026-08-04

This document is the source of truth for adding PDF reading, searchable OCR,
and mobile document scanning to Papercut. It records the research, current
architecture constraints, decisions, risks, and staged delivery checklist.

Update this file after every stage:

1. Check completed work and record validation evidence.
2. Record decisions in the decision log.
3. Correct assumptions invalidated by implementation or testing.
4. Update risks, deferred work, and the next stage.
5. Do not start the next stage until the current decision gate passes.

## Product Goal

Papercut should eventually let users:

- Import and read text-native PDFs offline.
- Search PDF text and open a result at the correct page and location.
- Use Find, TTS, highlighting, bookmarks, and location restoration with
  behavior comparable to HTML and EPUB.
- Import image-only or hybrid PDFs and create a searchable text layer through
  on-device OCR.
- On mobile, scan one page or a multi-page document, review the captured pages,
  and save them as one searchable Papercut document.
- Retain the original PDF or page images so OCR can be retried without losing
  the source.

Feature parity does not mean pretending a PDF is HTML. Papercut should preserve
the PDF's fixed-layout pages while deriving the text and coordinates needed for
search and TTS.

## First PDF Release

The first release is deliberately limited to text-native PDFs that already
contain usable text.

Included:

- Import, duplicate detection, deletion, and local storage.
- Original fixed-layout page rendering.
- Page navigation, zoom, fit-width, and fit-page.
- Document outline navigation when an outline exists.
- Page-level SQLite FTS indexing and search-result navigation.
- In-document Find across the complete document.
- TTS generation, playback, and page-coordinate highlighting.
- Page/location restoration and a first-page Library thumbnail.
- Library transfer compatibility.

Not included:

- OCR, camera scanning, or image imports.
- Password entry or PDF decryption. The first release detects and clearly
  rejects encrypted/password-protected files.
- PDF editing, forms, annotations, signatures, printing, or text correction.
- Reflowed/simplified HTML reading mode.
- Exporting or creating PDFs.
- Desktop webcam capture.

These exclusions avoid building OCR and authoring features before Papercut has
a proven page-aware reading path.

## Scope Decisions

These are the working decisions. Ask before changing one because each affects
storage, compatibility, and the order of implementation.

1. **Implement text-native PDF support before camera scanning.** PDF establishes
   the page model, viewer, search locators, and TTS mapping that OCR will reuse.
2. **Do not convert PDF or scanned pages into canonical HTML.** Preserve the
   original PDF or images and create a derived page text layer.
3. **Use a real PDF viewer.** HTML/EPUB can keep the shared DOM reader; PDF
   should render original pages with PDF-specific navigation and highlighting.
4. **Keep camera capture mobile-only for the first release.** Desktop users can
   import existing PDFs or images. Desktop webcam scanning is deferred until
   there is demonstrated demand.
5. **Keep processing offline and on-device.** No cloud OCR or account is
   required.
6. **Treat “unlimited pages” as a resumable workflow, not an unbounded in-memory
   operation.** Capture and processing must use bounded batches and durable
   intermediate state.
7. **Do not release PDF as complete until Find, search-result navigation, and
   TTS highlighting meet the agreed acceptance criteria.**
8. **Use Tesseract.js as the first common OCR engine.** Keep it offline, lazy,
   and behind the shared page-text contract. Adjust or replace it only when
   real documents demonstrate a quality, language, or device-performance gap.
9. **Classify conservatively before starting OCR.** A finalized PDF needs
   recognition when at least one image-backed page lacks usable native text.
   Empty pages without image operations remain blank; ordinary native pages
   avoid image-operator inspection after their extracted text passes the
   bounded prose heuristic.

## Recommended Architecture

```text
Original PDF or scanned pages
├── PDF viewer renders the original pages
└── Extractor or OCR produces PageTextLayer
    ├── SQLite FTS search records
    ├── Find and search-result navigation
    ├── TTS readable segments
    └── Page-coordinate highlights
```

The original source is canonical. Text extraction and OCR are derived data that
can be rebuilt when parsers, OCR models, or indexing rules change.

### Page Text Layer

Stage 2 establishes this versioned, per-page JSON sidecar:

```text
PageTextLayer {
  schema_version,
  page_index,
  width,
  height,
  blocks: [{
    text,
    bounds,
    order,
    confidence?
  }]
}
```

Each page is stored independently below the upload directory and is capped at
4 MB, 50,000 blocks, and 2 MB of text. Dimensions, bounds, and optional
confidence values must be finite. The sidecars are derived data: the original
PDF remains canonical and corrupt, stale, or missing sidecars are rebuilt.

SQLite should store page-level searchable text and page locators. Richer block
coordinates can live in a bounded sidecar file so the FTS database does not
become a layout-data store.

Search results should identify at least the document, page, and matching text.
TTS chunks should retain enough source mapping to convert a spoken range back
to one or more page-coordinate rectangles.

### Viewer Capabilities

Avoid rewriting the whole reader before PDF support. Introduce only the
capabilities needed to let the existing reader shell coordinate different
viewer types:

```text
navigateToLocator(locator)
highlightRange(range)
clearHighlight()
```

HTML and generated EPUB HTML keep their DOM ranges. PDF uses page and coordinate
locators. Any broader viewer interface should wait until a second implementation
demonstrates a real need.

## Current Papercut Architecture

The existing code is close to supporting another import format, but several
assumptions must change deliberately:

- `src-tauri/src/document_uploads/parsed.rs` exposes a format-neutral
  `ParsedDocument` and ordered `ParsedSection` with an optional page locator.
- `src-tauri/src/document_uploads/store.rs` now stores explicit source kind and
  an optional page index beside each section. PDF imports populate one FTS row
  per page; block coordinates remain in bounded PDF sidecars.
- `src-tauri/src/document_uploads/pipeline.rs` keeps HTML/EPUB reader sources as
  `source.html`. `src-tauri/src/document_uploads/pdf/` owns canonical
  `source.pdf`, bounded page-text sidecars, and atomic page-index finalization.
- `src/pdf/pdfImport.ts` lazy-loads the selected PDF.js parser, extracts one page
  at a time, and sends only that page's text layer across IPC. Rust retains the
  original PDF and commits searchable rows only after every expected page
  sidecar validates.
- Library transfer package v3 carries original PDF sources and deliberately
  omits rebuildable page-text sidecars. Existing v1/v2 HTML packages remain
  accepted.
- `src/viewers/PdfViewer.tsx` uses PDF.js's rendering queue and bounded page
  cache for a scrollable, selectable document surface. Stored PDFs are exposed
  only through a validated, app-data-scoped Tauri asset URL so PDF.js can issue
  range requests instead of copying the full source through IPC.
- `src/hooks/useDocumentViewerState.ts` and
  `src/components/DocumentViewer/DocumentViewer.tsx` assume a loaded HTML string
  and one live reader DOM.
- `src/tts/utils/text.ts` already provides the reusable,
  format-neutral `chunkReadableSegments` entry point. A PDF adapter should emit
  readable segments rather than duplicate chunking rules.
- `src/tts/hooks/useAudiobookManager.ts` currently treats opened documents as
  non-PDF.
- The Library gallery already treats future PDF records as books and can use a
  first-page thumbnail when cover metadata is unavailable.

The smallest sound refactor is therefore source-kind storage, page locators,
and narrow viewer capabilities. A broad document subsystem rewrite is not a
prerequisite.

### OCR Readiness Preflight

The first OCR preparation pass adds one persisted document-level text status to
the existing upload metadata rather than introducing an OCR subsystem:

- `processing` while PDF.js is building page sidecars and the shared index.
- `ready` when a finalized document contains searchable text.
- `recognition-required` when a finalized PDF contains no non-whitespace page
  text or at least one image-backed page lacks usable native text.

Schema version 6 backfills previously finalized, fully textless PDFs from their
existing SQLite page rows. The Library gallery and uploaded-document list show
**Text Recognition Required** so a rendered scan no longer looks searchable.
The original PDF remains canonical and no duplicate index or compatibility
format is added by this pass.

Hybrid readiness is computed from PDF.js text and image operators during the
bounded import pass. The document stores only the aggregate status; recognition
recomputes the same page decision from the canonical PDF, leaves usable native
and blank page sidecars untouched, and replaces only image-backed weak-text
pages. If English OCR still returns no text for one of those pages, the
document remains recognition-required after the successful partial rebuild.
Page-level provenance remains deferred until language/model metadata needs a
durable representation.

## Stage 0 Baseline

### Supported Platform Baseline

Papercut currently builds for Linux, Windows, Intel/Apple Silicon macOS,
Android, and iOS. The configured minimums relevant to this work are:

| Platform | Current baseline |
| --- | --- |
| Android | API 26; universal release builds currently cover arm64, armv7, x86, and x86_64 |
| iOS | iOS 14 |
| macOS | macOS 10.13 |
| Rust | 1.88 declared minimum; CI builds with stable |
| Frontend | React 19, Tauri 2, and the platform WebView |

Passing desktop development tests alone is insufficient. Every Stage 1
candidate must build for Android and iOS before it can become the production
choice. The declared Rust minimum may be raised when a selected dependency
provides enough value to justify it; it is not a reason to reject a spike
candidate before testing.

### Existing Behavior To Preserve

The PDF work must not regress these HTML/EPUB behaviors:

| Capability | Current baseline |
| --- | --- |
| Import | HTML and EPUB share one bounded batch pipeline with duplicate detection, cancellation between files, partial results, and atomic cleanup |
| Search | Explicit-submit Pagefind and SQLite FTS queries return one shared result shape |
| Search navigation | Results can open a document and target matching reader text |
| Find | Matches normalized text across adjacent inline formatting nodes without highlighting surrounding paragraph text |
| TTS | Format adapters feed `ReadableSegment` values into shared chunking; saved audio can reopen and highlight mapped source ranges |
| Reader state | Reader appearance and location are app-owned state rather than changes to stored source |
| Library | Uploaded formats share listing, folders, Saved Audio filtering, deletion, and Gallery/List views |
| Transfer | Uploaded source and selected saved audiobooks can move between Papercut devices and rebuild derived search data |

The current frontend baseline is 10 passing Vitest files and 21 passing tests as
of 2026-07-25. The sandbox shell lacks the `javascriptcoregtk-4.1` development
package, so focused Rust tests run through the host toolchain. CI remains the
required full Rust and mobile build baseline.

### Fixture Corpus

Use two validation tiers:

1. **Committed synthetic fixtures** are small, license-safe, deterministic, and
   suitable for automated parser/search tests.
2. **Manual acceptance fixtures** may be larger or externally sourced. Record
   provenance, license, checksum, page count, and expected behavior, but do not
   commit copyrighted or oversized files.

Do not hand-maintain opaque binary fixtures when a short generator can produce
the same case. OCR-specific generated benchmark artifacts were removed after
engine selection; P09/P10 remain behavioral acceptance cases.

| ID | Fixture | Required assertion |
| --- | --- | --- |
| P01 | Basic Latin text, headings, and links | Text order, metadata fallback, links, page count, Find, and search target are correct |
| P02 | Inline font/style changes inside one phrase | Find and search match only the requested phrase across spans |
| P03 | Arabic RTL text with diacritics | Extraction order, glyph placement, search, TTS text, and highlight coordinates remain correct |
| P04 | Hindi/Devanagari text | Unicode extraction, search, and highlighting remain correct |
| P05 | Simplified Chinese text | Unicode extraction, search, and highlighting remain correct |
| P06 | Two-column page | Reading order completes the first column before the second |
| P07 | Body text with footnotes | Body and note order is deterministic and page navigation targets the correct region |
| P08 | Table plus surrounding paragraphs | Table extraction does not reorder or duplicate surrounding prose |
| P09 | Image-only pages | Import persists `recognition-required` and does not present the document as searchable |
| P10 | Hybrid native-text and image-only pages | Native pages are retained and only missing pages are marked for later OCR |
| P11 | Encrypted/password-protected PDF | Import rejects early with a specific, non-destructive error |
| P12 | Truncated or malformed PDF | Import fails within limits and leaves no source or index residue |
| P13 | 100-page performance document | Measures normal import, first-page render, Find, navigation, memory, and cleanup |
| P14 | 500-page stress document | Verifies bounded page rendering, cancellation, and absence of unbounded memory growth |
| O01 | Clean photographed Latin page | Establishes OCR character accuracy and coordinate baseline |
| O02 | Noisy/skewed Arabic page | Measures recognition, reading order, diacritics handling, and confidence |
| O03 | Chinese and Devanagari pages | Prevents selecting an OCR engine based only on Latin accuracy |
| O04 | Multi-page interrupted scan | Verifies durable capture, resume, reorder, delete, and retry behavior |

### Functional Acceptance Criteria

- Importing an unchanged PDF returns the existing document rather than creating
  another copy.
- A failed, cancelled, malformed, or encrypted import leaves no orphaned source,
  thumbnail, text layer, or SQLite rows.
- Search results identify the correct page and highlight only the matching text.
- Find searches every page, reports the correct occurrence count, supports
  next/previous navigation, and does not require all pages to be rendered.
- Text extraction never silently duplicates or drops golden-fixture paragraphs.
- TTS narration follows the fixture's expected reading order and active
  highlighting stays on the spoken words.
- Closing and reopening restores the saved page/location within one visible
  text block.
- PDF source, metadata, and organization survive Library Transfer; derived FTS
  and thumbnails may be rebuilt.
- Existing HTML/EPUB import, search, Find, TTS, saved audio, and transfer tests
  continue to pass.

### Performance Budgets

These are initial engineering budgets, not marketing claims. Record the exact
hardware, OS, build type, fixture checksum, and cold/warm state with every
measurement.

| Measure | Text-native PDF target |
| --- | --- |
| First visible page after opening | <= 2 seconds on reference desktop; <= 3 seconds on reference mobile |
| Navigate to an already extracted page | Feedback begins within 100 ms; target page becomes visible within 1 second |
| Visible page window | At most the visible pages plus two adjacent pages in each direction |
| Blank page while ordinary scrolling | No visible blank page lasting more than 500 ms after scrolling stops |
| 100-page import/index | <= 10 seconds desktop; <= 20 seconds mobile |
| 500-page import/index | <= 60 seconds on each reference device or clear cancellable progress if the budget cannot be met |
| Peak PDF viewer memory above idle | <= 250 MB desktop; <= 150 MB mobile for P14 |
| Compressed application-size increase before OCR | <= 10 MB per shipped platform |
| Search/Find result correctness | 100% of golden fixture queries target the expected page and text |
| TTS text integrity | Zero omitted or duplicated golden-fixture blocks |

Before public release, measure one lower-end supported Android device, one
supported iPhone/iPad, and one desktop in each platform family used for release
smoke testing. A candidate that passes only on the development machine fails
the gate.

### OCR Acceptance Budgets

These are release targets for the selected engine, not a separate engine
selection project:

| Measure | Initial OCR target |
| --- | --- |
| Clean printed Latin character error rate | <= 2% |
| Clean printed Arabic, Chinese, and Devanagari character error rate | <= 5% per tested script |
| Golden-page block reading order | >= 95% correctly ordered blocks |
| Coordinate coverage | >= 98% of recognized non-whitespace text has usable bounds |
| Typical photographed page | <= 8 seconds per page on the lower-end reference mobile device |
| Cancellation | Stops after the active page without losing completed pages |
| Source integrity | Original page image remains byte-for-byte available after failure or retry |

Language-specific human review remains required. A character-error score alone
does not prove that generated speech is understandable.

## PDF Rendering And Extraction

### Rendering Recommendation

Use [PDF.js](https://github.com/mozilla/pdf.js) through `pdfjs-dist` for the
shared React viewer unless the spike reveals a blocking Tauri WebView issue.
PDF.js is mature, Apache-2.0 licensed, supports page rendering and text content,
and avoids owning separate Apple, Android, and desktop viewers.

Render only visible pages plus a small adjacent buffer. Recycle canvases and
text layers as pages leave that window. Loading every page canvas is not
acceptable for large books or mobile memory limits.

PDF pages should retain their original colors. Dark mode should theme viewer
chrome and the surrounding surface, not recolor document pixels by default.

Do not add `react-pdf` for the first implementation. It wraps PDF.js with
React `Document` and `Page` components but still requires the same worker,
decoder, font, and PDF.js options. Papercut also needs direct text-coordinate,
render-cancellation, virtualization, Find, and TTS-highlight control. Revisit
the wrapper only if those requirements can remain entirely behind its public
API and it demonstrably supports Papercut's minimum WebViews.

Native PDFKit is strong on Apple platforms, but choosing it as the primary
viewer would require separate Android and desktop implementations with different
selection and highlight behavior. That complexity is not justified for the
first PDF release.

### Extraction Candidates

The extractor must be selected by a fixture-driven spike:

- [`pdf-extract`](https://docs.rs/pdf-extract/latest/pdf_extract/) is a small,
  MIT-licensed option for page-separated text. Its public API is simple, but it
  does not provide the rich coordinate and reading-order model needed without
  additional work.
- [`pdf_oxide`](https://docs.rs/pdf_oxide/latest/pdf_oxide/) exposes spans,
  bounds, and reading-order features under MIT/Apache licensing, but failed the
  word-boundary checks described below and was removed after the spike.
- PDF.js can extract text through `getTextContent`, keeping rendering and
  extraction on one parser. Import must process bounded page batches so large
  documents do not create an unbounded WebView or IPC payload.

Use PDF.js for both rendering and text extraction. One parser avoids
renderer/extractor disagreement and does not add another production dependency.

### Stage 1 Spike Evidence

The first dependency comparison used Mozilla PDF.js fixtures for a 14-page
Latin technical paper and a one-page Arabic/RTL document. These are useful
parser probes, not a replacement for the full Stage 0 corpus or device testing.
Timings include process and parser startup on the development machine, so they
are directional rather than release benchmarks.

| Probe fixture | SHA-256 | Purpose |
| --- | --- | --- |
| PDF.js `tracemonkey.pdf` | `3662ff519e485810520552bf301d8c3b2b917fd2f83303f4965d7abed367e113` | Multi-page Latin reading order and coordinates |
| PDF.js `ArabicCIDTrueType.pdf` | `1cad1de912ba29f89a6d8b08bc5b0f84382874ffedab2c8f9e05ef608c265bb1` | RTL rendering, logical text, direction, and coordinates |
| PDF.js `pr6531_2.pdf` | `e85d22b832a61be1d302a811e29c5df5ccd2a3795633178449d0b2e0e2451118` | Missing-password detection |
| First 12,000 bytes of `tracemonkey.pdf` | `07fdfd318a374e1802af3a1bb697a8c0e0faea72b7625ee994bb665f81ca306f` | Truncated/invalid input rejection |
| Local 40-page image-decoding fixture | `e79694e2bab2a7b7e8e4db45cf7f46bcceea661343c80dd1ac2b851549e18b95` | JPEG 2000 page-image decoding and standard-font asset loading |
| Local 539-page large-book fixture | `a9d99e48f591a81db649dbfaa27b1b7edffec64a794de84a38545e92d0c98349` | Large-book extraction throughput and image-only first-page rendering |
| Generated inline-format and two-column fixture | `8dc8c66f00e035339bc0fd3c2863a34c81c050ee2a5ecc625499627b5fa38248` | Stable word boundaries across font changes, reading order, and finite coordinates |

| Candidate | Version / license | Evidence | Current disposition |
| --- | --- | --- | --- |
| PDF.js | `pdfjs-dist` 6.1.200, Apache-2.0 | Rendered the Arabic page to a 595x842 command-line canvas; exposed text items with transforms, dimensions, direction, and line boundaries; preserved the generated inline-style word boundary and the manual Arabic fixture's visible word separators; distinguished missing-password and invalid/truncated-PDF failures; and has an exposed first-page WebView spike with a selectable text layer | Selected renderer and extractor; hands-on mobile WebView validation is deferred |
| `pdf_oxide` | 0.3.75, MIT OR Apache-2.0, Rust 1.88 | Extracted finite spans quickly and cross-compiled for Android and iOS, but inserted a false space across the generated font-style boundary and fused visible words in the manual Arabic/RTL fixture. Its public extraction API does not expose the internal spacing configuration needed to tune both cases. | Rejected; fixing it would require Papercut-owned extraction heuristics while retaining a second parser and a 172-package Rust graph |
| `pdf-extract` | 0.12.0, MIT | Extracted Latin text with a small API and release probe, but reversed the Arabic fixture's logical order and exposes no direct coordinate model | Rejected for Papercut's multilingual page-aware requirements |

PDF.js and `pdf_oxide` reported matching page-space positions for corresponding
Latin and Arabic text within normal floating-point rounding, but matching
coordinates did not compensate for different extracted word boundaries.
Using PDF.js for both surfaces removes that split-parser failure mode.

Both candidates surfaced Arabic presentation-form characters in this fixture.
The production text layer must normalize extracted searchable text, while
retaining source-to-glyph mapping for highlights. Normalization must be covered
by Arabic search and TTS fixtures rather than applied as an untested global
string rewrite.

The costs are material but bounded:

- PDF.js installs as roughly 41 MB unpacked with the current npm resolution.
  Papercut's optimized build emits a 487 KB lazy renderer chunk, 225 KB lazy
  viewer stylesheet, 1.30 MB local worker, and a 5 KB text-selection cursor
  asset. Together these add about 2.02 MB minified, or 579 KB gzip. Dynamic
  imports keep the normal startup delta to about 2.4 KB JavaScript and 0.5 KB
  CSS before gzip. Runtime WASM decoders and standard fonts add another 2.21
  MiB to the package and are fetched only when a PDF requires them.
- The rejected `pdf_oxide` spike resolved 172 packages, produced about 3.0 GB
  of build artifacts, and yielded a 9,982,632-byte optimized probe. Removing it
  avoids raising Papercut's Rust minimum and carrying a second PDF parser.
- At the time of the extraction spike, `pdf-extract` and its transitive
  dependency resolution did not compile with Papercut's then-current Rust
  1.77.2 minimum. Raising the minimum later did not change its RTL and
  coordinate limitations.

Papercut now declares Rust 1.88 for unrelated production dependencies. The
rejected native spike still does not justify carrying a second PDF parser.

The native probe was fast and produced finite bounds, but throughput did not
pass the correctness gate. A deterministic one-page comparison showed a false
space inside a word split only by font styling. A separate manual Arabic/RTL
fixture also showed fused visible words. PDF.js preserved the expected word
separators in both comparisons, so the native spike and its lockfile were
removed.

The remaining PDF.js extraction check is intentionally small:

```sh
npm run test:pdf-extraction
```

It verifies the generated fixture's inline-style word boundary, two-column
reading order, and finite text coordinates. Real multilingual and large-file
documents remain local, uncommitted acceptance fixtures.

`npm run dev` and `npm run build:vite` copy PDF.js's installed WASM decoders
and standard fonts into generated local assets. The renderer does not fetch
runtime resources from a CDN. With those assets configured, the 40-page
JPEG 2000 benchmark renders a nonblank 430x695 first page and exposes its title
through the text layer. The temporary first-page WebView harness used for this
selection was removed after the production import and reader paths passed the
same checks. Hands-on Tauri mobile WebView validation remains explicitly
deferred rather than treated as a pass.

Use the production import and reader paths for the same smoke matrix on every
target:

1. The 40-page image-decoding fixture renders page one instead of a blank white
   page, and its title text can be selected.
2. The 539-page large-book fixture renders its image-only first page.
3. The Arabic fixture preserves its visual direction and selectable text.
4. Switching among the three files does not retain the previous canvas or
   produce an error.
5. The worker, OpenJPEG WASM decoder, and standard fonts load from the local
   application/dev-server origin rather than a CDN.

The optimized frontend build, TypeScript check, ESLint check, local worker
emission, and lazy chunk split pass. The desktop Tauri launch on this machine
is blocked before WebView startup by the missing `javascriptcoregtk-4.1`
development package, so desktop and mobile hands-on rendering remain open
instead of being inferred from the browser build.

An arm64 Android debug APK also builds successfully. Inspection of its embedded
Tauri asset table confirms the PDF worker, `openjpeg.wasm`, and standard fonts
are packaged. No Android device or emulator is available in the current
environment, and iOS execution requires macOS/Xcode, so neither packaging
result is recorded as a visual WebView pass. Changes to `package.json`
automatically schedule both mobile packaging jobs in Papercut CI.

### Tauri Data Boundary

Do not send PDF binaries or complete page-image sets through JSON/base64 IPC.
Stage 3 reads a selected source once through Tauri's raw binary response, then
sends bounded page-text payloads back one page at a time. Stage 4 should use a
scoped local asset boundary for repeated viewer access instead of repeatedly
copying the full source. Tauri's mobile plugin bridge is appropriate for the
later native scanner integration.

## OCR Strategy

OCR is a second pipeline layered onto the PDF page model:

1. Detect whether each page already has usable text.
2. Extract native text where available.
3. OCR only image-only or unusable pages.
4. Normalize both paths into `PageTextLayer`.
5. Store extraction/OCR provenance and version so derived data can be rebuilt.

An OCR failure must not delete or invalidate the original PDF or scan.

### Tesseract OCR Foundation

Papercut uses Tesseract.js 7 as the first shared OCR engine across its WebViews.
It runs Tesseract in a Web Worker, is imported only when OCR starts, and writes
recognized words, confidence, reading order, and scaled page coordinates into
the existing `PageTextLayer`. Search and TTS therefore reuse the existing
derived-data path; the viewer adds a page-local selectable overlay, while Find
still needs a sidecar-backed adapter because PDF.js only searches embedded text.

English trained data is pinned as an npm dependency and copied with the worker
and core into the ignored `public/tesseract/` build tree. Generated
`*.traineddata` files are not committed at the repository root; each future
language must be an explicitly pinned package selected by the same preparation
script so offline builds remain reproducible and only ship supported languages.

Build preparation copies the installed worker, SIMD-capable LSTM cores, and the
integer English trained data into ignored `public/tesseract` assets. Runtime
URLs are app-local; no CDN or account is required. One worker should be reused
for a complete job and terminated when the job finishes or is cancelled.

Only English data is included in this foundation. Automatic recognition must
not silently apply it to every document. The job UI must obtain a supported
language before OCR begins; additional trained-data packages can then follow
the same local asset pattern. Tesseract quality, photographed-page cleanup,
non-Latin language support, mobile speed, and package size remain acceptance
work. PaddleOCR and native engines are deferred unless real failures justify
their extra runtimes and platform-specific behavior.

The first production slice exposes **Recognize English Text** only for PDFs
already classified as fully textless. It reuses one worker across the document,
renders and persists one bounded page at a time, reports page progress, and
supports cooperative cancellation. Search rows and ready metadata are replaced
only through the existing atomic PDF finalizer, so cancellation or failure does
not expose partial OCR output or alter the canonical PDF. The finalizer also
preserves an existing gallery thumbnail when no replacement thumbnail is
provided by the recognition job.

## Mobile Scanning

Capture and OCR should remain separate:

- iOS uses
  [`VNDocumentCameraViewController`](https://developer.apple.com/documentation/visionkit/vndocumentcameraviewcontroller)
  for capture, edge guidance, crop, rotation, page review, and multi-page
  output. The isolated plugin streams those reviewed pages into an app-owned
  image-only PDF and hands only its path to Rust.
- Android uses CameraX 1.5.3 for capture and a Papercut-owned four-corner crop
  and review screen. Android's native perspective transform corrects reviewed
  pages without OpenCV, ML Kit, or Google Play Services. Automatic edge
  detection remains deferred until physical-device acceptance documents show
  that full-page default corners plus manual correction are inadequate.
- Do not use ML Kit Document Scanner as the primary Android path. Its scanner
  module and UI are delivered through Google Play Services, which Papercut
  cannot assume exists on every supported offline device.
- One local Tauri plugin normalizes both platforms into an app-owned PDF path.
  Native image bytes never cross the JSON bridge, and the existing PDF import,
  readiness, OCR, indexing, viewer, search, and TTS pipeline remains the only
  document-processing path.
- Existing photos use Android's system document picker and iOS PhotosUI. Both
  copy only the chosen files into plugin-owned temporary storage, normalize one
  image at a time, and emit the same app-owned PDF contract without broad photo
  permissions or Google Play Services.

The camera review step lets users inspect thumbnails, crop, rotate, delete,
reorder, and rescan. Existing-photo import preserves the framing and picker
order the user already chose instead of maintaining a second native editor;
photos that need correction should be edited before selection or captured with
the scanner. Source pages are committed into the canonical PDF before OCR
begins so recognition can be retried.

Scanner-specific code is intentionally removable:

```text
src-tauri/plugins/document-scanner/   native iOS/Android adapters
src-tauri/src/document_scanner/       app-owned staging and import bridge
src/document-scanner/                 typed React availability/capture API
```

Registration and the Library import option are the only integration points.
Deleting those directories and their registrations removes capture without
changing PDF import or OCR.

Desktop camera and direct-image capture are deferred. Importing existing PDFs,
including PDFs prepared from desktop images, covers the current desktop use case
without introducing webcam permissions or a third capture workflow.

## User Experience

The workflow should expose meaningful stages rather than one indefinite
spinner:

1. Preparing pages.
2. Recognizing page X of Y.
3. Indexing document.
4. Ready.

Long operations need progress, cancel, and a resumable state. Per-page failures
should identify the failed page and allow retry without discarding successful
pages. This follows Nielsen Norman Group guidance on
[visibility of system status](https://www.nngroup.com/articles/visibility-system-status/)
and [user control and freedom](https://www.nngroup.com/articles/user-control-and-freedom/).

The first PDF UI should include:

- Import through the existing Library import menu.
- Password/encryption and unsupported-file errors before indexing.
- Page number, previous/next page, zoom, fit-width, and fit-page controls.
- Outline navigation when the PDF contains one.
- Find across the document.
- Search-result navigation to the matching page and highlight.
- Loading and failure states that distinguish rendering from extraction.

The scan UI should be mobile-only and should not appear on unsupported devices.
Do not advertise “unlimited” pages; communicate that users can add pages in
multiple resumable batches.

## Security, Privacy, And Accessibility

- Treat every PDF and image as untrusted input. Enforce size, page-count,
  decompression, image, and processing-time limits.
- Reject or clearly handle malformed, encrypted, and password-protected files.
- Keep parser and renderer versions current because PDFs are a complex input
  boundary.
- Keep OCR and source files local unless the user explicitly exports them.
- Do not expose arbitrary local filesystem paths to the WebView. Scope viewer
  access to validated app-data files.
- Provide a selectable/accessibility text layer over PDF canvases.
- Preserve reading order and language metadata where known.
- Validate keyboard, screen-reader, high-contrast, zoom, RTL, and mobile touch
  behavior.
- Never use OCR confidence as proof that text is correct. TTS and search should
  identify OCR-derived content where correction may matter.

## Principal Risks

| Risk | Consequence | Mitigation |
| --- | --- | --- |
| Incorrect reading order | Search snippets and TTS read columns, tables, or footnotes incorrectly | Benchmark representative fixtures and retain ordered blocks, not one unexamined page string |
| Extractor/viewer disagreement | Search result or TTS highlight lands on the wrong glyphs | Compare extracted text/coordinates with PDF.js output and reject mismatched candidates during the spike |
| Mobile canvas memory | Crashes or disappearing pages during scroll | Virtualize pages, bound the adjacent-page window, and release canvases/text layers |
| OCR errors | Incorrect search, pronunciation, or omitted words | Preserve source images, confidence/provenance, per-page retry, and later correction tools |
| Native packaging size | Larger apps and difficult iOS/Android builds | Measure engines/models before selection and ship only required language assets |
| Parser vulnerabilities | Malicious documents affect the app | Strict limits, current dependencies, isolated app-data access, and malformed-file tests |
| Storage migration assumptions | Existing HTML/EPUB, transfer, or audiobook bundles break | Add explicit migrations and compatibility fixtures before changing source storage |
| Canvas-only viewer | Poor selection and screen-reader behavior | Maintain an aligned text/accessibility layer |
| Encrypted PDFs | Confusing failures or unsupported imports | Detect early and provide a specific message; password support is a separate product decision |
| Partial imports | Orphaned files or index rows | Stage writes and commit source, metadata, FTS, and derived artifacts atomically where possible |
| False-positive OCR classification | Blank covers or separator pages trigger needless OCR | Label only fully textless PDFs now; require corpus-backed page classification before hybrid OCR |

## Delivery Checklist

Legend: unchecked items are not started. Every stage ends with a decision gate.

### Stage 0: Baseline And Acceptance Criteria

Stage status: Complete

- [x] Define the text-native English, Arabic/RTL, Hindi/Devanagari, and Chinese
      fixture corpus.
- [x] Define multi-column, footnote, table, image-only, hybrid, encrypted,
      malformed, and large-document cases.
- [x] Define required text order, page targets, and highlight assertions.
- [x] Define import-time, memory, viewer-scroll, app-size, and OCR-accuracy
      budgets for desktop, Android, and iOS.
- [x] Define PDF MVP controls and explicitly defer nonessential features.
- [x] Confirm the scope decisions, fixture matrix, and budgets with the product
      owner.
- [x] Record baseline HTML/EPUB Find, search, TTS, bookmark, Library, and
      transfer behavior that PDF must not regress.

Decision gate: the corpus and measurable acceptance criteria are agreed before
selecting dependencies.

### Stage 1: Renderer And Extractor Spike

Stage status: Complete, with hands-on mobile WebView validation deferred to
Stage 10 by product-owner decision

- [x] Render representative PDFs with PDF.js in the browser harness and package
      the same assets in an Android build.
- [x] Render a representative RTL page with the selected PDF.js build in the
      command-line canvas probe.
- [x] Add an exposed first-page PDF.js WebView harness with a local file input,
      high-DPI canvas, selectable text layer, and effect cleanup.
- [x] Package PDF.js's local WASM decoders and standard fonts so JPEG 2000
      images and unembedded standard fonts render in development and packaged
      builds.
- [x] Verify the optimized frontend build emits a local worker and lazy-loads
      the renderer and text-layer CSS outside normal app startup.
- [x] Build an arm64 Android APK and verify its embedded Tauri assets include
      the PDF worker, OpenJPEG WASM decoder, and standard fonts.
- [x] Re-run the exposed browser WebView harness with the representative local
      40-page and 539-page PDFs and verify both first pages render.
- [ ] Verify worker loading, local asset access, canvas cleanup, text selection,
      and accessibility-layer behavior in target Tauri WebViews during Stage 10
      hardening.
- [ ] Run the PDF.js harness inside Android and iOS Tauri WebViews during Stage
      10 hardening. Hands-on mobile validation is intentionally deferred.
- [x] Compare `pdf-extract`, `pdf_oxide`, and PDF.js text extraction against the
      available fixture expectations.
- [x] Compare all three candidates on shared Latin and Arabic smoke fixtures.
- [x] Add a bounded, reproducible `pdf_oxide` extraction probe isolated from
      Papercut's Tauri build graph.
- [x] Measure preliminary text order, coordinate fidelity, throughput, package
      size, and native-candidate mobile cross-compilation.
- [x] Verify the preferred native extractor candidate compiles for Android
      arm64 and iOS arm64 on stable Rust.
- [x] Verify the native extractor rejects malformed non-PDF input.
- [x] Verify PDF.js distinguishes a missing password from a truncated/invalid
      PDF in the command-line probe.
- [x] Add a deterministic PDF.js check for inline-format word boundaries,
      two-column order, and finite coordinates.
- [x] Select PDF.js as the production extraction path and remove abandoned
      native spike code.
- [x] Record candidate versions, licensing, preliminary evidence, and rejected
      options
      in the decision log.

Decision gate: PDF.js meets the available corpus, licensing, packaging, and
performance evidence. Physical-device behavior remains a release-hardening
gate rather than blocking the storage foundation.

### Stage 2: Source Storage And Locator Foundation

Stage status: Complete

- [x] Add explicit source-kind metadata and store original PDFs as `source.pdf`
      without changing existing HTML/EPUB URLs.
- [x] Add a schema migration for page-aware section/search locators.
- [x] Define and version the bounded `PageTextLayer` sidecar format.
- [x] Update source loading so callers do not assume `source.html`.
- [x] Update delete, duplicate detection, disk accounting, and partial-import
      cleanup for PDF artifacts.
- [x] Update library transfer and relevant export/import paths for original PDF
      sources and derived-data rebuild rules.
- [x] Add migration and backward-compatibility tests for existing app data.
- [x] Run the manual HTML/EPUB import, search, delete, saved-audiobook, and
      library-transfer regression smoke test.

Automated evidence: the TypeScript build check passes, and the focused Rust
suite passes the upload, migration, search, cleanup, and PDF sidecar tests.
Library-transfer package tests cover v1/v2 compatibility, v3 PDF source
round-tripping, checksum enforcement, and rejection of PDFs mislabeled as
legacy packages.

Decision gate: existing HTML/EPUB documents, saved audiobooks, deletion, and
library transfer pass unchanged while a fixture PDF can be stored and located.

### Stage 3: Text-Native PDF Import And Search

Stage status: Complete

- [x] Add PDF validation, metadata/title extraction, page count, and size limits.
- [x] Extract ordered page text and page locators outside the React render path.
- [x] Index page-level text in SQLite FTS without storing binary data in SQLite.
- [x] Return page-aware search results and sanitized snippets.
- [x] Keep quoted PDF search index-backed, then verify normalized literal
      phrases only in the bounded FTS candidate set.
- [x] List indexed PDFs in the shared Library and Search document tree.
- [x] Generate a bounded first-page thumbnail for the Library gallery.
- [x] Add bounded import progress, between-page cancellation, clear failures, and
      cleanup of failed or cancelled staged PDFs.
- [x] Add parser/index tests for the committed text-native Stage 0 corpus;
      retain external multilingual, malformed, encrypted, and large fixtures
      for the later release-hardening matrix.

Automated evidence through the preceding checkpoint: the production frontend
build, PDF.js extraction fixture, TypeScript check, focused ESLint pass, all
frontend tests, all locale checks, and the Rust library suite passed. This
quoted-search pass also has focused coverage that rejects
non-contiguous terms and Porter-stem candidates that do not satisfy Papercut's
normalized literal phrase semantics. Its TypeScript check passes locally; the
Rust test binary awaits CI because this workstation lacks
`javascriptcoregtk-4.1`. PDF imports now reuse the first PDF.js page already
opened for text extraction to create a best-effort 480 by 720 maximum PNG.
Rust revalidates and normalizes that image through the existing uploaded-cover
pipeline; thumbnail failure leaves a valid searchable PDF with the normal
placeholder rather than failing the import. The committed extraction check now
rejects missing, reordered, or duplicated golden markers and invalid page/text
geometry. Focused adapter and SQLite tests retain multilingual text, literal
inline-format phrases, and page locators. Manual smoke tests passed fuzzy and
exact-phrase search plus first-page gallery cover generation on imported PDFs.

Decision gate: text-native fixtures import, list, fuzzy/exact search, thumbnail,
transfer, and delete through shared storage/index contracts without a
viewer-specific search workaround. Opening and in-document Find remain Stage 4
and Stage 5 work.

### Stage 4: PDF Viewer

Stage status: Implementation complete; initial-layout and deep-navigation
release validation pending

- [x] Implement the PDF.js viewer as a focused viewer component.
- [x] Load source through a scoped local asset boundary rather than JSON/base64.
- [x] Render visible and adjacent pages through PDF.js's bounded page cache and
      release off-window resources.
- [x] Add page navigation, zoom, fit-width, fit-page, and available outline
      navigation.
- [x] Add selectable/accessibility text layers aligned to rendered pages.
- [x] Finish reader controls, localized status text, mobile, and keyboard
      behavior.
- [x] Smoke-test controls at desktop and narrow widths.
- [x] Smoke-test controls in RTL.
- [x] Keep dark mode on viewer chrome without recoloring PDF page content.
- [x] Add an optional wide-screen two-page spread through PDF.js.
- [x] Smoke-test single/spread navigation and narrow-screen fallback.
- [ ] Smoke-test a deep page jump followed by immediate upward scrolling with
      automatic page-metadata fetching enabled.
- [x] Synchronize PDF.js with React-driven viewport size changes.
- [x] Smoke-test responsive fit recalculation between wide and narrow
      viewport sizes.
- [ ] Smoke-test complete initial rendering of image-heavy pages without
      changing zoom.

Implementation evidence: the viewer lazy-loads PDF.js's `PDFViewer`,
`PDFLinkService`, and `EventBus`; starts wide layouts at 100% and narrow
layouts at fit width; disables external and auto-detected links; and requests
source data in 1 MiB ranges. PDF.js may prefetch page metadata through those
ranges so deep navigation uses real page dimensions; canvas rendering remains
bounded to the visible neighborhood. A viewport `ResizeObserver` mirrors the
full PDF.js viewer's resize contract: responsive fit modes are recalculated
only when their relevant dimensions change, while the visible-page queue is
updated without resetting explicit user zoom. PDF.js also renders directly
into the displayed canvas because WebKit may not repaint its delayed
temporary-canvas copy after an image-heavy page finishes.
Tauri's asset protocol is limited to
`$APPDATA/document_uploads/*/source.pdf`, while Rust verifies URL, database
metadata, source kind, existence, and the 250 MB limit before returning a path.
The responsive `PdfControls` toolbar uses those same viewer primitives for
bounded page input, 25-400% zoom, fit-width, fit-page, optional two-page
spreads, and conditional outline navigation. Page and zoom controls stay
centered on wide screens while view controls remain in the same compact
control cluster. Fine-pointer layouts use
compact controls, narrow and coarse-pointer layouts retain 44-pixel touch
targets, and all layouts expose labels and pressed state to assistive
technology. Reading appearance controls are omitted for PDFs because PDF.js
renders the document's own page appearance. Viewer chrome follows the app
theme, and the PDF viewport consumes
the reader height left by optional Find and Diagnostics UI instead of using a
fixed viewport percentage. Wide layouts tighten the PDF header spacing while
narrow layouts use one reader-action row plus the PDF toolbar and do not reserve
the main app navigation's unused bottom padding.
At desktop widths of 1100 CSS pixels and above, the existing PDF toolbar is
portaled into the reader header's centered slot; smaller layouts retain a
separate centered row without duplicating viewer state or controls.
The production frontend build, locale check,
TypeScript check, and 25 focused tests pass. A full local Rust check remains
blocked in this shell by the missing `javascriptcoregtk-4.1` development
package, before Papercut compilation begins.

Manual evidence: a large-document desktop smoke test scrolled normally without
disappearing pages or a noticeable memory spike. Page, zoom, fit, and outline
controls also worked at desktop and narrow widths, including Arabic RTL.

Decision gate passed: large PDFs scroll without disappearing pages, unbounded
memory growth, or blocking the rest of the app; controls remain usable at
desktop and narrow widths with keyboard and RTL layouts.

### Stage 5: Find, Search Navigation, TTS, And Bookmarks

Stage status: Complete; semantic HTML/EPUB bookmark smoke test pending

- [x] Add Find across all extracted pages without rendering all pages.
- [x] Navigate search results to the correct page and matched coordinates.
- [x] Reconstruct logical PDF prose and adapt it to existing
      `ReadableSegment` chunking.
- [x] Retain UTF-16 segment offsets mapped to persisted page/block source runs.
- [x] Resolve active chunk source spans through page/block runs and text-layer
      items.
- [x] Highlight active TTS text across block and line boundaries.
- [x] Restore page/location bookmarks after reopen.
- [x] Verify TTS highlight bands remain within one column on a multi-column
      page.
- [x] Verify saved audiobook create, play, reopen, export, and import behavior.
- [x] Compare all agreed parity cases against HTML/EPUB.
- [x] Re-verify PDF bookmark restore and active-state visibility after changing
      zoom, viewport width, and spread mode.
- [x] Replace HTML/EPUB window-scroll bookmarks with semantic text offsets.
- [x] Normalize visual line-end hyphenation for PDF search and Find.
- [ ] Verify HTML/EPUB bookmark restoration and active styling after changing
      typography, reading width, and viewport width.

Decision gate passed: Find, global search, TTS playback/highlighting, location
restoration, and portable saved audiobooks pass the Stage 0 parity suite.

Implementation evidence: the shared reader Find bar now delegates PDF searches
to PDF.js's `PDFFindController`, which schedules text extraction across the
document, updates Papercut's existing match count, renders highlights through
the PDF.js text layer, and navigates next/previous matches without rendering
every page. HTML and EPUB retain their existing DOM-range implementation.
Papercut supplies a bounded set of aliases when a PDF query visibly contains
internal hyphens, allowing each of the first few compounds to vary independently
between joined, compact-hyphen, and copied hyphen-space forms while retaining
PDF.js's existing offset-aware highlights.

Uploaded-PDF SQLite hits now preserve their indexed zero-based page locator
through the shared search-result shape. Opening a result applies that locator
during PDF.js `pagesinit`, making the indexed page the viewer's first meaningful
render instead of rendering page one and then navigating. Once its text layer
is ready, Papercut's normalized range matcher highlights the marked term or
original quoted phrase on that page. It falls back to `PDFFindController` only
when the indexed page cannot reproduce the match. A nonmodal live status
distinguishes opening the indexed page from the rare whole-document
verification fallback. The PDF source and complete extracted text are not
copied into React state.
The PDF search projection also removes conservative lowercase or uppercase
line-end hyphens only across geometry-confirmed paragraph continuations.
Same-line compounds retain their punctuation, while explicitly hyphenated
queries use a bounded original-or-joined FTS alias. PDFs indexed before this
projection change must be reindexed from their retained page sidecars or
reimported; no source PDF extraction behavior changed.
The focused PDF Find adapter and reader range tests, full frontend suite, TypeScript
check, focused ESLint pass, and production Vite build pass. Manual acceptance
should cover a match on a late unrendered page, next/previous wraparound, a
quoted global-search result, RTL text, no matches, and closing Find.

PDF narration now reads the already validated page sidecars and reconstructs
logical prose before entering the same `ReadableSegment` chunker used by HTML
and EPUB. The PDF-owned reconstructor rejoins inline text items, visual line
wraps, conservative page-spanning sentences, and standard presentation
ligatures without reordering extraction output. Paragraph spacing and backwards
vertical jumps remain segment boundaries. Every reconstructed segment carries
compact UTF-16 ranges back to page and block order, while full geometry remains
in bounded sidecars until the active-page highlighter requests it.

The currently opened PDF is cached once in the audiobook hook, then model
changes re-run only the shared chunker rather than filesystem I/O or PDF
parsing. Runtime PDF chunks resolve their deterministic segment spans into
page/text-item offsets; persisted audiobook manifests retain the existing
format and rebuild those bounded runtime offsets on reopen.

Re-importable audiobook bundles now carry either sanitized HTML or the
canonical PDF source in the existing container. HTML exports remain version 2
for backward compatibility; version 3 adds `sourceKind: "pdf"` and one
`sourcePdf` payload. Rust streams the stored PDF directly into the bundle
instead of copying it through frontend IPC. Import verifies its content-derived
document URL, restores it into the normal upload store, and then calls the
existing PDF.js indexing path so page text, FTS rows, metadata, and the cover
remain derived and rebuildable. Version 2 HTML imports remain accepted.

The PDF TTS highlighter reuses PDF.js's pinned text-item mapping. It navigates
to the active chunk's first page and adds same-line visual bands only for text
layers PDF.js has rendered; virtualized pages receive their bands when
`textlayerrendered` fires. Find-driven text-layer rewrites also reapply the
active range without modifying PDF.js Find markup. This keeps work proportional
to the active chunk and rendered page cache rather than the document's page
count. Desktop, narrow-width, zoom, RTL, and single-page multi-column smoke
tests pass. The multi-column check confirmed that narration and highlight
progress remain within the first column before continuing into the next.

PDF bookmarks continue using the shared explicit-bookmark hook and localStorage
record. PDF.js supplies a small viewer adapter that stores the current page and
PDF-space coordinates, restores that location through its native destination
API after the first page renders, and drives the existing bookmark/top controls
from its internal scroll container. The active bookmark indicator follows
whether the saved vertical PDF point is visible, so zoom-driven page recentering
does not make a visible bookmark appear inactive.
HTML and generated EPUB reading documents use the same viewer adapter contract
with document-wide text offsets. Restoring resolves the saved text rather than
reusing a rendered scroll height, so font, line-height, reading-width, and
viewport changes do not move the bookmark to unrelated content. The resolved
range is cached for constant-time visibility checks while scrolling. Legacy
HTML/EPUB window-scroll records are discarded rather than migrated.
The PDF save/update/remove, scroll-to-top, changed zoom, viewport width, and
spread-mode acceptance checks pass.

PDF format adapters use the audiobook-save chunk profile explicitly. This keeps
reconstructed paragraphs under the same native request ceiling as HTML/EPUB
instead of inheriting the larger interactive-playback default; SILMA retains its
smaller model-specific profile.

Automated validation for this slice passes TypeScript, focused ESLint, all 42
frontend tests, i18n validation, and the production build/search-index pipeline.
Both focused reconstruction tests also pass when the PDF narration module is
compiled independently. Focused Rust coverage now checks version 2 HTML
compatibility, version 3 PDF sources, and source-kind/role mismatches; local
execution remains blocked before Papercut compilation by the missing
`javascriptcoregtk-4.1` development package already recorded above. Manual
acceptance passes for text-native PDF audiobook creation and playback, chunk
order across page and column boundaries, clean-library bundle export/import,
bookmarks, Find, global search, and HTML/EPUB parity.

### Stage 6: OCR Engine Foundation

Stage status: Foundation complete; desktop English WebView smoke test passed

- [x] Persist aggregate upload text status without changing the canonical PDF
      or page-sidecar schema.
- [x] Mark PDFs as processing until final page/index commit.
- [x] Mark newly finalized and previously indexed fully textless PDFs as
      requiring recognition.
- [x] Surface the recognition-required state in Library Gallery and List views.
- [x] Defer hybrid page detection until text and image signals could be
      evaluated together in Stage 7.
- [x] Select Tesseract.js as the initial common engine.
- [x] Pin the worker, core, and English trained-data packages.
- [x] Package only local runtime assets needed by the LSTM worker.
- [x] Keep Tesseract and its runtime out of normal app startup.
- [x] Normalize OCR words, confidence, order, and image-pixel bounds into
      `PageTextLayer`.
- [x] Add an explicit English recognition action for PDFs with missing text.
- [x] Reuse one worker and one bounded page render at a time.
- [x] Report preparing, page recognition, indexing, cancellation, and failure.
- [x] Commit recognized search data only through the atomic PDF finalizer.
- [x] Remove the temporary synthetic OCR benchmark harness and generated PDFs.
- [x] Run one English image-page recognition smoke test in the desktop WebView.
- [ ] Confirm the generated build has no OCR CDN requests.
- [ ] Record first-run worker startup, recognition time, and peak memory.

Decision gate: a local English page produces ordered text and finite bounds in
the supported WebView without affecting non-OCR startup.

### Stage 7: Image-Only And Hybrid PDF OCR

Stage status: Complete; desktop English image-only and hybrid acceptance passed

- [x] Detect usable native text page by page.
- [x] OCR every page of a fully textless English PDF.
- [x] OCR only missing pages in a hybrid PDF.
- [x] Normalize OCR output into the same `PageTextLayer` contract.
- [ ] Store OCR engine/model version, language, provenance, and confidence.
- [ ] Add language selection or detection with a retry path.
- [x] Add page-level progress, cancellation, failure, and retry from the Library.
- [ ] Add durable job resume across app restarts.
- [x] Prevent partial OCR sidecars from becoming searchable before final commit.
- [x] Prevent duplicate native and OCR text in hybrid PDFs.
- [x] Overlay persisted OCR words on rendered textless and weak-native pages
      for selection without modifying or duplicating the canonical PDF.
- [x] Teach viewer Find and indexed-result highlighting to use finalized OCR
      text and page sidecars, including native pages inside hybrid PDFs, while
      retaining PDF.js Find for fully native-text PDFs.
- [x] Re-run PDF Find, search, TTS, and highlight acceptance tests.

Decision gate: image-only and hybrid fixtures are searchable and speakable
without harming native-text PDF behavior or source files.

Manual acceptance passed with native, image-only, blank, and weak-native image
pages in one hybrid document. Native text remained searchable without
duplication; recognized pages exposed selectable overlays; Find highlighted and
navigated matches across native and OCR pages; TTS retained page order; and the
canonical PDF remained unchanged.

### Stage 8: Native Mobile Capture

Stage status: In progress; Android capture and same-process resilience accepted;
Android restart recovery, native photo-picker, and iOS physical-device
acceptance open. The restart-recovery and photo-picker implementation passes
automated checks, but their latest physical smoke-test passes were explicitly
deferred.

- [x] Add one isolated local Tauri plugin with Android and iOS adapter boundaries.
- [x] Integrate CameraX capture and a manual four-corner review flow on Android
      without requiring Google Play Services.
- [x] Integrate VisionKit document camera on supported iOS devices.
- [x] Support page thumbnails, crop, rotation, delete, reorder, and retake
      across both platforms. Android keeps only one bounded management preview
      in memory and uses small on-disk thumbnails for the accepted-page strip.
- [x] Support importing existing images across both platforms through native
      system pickers without broad media permissions or image bytes over IPC.
- [x] Save reviewed iOS and Android scans as canonical PDFs before OCR begins.
- [ ] Process bounded batches and allow users to append/resume large scans.
      Android now persists only ordered accepted-page filenames and offers to
      continue or discard the newest unfinished scan after app restart. Physical
      restart acceptance and equivalent iOS recovery remain open.
- [ ] Handle unavailable scanner services, resource download, permissions,
      interruption, low storage, and unsupported devices. Android now handles
      camera availability, runtime permission recovery, cancellation, and
      temporary-file cleanup. It also restores accepted pages after Activity
      recreation, expires abandoned cache sessions after seven days, checks
      free space before capture and final PDF assembly, and keeps accepted pages
      after an assembly failure. Physical interruption and low-storage tests,
      iOS resource cases, Android restart acceptance, and iOS app-restart resume
      remain open.
- [x] Keep scanning controls absent from desktop and unsupported mobile builds.

Decision gate: a multi-page scan survives interruption and produces a durable
source PDF or page set without holding the entire book in memory.

Implementation evidence: the Android plugin compiles and passes Android lint;
its manifest merges into the arm debug app. CameraX writes one temporary JPEG,
the review screen holds one bounded bitmap, accepted pages return to compressed
session files, and Android's `PdfDocument` decodes one accepted page at a time.
Accepted pages now have small session thumbnails; the page manager rewrites
only the in-memory file order when moving pages and deletes both files when a
page is removed, without recompressing retained page images.
Android physical-device acceptance passed camera framing, touch crop, rotation,
retake, multi-page capture, page reorder/delete, permission denial/recovery,
cancellation, final PDF import, and OCR handoff. Accepted page filenames and
order now use Android saved Activity state for same-process Activity recreation;
unaccepted camera/review frames are intentionally retaken. A native `StatFs`
preflight reserves 64 MiB plus the accepted JPEG size before PDF assembly, and
transient capture, preview, page-write, and final assembly failures no longer
discard pages that were already accepted.
Android Activity recreation, low-storage recovery, and retained-page behavior
also passed physical-device smoke testing. Existing-photo import uses the
platform picker and normalizes one selected image at a time before the same
canonical PDF import; Android and iOS picker acceptance was deferred and remains
open. Android also writes a tiny atomic manifest after each accepted-page add,
move, or delete. A fresh scanner launch can continue the newest complete draft
or explicitly start over without loading page pixels into memory. This restart
path compiles and passes Android lint but has not yet received physical-device
acceptance.
The finished path enters the existing Rust PDF importer, so readiness, OCR,
indexing, viewer, search, and TTS behavior are not duplicated in native code.
Physical-device acceptance remains open for the iOS capture path and both
platform photo-picker paths.

### Stage 9: Scan-To-Book Integration

Stage status: In progress; pre-capture metadata and English OCR handoff
implemented, physical mobile acceptance open

- [x] Let users set the display title and choose English recognition or
      import-only handling before native capture/photo selection. Specific
      non-English languages stay out of the picker until their OCR assets ship.
- [x] Feed English captured pages that need recognition through the existing
      bounded OCR and atomic PDF indexing path. Native-text scans and
      import-only languages keep the normal PDF import/view path.
- [ ] Show low-confidence and failed pages with targeted retry.
- [ ] Allow pages to be appended to an existing unfinished scan.
- [ ] Restore interrupted scan state after app restart. Android implementation
      is complete pending physical acceptance; iOS remains open.
- [ ] Verify folder organization, gallery thumbnail, saved audio, transfer, and
      deletion.

Decision gate: scan-to-book works end to end on one supported Android and one
supported iOS device with representative multi-language fixtures.

Implementation evidence: the setup dialog reuses the app's accessible modal
and select controls. Its title enters the initial Rust upload transaction and
is retained through PDF.js finalization; it is not patched onto the document
afterward. Choosing English starts the existing local recognizer only when PDF
readiness reports missing usable text. Choosing another language imports the
canonical PDF without making an unsupported OCR promise. No second PDF copy,
native OCR implementation, language detector, or scanner-specific index was
added. Automated validation is complete, but the new dialog and automatic OCR
handoff have not yet received physical Android or iOS smoke testing.

### Stage 10: Hardening And Release

Stage status: Not started

- [ ] Run malformed, encrypted, large, high-page-count, and low-storage tests.
- [ ] Run memory and performance tests on minimum supported desktop and mobile
      hardware.
- [ ] Complete keyboard, screen-reader, RTL, localization, zoom, and touch
      reviews.
- [ ] Review parser, local asset, IPC, native plugin, and file cleanup security.
- [ ] Verify upgrades, library transfer, backup/export, and deletion against
      production-like app data.
- [ ] Add user documentation, privacy wording, known limitations, and release
      notes.
- [ ] Decide whether PDF/OCR remains behind Developer Mode or enters a public
      beta.
- [ ] Remove diagnostic-only code, unused dependencies, and abandoned feature
      flags.

Decision gate: all supported platforms pass automated checks and the complete
manual acceptance matrix before public release.

### Stage 11: Evidence-Driven Follow-Ups

Stage status: Deferred

- [ ] Add OCR text correction only if real users need it.
- [ ] Add a simplified/reflow reading mode only if fixed-layout reading is
      insufficient; keep it derived rather than canonical.
- [ ] Export searchable PDFs only after import/search is stable.
- [ ] Add desktop camera capture only if desktop users request it.
- [ ] Add advanced table, equation, or layout analysis only after corpus data
      demonstrates the need.

## Decision Log

| Date | Stage | Decision | Evidence / Notes |
| --- | --- | --- | --- |
| 2026-07-24 | Planning | Build text-native PDF before OCR scanning | It establishes the page, viewer, locator, search, and TTS contracts OCR needs |
| 2026-07-24 | Planning | Preserve PDF/images as canonical source | HTML conversion would lose fixed-layout fidelity and make OCR correction/rebuild harder |
| 2026-07-24 | Planning | Use mobile-native capture for the first scan release | It provides mature crop/edge/reorder UX without owning camera processing on three platforms |
| 2026-07-24 | Planning | Defer OCR engine selection to a benchmark | No reviewed option simultaneously proves all target languages, coordinate quality, mobile packaging, and performance |
| 2026-07-24 | Stage 0 | Keep large/copyrighted benchmark documents out of Git | Small generated fixtures cover automation; checksummed external fixtures cover realistic performance without repository bloat |
| 2026-07-24 | Stage 0 | Treat text-native PDF as the first release boundary | It delivers useful PDF support while postponing OCR/runtime packaging until the page-aware reader path is proven |
| 2026-07-24 | Stage 0 | Approve the scope, fixture matrix, and measurable budgets | Product-owner approval opened the renderer/extractor spike |
| 2026-07-24 | Stage 1 | Prefer PDF.js 6.1.200 as the renderer candidate | It rendered the RTL fixture and exposes the canvas, text, direction, and coordinate data needed by the viewer; device WebViews remain the gate |
| 2026-07-24 | Stage 1 | Reject `pdf-extract` 0.12.0 for production | It reversed the Arabic fixture's logical order and lacks the coordinate model required for search/TTS highlighting |
| 2026-07-24 | Stage 1 | Reject `pdf_oxide` 0.3.75 | The native candidate inserted a false inline-style space and fused visible Arabic words; fixing this would require custom heuristics while retaining a second parser |
| 2026-07-24 | Stage 1 | Select PDF.js 6.1.200 for rendering and extraction | The installed parser preserved expected word boundaries, reading order, and finite coordinates while avoiding renderer/extractor disagreement |
| 2026-07-24 | Stage 1 | Lazy-load PDF.js outside normal app startup | The optimized spike adds about 2.02 MB minified to the package, but only about 2.9 KB of eagerly loaded JavaScript and CSS before gzip |
| 2026-07-24 | Stage 1 | Generate local PDF.js runtime assets from the pinned npm package | PDF.js resolves JPEG 2000 decoders and standard fonts by stable filename; copying only those installed directories avoids CDN access, committed binary duplication, and another build dependency |
| 2026-07-24 | Stage 1 | Use PDF.js directly instead of adding `react-pdf` | The wrapper still depends on PDF.js and its runtime setup, while Papercut needs direct page-coordinate, cancellation, virtualization, Find, and TTS-highlight control |
| 2026-07-24 | Stage 1 | Remove the rejected native extraction spike | Its comparison evidence is recorded; retaining 172 Rust packages after selecting PDF.js would add maintenance without product value |
| 2026-07-24 | Stage 1 | Defer hands-on mobile WebView validation | The browser harness and Android asset packaging pass are recorded separately; physical Android and iOS WebView behavior remains an explicit unchecked gate |
| 2026-07-24 | Stage 2 | Isolate PDF persistence under `document_uploads/pdf` | Shared code knows only source kind, URL/path selection, and nullable page locators; canonical PDF and page-sidecar details remain removable |
| 2026-07-24 | Stage 2 | Transfer only canonical PDF sources | Package v3 adds `sourceKind` and `source.pdf`; derived page-text data is rebuilt instead of copied across devices |
| 2026-07-24 | Stage 2 | Keep existing HTML/EPUB URLs and package compatibility | Database defaults migrate old rows to HTML, and transfer v1/v2 remain accepted as HTML-only |
| 2026-07-24 | Stage 3 | Reuse PDF.js for import-time text extraction | One selected parser supplies metadata, page text, reading order, and coordinates without adding a second PDF dependency |
| 2026-07-24 | Stage 3 | Keep PDF import memory and IPC bounded by page | Rust caps source size at 250 MB, PDF.js caps documents at 2,000 pages, and only one page-text layer is persisted per IPC call |
| 2026-07-24 | Stage 3 | Commit FTS rows only after all page sidecars validate | Failed or cancelled imports remove newly staged PDFs; partial page rows never become searchable |
| 2026-07-25 | Stage 3 | Reuse the shared uploaded-document tree for PDF visibility | The upload URL parser now accepts both canonical `.html` and `.pdf` URLs, keeping Library and Search document lists on one hierarchy |
| 2026-07-25 | Stage 3 | Build uploaded-document snippets from indexed body text | PDF pages intentionally have no heading, so FTS snippets target the text column and retain headingless page coverage |
| 2026-07-25 | Stage 3 | Verify quoted PDF searches behind the FTS candidate filter | SQLite narrows the candidate set without loading PDF sources into React; Rust then preserves the existing normalized literal-phrase contract despite Porter stemming |
| 2026-07-25 | Stage 3 | Reuse the first PDF.js page render for gallery thumbnails | One bounded best-effort PNG feeds the existing uploaded-cover pipeline without adding a renderer or making cover generation part of import correctness |
| 2026-07-25 | Stage 3 | Close text-native import and search before building the viewer | The committed fixture covers deterministic parser/index contracts; external multilingual, malformed, encrypted, and large files stay in the later release-hardening matrix |
| 2026-07-25 | Stage 4 | Use PDF.js viewer primitives directly | Its rendering queue, bounded page cache, text layer, and link service cover the performance foundation without adding `react-pdf` or hand-rolling virtualization |
| 2026-07-25 | Stage 4 | Serve validated PDFs through Tauri's scoped asset protocol | Range-capable URLs avoid full-file IPC copies and let PDF.js read source data in 1 MiB chunks |
| 2026-07-25 | Stage 4 | Keep reader controls as a focused PDF.js adapter | A separate responsive `PdfControls` component directly drives page, zoom, fit, and optional outline APIs without adding `react-pdf`, a toolbar dependency, or speculative controls |
| 2026-07-25 | Stage 5 | Delegate PDF Find to `PDFFindController` | Reusing PDF.js search, text-layer highlighting, and page navigation preserves bounded rendering and avoids a second PDF text search implementation |
| 2026-07-25 | Stage 5 | Carry indexed PDF page locators through shared search results | SQLite already knows the matching page; preserving that value lets PDF.js target the correct page and phrase without loading full extracted documents into React |
| 2026-07-25 | Stage 5 | Reuse persisted PDF blocks for TTS chunking | A text-only IPC read feeds the existing `ReadableSegment` chunker, keeps coordinate payloads out of whole-document narration setup, and avoids reparsing PDFs or adding another chunking path |
| 2026-07-25 | Stage 5 | Reconstruct logical prose before PDF TTS chunking | PDF.js visual line and inline-style fragments are joined in a PDF-owned Rust adapter; shared chunk limits stay unchanged, and compact UTF-16 page/block runs prepare highlighting without transferring whole-document geometry |
| 2026-07-25 | Stage 5 | Apply save-sized chunks after PDF reconstruction | Logical paragraphs expose the generic 900-character playback default; a shared save wrapper restores the 360-character native request ceiling used by HTML/EPUB and retains SILMA's smaller override |
| 2026-07-25 | Stage 4 | Pass the large-document viewer performance gate | Manual scrolling completed without disappearing pages or a noticeable memory spike; responsive and RTL control behavior remains the final Stage 4 smoke test |
| 2026-07-25 | Stage 4 | Pass responsive PDF control smoke testing | Page, zoom, fit, and outline controls worked at desktop and narrow widths; RTL remains the final visual gate |
| 2026-07-25 | Stage 4 | Close the PDF viewer stage | The compact controls remained usable at desktop and narrow widths in both LTR and Arabic RTL layouts |
| 2026-07-25 | Stage 4 | Let the reader shell size the PDF viewport | Flex layout reclaims space when optional Find or Diagnostics UI is absent; wide controls center primary navigation while preserving 44-pixel touch targets |
| 2026-07-25 | Stage 4 | Retain the reader divider with responsive spacing | Wide PDF layouts reclaim vertical space around the shared header divider while narrow layouts keep the established borderless-reader separation |
| 2026-07-25 | Stage 4 | Use PDF.js for optional two-page spreads | Wide layouts pair pages 1-2, 3-4, and so on through `SpreadMode.ODD`; narrow layouts return to the default single-page view without another renderer or persisted preference |
| 2026-07-25 | Stage 4 | Share the wide reader header with PDF controls | One portaled toolbar occupies the centered header zone on wide screens and falls back to a second row when space is constrained; primary page and zoom actions remain visible |
| 2026-07-28 | Stage 4 | Allow PDF.js page-metadata prefetch | Real page dimensions keep deep jumps and immediate upward scrolling stable; source transport remains range-based and canvas rendering remains bounded |
| 2026-07-25 | Stage 5 | Resolve active narration spans through PDF.js text items | Runtime-only page/item offsets rebuild from durable segment spans, same-line range bands paint only rendered active text without PDF.js word seams, and saved-audiobook manifests remain unchanged |
| 2026-07-28 | Stage 5 | Store HTML and generated EPUB bookmarks as text offsets | One shared viewer adapter restores the same passage across typography and viewport changes while deleting the brittle scroll-height fallback and avoiding a second persistence path |
| 2026-07-28 | Stage 5 | Store PDF bookmarks in PDF page coordinates | The shared bookmark hook keeps one persistence path while PDF.js view-area coordinates and native destinations restore the same content across viewport and zoom changes without relying on rendered DOM heights |
| 2026-07-25 | Stage 5 | Extend the existing audiobook bundle for canonical PDFs | Version 3 adds a typed PDF source while HTML stays on version 2; imports reuse PDF.js indexing instead of copying derived page text, FTS rows, or thumbnails |
| 2026-07-25 | Stage 4 | Start wide PDF readers at 100% | Desktop avoids unexpectedly large fit-width scales while narrow layouts retain fit width for usable first-open framing |
| 2026-07-25 | Stage 5 | Close text-native PDF reader parity | Manual acceptance passed Find, global search, TTS, highlighting, bookmarks, portable audiobooks, responsive/RTL controls, and single-page multi-column reading order |
| 2026-07-26 | Stage 5 | Render indexed search-result pages first | PDF import already stores page-level text and FTS locators; applying that locator during `pagesinit` removes the redundant page-one render while retaining PDF.js text-layer highlighting and whole-document Find as a correctness fallback |
| 2026-07-28 | Stage 3 | Preserve PDF visual lines in indexed text | Reusing the geometry-aware narration grouping prevents unrelated adjacent lines from becoming one word while normalized phrase search continues to treat line breaks as whitespace |
| 2026-07-29 | Stage 5 | Normalize PDF line-end hyphenation once at indexing | Geometry-confirmed broken words become canonical FTS terms; explicitly hyphenated queries add bounded aliases while PDF.js retains viewer matching and offset-aware highlights |
| 2026-07-25 | Stage 5 | Remove the temporary PDF WebView harness | The production import and reader paths now cover its worker, canvas, text-layer, and cleanup responsibilities without maintaining a second app entry point |
| 2026-07-30 | Stage 4 | Synchronize PDF.js after viewport layout changes | A frame-coalesced `ResizeObserver` updates only PDF.js's visible-page queue and relevant fit mode, preserving lazy rendering and explicit user zoom |
| 2026-07-30 | Stage 4 | Render PDF.js directly into the visible canvas | Disabling the delayed temporary-canvas update avoids a WebKit repaint failure that could leave image-heavy pages showing only an intermediate frame until zoom changed |
| 2026-08-03 | Stage 6 | Persist conservative OCR readiness before selecting an engine | Finalized PDFs with no extracted text are marked `recognition-required` and exposed in the Library; hybrid page classification remains deferred because empty pages alone are not reliable evidence of missing OCR |
| 2026-08-03 | Stage 6 | Select Tesseract.js as the initial OCR engine | A shared Web Worker avoids native builds across five platforms, stays lazy, and emits text, confidence, and bounds that fit `PageTextLayer`; quality and device performance will be adjusted from real acceptance results rather than a retained comparison harness |
| 2026-08-03 | Stage 6 | Package English OCR assets locally | Pinned npm packages provide the worker, SIMD-aware LSTM cores, and trained data without runtime CDN access; other languages wait for explicit language selection |
| 2026-08-03 | Stage 7 | Ship the smallest explicit English image-only OCR slice | Fully textless PDFs get one local action, one reused worker, bounded page processing, shared mutation status, and atomic finalization; hybrid classification, automatic language detection, and durable queues wait for evidence that they are needed |
| 2026-08-03 | Stage 7 | Route OCR Find through finalized indexed pages | One document-scoped backend query returns compact per-page match counts off the WebView thread; only the current virtualized page loads OCR geometry for highlighting, while native-text PDFs keep PDF.js Find |
| 2026-08-04 | Stage 7 | Reuse bounded readiness signals for hybrid PDFs | Import records one aggregate recognition flag, while OCR recomputes page readiness from native text and image operators, preserves native and blank sidecars, and replaces only weak-text image pages without adding page-state storage |
| 2026-08-04 | Stage 7 | Close English image-only and hybrid OCR acceptance | Manual checks passed native/OCR search and Find navigation, selectable OCR overlays, ordered TTS, blank-page handling, source preservation, and no duplicate native text; broader languages, durable resume, and mobile capture remain separate stages |
| 2026-08-04 | Stage 8 | Isolate mobile capture from PDF processing | A local Tauri plugin owns only native camera UI and app-owned PDF output; the existing importer remains the sole validation, persistence, readiness, indexing, and OCR path |
| 2026-08-04 | Stage 8 | Start with VisionKit and defer Android capture to its own slice | iOS provides a complete native multi-page review flow now; Android requires a CameraX capture and manual crop/review workflow that deserves separate implementation and device validation |
| 2026-08-04 | Stage 8 | Reject ML Kit Document Scanner as the primary Android path | Its scanner resources and UI depend on Google Play Services, which is incompatible with Papercut's supported offline and de-Googled Android devices |
| 2026-08-04 | Stage 8 | Use CameraX plus Android platform graphics for the first Android scanner | CameraX covers broad camera lifecycle compatibility, while a manual four-corner perspective transform and page-at-a-time `PdfDocument` output avoid Google Play Services, OpenCV, another OCR path, and unbounded bitmap retention |
| 2026-08-04 | Stage 8 | Manage Android pages through files and bounded previews | Small on-disk thumbnails support selection, accepted-page order drives final PDF order without rewriting JPEGs, and only the selected page gets a larger bounded preview |
| 2026-08-04 | Stage 8 | Recover accepted Android pages without a draft database | Android saved Activity state handles same-process recreation, while one atomic app-private filename manifest supports a continue-or-start-new prompt after app restart; explicit completion/cancellation cleans immediately, abandoned cache sessions expire after seven days, and image data is never duplicated |
| 2026-08-04 | Stage 8 | Use native storage preflight before scanner writes | `StatFs` checks a fixed 64 MiB working reserve and accepted JPEG bytes before PDF assembly; this prevents predictable low-space failures without adding a storage dependency or pretending the estimate guarantees a later write |
| 2026-08-04 | Stage 8 | Import existing photos through native system pickers | Android `ACTION_OPEN_DOCUMENT` works without Google Play Services or broad media access, iOS PhotosUI grants only selected files, and both normalize one image at a time into the existing canonical PDF import instead of adding another editor or processing path |
| 2026-08-04 | Stage 9 | Ask only for actionable recognition language before capture | English runs the packaged local recognizer when readiness requires it; every other language remains importable without a misleading unsupported-language list or automatic language detector |
| 2026-08-04 | Stage 9 | Carry scan titles through the canonical import transaction | The scanner command validates the chosen title before native UI opens, and PDF finalization honors it instead of relying on a post-import metadata edit |

## References

- [PDF.js repository](https://github.com/mozilla/pdf.js)
- [PDF.js examples](https://mozilla.github.io/pdf.js/examples/)
- [PDF.js `PDFPageProxy` API](https://mozilla.github.io/pdf.js/api/draft/module-pdfjsLib-PDFPageProxy.html)
- [`pdf-extract` documentation](https://docs.rs/pdf-extract/latest/pdf_extract/)
- [`pdf_oxide` documentation](https://docs.rs/pdf_oxide/latest/pdf_oxide/)
- [Tauri command channels](https://v2.tauri.app/develop/calling-rust/)
- [Tauri mobile plugin development](https://v2.tauri.app/develop/plugins/develop-mobile/)
- [Apple PDFKit](https://developer.apple.com/documentation/pdfkit)
- [Apple VisionKit document camera](https://developer.apple.com/documentation/visionkit/vndocumentcameraviewcontroller)
- [Apple Vision text recognition](https://developer.apple.com/documentation/vision/vnrecognizetextrequest)
- [ML Kit Document Scanner](https://developers.google.com/ml-kit/vision/doc-scanner)
- [ML Kit Text Recognition languages](https://developers.google.com/ml-kit/vision/text-recognition/v2/languages)
- [Android CameraX](https://developer.android.com/media/camera/camerax)
- [Android `Matrix.setPolyToPoly`](https://developer.android.com/reference/android/graphics/Matrix#setPolyToPoly(float[],%20int,%20float[],%20int,%20int))
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract)
- [Tesseract.js repository](https://github.com/naptha/tesseract.js)
- [PaddleOCR multilingual recognition](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.en.md)
- [Speechify scan workflow overview](https://speechify.com/blog/scan-books-and-printed-text/)
- [Nielsen Norman Group: Visibility of system status](https://www.nngroup.com/articles/visibility-system-status/)
- [Nielsen Norman Group: User control and freedom](https://www.nngroup.com/articles/user-control-and-freedom/)
