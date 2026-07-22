# User Document Uploads And Search Indexing

Papercut has two search-indexing paths on purpose:

- **Bundled documents** are known at build time and are indexed by Pagefind into the production frontend bundle.
- **User uploads** are unknown until runtime, so they are imported through Tauri and indexed into a local SQLite FTS5 database in app data.

This avoids the trap of trying to rebuild Pagefind on a user's device every time they add a document. Pagefind remains excellent for the shipped corpus, while SQLite FTS gives us incremental offline search for user-owned content.

## Current Scope

The current upload branch supports local HTML and EPUB files:

- Users open **Import** from the document list and choose **Files** for one or more HTML/EPUB files, or **Folder** for a desktop folder.
- Tauri opens the native filesystem picker for `.html`/`.htm` or `.epub` files. Desktop folder import includes supported files directly inside the selected folder and skips subfolders.
- Rust reads the selected HTML file, enforces a 25 MB limit, decodes UTF-8 or declared legacy browser encodings, sanitizes the HTML, extracts readable sections, stores the sanitized source, and indexes the sections into SQLite FTS5.
- Rust reads the selected EPUB file, enforces a 100 MB limit, validates the EPUB ZIP/container, follows OPF spine order, sanitizes XHTML chapters into generated reading HTML, rewrites and target-validates local chapter, TOC, and footnote links, retains supported local raster images within safety caps, extracts readable sections, and indexes the sections into SQLite FTS5.
- New imports use the original file's SHA-256 as their stable upload id. Selecting the same unchanged file again reuses its existing stored document instead of creating another copy.
- React lists imported files under **User Uploads** and opens them through the shared document reader. EPUB uses the generated reading HTML for the first release.
- Search queries run through `src/hooks/useSearch.ts`, which queries Pagefind and SQLite FTS in parallel and returns one shared result shape.
- Users can open **Manage**, select uploaded HTML and EPUB documents, and delete them in one confirmed batch. Delete removes SQLite metadata, section rows, FTS rows, folder organization, and stored source directories. Progress is count-based, partial failures are listed, and failed documents remain selected for retry.

This path is intentionally independent from `.papercut-audiobook` import/export. Audiobook bundle import remains TTS-specific, while generic document import is designed to be shippable as its own branch. EPUB implementation notes and remaining reader-quality work live in [epub-implementation-plan.md](epub-implementation-plan.md).

## Code Map

Frontend:

- `src/uploads/DocumentUploads.ts` is the small client API for user-upload commands and shared TypeScript types.
- `src/App.tsx` wires upload/search state into reusable hooks and components, and provides source loading for uploaded URLs.
- `src/components/DocumentsPanel/DocumentsPanel.tsx` owns the document dropdown UI, including the option-driven Import menu, Saved audio filtering, uploaded-document management, and active filter chips. Uploaded documents can be rendered through the folder-aware tree UI, while bundled documents and audiobook imports keep the existing grouped list. Developer Mode also exposes an experimental Gallery/List preference; `LibraryGalleryView` derives EPUB/PDF books from the existing format field and keeps HTML documents in the shared dense list. Newly imported EPUBs retain declared EPUB 2/3 raster covers and expose them to visible gallery cards through a validated, size-bounded read command. Existing and coverless imports keep the generated title placeholder.
- `src/components/UploadedLibraryTree/UploadedLibraryTree.tsx` renders uploaded-document organization using React Aria Components tree primitives. `src/components/SearchScope/SearchScope.tsx` reuses the same tree for the Search tab's **Filter By Document** panel, while Library **Manage** mode owns select-all, move, batch-delete, and create/rename/delete-empty-folder actions. Document and folder selection remain mutually exclusive so each action has one clear target type.
- `src/components/DocumentViewer/DocumentViewer.tsx` owns the reader shell, viewer plugin resolution, in-document Find, same-document link scrolling, scroll-to-top behavior, and the slots used by TTS controls/diagnostics.
- Search-result clicks can pass a lightweight reader target into `DocumentViewer`: a Pagefind heading hash when available and/or the first highlighted snippet text. The reader keeps the clean document URL, then jumps to the likely match after the document renders. Reader text matching spans inline markup such as emphasis, links, and bold text without crossing readable block boundaries. The preferred visual marker is a named CSS Highlight range so large iOS/WebKit reader DOMs are not rewritten just to mark a search result.
- `src/hooks/useDocumentFilters.ts` owns document filter text, selected filters, author grouping, collapsed groups, and the optional inclusion predicate used by the Saved audio filter.
- `src/hooks/useSearch.ts` owns the combined Pagefind + SQLite query flow and maps uploaded matches into the shared `SearchResult` shape.

