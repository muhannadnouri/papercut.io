# PDF, OCR, And Document Scanning Plan

Status: Stage 3 text-native import/search slice implemented; manual gate pending
Last updated: 2026-07-25

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
8. **Choose an OCR engine only after a representative benchmark.** Arabic,
   Chinese, Devanagari, Latin text, page coordinates, mobile packaging, and
   runtime cost must be tested.

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
- `src/viewers/PdfViewer.tsx` remains a first-page validation harness rather
  than the production virtualized viewer planned for Stage 4.
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

## Stage 0 Baseline

### Supported Platform Baseline

Papercut currently builds for Linux, Windows, Intel/Apple Silicon macOS,
Android, and iOS. The configured minimums relevant to this work are:

| Platform | Current baseline |
| --- | --- |
| Android | API 26; universal release builds currently cover arm64, armv7, x86, and x86_64 |
| iOS | iOS 14 |
| macOS | macOS 10.13 |
| Rust | 1.77.2 declared minimum; CI builds with stable |
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

The current frontend baseline is 8 passing Vitest files and 17 passing tests as
of 2026-07-24. The sandbox shell lacks the `javascriptcoregtk-4.1` development
package, so focused Rust tests run through the host toolchain. CI remains the
required full Rust and mobile build baseline.

### Fixture Corpus

Use two fixture tiers:

1. **Committed synthetic fixtures** are small, license-safe, deterministic, and
   suitable for automated parser/search tests.
2. **Manual benchmark fixtures** may be larger or externally sourced. Record
   provenance, license, checksum, page count, and expected behavior, but do not
   commit copyrighted or oversized files.

Materialize the committed PDFs during Stage 1 after the spike identifies the
smallest deterministic generation method. Do not hand-maintain opaque binary
fixtures when a short generator can produce the same case.

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
| P09 | Image-only pages | Text-native import identifies that OCR is required instead of indexing empty text |
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

### Later OCR Budgets

These guide the Stage 6 benchmark and do not select an engine today:

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
- `pdf-extract` and its current transitive dependency resolution do not compile
  cleanly with Cargo 1.77.2 because dependencies now use Rust 2024 manifests.
  Pinning old transitives would add maintenance without solving its RTL and
  coordinate limitations.

Rust 1.77.2 remains Papercut's declared minimum. The rejected native spike ran
on stable Rust 1.96.1 because `pdf_oxide` required Rust 1.88, but an abandoned
candidate is not a reason to raise the product minimum.

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

The branch-local WebView harness is exposed at `/?pdf-spike`. It uses a native
file input and renders only page one because its purpose is to validate worker
loading, canvas output, selectable text, and cleanup:

`npm run dev` and `npm run build:vite` copy PDF.js's installed WASM decoders
and standard fonts into generated local assets. The renderer does not fetch
runtime resources from a CDN. With those assets configured, the 40-page
JPEG 2000 benchmark renders a nonblank 430x695 first page and exposes its title
through the text layer. The product owner re-ran the local browser harness with
both representative PDFs and confirmed that both first pages render. Hands-on
Tauri mobile WebView validation is explicitly deferred for now rather than
treated as a pass.

```sh
npm run dev
# Open http://localhost:5173/?pdf-spike
```

The same route can be supplied to a Tauri development build with a temporary
configuration override:

```sh
npx tauri dev --features native-tts-shared --no-watch \
  --config '{"build":{"devUrl":"http://localhost:5173/?pdf-spike"}}'
```

For a physical Android or iOS device, replace `<LAN_IP>` with the development
machine's address on the same network:

```sh
npx tauri android dev \
  --config '{"build":{"beforeDevCommand":"npm run dev -- --host 0.0.0.0","devUrl":"http://<LAN_IP>:5173/?pdf-spike"}}'

# Run on macOS with Xcode:
npx tauri ios dev \
  --config '{"build":{"beforeDevCommand":"npm run dev -- --host 0.0.0.0","devUrl":"http://<LAN_IP>:5173/?pdf-spike"}}'
```

Use the same smoke matrix on every target:

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

### OCR Candidates

- [Tesseract](https://github.com/tesseract-ocr/tesseract) supports more than
  100 languages, including Papercut's Arabic, Hindi, Chinese, and Latin-script
  targets. It can emit text plus hOCR, TSV, ALTO, or PAGE coordinate data. Its
  C++ packaging, model footprint, and quality on photographed pages require
  measurement.
- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.en.md)
  provides multilingual and mobile-oriented models, including Arabic and
  Devanagari coverage. It brings a heavier runtime and model-management stack.
- Apple Vision provides on-device recognition, confidence, and bounding boxes
  on Apple platforms, but it would create platform-dependent OCR output.
- Android ML Kit Text Recognition v2 covers Latin, Chinese, Devanagari,
  Japanese, and Korean scripts, but its documented language table does not
  cover Arabic. It cannot be Papercut's only OCR engine.

Benchmark Tesseract and PaddleOCR against the same corpus. Include Apple Vision
as a quality/performance comparison on iOS. Prefer one common OCR engine if it
meets quality and packaging requirements; use native OCR selectively only if
the common engine demonstrably fails platform constraints.

## Mobile Scanning

Capture and OCR should remain separate:

- Android's [ML Kit Document Scanner](https://developers.google.com/ml-kit/vision/doc-scanner)
  supplies native capture UI, edge detection, crop, rotation, cleanup, page
  reordering, and PDF/JPEG output. It runs on-device and does not require the
  app to request camera permission, though its scanner resources may download
  on first use and device requirements must be enforced.
