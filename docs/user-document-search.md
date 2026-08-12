# User Document Uploads And Search Indexing

Papercut has two search-indexing paths on purpose:

- **Bundled documents** are known at build time and are indexed by Pagefind into the production frontend bundle.
- **User uploads** are unknown until runtime, so they are imported through Tauri and indexed into a local SQLite FTS5 database in app data.

This avoids the trap of trying to rebuild Pagefind on a user's device every time they add a document. Pagefind remains excellent for the shipped corpus, while SQLite FTS gives us incremental offline search for user-owned content.

## Current Scope

The current app supports local HTML, EPUB, and PDF files. PDF search and TTS
require a usable extracted text layer; fully textless PDFs remain readable and
are marked for opt-in English text recognition:

- Users open **Import** from the document list and choose **Files** for one or more HTML, EPUB, or PDF files, or **Folder** for a desktop folder.
- Tauri opens the native filesystem picker for `.html`/`.htm`, `.epub`, or `.pdf` files. Desktop folder import preserves the selected folder and supported subfolders through the Library's five visible levels (the selected folder plus four descendant levels). If its top-level name already exists, Papercut creates a suffixed folder such as `Books (2)` rather than merging the trees.
- Rust reads the selected HTML file, enforces a 25 MB limit, decodes UTF-8 or declared legacy browser encodings, sanitizes the HTML, extracts readable sections, stores the sanitized source, and indexes the sections into SQLite FTS5.
- Rust reads the selected EPUB file, enforces a 100 MB limit, validates the EPUB ZIP/container, follows OPF spine order, sanitizes XHTML chapters into generated reading HTML, rewrites and target-validates local chapter, TOC, and footnote links, retains supported local raster images within safety caps, extracts readable sections, and indexes the sections into SQLite FTS5.
- Rust stages a selected PDF under a 250 MB limit, then the existing lazy-loaded PDF.js path extracts and stores bounded page text, page locators, and a best-effort first-page thumbnail before atomically finalizing its SQLite FTS5 rows. PDFs are limited to 2,000 pages. Fully textless and hybrid PDFs with image-backed pages lacking usable text remain renderable but are marked **Text Recognition Required**. **Recognize English Text** runs the local Tesseract worker one page at a time, reports progress, supports cancellation, and exposes recognized text to search, Find, and TTS only after the existing atomic finalizer succeeds. Failed or conservatively low-confidence pages remain listed for targeted retry without reprocessing successful pages.
- New imports use the original file's SHA-256 as their stable upload id. Selecting the same unchanged file again reuses its existing stored document instead of creating another copy. Batch results report that source under **Already in Library** rather than counting it as newly added.
- New imports retain the original filename as read-only provenance. In Library list view, **Manage** exposes a pencil action for each uploaded document; its **Document Info** dialog lets users correct Papercut's display title without renaming the source, changing its stable URL, or invalidating folders, bookmarks, and saved audio.
- Library gallery and list views mark documents that have an explicit reader bookmark, and the Library toolbar can filter to those documents. The reader saves a first bookmark directly; an existing bookmark exposes explicit actions to return, move, or remove it, with Undo after a move or removal.
- React lists imported files under **User Uploads** and opens them through the shared document reader. EPUB uses generated reading HTML; PDF uses the dedicated PDF.js viewer.
- Search queries run through `src/hooks/useSearch.ts`, which queries Pagefind and SQLite FTS in parallel and returns one shared result shape.
- Users can open **Manage**, select uploaded HTML, EPUB, and PDF documents, and delete them in one confirmed batch. Delete removes SQLite metadata, section rows, FTS rows, folder organization, and stored source directories. Documents referenced by saved audiobooks cannot be deleted until their audio is removed from the Audiobooks tab; the same native guard covers single and batch deletion. Progress is count-based, partial failures are listed, and failed documents remain selected for retry.

Generic document import remains separate from `.papercut-audiobook` import/export. A PDF audiobook bundle restores its canonical PDF through the same PDF import/index path rather than copying derived page text, search rows, or thumbnails. EPUB implementation notes live in [epub-implementation-plan.md](epub-implementation-plan.md); PDF/OCR decisions and remaining work live in [pdf-ocr-scanning.md](pdf-ocr-scanning.md).