Rust:

- `src-tauri/src/document_uploads/` owns the runtime upload feature, split one concern per file (dependencies point downward, currently `commands → { batch, pipeline, organization, search, store } → { epub, html, parsed, storage, types }`):
  - `commands.rs` — the `#[tauri::command]` edge; each command just moves the blocking work onto the thread pool and delegates.
  - `batch.rs` / `state.rs` — sequential multi-file/folder import and document deletion, progress events, partial-success results, and cooperative import cancellation between files.
  - `pipeline.rs` — import / get-source / delete orchestration (no SQL or parsing of its own).
  - `html/` — HTML-specific parsing (`parser.rs`), sanitization (`sanitize.rs`), and small shared HTML helpers (`util.rs`).
  - `epub/` — EPUB ZIP/container/OPF/spine parsing, with path resolution (`paths.rs`), bounded image inlining (`assets.rs`), DOM-based link/resource rewriting (`rewrite.rs`), and generated reading HTML assembly (`render.rs`) split into focused helpers.
  - `parsed.rs` — format-neutral `ParsedDocument` / `ParsedSection` shape used by HTML, EPUB, storage, and future PDF work.
  - `store.rs` — SQLite schema, the index write path, listing, and deletes.
  - `organization.rs` — uploaded-document folder and manual ordering metadata. It never rewrites document URLs or stored source files, so folder moves do not invalidate search rows, saved audiobook ids, or TTS highlight mapping.
  - `search.rs` — FTS5 query building and execution (read-only).
  - `storage.rs` — app-data paths, upload ids, size accounting, clock, and the URL-prefix/size-limit constants.
  - `types.rs` — serde DTOs shared across the boundary.
- `src-tauri/src/lib.rs` registers the Tauri commands, referenced through the `document_uploads::commands` path so the macro-generated command helpers resolve.
- `src-tauri/Cargo.toml` includes `rusqlite` with the bundled SQLite feature so FTS5 support is available consistently across supported build targets. SHA-256 provides stable source identities for exact duplicate detection. EPUB parsing uses focused crates for ZIP, XML, sanitization, DOM rewriting, base64 image data URLs, and percent-decoded archive hrefs instead of handwritten decoders or tag scanners. The DOM rewriter is post-sanitizer plumbing, not the security boundary, so it can be swapped if a better-maintained HTML mutation crate fits later.

Storage:

- Sanitized uploaded HTML is stored under Tauri app data at `document_uploads/{upload_id}/source.html`.
- New upload ids are full SHA-256 hashes of the original file bytes. Existing timestamp-derived ids remain valid; the first re-import of a document created by an older app version may create one hash-identified copy, after which exact re-imports reuse it.
- File and folder entry points share the same picker-independent per-file importer, so limits, parsing, duplicate handling, storage, and indexing cannot drift between UI paths.
- **Files** can select one or up to 500 HTML/EPUB files. A single successful import opens immediately; larger selections remain in the Library with count-based progress, the current filename, cooperative cancellation between files, retained successful files when siblings fail, and an expandable per-file failure summary.
- Android document providers may expose selected files through extensionless content URLs. In that ambiguous case Papercut reads only a small prefix to distinguish ZIP-based EPUB from HTML, then runs the same full size, EPUB-container, parser, and sanitizer validation used for named files.
- Desktop builds can select one folder and feed up to 500 direct HTML/EPUB children into that same batch pipeline. Folder traversal is non-recursive and does not recreate the filesystem folder in Papercut.
- Batch deletion is sequential and bounded like import: the frontend subscribes before invoking the native command, displays count progress, refreshes shared library state once, and keeps only failed documents selected after a partial result. Folder deletion remains metadata-only and requires an empty folder.
- EPUB stores a sanitized generated reading HTML copy at the same stored-source path. Search and TTS depend on the generated safe reading copy and normalized sections, not on rendering the raw EPUB archive in React. Local PNG, JPEG, GIF, and WebP manifest images referenced by retained reader content are inlined as data URLs with a 5 MB per-image cap and 30 MB total-image cap; remote images and SVG are skipped. The original EPUB archive is not retained by the current MVP.
- The runtime search index lives at `document_uploads/search.sqlite3`.
- Uploaded-document folders and manual order live in SQLite metadata tables beside the search index. Existing uploaded documents are assigned root-level locations automatically. Moving a document between folders changes only organization metadata, not the uploaded document URL, source HTML, FTS rows, or audiobook cache identity.

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
- More specific URL formats must be registered before the HTML fallback. PDF and raw `.epub` entries remain reserved ahead of the catch-all HTML viewer.
- `src/viewers/HtmlViewer.tsx` parses the stored full HTML document, extracts body content, and renders it into an app-owned sanitized reader surface instead of a `srcDoc` iframe. Imported head styles are intentionally not injected into the app DOM. Reader settings apply through CSS variables on the viewer shell, so changing font, font size, line height, or width does not rewrite stored source or invalidate audiobook metadata.
- Uploaded EPUB documents currently resolve to the HTML viewer because their stored source is generated reading HTML. The shared DOM reader handles generated hash links so TOC entries and footnotes scroll within the stored document. TTS highlighting caches the generated reader DOM while it is stable and invalidates those caches when Find or reader updates replace text nodes. A richer EPUB viewer can replace that later if it declares which reader capabilities it supports, because Find, scrolling, TTS highlighting, and locator navigation may differ by format.
- App theme is separate from reader settings. `src/hooks/useTheme.ts` persists Light, System, and Dark choices and writes the resolved theme to the root element, while CSS tokens theme app chrome, dialogs, HTML/EPUB reader content, Find marks, and TTS highlights without changing stored documents or saved audiobook metadata. Future PDF viewing should keep the app chrome themed but should not assume PDF page contents can be recolored safely.

This keeps the runtime upload pipeline independent from the viewer shell. The upload backend produces safe stored source and normalized searchable sections; the viewer shell decides how the document is presented and how reader-level controls attach to it.

Reader typography is intentionally owned by the shared DOM reader, not by each upload parser:

- The reader bundles offline fonts under `public/fonts/reader/` so desktop and Android do not collapse every serif choice into the same platform fallback.
- Literata is the default long-form reading face, Atkinson Hyperlegible is available for accessibility-focused reading, and system serif/sans remain available for users who prefer platform defaults.
- Naskh Arabic, Droid Arabic Naskh, Scheherazade New, and Readex Pro are exposed as explicit Arabic-focused options. They are not inserted into every stack because Arabic font metrics can change spacing and line flow in documents that previously rendered better with their platform fallback.
- These controls are reader presentation only. They do not rewrite stored HTML, EPUB-generated reading HTML, search sections, TTS chunks, saved audiobook metadata, or highlight locators.

## Import Pipeline

The runtime upload path follows a parser pipeline that can be reused for future formats:

1. **Pick source**: a native Tauri dialog selects one file, multiple files, or one desktop folder. Selection stays separate from the per-file importer so every picker reuses the same validation and persistence path.
2. **Validate input**: enforce size limits, derive the stable source id, reuse an exact existing upload when present, and decode HTML bytes before parsing.
3. **Parse format**: HTML is parsed into a title and readable content blocks.
4. **Sanitize source**: remove active or risky HTML before storing/rendering it.
5. **Normalize sections**: convert the document into title + ordered text sections with optional headings.
6. **Store source**: save the sanitized viewable document to app data.
7. **Index sections**: write metadata and sections to SQLite, then populate the FTS5 table.
8. **Render/search**: React opens the stored source and searches through the same result-card UI as bundled docs.
9. **Delete**: when requested, Rust stages the stored source directory, removes the document rows in one SQLite transaction, and then removes the staged files. A database failure restores the directory. Batch requests are capped at 500 upload URLs, deduplicated before execution, run sequentially, and retain per-document failures alongside successes.

EPUB plugs in at step 3 by validating the ZIP/container, reading OPF metadata and spine order, sanitizing each XHTML spine item, generating safe reading HTML, and outputting the same normalized section shape. PDF should later plug into the same shared store/search path with page-aware locators and a PDF-specific viewer.

## Search Flow

When a user submits a search:

1. React lowercases and normalizes the query.
2. Selected **Filter By Document** URLs are treated as the active search scope.
3. Pagefind searches bundled build-time documents when the Pagefind index is available, then `useSearch` keeps matching results that are inside the selected scope.
4. SQLite FTS5 searches uploaded runtime documents when the app is running in Tauri, with selected upload URLs passed into the SQL query when a scope is active.
5. `useSearch` maps both providers into the existing `SearchResult` shape.
6. Uploaded section matches are collapsed to one result per uploaded document, keeping the first/best SQLite snippet for the document card.
7. Quoted exact-phrase searches use Pagefind/SQLite only as a candidate finder, then verify the phrase against the real bundled or uploaded document source before rendering results.
8. Results are combined and rendered in the same panel.
9. Opening a result keeps the canonical document URL, then passes a best-effort target to the reader so Pagefind heading hashes or highlighted snippet text can scroll to the likely match. Search-result navigation and in-document Find share block-aware text matching, so inline formatting does not split a visible phrase into separate matches. Starting an explicit Find retires the transient result highlight before Find marks the reader DOM, while audiobook playback highlights remain independent.
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
- Keep format parsing out of React so the WebView does not lock up on large imports.

This branch already follows those rules for HTML and EPUB uploads. PDF can stay lightweight if it produces page records and avoids rendering or indexing entire binary files in the frontend.

TTS highlighting currently maps saved audiobook chunks back onto the live reader DOM. The hook caches the reader's text-node segment index while active playback highlighting needs it, then invalidates that cache when Find, document loading, or other reader mutations replace text nodes under the same root. That keeps existing saved audio compatible and avoids rescanning on every chunk advance or immediately after merely opening a large book, but a very large fully-rendered book can still pay a one-time segment-index rebuild when playback highlighting first starts after a mutation. New audiobook bundle exports preserve optional chunk source spans, so re-imported HTML/EPUB generated-reader bundles can often highlight without text rediscovery. Imported bundles without stored spans keep a compatibility fallback for legacy HTML: when rebuilt spans are unavailable or validate against the wrong text, the reader can build one cached text map from the live DOM and match the current chunk text back to a source span. Exact matching stays first; the fallback also tolerates Arabic Unicode differences such as tashkeel, tatweel, bidi controls, and common Arabic/Persian letter variants while mapping matches back to real DOM offsets. This work is lazy and cached, and the audio controls surface a short "Preparing highlights..." notice while imported highlight spans are being rebuilt. The fallback is bounded to the rendered document text, not repeated for every chunk, but it still depends on the whole rendered DOM. The long-term scaling path is chapter/page-level rendering with locator-aware TTS ranges, so the app only indexes the visible or active chapter instead of one giant reader DOM.

## Sanitization And Format Modules

Yes, upload formats should have separate sanitization/parser modules. HTML, PDF, and EPUB have different risks and extraction behavior:

- HTML needs active content stripping, URL cleanup, and safe stored rendering.
- PDF needs text-layer extraction, page boundaries, metadata extraction, and possibly OCR later.
- EPUB needs ZIP/container validation, OPF spine parsing, XHTML sanitization, and chapter ordering.