- iOS should use
  [`VNDocumentCameraViewController`](https://developer.apple.com/documentation/visionkit/vndocumentcameraviewcontroller)
  for the native capture experience.
- A small Tauri mobile plugin should normalize both platforms into the same
  page-image or PDF result consumed by the import pipeline.

The review step should let users inspect thumbnails, crop, rotate, delete,
reorder, rescan, and import existing photos. The source pages should be saved
before OCR begins so recognition can be retried.

Desktop camera capture is deferred. Importing existing PDFs and images covers
the desktop use case without introducing webcam permissions and a third capture
experience.

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

Stage status: In progress; import/search slice implemented, decision gate pending

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
- [ ] Add parser/index tests for the Stage 0 corpus.

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
placeholder rather than failing the import.

Decision gate: text-native fixtures import, list, search, reopen, transfer, and
delete correctly without a PDF viewer-specific workaround in the search index.

### Stage 4: PDF Viewer

Stage status: Not started

- [ ] Implement the PDF.js viewer as a focused viewer component.
- [ ] Load source through a scoped local asset boundary rather than JSON/base64.
- [ ] Render visible and adjacent pages only and release off-window resources.
- [ ] Add page navigation, zoom, fit-width, fit-page, and available outline
      navigation.
- [ ] Add selectable/accessibility text layers aligned to rendered pages.
- [ ] Integrate reader loading, error, theme, mobile, and keyboard behavior.
- [ ] Confirm dark mode leaves PDF page colors unchanged by default.

Decision gate: large PDFs scroll without disappearing pages, unbounded memory
growth, or blocking the rest of the app.

### Stage 5: Find, Search Navigation, TTS, And Bookmarks

Stage status: Not started

- [ ] Add Find across all extracted pages without rendering all pages.
- [ ] Navigate search results to the correct page and matched coordinates.
- [ ] Adapt PDF page blocks to existing `ReadableSegment` chunking.
- [ ] Persist source ranges needed for page-coordinate TTS highlighting.
- [ ] Highlight active TTS text across block and line boundaries.
- [ ] Restore page/location bookmarks after reopen.
- [ ] Verify saved audiobook create, play, reopen, export, and import behavior.
- [ ] Compare all agreed parity cases against HTML/EPUB.

Decision gate: Find, global search, TTS playback/highlighting, and location
restoration pass the Stage 0 parity suite.

### Stage 6: OCR Engine Benchmark

Stage status: Not started

- [ ] Build one benchmark harness and corpus for Tesseract, PaddleOCR, and Apple
      Vision comparison where available.
- [ ] Measure Arabic, Chinese, Devanagari, and Latin text accuracy.
- [ ] Measure reading order, bounding boxes, confidence, skew/noise tolerance,
      speed, RAM, package/model size, and mobile integration.
- [ ] Test photographed pages as well as clean image-only PDFs.
- [ ] Review licenses and model redistribution terms.
- [ ] Select one common engine or document evidence for a narrowly scoped native
      exception.
- [ ] Pin runtime/model versions and record the decision.

Decision gate: the selected OCR approach meets language, coordinate, offline,
packaging, and performance requirements on supported devices.

### Stage 7: Image-Only And Hybrid PDF OCR

Stage status: Not started

- [ ] Detect usable native text page by page.
- [ ] OCR only pages without an acceptable text layer.
- [ ] Normalize OCR output into the same `PageTextLayer` contract.
- [ ] Store OCR engine/model version, language, provenance, and confidence.
- [ ] Add language selection or detection with a retry path.
- [ ] Add page-level progress, cancellation, resume, failure, and retry.
- [ ] Prevent duplicate native and OCR text in hybrid PDFs.
- [ ] Re-run PDF Find, search, TTS, and highlight acceptance tests.

Decision gate: image-only and hybrid fixtures are searchable and speakable
without harming native-text PDF behavior or source files.

### Stage 8: Native Mobile Capture

Stage status: Not started

- [ ] Add one small Tauri mobile plugin with Android and iOS scanner adapters.
- [ ] Integrate ML Kit Document Scanner on supported Android devices.
- [ ] Integrate VisionKit document camera on supported iOS devices.
- [ ] Support page thumbnails, crop, rotation, delete, reorder, rescan, and
      importing existing images.
- [ ] Save source pages before OCR begins.
- [ ] Process bounded batches and allow users to append/resume large scans.
- [ ] Handle unavailable scanner services, resource download, permissions,
      interruption, low storage, and unsupported devices.
- [ ] Keep scanning controls absent from unsupported desktop builds.

Decision gate: a multi-page scan survives interruption and produces a durable
source PDF or page set without holding the entire book in memory.

### Stage 9: Scan-To-Book Integration

Stage status: Not started

- [ ] Let users set title and recognition language before processing.
- [ ] Feed captured pages through OCR, indexing, PDF viewing, search, and TTS.
- [ ] Show low-confidence and failed pages with targeted retry.
- [ ] Allow pages to be appended to an existing unfinished scan.
- [ ] Restore interrupted scan state after app restart.
- [ ] Verify folder organization, gallery thumbnail, saved audio, transfer, and
      deletion.

Decision gate: scan-to-book works end to end on one supported Android and one
supported iOS device with representative multi-language fixtures.

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
| 2026-07-24 | Stage 1 | Reject `pdf_oxide` 0.3.75 and keep Rust 1.77.2 | The native candidate inserted a false inline-style space and fused visible Arabic words; fixing this would require custom heuristics while retaining a second parser and raising Papercut's minimum Rust version |
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
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract)
- [PaddleOCR multilingual recognition](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.en.md)
- [Speechify scan workflow overview](https://speechify.com/blog/scan-books-and-printed-text/)
- [Nielsen Norman Group: Visibility of system status](https://www.nngroup.com/articles/visibility-system-status/)
- [Nielsen Norman Group: User control and freedom](https://www.nngroup.com/articles/user-control-and-freedom/)