## Code Map

Frontend:

- `src/uploads/DocumentUploads.ts` is the small client API for user-upload commands and shared TypeScript types, including the persisted processing/ready/recognition-required text status.
- `src/App.tsx` wires upload/search state into reusable hooks and components, and provides source loading for uploaded URLs.
- `src/components/DocumentsPanel/DocumentsPanel.tsx` owns the document dropdown UI, including the option-driven Import menu, Saved audio filtering, uploaded-document management, active filter chips, and the persisted Gallery/List preference. Gallery is the default for users without a saved preference; `LibraryGalleryView` derives EPUB/PDF books from the existing format field, preserves bundled HTML folders in its Documents category, and keeps other HTML documents in the shared dense list. Newly imported EPUBs retain declared EPUB 2/3 raster covers and expose persisted display-sized thumbnails to visible gallery cards through a validated, size-bounded read command. Existing retained covers are thumbnailed lazily, while coverless imports keep the generated title placeholder.
- `src/components/DocumentBrowser/bundledDocuments.ts` derives the read-only bundled-document hierarchy from canonical `/documents/...` URLs, and `src/components/BundledDocumentTree/BundledDocumentTree.tsx` renders it consistently in the Library list, Gallery Documents category, and Search scope. Vite and Pagefind already preserve those paths, so bundled folders do not need a second manifest or SQLite organization records.
- `src/components/UploadedLibraryTree/UploadedLibraryTree.tsx` renders uploaded-document organization using React Aria Components tree primitives. `src/components/SearchScope/SearchScope.tsx` reuses the same tree for the Search tab's **Filter By Document** panel. Active text filters hide empty branches and temporarily expand the ancestor folders of matching documents without replacing the user's saved expansion state. Library **Manage** keeps its contextual select-all, move, batch-delete, and create/rename/delete-empty-folder actions visible while the document list scrolls. Document and folder selection remain mutually exclusive so each action has one clear target type.
- `src/components/DocumentInfoDialog/DocumentInfoDialog.tsx` displays retained upload metadata and edits only the app-owned display title. Rust updates the document row and duplicated FTS titles in one SQLite transaction; source bytes and saved-audiobook title snapshots remain unchanged.
- `src/components/DocumentViewer/DocumentViewer.tsx` owns the reader shell, viewer plugin resolution, in-document Find, same-document link scrolling, scroll-to-top behavior, and the slots used by TTS controls/diagnostics. `src/viewers/PdfViewer.tsx` adapts PDF.js rendering, Find, bookmarks, page/zoom controls, and TTS highlights without loading PDF support during non-PDF startup.
- `src/pdf/pdfImport.ts` incrementally extracts PDF pages through PDF.js, persists one bounded page layer at a time, finalizes the shared search index, and reuses the first rendered page for a gallery thumbnail.
- Search-result clicks can pass a lightweight reader target into `DocumentViewer`: a Pagefind heading hash when available and/or the first highlighted snippet text. The reader keeps the clean document URL, then jumps to the likely match after the document renders. Reader text matching spans inline markup such as emphasis, links, and bold text, treats explicit line breaks as normalized whitespace, and does not cross readable block boundaries. The preferred visual marker is a named CSS Highlight range so large iOS/WebKit reader DOMs are not rewritten just to mark a search result.
- `src/hooks/useDocumentFilters.ts` owns document filter text, selected filters, author grouping, collapsed groups, and the optional inclusion predicate used by the Saved audio filter.
- `src/hooks/useSearch.ts` owns the combined Pagefind + SQLite query flow and maps uploaded matches into the shared `SearchResult` shape.

Rust:

- `src-tauri/src/document_uploads/` owns the runtime upload feature, split one concern per file (dependencies point downward, currently `commands → { batch, pipeline, organization, search, store } → { epub, html, pdf, parsed, storage, types }`):
  - `commands.rs` — the `#[tauri::command]` edge; each command just moves the blocking work onto the thread pool and delegates.
  - `batch.rs` / `state.rs` — sequential multi-file/folder import and document deletion, progress events, partial-success results, and cooperative import cancellation between files.
  - `pipeline.rs` — import / get-source / delete orchestration (no SQL or parsing of its own).
  - `html/` — HTML-specific parsing (`parser.rs`), sanitization (`sanitize.rs`), and small shared HTML helpers (`util.rs`).
  - `epub/` — EPUB ZIP/container/OPF/spine parsing, with path resolution (`paths.rs`), bounded image inlining (`assets.rs`), DOM-based link/resource rewriting (`rewrite.rs`), and generated reading HTML assembly (`render.rs`) split into focused helpers.
  - `pdf/` — canonical PDF source storage, bounded per-page PDF.js text sidecars, index finalization, and page-aware narration reconstruction.
  - `parsed.rs` — format-neutral `ParsedDocument` / `ParsedSection` shape used by HTML, EPUB, PDF index finalization, and storage.
  - `store.rs` — SQLite schema, the index write path, listing, and deletes.
  - `organization.rs` — uploaded-document folder and manual ordering metadata. It never rewrites document URLs or stored source files, so folder moves do not invalidate search rows, saved audiobook ids, or TTS highlight mapping.
  - `search.rs` — FTS5 query building and execution (read-only).
  - `storage.rs` — app-data paths, upload ids, size accounting, clock, and the URL-prefix/size-limit constants.
  - `types.rs` — serde DTOs shared across the boundary.
- `src-tauri/src/lib.rs` registers the Tauri commands, referenced through the `document_uploads::commands` path so the macro-generated command helpers resolve.
- `src-tauri/Cargo.toml` includes `rusqlite` with the bundled SQLite feature so FTS5 support is available consistently across supported build targets. SHA-256 provides stable source identities for exact duplicate detection. EPUB parsing uses focused crates for ZIP, XML, sanitization, DOM rewriting, base64 image data URLs, and percent-decoded archive hrefs instead of handwritten decoders or tag scanners. The DOM rewriter is post-sanitizer plumbing, not the security boundary, so it can be swapped if a better-maintained HTML mutation crate fits later.

Storage:

- Sanitized uploaded HTML is stored under Tauri app data at `document_uploads/{upload_id}/source.html`.
- Canonical uploaded PDFs are stored at `document_uploads/{upload_id}/source.pdf`; rebuildable PDF.js page layers live beside the source and feed search, navigation, and TTS locators.
- New upload ids are full SHA-256 hashes of the original file bytes. Existing timestamp-derived ids remain valid; the first re-import of a document created by an older app version may create one hash-identified copy, after which exact re-imports reuse it.
- File and folder entry points share the same picker-independent per-file importer, so limits, parsing, duplicate handling, storage, and indexing cannot drift between UI paths. Their completion notice distinguishes documents that were added, already in the Library, or failed; reused and failed source filenames are available in expandable lists rather than hover-only details.
- **Files** can select one or up to 500 HTML, EPUB, or PDF files. A single successful import opens immediately; larger selections remain in the Library with count-based progress, the current filename, cooperative cancellation between files, retained successful files when siblings fail, and an expandable per-file failure summary. Successful and cancelled import notices dismiss automatically after six seconds; failures remain available until dismissed.
- Android document providers may expose selected files through extensionless content URLs. In that ambiguous case Papercut reads only a small prefix to distinguish ZIP-based EPUB, PDF, or HTML, then runs the same full format-specific validation used for named files.
- Desktop builds can select one folder and feed up to 500 HTML, EPUB, or PDF files into that same batch pipeline. Papercut recreates the selected folder and supported descendants through five visible Library levels, skips symlinks and deeper descendants, and gives a conflicting top-level folder a numeric suffix instead of merging it. Exact duplicate documents already in the Library are reported and remain in their current location.
- Batch deletion is sequential and bounded like import: the frontend subscribes before invoking the native command, displays count progress, refreshes shared library state once, and keeps only failed documents selected after a partial result. Folder deletion remains metadata-only and requires an empty folder.
- EPUB stores a sanitized generated reading HTML copy at the same stored-source path. Search and TTS depend on the generated safe reading copy and normalized sections, not on rendering the raw EPUB archive in React. Local PNG, JPEG, GIF, and WebP manifest images referenced by retained reader content are inlined as data URLs with a 5 MB per-image cap and 30 MB total-image cap; remote images and SVG are skipped. The original EPUB archive is not retained by the current MVP.
- PDF retains the original source because PDF.js needs it for rendering. Derived page text, FTS rows, thumbnails, and text-readiness status can be rebuilt from that canonical source and are not embedded in portable audiobook bundles.
- The runtime search index lives at `document_uploads/search.sqlite3`.
- Uploaded-document folders and manual order live in SQLite metadata tables beside the search index. Existing uploaded documents are assigned root-level locations automatically. Moving a document between folders changes only organization metadata, not the uploaded document URL, stored source, FTS rows, or audiobook cache identity.
- Original filenames are nullable because older uploads did not retain them. Papercut reports those values as unavailable instead of guessing from parsed titles or app-owned storage names.