The shared output should be boring and stable:

`{ title, format, viewHtml, sections: [{ ordinal, heading?, text, locator? }], cover? }`

Keeping this shape stable lets the UI and SQLite indexing remain format-agnostic. See [epub-implementation-plan.md](epub-implementation-plan.md) for the ordered EPUB task list and acceptance checks.

## Current Limitations

- Runtime import supports HTML and EPUB; PDF is not implemented yet.
- HTML files must be at most 25 MB. UTF-8 is used directly; non-UTF-8 HTML can import when it declares a supported browser charset such as Windows-1252.
- EPUB files must be readable ZIP-based EPUB archives with a valid container and OPF spine, and must be at most 100 MB. Only local PNG, JPEG, GIF, and WebP images are retained, with 5 MB per image and 30 MB total reader-image caps. A declared EPUB 2/3 cover is also retained beside the generated source under the same 5 MB per-image cap. Existing imports are not retroactively reparsed for covers.
- HTML and EPUB content use the parser-based `ammonia` allowlist before storage. The shared reader boundary validates browser-decoded URL schemes, preserves same-document footnote anchors and EPUB-generated raster images, and strips active elements, event handlers, inline styles, remote images, SVG data URLs, and unsupported links. Uploaded documents and audiobook-bundle sources are sanitized again when read so files retained from older app versions receive the current policy before entering the app DOM.
- Uploaded-document search only runs inside the Tauri app, not plain browser preview.
- Folder import is desktop-only, reads direct supported files only, and skips nested folders and symlinks.
- There is no user-facing reindex action for generic uploaded documents yet.
- Uploaded documents are not exported as part of a library backup yet.
- Quoted exact-phrase results are document-level matches with source-verified occurrence counts, not provider section counts.
- Very large uploaded books currently render as one generated reader DOM. The app now treats document opening as one global reader transition, disables competing View/Open actions while source HTML loads, avoids DOM mutation for search-result target highlights on modern WebViews, and avoids building the TTS text-node index until playback highlighting needs it. The first audiobook highlight after a large DOM mutation may still rebuild the text segment index. Chapter/page-level rendering with locator-aware highlighting is the preferred long-term fix.
- Light/Dark/System theme selection is UI state, not document state. Theme polish should stay in CSS tokens where possible so imported HTML, generated EPUB HTML, and saved audiobook highlighting remain stable across theme changes.

## Recommended Next Steps

1. Add more EPUB parser fixtures for malformed OPF/container cases, spine edge cases, oversized image skipping, and metadata fallback.
2. Include retained covers in library-transfer packages so covers survive device-to-device transfer.
3. Add a reindex action for uploaded documents if parser or sanitizer behavior changes after import.
4. Add richer EPUB reader features such as TOC, location restore, pagination, EPUB-specific appearance controls, or a foliate-js/epub.js-backed viewer if generated reading HTML is not enough.
5. Add a runtime PDF import module later that extracts page text and stores page records in the same SQLite schema.
6. Keep future viewer-specific theme work format-aware: EPUB/HTML can inherit CSS tokens, while PDF should theme surrounding controls without recoloring document pages by default.
7. Decide whether Pagefind remains the bundled-document engine long term or whether all documents should eventually share SQLite FTS.

## Branching Guidance

For the first shippable upload branch, keep the generic document work separate from TTS:

- Include `src/uploads/DocumentUploads.ts`.
- Include `src-tauri/src/document_uploads/` and the command registration in `src-tauri/src/lib.rs`.
- Include the `rusqlite` dependency.
- Include App UI changes for **Import > Files**, **User Uploads**, uploaded-document delete, and merged search.
- Exclude `.papercut-audiobook` import/export work if you want a non-TTS branch.

That gives users HTML import and runtime search first. TTS bundle import can remain a later branch that reuses the same reader/playback surface once the document source exists in the app.