## Frontend And Viewer Architecture

The detailed upload and viewer architecture lives here instead of the README so the README can stay focused on setup, builds, and release usage. Keep this section current when upload formats, viewer routing, or search ownership changes.

The frontend keeps upload, search, and viewing responsibilities separated:

- `src/App.tsx` is the composition point for search, library, reader, audiobook, and upload state. It wires the pieces together but delegates upload commands, filtering, search, and rendering to narrower modules.
- `src/uploads/DocumentUploads.ts` is the upload API boundary. React code calls these helpers instead of invoking Tauri commands directly throughout the UI.
- `src/hooks/useSearch.ts` merges bundled Pagefind results with uploaded-document SQLite results and returns the shared `SearchResult` shape.
- `src/components/DocumentsPanel/DocumentsPanel.tsx` owns the library-facing import/delete/filter controls. Import options stay option-driven so generic document import and audiobook bundle import can appear together without sharing backend code.
- `src/components/DocumentViewer/DocumentViewer.tsx` owns the reader chrome: Back, Find, reader settings, header slots, same-document link scrolling, scroll-to-top behavior, loading/error display for document opens, and TTS highlight integration.

Viewer rendering is plugin-based:

- `src/viewers/registry.ts` chooses a `ViewerPlugin` by URL and optional document format.
- More specific URL formats must be registered before the HTML fallback. PDF resolves to the PDF.js viewer; raw `.epub` remains reserved ahead of the catch-all HTML viewer.
- `src/viewers/HtmlViewer.tsx` parses the stored full HTML document, extracts body content, and renders it into an app-owned sanitized reader surface instead of a `srcDoc` iframe. Imported head styles are intentionally not injected into the app DOM. Reader settings apply through CSS variables on the viewer shell, so changing font, font size, line height, or width does not rewrite stored source or invalidate audiobook metadata.
- Uploaded EPUB documents currently resolve to the HTML viewer because their stored source is generated reading HTML. The shared DOM reader handles generated hash links so TOC entries and footnotes scroll within the stored document. TTS highlighting caches the generated reader DOM while it is stable and invalidates those caches when Find or reader updates replace text nodes. A richer EPUB viewer can replace that later if it declares which reader capabilities it supports, because Find, scrolling, TTS highlighting, and locator navigation may differ by format.
- App theme is separate from reader settings. `src/hooks/useTheme.ts` persists Light, System, and Dark choices and writes the resolved theme to the root element. HTML/EPUB readers can independently use Default, an opaque low-glare Gray, or AMOLED Black page colors through reader-scoped semantic CSS tokens; changing page color does not rewrite the rendered DOM, stored documents, search data, TTS chunks, or highlight locators. The PDF viewer themes its chrome but leaves rendered page contents unchanged and does not expose reader page colors.

This keeps the runtime upload pipeline independent from the viewer shell. The upload backend produces safe stored source and normalized searchable sections; the viewer shell decides how the document is presented and how reader-level controls attach to it.

Reader typography is intentionally owned by the shared DOM reader, not by each upload parser:

- The reader bundles offline fonts under `public/fonts/reader/` so desktop and Android do not collapse every serif choice into the same platform fallback.
- Literata is the default long-form reading face, Atkinson Hyperlegible is available for accessibility-focused reading, and system serif/sans remain available for users who prefer platform defaults.
- Naskh Arabic, Droid Arabic Naskh, Scheherazade New, and Readex Pro are exposed as explicit Arabic-focused options. They are not inserted into every stack because Arabic font metrics can change spacing and line flow in documents that previously rendered better with their platform fallback.
- Font, spacing, width, and page-color controls are reader presentation only. They do not rewrite stored HTML, EPUB-generated reading HTML, search sections, TTS chunks, saved audiobook metadata, or highlight locators.

## Import Pipeline

The runtime upload path follows one shared persistence/search contract with format-specific parsing:

1. **Pick source**: a native Tauri dialog selects one file, multiple files, or one desktop folder. Selection stays separate from the per-file importer so every picker reuses the same validation and persistence path.
2. **Validate input**: enforce size limits, derive the stable source id, reuse an exact existing upload when present, and decode HTML bytes before parsing.
3. **Parse format**: HTML and EPUB become safe readable blocks; PDF.js extracts bounded page text and page locators.
4. **Sanitize source**: remove active or risky HTML before storing/rendering it; retain validated PDF bytes as the canonical render source.
5. **Normalize sections**: convert the document into title + ordered text sections with optional headings.
6. **Store source**: save the sanitized viewable document to app data.
7. **Index sections**: write metadata and sections to SQLite, then populate the FTS5 table.
8. **Render/search**: React opens the stored source and searches through the same result-card UI as bundled docs.
9. **Delete**: when requested, Rust serializes deletion with the short save-start transaction, then checks whether any saved or in-progress audiobook manifest references the document and refuses deletion if so. A save that wins this ordering publishes its pending manifest before generation; a deletion that wins causes a later save to reject the missing source. Otherwise deletion stages the stored source directory, removes the document rows in one SQLite transaction, and then removes the staged files. A database failure restores the directory. Batch requests are capped at 500 upload URLs, deduplicated before execution, run sequentially, and retain per-document failures alongside successes.

EPUB plugs in at step 3 by validating the ZIP/container, reading OPF metadata and spine order, sanitizing each XHTML spine item, generating safe reading HTML, and outputting the same normalized section shape. PDF stages a canonical source in Rust, lets PDF.js produce bounded page records, then finalizes those records through the same SQLite document/FTS store. Its architecture, research, and remaining staged checklist live in [pdf-ocr-scanning.md](pdf-ocr-scanning.md).

## Search Flow

When a user submits a search:

1. React lowercases and normalizes the query.
2. **Filter By Document** treats checked documents as either the included search scope or the documents excluded from it. With no checked documents, search uses the full library.
3. Pagefind searches bundled build-time documents when the Pagefind index is available, then `useSearch` consumes matching results from the resolved allowed-URL set before applying its visible-result limit.
4. SQLite FTS5 searches uploaded runtime documents when the app is running in Tauri, with allowed upload URLs passed into the SQL query when a scope is active.
5. `useSearch` maps both providers into the existing `SearchResult` shape.
6. Uploaded section matches are collapsed to one result per uploaded document, keeping the first/best SQLite snippet for the document card.
7. Quoted exact-phrase searches use Pagefind/SQLite only as a candidate finder, then verify the phrase against the real bundled or uploaded document source before rendering results.
8. Results are combined and rendered in the same panel.
9. Opening a result keeps the canonical document URL, then passes a best-effort target to the reader so Pagefind heading hashes or highlighted snippet text can scroll to the likely match. Search-result navigation and in-document Find share block-aware text matching, so inline formatting does not split a visible phrase into separate matches. Starting an explicit Find retires the transient result highlight, caches the rendered reader's normalized text map, and uses CSS highlights without rewriting the reader DOM; audiobook playback highlights remain independent.
10. If a result points to an uploaded document URL, source loading calls `document_uploads_get_source` instead of fetching from `dist/`.

SQLite FTS uses a Porter/unicode tokenizer with diacritic removal and BM25 ranking. Uploaded snippets are generated by SQLite and sanitized again in React before rendering, which keeps `<mark>` highlighting but prevents snippet HTML from becoming executable UI. Because the reader has its own in-document Find/highlight workflow, the search panel shows one uploaded-document card with the first relevant snippet instead of one card per matching section.

For quoted searches, broad provider counts are not treated as exact phrase counts. The UI reports document-level phrase matches after source verification and shows source-verified occurrence counts on result cards; unquoted searches can still show matching section counts when SQLite or Pagefind exposes them.

## Performance Notes

SQLite FTS is a good fit for user uploads because it indexes incrementally. Importing one file touches one source file and one local database transaction; it does not regenerate the shipped corpus index.

For 500+ user documents, the important scaling rules are:

- Keep indexing per document incremental.
- Store text as sections/pages/chapters instead of one huge blob per document.
- Query only on explicit Search/Enter, not on every keystroke.
- Limit initial result counts and fetch/open full source only when the user chooses a document.
- Keep heavy extraction off the WebView main thread: Rust handles HTML/EPUB parsing and PDF.js uses its worker for PDF parsing.

The current HTML, EPUB, and PDF paths follow those rules. PDF extraction is sequential by page, search uses persisted SQLite page rows, and the viewer relies on PDF.js virtualization rather than mounting every rendered page eagerly.

HTML/EPUB in-document Find builds one compact text-to-DOM index per opened document and reuses it as the query changes. Match counting and next/previous navigation still cover the complete rendered document. Visual highlighting is capped for extremely common queries so a one-character search cannot create tens of thousands of ranges or DOM elements; the current match remains highlighted and navigable. Older WebViews without CSS Custom Highlights fall back to the browser selection for the current match rather than mutating the document.

TTS highlighting currently maps saved audiobook chunks back onto the live reader DOM. The hook caches the reader's text-node segment index while active playback highlighting needs it, then invalidates that cache when document loading or another reader mutation replaces text nodes under the same root. Find uses CSS ranges without changing those nodes, so changing a Find query no longer invalidates TTS alignment. That keeps existing saved audio compatible and avoids rescanning on every chunk advance or immediately after merely opening a large book, but a very large fully-rendered book can still pay a one-time segment-index rebuild when playback highlighting first starts after a mutation. New audiobook bundle exports preserve optional chunk source spans, so re-imported HTML/EPUB generated-reader bundles can often highlight without text rediscovery. Imported bundles without stored spans keep a compatibility fallback for legacy HTML: when rebuilt spans are unavailable or validate against the wrong text, the reader can build one cached text map from the live DOM and match the current chunk text back to a source span. Exact matching stays first; the fallback also tolerates Arabic Unicode differences such as tashkeel, tatweel, bidi controls, and common Arabic/Persian letter variants while mapping matches back to real DOM offsets. This work is lazy and cached, and the audio controls surface a short "Preparing highlights..." notice while imported highlight spans are being rebuilt. The fallback is bounded to the rendered document text, not repeated for every chunk, but it still depends on the whole rendered DOM. The long-term scaling path is chapter/page-level rendering with locator-aware TTS ranges, so the app only indexes the visible or active chapter instead of one giant reader DOM.

## Sanitization And Format Modules

Yes, upload formats should have separate sanitization/parser modules. HTML, PDF, and EPUB have different risks and extraction behavior:

- HTML needs active content stripping, URL cleanup, and safe stored rendering.
- PDF needs text-layer extraction, page boundaries, metadata extraction, and possibly OCR later.
- EPUB needs ZIP/container validation, OPF spine parsing, XHTML sanitization, and chapter ordering.

The shared indexed metadata should be boring and stable:

`{ title, format, sections: [{ ordinal, heading?, text, locator? }], cover? }`

Keeping this shape stable lets the Library and SQLite indexing remain format-agnostic while each viewer loads its own stored source type. See [epub-implementation-plan.md](epub-implementation-plan.md) for the ordered EPUB task list and acceptance checks, and [pdf-ocr-scanning.md](pdf-ocr-scanning.md) for the PDF, OCR, and mobile scanning plan.

## Current Limitations

- Runtime import supports HTML, EPUB, text-native PDF, and local English or Arabic recognition for image-only or hybrid PDFs. Pages with usable native text and intentionally blank pages are preserved, while image-backed pages without usable text are recognized. Additional recognition languages remain deferred until their local assets and acceptance cases are ready.
- HTML files must be at most 25 MB. UTF-8 is used directly; non-UTF-8 HTML can import when it declares a supported browser charset such as Windows-1252.
- EPUB files must be readable ZIP-based EPUB archives with a valid container and OPF spine, and must be at most 100 MB. Only local PNG, JPEG, GIF, and WebP images are retained, with 5 MB per image and 30 MB total reader-image caps. A declared EPUB 2/3 cover is retained beside the generated source only after the bounded thumbnail decoder validates it under the same 5 MB per-image cap; an invalid optional cover falls back to the generated title placeholder without failing the document import. The Library gallery serves the persisted display-sized PNG thumbnail rather than decoding the original cover. Existing imports with retained covers receive that thumbnail lazily on first gallery access; imports without cover metadata are not retroactively reparsed.
- PDF files must have a valid PDF signature and be at most 250 MB and 2,000 pages. Search and TTS require native extracted text or a completed supported recognition job. PDF.js rendering remains page-faithful and does not recolor page contents for app themes.
- HTML and EPUB content use the parser-based `ammonia` allowlist before storage. The shared reader boundary validates browser-decoded URL schemes, preserves same-document footnote anchors and EPUB-generated raster images, and strips active elements, event handlers, inline styles, remote images, SVG data URLs, and unsupported links. Uploaded documents and audiobook-bundle sources are sanitized again when read so files retained from older app versions receive the current policy before entering the app DOM.
- Uploaded-document search only runs inside the Tauri app, not plain browser preview.
- Folder import is desktop-only. It preserves supported files through five visible folder levels, skips deeper descendants and symlinks, and leaves already-imported duplicate documents in their current Library location.
- There is no user-facing reindex action for generic uploaded documents yet.
- Uploaded documents are not exported as part of a library backup yet.
- Quoted exact-phrase results are document-level matches with source-verified occurrence counts, not provider section counts.
- Very large HTML/EPUB books currently render as one generated reader DOM. The app now treats document opening as one global reader transition, disables competing View/Open actions while source HTML loads, avoids DOM mutation for search-result target highlights on modern WebViews, and avoids building the TTS text-node index until playback highlighting needs it. The first audiobook highlight after a large DOM mutation may still rebuild the text segment index. Chapter-level rendering with locator-aware highlighting is the preferred long-term fix for those formats; PDF already uses page virtualization and page-aware locators.
- Light/Dark/System theme selection is UI state, not document state. Theme polish should stay in CSS tokens where possible so imported HTML, generated EPUB HTML, and saved audiobook highlighting remain stable across theme changes.

## Recommended Next Steps

1. Add more EPUB parser fixtures for malformed OPF/container cases, spine edge cases, oversized image skipping, and metadata fallback.
2. Include retained covers in library-transfer packages so covers survive device-to-device transfer.
3. Add a reindex action for uploaded documents if parser or sanitizer behavior changes after import.
4. Add richer EPUB reader features such as TOC, location restore, pagination, EPUB-specific appearance controls, or a foliate-js/epub.js-backed viewer if generated reading HTML is not enough.
5. Follow [pdf-ocr-scanning.md](pdf-ocr-scanning.md) for remaining PDF parity, OCR, and mobile scanning work; do not implement those concerns as isolated additions here.
6. Decide whether Pagefind remains the bundled-document engine long term or whether all documents should eventually share SQLite FTS.
