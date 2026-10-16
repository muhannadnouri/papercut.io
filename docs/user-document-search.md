# User Document Uploads And Search Indexing

Papercut has two search-indexing paths on purpose:

- **Bundled starter documents** are known at build time and are indexed by Pagefind into the production frontend bundle.
- **User uploads** are unknown until runtime, so they are imported through Tauri and indexed into a local SQLite FTS5 database in app data.

This avoids the trap of trying to rebuild Pagefind on a user's device every time they add a document. Pagefind remains the starter-document engine, while SQLite FTS gives us incremental offline search for user-owned content.

## Current Scope

Keyword search is the default. Native shipping builds also compile an optional,
default-off conceptual-search provider. Users explicitly download model files,
enable semantic search and build its index; searching or opening Settings starts
none of that work. Keyword and Conceptual have independent result views sharing
the submitted query and document scope.

This is the current behavior/architecture reference. [Search validation](search-validation.md)
owns CI evidence, owner acceptance, reproducible checks and the deferred manual
checklist. Historical development and rejected experiments live in Git, not in a
second maintained implementation guide.

The current app supports local HTML, EPUB, PDF, TXT, and Markdown files. PDF search and TTS
require a usable extracted text layer; fully textless PDFs remain readable and
are marked for opt-in English or Arabic text recognition:

- Users open **Import** from the document list and choose **Files** for one or more HTML, EPUB, PDF, TXT, or Markdown files, or **Folder** for a desktop folder. Desktop users can also drop supported files onto the open Library. Installed desktop, Android, and iOS builds can receive supported files from the operating system's file-opening UI.
- Tauri opens the native filesystem picker for `.html`/`.htm`, `.epub`, `.pdf`, `.txt`, `.md`, or `.markdown` files. Desktop folder import preserves the selected folder and supported subfolders through the Library's five visible levels (the selected folder plus four descendant levels). If its top-level name already exists, Papercut creates a suffixed folder such as `Books (2)` rather than merging the trees.
- Rust reads the selected HTML file, enforces a 25 MB limit, decodes UTF-8 or declared legacy browser encodings, sanitizes the HTML, extracts readable sections, stores the sanitized source, and indexes the sections into SQLite FTS5.
- Rust reads the selected EPUB file, enforces a 100 MB limit, validates the EPUB ZIP/container, follows OPF spine order, sanitizes XHTML chapters into generated reading HTML, rewrites and target-validates local chapter, TOC, and footnote links, retains supported local raster/SVG reader assets within safety caps, extracts readable sections, and indexes the sections into SQLite FTS5.
- Rust stages a selected PDF under a 250 MB limit, then the existing lazy-loaded PDF.js path extracts and stores bounded page text, page locators, and a best-effort first-page thumbnail before atomically finalizing its SQLite FTS5 rows. PDFs are limited to 2,000 pages. Fully textless and hybrid PDFs with image-backed pages lacking usable text remain renderable but are marked **Text Recognition Required**. **Recognize Text** runs the selected English or Arabic local Tesseract worker one page at a time, reports progress, supports cancellation, and exposes recognized text to search, Find, and TTS only after the existing atomic finalizer succeeds. Failed or conservatively low-confidence pages remain listed for targeted retry without reprocessing successful pages.
- New imports use the original file's SHA-256 as their stable upload id. Selecting the same unchanged file again reuses its existing stored document instead of creating another copy. Batch results report that source under **Already in Library** rather than counting it as newly added.
- New imports retain the original filename as read-only provenance. In Library list view, **Manage** exposes a pencil action for each uploaded document; its **Document Info** dialog lets users correct Papercut's display title without renaming the source, changing its stable URL, or invalidating folders, bookmarks, and saved audio.
- Library gallery and list views mark documents that have an explicit reader bookmark, and the Library toolbar can filter to those documents. The reader saves a first bookmark directly; an existing bookmark exposes explicit actions to return, move, or remove it, with Undo after a move or removal.
- React lists imported files under **User Uploads** and opens them through the shared document reader. EPUB uses generated reading HTML; PDF uses the dedicated PDF.js viewer.
- Search queries run through `src/hooks/useSearch.ts`, which queries Pagefind and SQLite FTS in parallel, keeps uploaded results first in one Keyword list and identifies bundled cards with a **Starter Document** badge.
- Users can open **Manage**, select uploaded documents, and delete them in one confirmed batch. Selecting a populated folder offers **Delete Folder and Contents** with the folder name plus exact nested-folder and document counts from the unfiltered hierarchy, even when a Library filter hides some descendants. Delete removes SQLite metadata, section rows, FTS rows, folder organization, and stored source directories. Documents referenced by saved audiobooks cannot be deleted until their audio is removed from the Audiobooks tab; the same native guard covers single, batch, and recursive-folder deletion. Progress is count-based and remains with the sticky Manage controls while scrolling; partial failures are listed, and the surviving folder remains selected for retry.

Generic document import remains separate from `.papercut-audiobook` import/export. A PDF audiobook bundle restores its canonical PDF through the same PDF import/index path rather than copying derived page text, search rows, or thumbnails. EPUB implementation notes live in [epub-implementation-plan.md](epub-implementation-plan.md); PDF/OCR decisions and remaining work live in [pdf-ocr-scanning.md](pdf-ocr-scanning.md).

## Code Map

Frontend:

- `src/uploads/DocumentUploads.ts` is the small client API for user-upload commands and shared TypeScript types, including the persisted processing/ready/recognition-required text status. Uploaded search gives each native invocation its own Tauri channel for sparse semantic progress, so overlapping searches cannot consume each other's stages.
- `src/App.tsx` wires upload/search state into reusable hooks and components, and provides source loading for uploaded URLs.
- `src/components/DocumentsPanel/DocumentsPanel.tsx` owns the document dropdown UI, including the option-driven Import menu, Saved audio filtering, uploaded-document management, active filter chips, and the persisted Gallery/List preference. Gallery is the default for users without a saved preference; `LibraryGalleryView` derives EPUB/PDF books from the existing format field, preserves bundled HTML folders in its Documents category, and keeps other HTML documents in the shared dense list. Newly imported EPUBs retain declared EPUB 2/3 raster covers and expose persisted display-sized thumbnails to visible gallery cards through a validated, size-bounded read command. Existing retained covers are thumbnailed lazily, while coverless imports keep the generated title placeholder.
- `src/components/DocumentBrowser/bundledDocuments.ts` derives the read-only bundled-document hierarchy from canonical `/documents/...` URLs, and `src/components/BundledDocumentTree/BundledDocumentTree.tsx` renders it consistently in the Library list, Gallery Documents category, and Search scope. Vite and Pagefind already preserve those paths, so bundled folders do not need a second manifest or SQLite organization records.
- `src/components/UploadedLibraryTree/UploadedLibraryTree.tsx` renders uploaded-document organization using React Aria Components tree primitives. `src/components/SearchScope/SearchScope.tsx` reuses the same tree for the Search tab's **Filter By Document** panel. Active text, Saved Audio, and Saved Bookmarks filters hide empty branches and temporarily expand the ancestor folders of matching documents without replacing the user's saved expansion state. Uploaded-folder rows use a familiar folder icon and larger coarse-pointer disclosure targets to remain distinct from documents in browse and Manage modes. Library **Manage** keeps its contextual select-all, move, batch-delete, and create/rename/recursive-delete actions visible while the document list scrolls. Document and folder selection remain mutually exclusive so each action has one clear target type.
- `src/components/DocumentInfoDialog/DocumentInfoDialog.tsx` displays retained upload metadata and edits only the app-owned display title. Rust updates the document row and duplicated FTS titles in one SQLite transaction; source bytes and saved-audiobook title snapshots remain unchanged.
- `src/components/DocumentViewer/DocumentViewer.tsx` owns the reader shell, viewer plugin resolution, in-document Find, same-document link scrolling, scroll-to-top behavior, and the slots used by TTS controls/diagnostics. `src/viewers/PdfViewer.tsx` adapts PDF.js rendering, Find, bookmarks, page/zoom controls, and TTS highlights without loading PDF support during non-PDF startup.
- `src/pdf/pdfImport.ts` incrementally extracts PDF pages through PDF.js, persists one bounded page layer at a time, finalizes the shared search index, and reuses the first rendered page for a gallery thumbnail.
- Search-result clicks can pass a lightweight reader target into `DocumentViewer`: a Pagefind heading hash, an uploaded reflowable-document section ordinal, and/or highlighted snippet text. HTML, EPUB, TXT, and Markdown readers map SQLite's persisted section ordinal to a generated `data-papercut-section` marker, then search that block before highlighting. Broad uploaded searches containing two to six terms show their existing per-term section counts directly in each result card. One native **Explore matches** disclosure then groups two additional ranked passages, the bounded twelve-bin supporting-section distribution, and on-demand concordance controls for every available required phrase or term. Each bin reports how many supporting sections fall in that part of the document, opens the first supporting section/page in the bin, and identifies its approximate position to assistive technology. Concordance loads 50 context lines at a time, and its occurrence-within-section locator lets the reader highlight repeated matches precisely. Exact results with one occurrence omit the explorer because their primary action already opens the only match. Cross-section mixed queries target the first verified exact-phrase section rather than the best unrelated broad-term section. Reader text matching spans inline markup such as emphasis, links, and bold text, treats explicit line breaks as normalized whitespace, and does not cross readable block boundaries. The preferred visual marker is a named CSS Highlight range so large iOS/WebKit reader DOMs are not rewritten just to mark a search result.
- `src/hooks/useDocumentFilters.ts` owns document filter text, selected filters, author grouping, collapsed groups, and the optional inclusion predicate used by the Saved audio filter.
- `src/hooks/useSearch.ts` owns the combined Pagefind + SQLite query flow, maps uploaded matches into the shared `SearchResult` shape, and exposes the real index/results/phrase/excerpt phase currently running. Provider failures remain distinct from a valid empty result set.
- `src/search-semantic/` owns the separate saved opt-in/model and index lifecycle hook plus functional controls. One App-level owner survives Settings/reader navigation; functional Settings controls reuse the cache installer outside Developer mode. Explicit cached activation and build/resume/cancel/removal never change the lexical query flow. The separate conceptual hook/renderer invokes native retrieval only while opted in, forwards scope/quotes, rejects late responses and preserves plain source text/reader targets. Missing/incomplete/failed semantic work shows visible lexical fallback; disabled search has no semantic query/notice.

Rust:

- `src-tauri/src/open_documents.rs` owns the removable file-association edge: it filters desktop launch paths, retains Android provider and iOS security-scoped URLs, queues requests across frontend startup or an active Library operation, and exposes one atomic drain command. It does not import documents itself.
- `src-tauri/src/document_uploads/` owns the runtime upload feature, split one concern per file (dependencies point downward, currently `commands → { batch, pipeline, organization, search, store } → { epub, html, pdf, parsed, storage, types }`):
  - `commands.rs` — the `#[tauri::command]` edge; each command just moves the blocking work onto the thread pool and delegates.
  - `batch.rs` / `state.rs` — sequential multi-file/folder import and document deletion, progress events, partial-success results, and cooperative import cancellation between files.
  - `pipeline.rs` — import / get-source / delete orchestration (no SQL or parsing of its own).
  - `semantic.rs` / `semantic/store.rs` / `semantic/query.rs` — optional semantic-index commands/jobs, lazy derived SQLite storage and independent read-only conceptual queries. This separate boundary owns compatibility, passage checkpoints and source invalidation; ordinary search/import code does not invoke it. Separate React opt-in controls invoke explicit jobs; The separate React hook renders conceptual results or enabled-path lexical fallback. Native retrieval scopes ready documents and verifies required phrases before best-passage ranking; its plain source spans/provenance do not redefine lexical counts. Provider disable signals cooperative index cancellation before model cleanup.
  - `html/` — HTML-specific parsing (`parser.rs`), sanitization (`sanitize.rs`), and small shared HTML helpers (`util.rs`).
  - `epub/` — EPUB ZIP/container/OPF/spine parsing, with path resolution (`paths.rs`), bounded reader-image storage (`assets.rs`), DOM-based link/resource rewriting (`rewrite.rs`), and generated reading HTML assembly (`render.rs`) split into focused helpers.
  - `pdf/` — canonical PDF source storage, bounded per-page PDF.js text sidecars, index finalization, and page-aware narration reconstruction.
  - `parsed.rs` — format-neutral `ParsedDocument` / `ParsedSection` shape used by HTML, EPUB, TXT, Markdown, PDF index finalization, and storage.
  - `text.rs` — strict Unicode decoding plus plain-text escaping and CommonMark-to-sanitized-HTML adaptation.
  - `store.rs` — SQLite schema, the index write path, listing, and deletes.
  - `organization.rs` — uploaded-document folder and manual ordering metadata. It never rewrites document URLs or stored source files, so folder moves do not invalidate search rows, saved audiobook ids, or TTS highlight mapping.
  - `search.rs` — read-only SQLite orchestration, ranking, evidence, and concordance; `search/query.rs` owns query normalization and FTS5 expression construction, while `search/tests.rs` keeps the focused behavior suite beside that implementation.
  - `storage.rs` — app-data paths, upload ids, size accounting, clock, and the URL-prefix/size-limit constants.
  - `types.rs` — serde DTOs shared across the boundary.
- `src-tauri/src/lib.rs` registers the Tauri commands, referenced through the `document_uploads::commands` path so the macro-generated command helpers resolve.
- `src-tauri/Cargo.toml` includes `rusqlite` with the bundled SQLite feature so FTS5 support is available consistently across supported build targets. SHA-256 provides stable source identities for exact duplicate detection. EPUB parsing uses focused crates for ZIP, XML, sanitization, DOM rewriting, base64 image data URLs, and percent-decoded archive hrefs instead of handwritten decoders or tag scanners. The DOM rewriter is post-sanitizer plumbing, not the security boundary, so it can be swapped if a better-maintained HTML mutation crate fits later.

Storage:

- Sanitized uploaded HTML is stored under Tauri app data at `document_uploads/{upload_id}/source.html`.
- Canonical uploaded PDFs are stored at `document_uploads/{upload_id}/source.pdf`; rebuildable PDF.js page layers live beside the source and feed search, navigation, and TTS locators. EPUB reader images use generated filenames under `document_uploads/{upload_id}/assets/` and are exposed only through the scoped Tauri asset protocol.
- New upload ids are full SHA-256 hashes of the original file bytes. Existing timestamp-derived ids remain valid; the first re-import of a document created by an older app version may create one hash-identified copy, after which exact re-imports reuse it.
- File and folder entry points share the same picker-independent per-file importer, so limits, parsing, duplicate handling, storage, and indexing cannot drift between UI paths. Their completion notice distinguishes documents that were added, already in the Library, or failed; reused and failed source filenames are available in expandable lists rather than hover-only details.
- **Files** can select one or up to 500 HTML, EPUB, PDF, TXT, or Markdown files. A single successful import opens immediately; larger selections remain in the Library with count-based tonal progress, a shared activity spinner, the current filename, and localized semantic stages for format detection, reading, document/book preparation, storage, and PDF text extraction. While import is unavailable, its toolbar button keeps its accessible status label but replaces the repeated visible label with the shared spinner. The progress element stays indeterminate until a trustworthy total is available, then reports completed files; stage detail appears only when work lasts beyond a short delay. Papercut does not invent within-file percentages for parser work without trustworthy units. Cooperative cancellation remains between files, successful siblings survive failures, and per-file errors remain expandable. Successful and cancelled import notices dismiss automatically after six seconds; failures remain available until dismissed.
- **Paste Text** collects an editable title and plain text in an app dialog, then sends that input directly to the Rust TXT adapter. It does not request clipboard access, create a temporary file, or interpret Markdown. The dialog retains its contents when validation or persistence fails, and a successful save opens through the existing reflowable reader.
- **Desktop drag and drop** listens only while the Library is visible, gives immediate full-window feedback, and forwards Tauri-scoped file paths into the same bounded batch importer used by **Files**. Existing progress, cancellation, duplicate reporting, and partial failures therefore behave identically; drops are blocked while another Library operation is active. Folders are intentionally left to the explicit **Folder** action so hierarchy and depth-limit behavior remain clear.
- **Platform Open With** uses installed file associations for HTML, EPUB, PDF, TXT, and Markdown. Desktop, Android, and iOS cold/warm requests enter one small native queue and wait for any active Library operation before entering the same importer. Android registers only `VIEW`, deliberately excluding Share-sheet actions; mobile provider URLs remain intact until Tauri's filesystem layer reads them, and iOS security-scoped access is released after each bounded read. The Android and iOS release matrix passed with supported formats from local storage and representative providers while Papercut was cold and already running. macOS and iOS rank Papercut as an alternate viewer; raw, uninstalled AppImages still depend on desktop integration before an operating system can list them as handlers.
- Android document providers may expose selected files through extensionless content URLs. In that ambiguous case Papercut reads only a small prefix to distinguish ZIP-based EPUB, PDF, HTML, or readable plain text, then runs the same full format-specific validation used for named files. If a provider removes a Markdown filename, Papercut conservatively imports that otherwise ambiguous text as plain text instead of guessing Markdown semantics.
- Desktop builds can select one folder and feed up to 500 HTML, EPUB, PDF, TXT, or Markdown files into that same batch pipeline. Papercut recreates the selected folder and supported descendants through five visible Library levels, skips symlinks and deeper descendants, and gives a conflicting top-level folder a numeric suffix instead of merging it. Exact duplicate documents already in the Library are reported and remain in their current location.
- Selected-document batch deletion is sequential and capped at 500 items like import. Recursive folder deletion instead derives its complete document list from the native SQLite hierarchy, so it cannot omit descendants or inherit the frontend selection cap. Both paths reuse the saved-audio guard, staged filesystem/SQLite deletion, count progress, shared-state refresh, and partial-failure reporting. A populated folder stays available for retry if any document fails; only a fresh transaction that finds no remaining subtree documents may cascade-delete its folder metadata.
- EPUB stores a sanitized generated reading HTML copy at the same stored-source path. Search and TTS depend on the generated safe reading copy and normalized sections, not on rendering the raw EPUB archive in React. Local PNG, JPEG, GIF, WebP and SVG manifest images referenced by retained reader content—including image-only spine sections—are stored as content-hashed files with a 5 MB per-image cap and 100 MB total-image cap. The reader keeps one continuous page while loading and decoding those images lazily. SVG remains an external HTML image with scripts/external resource loading disabled, never active inline reader markup; supported inline SVG covers are converted to image assets. Remote images, CSS backgrounds and `srcset` are skipped; unused manifest images are not copied. See the EPUB notes for image diagnostics and remaining limits. The original EPUB archive is not retained by the current MVP. Older inline-image imports remain readable and are upgraded to stored assets on open when possible.
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
- `src/components/DocumentViewer/DocumentViewer.tsx` owns the reader chrome: Back, Find, reader settings, header slots, same-document link scrolling, scroll-to-top behavior, loading/error display for document opens, and TTS highlight integration. Document opening keeps its staged status text in one centered loading group. Bookmark actions remain labelled menu rows with supporting icons and touch-sized targets rather than nested buttons.

Viewer rendering is plugin-based:

- `src/viewers/registry.ts` chooses a `ViewerPlugin` by URL and optional document format.
- More specific URL formats must be registered before the HTML fallback. PDF resolves to the PDF.js viewer; raw `.epub` remains reserved ahead of the catch-all HTML viewer.
- `src/viewers/HtmlViewer.tsx` parses the stored full HTML document, extracts body content, and renders it into an app-owned sanitized reader surface instead of a `srcDoc` iframe. Imported head styles are intentionally not injected into the app DOM. Reader settings apply through CSS variables on the viewer shell, so changing font, font size, line height, or width does not rewrite stored source or invalidate audiobook metadata.
- Uploaded EPUB documents currently resolve to the HTML viewer because their stored source is generated reading HTML. The shared DOM reader handles generated hash links so TOC entries and footnotes scroll within the stored document. TTS highlighting caches the generated reader DOM while it is stable and invalidates those caches when Find or reader updates replace text nodes. A richer EPUB viewer can replace that later if it declares which reader capabilities it supports, because Find, scrolling, TTS highlighting, and locator navigation may differ by format.
- App theme is separate from reader settings. `src/hooks/useTheme.ts` persists Light, System, and Dark choices and writes the resolved theme to the root element. Reflowable HTML, EPUB, TXT, and Markdown documents can independently use Default, an opaque low-glare Gray, or AMOLED Black page colors through reader-scoped semantic CSS tokens; changing page color does not rewrite the rendered DOM, stored documents, search data, TTS chunks, or highlight locators. The PDF viewer themes its chrome but leaves rendered page contents unchanged and does not expose reader page colors.

This keeps the runtime upload pipeline independent from the viewer shell. The upload backend produces safe stored source and normalized searchable sections; the viewer shell decides how the document is presented and how reader-level controls attach to it.

Reader typography is intentionally owned by the shared DOM reader, not by each upload parser:

- The reader bundles offline fonts under `public/fonts/reader/` so desktop and Android do not collapse every serif choice into the same platform fallback.
- Literata is the default long-form reading face for non-Arabic interface locales; Arabic defaults the reader to Readex Pro. Explicit reader font choices remain independent of later interface-language changes. Atkinson Hyperlegible is available for accessibility-focused reading, and system serif/sans remain available for users who prefer platform defaults.
- Naskh Arabic, Droid Arabic Naskh, Scheherazade New, and Readex Pro are exposed as explicit Arabic-focused options. They are not inserted into every stack because Arabic font metrics can change spacing and line flow in documents that previously rendered better with their platform fallback.
- Font, spacing, width, and page-color controls are reader presentation only. They do not rewrite stored reflowable reading HTML, search sections, TTS chunks, saved audiobook metadata, or highlight locators.

## Import Pipeline

The runtime upload path follows one shared persistence/search contract with format-specific parsing:

1. **Choose source**: a native Tauri dialog selects one file, multiple files, or one desktop folder; desktop users can instead drop files onto the Library, while installed desktop and mobile builds can receive supported file-open requests from the operating system. Source selection stays separate from the per-file importer so every entry point reuses the same validation and persistence path.
2. **Validate input**: enforce size limits, derive the stable source id, reuse an exact existing upload when present, and decode HTML bytes before parsing.
3. **Parse format**: HTML and EPUB become safe readable blocks; TXT is escaped into reflowable paragraphs; CommonMark Markdown is rendered and sanitized; PDF.js extracts bounded page text and page locators.
4. **Sanitize source**: remove active or risky HTML before storing/rendering it; retain validated PDF bytes as the canonical render source.
5. **Normalize sections**: convert the document into title + ordered text sections with optional headings.
6. **Store source**: save the sanitized viewable document to app data.
7. **Index sections**: write metadata and sections to SQLite, then populate the FTS5 table.
8. **Render/search**: React opens the stored source and searches through the same result-card UI as bundled docs.
9. **Delete**: when requested, Rust serializes deletion with the short save-start transaction, then checks whether any saved or in-progress audiobook manifest references the document and refuses deletion if so. A save that wins this ordering publishes its pending manifest before generation; a deletion that wins causes a later save to reject the missing source. Otherwise deletion stages the stored source directory, removes the document rows in one SQLite transaction, and then removes the staged files. A database failure restores the directory. Batch requests are capped at 500 upload URLs, deduplicated before execution, run sequentially, and retain per-document failures alongside successes.

EPUB plugs in at step 3 by validating the ZIP/container, reading OPF metadata and spine order, sanitizing each XHTML spine item, generating safe reading HTML, and outputting the same normalized section shape. PDF stages a canonical source in Rust, lets PDF.js produce bounded page records, then finalizes those records through the same SQLite document/FTS store. Its architecture, research, and remaining staged checklist live in [pdf-ocr-scanning.md](pdf-ocr-scanning.md).

## Search Flow

The flow below describes Keyword search. [Conceptual search](#conceptual-search) adds an independent opt-in channel; [validation](search-validation.md#automated-checks) documents the original lexical baseline and evaluator.

When a user submits a search:

1. React parses straight or curly quotation marks once for both execution and presentation, preserving differently delimited inner quotes within an outer phrase. In All words mode, every unquoted word is required, while each quoted span adds a literal phrase requirement. Mixed input such as `anne "green gables"` therefore requires both clauses and displays both in the result summary; an unmatched quote is rejected beside the search field. React rejects queries beyond 128 normalized terms or 4,096 Unicode characters across broad and quoted clauses before starting starter/uploaded work; native requests independently enforce the same budget. Valid requests retain complete clauses; larger requests produce an inline error rather than silently searching a prefix. Literal comparison also canonicalizes curly quote glyphs and Unicode hyphen/en/em-dash punctuation to their straight equivalents without rewriting stored source.
2. **Filter By Document** treats checked documents as either the included search scope or the documents excluded from it. With no checked documents, search uses the full library.
3. Pagefind searches bundled build-time documents when the Pagefind index is available, then `useSearch` consumes matching results from the resolved allowed-URL set before applying its visible-result limit. The status region starts with the number of indexed documents in that submitted scope and follows actual phase transitions rather than estimating a percentage.
4. SQLite FTS5 searches uploaded runtime documents when the app is running in Tauri, with allowed upload URLs passed into the SQL query when a scope is active. Its blocking worker sends at most three semantic stages over the invocation's Tauri channel: finding candidate documents, verifying literal phrases when requested, and ranking results/building bounded evidence. A stage remains visible when its underlying work is slow; no artificial minimum duration is added merely to display every label.
5. SQLite groups matching sections by uploaded document before applying the result limit. It returns the best section, the complete supporting-section count for each visible document, up to three ranked passages, and at most twelve distribution bins. For a same-section result, the count covers sections matching the complete query; for a cross-section result, it covers sections containing at least one query word. The UI labels those meanings separately and does not combine them into one ambiguous corpus-wide section total. Each bin retains one source locator, its exact supporting-section count, and the first marked token from that section when available. Total matching-document/section counts are still computed before limiting the response.
6. `useSearch` maps both providers into the existing `SearchResult` shape; it no longer has to deduplicate uploaded section rows.
7. Quoted exact-phrase searches use Pagefind/SQLite only as a candidate finder. Rust verifies uploaded candidates against their indexed sections, then returns the complete phrase occurrence count, first literal-phrase locator, and a bounded highlighted excerpt for each visible result; React does not reopen uploaded sources. Bundled starter candidates are still verified against their source in React. Broad uploaded searches prefer a section containing every word, then fall back to a labelled document-level result when the required words occur across separate sections. For a mixed cross-section match, the native result targets the first section containing the first verified exact phrase. Exact results hide the broad matching-section distribution rather than presenting candidate sections as literal phrase evidence.
8. Keyword results form one list with one count, preserving uploaded-first order and a **Starter Document** badge on bundled cards. Keyword and Conceptual each initially display ten returned cards, with explicit Show More batches. Counts distinguish displayed and available results where known; conceptual returned-document counts are not exhaustive library totals. Keyword examples appear only for an empty Keyword draft and fill the input without submitting. Advanced uploaded evidence stays behind **Explore matches**: concordance scans the selected document, counts all occurrences and returns at most 50 context lines per request. Loading, errors and valid empty responses have distinct accessible status. A local-processing reassurance appears only after five seconds of lexical work.
9. Opening a result keeps the canonical document URL, then passes a best-effort target to the reader. Uploaded reflowable documents resolve their persisted SQLite section ordinal to a generated reader marker and constrain snippet-text matching to that indexed block; PDFs use their page target, while Pagefind starter documents keep heading-hash/text fallback behavior. Evidence bins include SQLite's literal marked spelling so stemming does not make the target highlight a different word form. Concordance lines also carry the zero-based occurrence within their section/page, so repeated terms highlight the selected instance rather than always selecting the first. Search-result navigation and in-document Find share block-aware text and punctuation matching, so inline formatting or equivalent straight/typographic punctuation does not prevent a visible phrase from being selected. Starting an explicit Find retires the transient result highlight, caches the rendered reader's normalized text map, and uses CSS highlights without rewriting the reader DOM; audiobook playback highlights remain independent.
10. If a result points to an uploaded document URL, source loading calls `document_uploads_get_source` instead of fetching from `dist/`.

SQLite FTS uses a Porter/unicode tokenizer with diacritic removal and BM25 ranking. Candidate scans give indexed title, heading, and text relative weights of 5:3:1; passage evidence uses body-only scoring. Two-to-six-word unquoted searches additionally prefer an adjacent body phrase within existing scope and coverage tiers, without making that phrase mandatory. Queries on the original index use SQLite snippets; Arabic search-form matches use authored section text for marked excerpts because normalization removes characters and changes offsets. Retained excerpts use up to 64 tokens; a section card's excerpt and source target follow its selected body passage. Cross-section cards disclose all retained passages, up to three, through Explore matches. React sanitizes every excerpt before rendering, retaining only `<mark>` highlighting. SQLite scans lightweight match metadata to group and count every matching uploaded document, then builds excerpts only for the bounded visible document set. Because the reader has its own in-document Find/highlight workflow, the search panel shows one uploaded-document card with the best relevant snippet instead of one card per matching section.

Arabic-containing uploaded queries use a versioned derived FTS search form that removes tashkeel and tatweel and folds common alef/hamza forms to bare alef. Authored sections and the original FTS table remain intact, and quoted phrases still require literal source verification. A background worker builds the derived index for existing libraries in small committed batches; Arabic queries retain their previous search behavior until it is complete, and an interrupted rebuild resumes on restart. Imports, OCR replacements, title changes, and deletes update both indexes in the same transaction. This improves selected orthographic variants, not Arabic morphology or general semantic matching. The synthetic scale check is recorded in validation; representative-device storage and latency remain unverified.

An empty uploaded **All words** search can offer one explicit English spelling retry drawn from authored document words, even when starter documents have partial hits. Choosing it runs a new search, so counts, excerpts, and navigation describe the corrected term rather than a fictional occurrence of the typo. Quoted phrases, broader mode, Arabic/mixed-script queries, uppercase names, identifiers, and short words do not receive suggestions. The per-document vocabulary is rebuilt in the background for existing libraries, resumes after interruption, and is maintained by import, title edit, and delete transactions. Its two-letter prefix and candidate caps intentionally trade some recall for less search work; representative-library/device behavior remains unverified.

There are no hard-coded related-word pairs or general synonym expansion. Porter stemming covers some inflections, and the bounded spelling retry handles some typos; neither establishes semantic equivalence. Optional conceptual results remain independent of lexical results and their counts, with the limits in [Conceptual search](#conceptual-search) and the evidence in [validation](search-validation.md).

The default **All Words** mode requires every unquoted term at document scope in uploaded documents, including matches across sections. Starter candidates must also occur in each distinct unquoted term's Pagefind quoted search before scope, count and display limits; source-verified phrases are checked separately. Starter ranking and uploaded evidence/count contracts remain different. The optional **Broader** mode searches uploaded documents only. With three to twelve distinct unquoted words it requires at least half, with a minimum of two, anywhere in the document, then ranks fuller coverage first. With one or two words, or more than twelve distinct words, it retains all-terms matching without dropping words. Exact quoted phrases always remain required and source-verified. Cards show which unquoted words are missing; a title match is not reported as a body-text occurrence. The small regression fixture and existing 100,000-section development-host probe pass; representative-device and interactive acceptance remain open.

For quoted searches, broad provider counts are not treated as exact phrase counts. Uploaded results use exact occurrence counts from the native indexed-section verification scan, while starter results use source-verified counts in React. Unquoted searches can still show matching-section counts when SQLite or Pagefind exposes them. The twelve-bin distribution also represents matching sections, not the number of individual word occurrences inside those sections.

## Performance Notes

SQLite FTS is a good fit for user uploads because it indexes incrementally. Importing one file touches one source file and one local database transaction; it does not regenerate the shipped corpus index.

For 500+ user documents, the important scaling rules are:

- Keep indexing per document incremental.
- Store text as sections/pages/chapters instead of one huge blob per document.
- Query only on explicit Search/Enter, not on every keystroke.
- Limit initial result counts and fetch/open full source only when the user chooses a document.
- Keep heavy extraction off the WebView main thread: Rust handles HTML/EPUB/TXT/Markdown parsing and PDF.js uses its worker for PDF parsing.

The current HTML, EPUB, PDF, TXT, and Markdown paths follow those rules. Uploaded search counts all lightweight matching section rows in Rust so one prolific document cannot consume the result limit, but sends at most 100 document cards over IPC. For visible documents, one additional matching-section FTS scan selects at most three snippets and aggregates every matching section into twelve fixed position bins. Eligible unquoted searches add one SQLite FTS aggregation per unique term (up to six by default or twelve in the uploaded broader mode), constrained to visible document ids. Each aggregation returns at most one count, first locator, and short marked token per visible document, so term-coverage data is capped at 1,200 small records over IPC and rendered as one wrapping text line per result rather than a second comparison table. Distribution bins reuse the snippets already produced by their existing evidence scan. Common individual terms can still add local SQLite work, but there is no stored-source-file read or chart runtime during search; legacy-library projection rebuild runs separately in the background. The result payload and rendered evidence DOM are therefore bounded per document even for common terms in a long book. Broad terms do more local SQLite work than the previous best-snippet-only query, but search remains explicit-submit and evidence does not require loading stored source files. Exact uploaded searches use an early-exit indexed-section pass to verify every narrowed candidate so totals remain truthful, then scan only the bounded visible results to compute complete counts and retain one excerpt. That replaces the former frontend source read, keeps memory bounded, skips broad evidence aggregation, and moves the work off the WebView thread. A visible long document whose first match is near the beginning—especially a PDF—still needs a complete second pass because an exact count requires the remaining sections, but non-visible candidates do not pay that cost. Progress reporting adds at most three tiny channel messages to an uploaded search plus small React state updates at existing async boundaries; it adds no SQLite query, polling loop, global listener, or persistent state. The indexed-document count is memoized across library/scope changes, and the five-second detail is a CSS reveal. PDF extraction is sequential by page, search uses persisted SQLite page rows, and the viewer relies on PDF.js virtualization rather than mounting every rendered page eagerly.

While the submitted query is still present, its Search button shows the shared dual-arc spinner, exposes its busy state, and blocks duplicate submission. The input remains editable; changing it restores the button so a newer query can supersede stale work. The spatially separate results status keeps its own decorative spinner beside the detailed search phase; only its visible text is announced. Successful searches announce the existing result summary. The shared CSS spinner uses no additional component or dependency and becomes static when reduced motion is requested; visible status text continues to carry the meaning.

Uploaded sections have an additive `(document_id, ordinal)` index. Existing libraries receive it idempotently when the database next opens; no source or FTS rebuild is required. On the August 20, 2026 development corpus (384 documents, 116,827 sections, 660 MB SQLite database), the same read-only retrieval of indexed sections for 50 common-phrase candidates fell from 4.787 seconds without the index to 0.061 seconds with it. FTS candidate-count queries for representative broad, phrase, and mixed searches took 14–24 ms through the SQLite CLI. The index added 6.8 MiB, about 1% of that database, and has a one-time creation cost proportional to the section count. These numbers are directional measurements from one desktop, not cross-device guarantees.

### Search Performance Acceptance

Use a release corpus with at least 500 documents and 100,000 indexed sections, including three documents with at least 1,000 sections or PDF pages. Run one cold and five warm submissions for a common two-term broad query, a common quoted phrase, and a mixed word-plus-phrase query. Debug builds write one `[search] native performance summary` containing term/phrase counts, scope size, candidate/match/result counts, and `db_ms`, `candidate_ms`, `verification_ms`, `exact_evidence_ms`, `result_evidence_ms`, `term_matches_ms`, `result_ms`, and `total_ms`; it deliberately omits query text, titles, and document ids.

The reference-device gate is a warm median at or below one second for broad search and three seconds for exact or mixed search, with no warm run above five seconds. The status region must remain visible and accurate throughout a slow phase, the five-second local-processing detail must appear only when needed, and opening an exact result must land on and highlight its native section/page evidence. A failure should be investigated by the logged phase before adding caches, workers, pagination, or another search dependency. Record desktop and mobile results separately because storage and CPU characteristics differ.

The recorded Search v2 development-host probe exercised 500 documents / 100,000 authored sections, including three 1,200-section documents, through the production store and release native search path. Warm medians for saturated common-word, quoted and mixed cases met those numerical thresholds on that host; candidate retrieval dominated. The [recorded scale validation](search-validation.md#performance-and-native-limits) includes scope, complete counts, evidence targets, payload bounds and measurement limits. Repeated synthetic text, warm OS caches and native-only timing do not establish real-book diversity, weak-device performance, WebView responsiveness or interactive smoke acceptance.

Concordance work is isolated from the default search payload. Each page scans only the selected document through the `(document_id, ordinal)` index, counts all literal matches, and sends at most 50 bounded excerpts over IPC. Requesting another page repeats that one-document scan; this deliberately avoids stored occurrence offsets or another schema until real paging measurements justify them.

Reflowable imports assign one numeric DOM marker per indexed section while the parser already has the sanitized document tree in memory. The sanitizer preserves only canonical non-negative marker values. New documents therefore add no extra parse, one linear serialization to persist the marked tree, and only a small attribute per section. Older stored sources have no markers, so each open pays one additional linear DOM parse/serialize after sanitization; this compatibility work happens on the Rust blocking thread, not during SQLite search or WebView interaction, and requires no database migration or source rewrite. If measured large-book open latency makes that fallback material, a future reindex/upgrade action can persist regenerated reader HTML once.

Reflowable HTML, EPUB, TXT, and Markdown in-document Find builds one compact text-to-DOM index per opened document and reuses it as the query changes. Its existing linear index pass now performs one constant-time punctuation lookup per source character so straight/curly quotes and dash glyphs share searchable forms while every match retains the original DOM offsets. Match counting and next/previous navigation still cover the complete rendered document. Visual highlighting is capped for extremely common queries so a one-character search cannot create tens of thousands of ranges or DOM elements; the current match remains highlighted and navigable. Older WebViews without CSS Custom Highlights fall back to the browser selection for the current match rather than mutating the document. Native PDF Find keeps its bounded line-wrap aliases and adds only whole-query dash-glyph variants, avoiding an unbounded punctuation combination set.

TTS highlighting currently maps saved audiobook chunks back onto the live reader DOM. The hook caches the reader's text-node segment index while active playback highlighting needs it, then invalidates that cache when document loading or another reader mutation replaces text nodes under the same root. Find uses CSS ranges without changing those nodes, so changing a Find query no longer invalidates TTS alignment. That keeps existing saved audio compatible and avoids rescanning on every chunk advance or immediately after merely opening a large book, but a very large fully-rendered book can still pay a one-time segment-index rebuild when playback highlighting first starts after a mutation. New audiobook bundle exports preserve optional chunk source spans, so re-imported HTML/EPUB generated-reader bundles can often highlight without text rediscovery. Imported bundles without stored spans keep a compatibility fallback for legacy HTML: when rebuilt spans are unavailable or validate against the wrong text, the reader can build one cached text map from the live DOM and match the current chunk text back to a source span. Exact matching stays first; the fallback also tolerates Arabic Unicode differences such as tashkeel, tatweel, bidi controls, and common Arabic/Persian letter variants while mapping matches back to real DOM offsets. This work is lazy and cached, and the audio controls surface a short "Preparing highlights..." notice while imported highlight spans are being rebuilt. The fallback is bounded to the rendered document text, not repeated for every chunk, but it still depends on the whole rendered DOM. The long-term scaling path is chapter/page-level rendering with locator-aware TTS ranges, so the app only indexes the visible or active chapter instead of one giant reader DOM.

## Sanitization And Format Modules

Yes, upload formats should have separate sanitization/parser modules. HTML, PDF, and EPUB have different risks and extraction behavior:

- HTML needs active content stripping, URL cleanup, and safe stored rendering.
- PDF needs text-layer extraction, page boundaries, metadata and resumable on-device OCR for pages without usable text.
- EPUB needs ZIP/container validation, OPF spine parsing, XHTML sanitization, and chapter ordering.

The shared indexed metadata should be boring and stable:

`{ title, format, sections: [{ ordinal, heading?, text, page_index? }], cover? }`

For reflowable formats, the persisted ordinal is the locator; for PDF, `page_index` is the locator. Keeping this shape stable lets the Library and SQLite indexing remain format-agnostic while each viewer loads its own stored source type. See [epub-implementation-plan.md](epub-implementation-plan.md) for the ordered EPUB task list and acceptance checks, and [pdf-ocr-scanning.md](pdf-ocr-scanning.md) for the PDF, OCR, and mobile scanning plan.

## Current Limitations

- Runtime import supports HTML, EPUB, TXT, CommonMark Markdown, text-native PDF, and local English or Arabic recognition for image-only or hybrid PDFs. Pages with usable native text and intentionally blank pages are preserved, while image-backed pages without usable text are recognized. Additional recognition languages remain deferred until their local assets and acceptance cases are ready.
- HTML files must be at most 25 MB. UTF-8 is used directly; non-UTF-8 HTML can import when it declares a supported browser charset such as Windows-1252.
- TXT and Markdown files must be at most 25 MB and use UTF-8 or explicitly BOM-marked UTF-16. Ambiguous legacy encodings are rejected instead of guessed. Plain text that resembles markup remains literal; Markdown output passes through the existing sanitizer, and remote or sibling-file images are intentionally omitted until the reader owns an explicit local-asset contract for those formats.
- EPUB files must be readable ZIP-based EPUB archives with a valid container and OPF spine, and must be at most 100 MB. Only referenced local PNG, JPEG, GIF, WebP and SVG reader images are retained, with 5 MB per image and 100 MB total reader-image caps. A declared EPUB 2/3 cover is retained beside the generated source only after the bounded thumbnail decoder validates it under the same 5 MB per-image cap; an invalid optional cover falls back to the generated title placeholder without failing the document import. The Library gallery serves the persisted display-sized PNG thumbnail rather than decoding the original cover. Existing imports with retained covers receive that thumbnail lazily on first gallery access; imports without cover metadata are not retroactively reparsed.
- PDF files must have a valid PDF signature and be at most 250 MB and 2,000 pages. Search and TTS require native extracted text or a completed supported recognition job. PDF.js rendering remains page-faithful and does not recolor page contents for app themes.
- HTML and EPUB content use the parser-based `ammonia` allowlist before storage. The shared reader boundary validates browser-decoded URL schemes, preserves same-document footnote anchors and validated EPUB image assets, and strips active elements, event handlers, inline styles, remote images, SVG data URLs, and unsupported links. Uploaded documents and audiobook-bundle sources are sanitized again when read so files retained from older app versions receive the current policy before entering the app DOM.
- Uploaded-document search only runs inside the Tauri app, not plain browser preview.
- Folder import is desktop-only. It preserves supported files through five visible folder levels, skips deeper descendants and symlinks, and leaves already-imported duplicate documents in their current Library location.
- There is no user-facing reindex action for generic uploaded documents yet.
- Keyword retrieval is capped at 50 results per provider, or up to 100 uploaded results with document scope. Its UI starts at ten cards and Show More reveals batches from the same bounded response, not server pagination. Provider counts remain distinct from displayed-card counts. Conceptual independently returns at most 50 documents.
- Starting a newer query suppresses stale results from an older in-flight search, and re-submitting the identical active query does not start duplicate provider work. Pagefind and the native SQLite command do not currently expose cooperative cancellation, so the UI does not offer a misleading Cancel action.
- Multi-passage evidence, supporting-section distribution strips, and concordance currently apply only to uploaded SQLite results. Each card retains at most three ranked passages and twelve position bins; a bin with several matches opens its first supporting section/page, while the same **Explore matches** disclosure lists literal occurrences on demand.
- Concordance can inspect any quoted phrase or required broad-search term available for that result and loads 50 lines per page. Exact results with one occurrence omit this redundant disclosure.
- Broad two-to-six-term uploaded results show per-term matching-section counts directly on each card. Those values describe section coverage, not repeated word occurrences or semantic agreement between terms; a separate selected-document comparison mode is intentionally not retained without evidence that it improves decisions.
- Selected uploaded documents can be exported/imported through [Library Transfer](library-transfer.md), including supported EPUB reader assets. This is a portable transfer package, not a synchronized backup service.
- Quoted exact-phrase results are document-level matches with native indexed-section occurrence counts for uploads and source-verified counts for bundled starters, not broad provider section counts.
- Very large HTML/EPUB books currently render as one generated reader DOM. The app treats document opening as one global reader transition, disables competing View/Open actions while source HTML loads, and progressively discloses localized preparation detail after 700 ms plus a large-document reassurance after five seconds. It avoids DOM mutation for search-result target highlights on modern WebViews and does not build the TTS text-node index until playback highlighting needs it. The first audiobook highlight after a large DOM mutation may still rebuild the text segment index. Chapter-level rendering with locator-aware highlighting is the preferred long-term fix for those formats; PDF already uses page virtualization and page-aware locators.
- Light/Dark/System theme selection is UI state, not document state. Theme polish should stay in CSS tokens where possible so imported HTML, generated EPUB HTML, and saved audiobook highlighting remain stable across theme changes.

## Other Upload And Reader Follow-Ups

1. Add more EPUB parser fixtures for malformed OPF/container cases, spine edge cases, oversized image skipping, and metadata fallback.
2. Track transfer transport changes separately in [Library Transfer](library-transfer.md#deferred-lan-redesign--2026-10-15); they are outside search-v2.
3. Add a reindex action for uploaded documents if parser or sanitizer behavior changes after import.
4. Add richer EPUB reader features such as TOC, location restore, pagination, EPUB-specific appearance controls, or a foliate-js/epub.js-backed viewer if generated reading HTML is not enough.
5. Follow [pdf-ocr-scanning.md](pdf-ocr-scanning.md) for remaining PDF parity, OCR, and mobile scanning work; do not implement those concerns as isolated additions here.
6. Existing English/Arabic and structural multilingual fixtures do not establish universal language quality. Corpus vocabulary statistics, distinctive-term/coverage views and word graphs remain unimplemented ideas, not queued search work.

### Inline Auxiliary Roles And Existing Imports

HTML/EPUB footnote/endnote, bibliography, navigation and index roles apply to
inline descendants as well as complete blocks. Derived body ranges retain
UTF-8 offsets into complete section text; auxiliary text stays available to
literal search and reader navigation. Schema 13 verifies saved HTML against all
stored section ordinals/text before repairing these ranges. A changed range
invalidates semantic coverage; **Build/Update Index** remains explicit. Missing
or mismatched saved source preserves existing classifications. Older EPUBs whose
saved reading copy already lost note roles still need reimport for classification.

### Reader And Phrase Regression Boundaries

The default `npm test` suite includes `pdfFind.integration.test.ts`: a generic
committed PDF with Unicode word wraps is extracted by the installed PDF.js worker
and searched through Papercut's actual Find wrapper/adapter. Match offsets are
checked against original UTF-16 source, with an emoji prefix, along with page
navigation and recovery. This covers the application/dependency boundary; canvas
painting remains an app smoke check. Modal tests use real React/DOM events through
Vitest's [jsdom environment](https://vitest.dev/guide/environment.html), with
explicit visibility rectangles because [jsdom does not perform layout](https://github.com/jsdom/jsdom#unimplemented-parts-of-the-web-platform).

Native phrase regressions call the shipping `search_uploads_with_db` entry point;
the retired `retain_exact_phrase_hits` test-only implementation is no longer an
alternate verification path. In-memory fixture/request helpers contain setup and
routing only.

## Conceptual Search

### Ownership And Shipping Builds

The feature uses existing upload storage and reader targets, not a second importer.

| Boundary | Owner |
| --- | --- |
| Model manifest, verified cache, provider/session and native commands | `src-tauri/src/search_embeddings.rs`, `search_embeddings/models.rs`, `cache.rs` |
| Shared ONNX Runtime (ORT) initialization | `src-tauri/src/onnx_runtime.rs`; also used by Arabic TTS preprocessing |
| Derived index/jobs, natural passage windows and read-only retrieval | `src-tauri/src/document_uploads/semantic.rs`, `semantic/store.rs`, `passages.rs`, `query.rs` |
| Saved opt-in, selected model, setup/index status and outcome notices | One `src/search-semantic/useSemanticSearch.ts` owner in `App` |
| Latest eligible conceptual request | `useConceptualSearch.ts`; one in-flight request, coalescing replacements without an unbounded queue |
| Result views, matching options and source opening | SearchTab, SearchOptions, ConceptualResults and the existing viewer boundary |
| Model preparation UI | `SearchModelSetup`; requires the App-owned controller, with local consent/confirmation state |

| Target | Existing shipping command | Provider feature |
| --- | --- | --- |
| Linux, Windows, macOS | `npm run desktop` | `native-tts-shared,experimental-search-embeddings` |
| Android arm64 | `npm run android:apk:native-tts` | Same shared/dynamic features |
| iOS signed export on macOS | `npm run ios:ipa:native-tts` | `native-tts-ios,experimental-search-embeddings-linked` |
| Unsigned iOS simulator/device checks on macOS | `npm run ios:ci -- --native-tts` / `npm run ios:ci:device` | Linked provider with the prepared static archives |

No model weights are bundled or fetched during builds. `desktop:static` and mobile
builds without native TTS are providerless alternatives; Android custom argument
lists own their feature selection. Cargo defaults are unchanged. Providerless
builds report `compiled: false` and allow disabling a saved preference; browser
preview has no native model controls.

FastEmbed requires ORT API 24 in provider builds; providerless TTS uses API 17.
Both consumers use ORT `2.0.0-rc.13` through one initializer and the packaged runtime.
iOS uses linked initialization; desktop/Android use dynamic loading. Initialization
errors are cached, so repairing a packaged runtime requires restart. Official
Libtashkeel remains a registry dependency with the unchanged attributed model and
a small app adapter; see [TTS runtime ownership](kokoro-tts.md). Android retains a
process-lifetime ORT environment reference, not model sessions, to avoid the observed
post-test teardown abort. Keep it until a compatible upstream fix is validated.
Unix build staging replaces shared libraries atomically rather than truncating
files potentially mapped by a running app; Windows stages DLLs beside executables.

### Models, Cache And Consent

The serialized selection is fixed; commands accept no arbitrary model, URL or path.
The [asset manifest](../src-tauri/src/search_embeddings/models.rs) owns pinned
revisions, sizes and SHA-256 values. Do not maintain another checksum list here.

| Selection | Installed bytes | Dimensions | Files |
| --- | ---: | ---: | ---: |
| Small FP32 (`float32`) | 487,352,505 | 384 | 5 |
| Small INT8 (`dynamic-int8`) | 135,430,819 | 384 | 5 |
| Base FP32 (`base-float32`) | 1,127,143,136 | 768 | 5 |
| Large FP32 (`large-float32`) | 2,252,993,226 | 1,024 | 6 |

Cache layout beneath Tauri app data is
`models/search/<model>/<revision>/<precision>/`. Each cache holds four tokenizer/
configuration files and `onnx/model.onnx`; Large also needs `onnx/model.onnx_data`.
The INT8 upstream artifact is named `model_qint8_avx512_vnni.onnx` and staged under
`model.onnx`; its name and a smaller download do not guarantee every CPU's suitability.
Disk size is not peak RAM. Base/Large currently offer FP32 only.

Fresh/invalid preferences start off. The local WebView key
`papercut.semantic-search.v1` saves only enabled state and the fixed selection.
Saving must succeed before enabling; disabling still stops work if saving fails
and warns that an old preference may return after restart. Saved opt-in reloads a
cached verified model, never downloads or resumes indexing automatically.

Download consent names Hugging Face and resets on selection changes and completed
attempts. **Verify Cache** uses the installer too: intact files need no HTTP request,
but missing files can be fetched, so consent is still required. Presence by regular
file/size is not checksum verification. A successful explicit preparation shows
session-local **Model Files Verified**; selection/reopening clears it. Activation
always rechecks the actual bytes supplied to the loader. Only one model is resident;
turn off before changing model/format or removing its active files.

The installer checks full payload plus 64 MiB reserve on the same filesystem. It
streams bounded, checksum-pinned assets over HTTPS (including redirects), writes a
sibling `<precision>.installing` directory, syncs/rechecks it and renames it into place.
Cancellation before publication prevents it; cancellation after committed publication
does not undo a valid cache. Failed/cancelled work cleans staging on normal return or
unwind. After process interruption, explicit retry discards stale staging and starts
over; byte-offset resume and automatic retry are absent. An incomplete/corrupt final
cache needs explicit removal and is never silently replaced. Removal can clear partial
final/staging files and never removes another model's cache or source documents.

Download/activation reservations remain occupied until workers finish cleanup.
Cancellation is cooperative between asset reads and native calls; ORT construction/
inference cannot be interrupted. HTTP connection/idle-read deadlines are 10 seconds,
with a 30-minute request deadline; cancellation may wait for an in-flight read.
Model unload releases the session, not the shared environment or a guaranteed quantity
of process memory. Expensive filesystem, network and provider operations use the
blocking pool; no SQLite write lock spans inference.

### Native Command Contracts

Registered handlers use camelCase requests/responses. Capability does not imply
activation, complete coverage, relevance or device acceptance.

| Command | Contract |
| --- | --- |
| `search_embedding_status` | Capability/platform plus active/loading/downloading selection and encoder identity; non-null active state follows verified native warm-up. |
| `search_embedding_cache_status` | Four selections with `expectedBytes`, `filesPresent` and `hasStoredFiles`; metadata presence is distinct from integrity and partial storage. |
| `search_embedding_configure` | A fixed selection loads already cached verified bytes; null cancels/unloads. Disabling also signals index/download cancellation. Native generations prevent old activation publication after disable. |
| `search_embedding_download` | Explicitly verify/reuse or install the selected manifest, without opt-in, activation or indexing. |
| `search_embedding_cancel_download` | Request cancellation, retaining busy state through cleanup. |
| `search_embedding_remove` | Remove only selected cache/staging; refuse conflicting jobs or an active selected model. |
| `search_semantic_index_status` | Read compatible completed-document coverage, job state and stored work; no activation or semantic-table creation. Without a compatible active provider, ready coverage is zero, not proof of deletion. |
| `search_semantic_index_build` | Explicit build/update with a verified active provider, one document at a time; reuse validated unchanged checkpoints. Returns on completion/failure. |
| `search_semantic_index_cancel` | Cooperative stop, preserving committed checkpoints and keeping the slot reserved until worker exit. |
| `search_semantic_index_remove` | Remove derived work under the job slot, invalidate writers and retain source/lexical data plus empty tables/triggers for future invalidation. |
| `search_semantic_query` | Read-only scoped conceptual retrieval; never download, activate, index or alter Keyword matching. |

Download progress events include selection, phase and bounded byte progress. Terminal
events also carry process-local job identity and nullable error after reservation
cleanup. The controller ignores stale/duplicate outcomes, retains failed-model
feedback across Settings navigation and polls only while busy with one outstanding
poll. New downloads clear old failure state; status retry/Review does not. This is
session feedback, not persistent diagnostic history. Index errors add document ID
and one-based section where known, without title/query/passage text. Confirmed
cancellation is normal **Indexing Cancelled** feedback; a real failure racing Stop
remains an error. Source/query content is not logged by conceptual retrieval.

### Index Identity, Source Roles And Upgrades

The three derived `upload_semantic_*` tables are created lazily by explicit build.
Five source-mutation triggers invalidate readiness transactionally, including when a
providerless/older app changes source metadata. Complete current documents alone are
queryable; partial-document checkpoints remain reusable but unavailable as results.
Unknown semantic schemas fail closed. Stored vectors have independent checksums,
dimension, finite-value and unit-norm validation.

Identity includes exact model/tokenizer/config bytes, dimensions, pooling/prefix
rules and the versioned extraction/body-range/window policy. A different model or
policy has no compatible coverage until explicit rebuild replaces the one derived
index. Returning to an older model after replacement requires another rebuild.
Title, source/page edits, OCR, deletion and imports invalidate affected generations;
checks before publication reject concurrent mutation, removal and stale writers.
Readiness is not inferred from cached files, saved opt-in or stored checkpoints.

Passages stay within one anchored body/auxiliary region and one section/page. Natural
sentence/paragraph boundaries are preferred within 320 tokens (including metadata/
prefix) and 40-token overlap bounds; character/token ceilings can force shorter cuts.
Complete authored text remains searchable and navigable even when notes/navigation/
references are excluded from preferred body evidence. Body matches take precedence
within a document; auxiliary-only matches remain possible. No synthetic joins across
sources or omitted notes are presented as authored text.

Upload migrations retain original text and locators. Schema-11 PDF role repair uses
matching saved page geometry, including wrapped indexes and parenthesized references;
missing/mismatched geometry preserves text conservatively. Unicode PDF line-wrap
repair updates derived lexical forms with original UTF-16 targeting. Schema 13 repairs
inline HTML/EPUB roles only after verifying saved text/ordinals; interrupted repair
resumes and missing/mismatched source preserves known classifications. Old saved EPUBs
that lost role markup need reimport. Affected semantic vectors require explicit update;
unrelated compatible work remains usable. Pre-7/pre-8 lexical/spelling migrations
invalidate stale projections/checkpoints after writes by older app versions.

For eventual removal of the feature, remove handlers/state, provider integration and
React preference/UI together. Migrate existing installs by dropping the five semantic
triggers before the three derived tables; preserve original upload/source/lexical data.

### Retrieval And Result Views

The conceptual request has `query`, optional `exactPhrases`, `documentUrls` and `limit`.
Absent/null scope means all uploads; an empty array means none. Scope rejects over
10,000 URLs, blank entries or URLs over 2,048 bytes. Limits are 1–50, default 50.
The shared 128-term/4,096-character query budget is enforced before frontend work
and independently at the native boundary. E5 encoding requires nonempty `query: ` /
`passage: ` prefixes, at most 4,096 characters including prefix and 512 actual tokens
including special tokens; overflow fails instead of truncating. Mean pooling,
normalized vectors, one text per call and at most two desktop/one mobile intra-op
threads retain the INT8 policy.

Before scoring/top-k, scope and required quotes are verified against authored body
text at document scope. Different phrases may occur in different sections; title-only
matches do not qualify. The conceptual passage may be elsewhere and is not literal
quote evidence. Unquoted names/dates/words are conceptual input, not requirements.
There is no rank fusion, synonym override list, confidence score or calibrated
no-answer threshold. Similarity cannot establish a complete supported answer.

Among the preferred passage role, best cosine similarity ranks each document; source
order breaks passage ties and document ID breaks document ties. Result memory is
bounded to 50 documents while vectors are scanned individually. Query cost is linear
in compatible scoped passages; there is no approximate-nearest-neighbour (ANN) index.
One read snapshot pins sources/index/vectors and a fresh generation check rejects
mutation before return. One native query slot has a cooperative ten-second budget
including queue time; a native call, SQLite wait or single-document phrase check can
exceed it before the next cancellation check. UI replacement requests drain only the
latest eligible request after the prior call settles; stale completions cannot publish.

Native availability is missing-index, incompatible-index, partial or ready; errors
remain distinct from a ready empty response. Scoped eligible/ready coverage precedes
phrase filtering. Partial results are withheld by the UI until that submitted scope
is complete. Pre-query guidance is library-wide; a ready scoped response takes
precedence afterward. Keyword stays usable independently. Missing/update warnings
show their icon/title above **Search Settings**; the cog precedes its text. Settings
opens directly, preserves query/scope/results, and restores focus to the opener or
Settings cog if setup removed it. Other empty/loading/error states stay distinct.

The compact tonal Keyword/Conceptual selector and Keyword-only All Words/Broader
radios are centered under the query, before helper/examples and the shared filter.
Only that control row sticks, respecting the same safe-area inset as scroll restoration.
Desktop segments have a 36px minimum, or 44px with any coarse pointer. Icons sit
physically left of labels; text direction remains automatic. Switching views starts
no query and never merges ranking. Each view retains its ten-card batch window,
expanded evidence and scroll place while mounted; new submitted queries/scopes reset
them. App retains the selected view during navigation; async completion never selects
another. Hidden panels stay mounted/inert. Keyword examples fill only, without submit;
Conceptual hides them. Keyword retains the two-line welcome, ready Conceptual a short
query prompt. Helpers omit repeated library scope and numeric rules without changing
retrieval semantics.

Conceptual cards contain plain authored text, no invented marks/counts. **Passage &
Nearby Text** initially shows the match and nearest available neighbour on each side,
in source order; More reveals remaining bounded previews, Less restores that window.
Matched Passage has a stronger semantic heading, tonal surface and persistent logical
start-edge accent rule. All context blocks reserve equal padding/border space; Nearby
Text stays secondary. Leading down/up chevrons are decorative; expanded/controls
associations, focus and per-card reset behavior survive either transition.

Previews cap at 400 Unicode characters, preferring word boundaries, six visible lines
and 80ch width. PDF display rejoins conservative ASCII/U+2010/U+2011 word wraps and
single line breaks, retaining paragraph breaks; non-PDF authored breaks remain.
Only display text changes. Reader actions use full original text, section/page and
UTF-8 source spans through the existing target boundary; byte offsets are not DOM
UTF-16 offsets. Open Passage reads the source instead of expanding a preview into
an unbounded document.

### Settings And Activity

Settings → Semantic Search → Manage uses the existing AppDialog, with header Back/
Close, four-side safe-area padding and a scrollable body. There is no introduction
under **Semantic Search (Experimental)**. Tonal sections retain field borders/focus
rings. The first card has one status/switch row, stably named by its Semantic Search
heading; live status and conditional Model Download Required describe it. Opt-in alone
never means ready. General Settings keeps its observed readiness summary.

Manage Model owns loading/verification/download feedback and collapses stored/loaded
summaries; current operations/errors remain discoverable. Library Index owns coverage,
processed counts, Build/Update/Stop and separate index removal. Empty libraries request
imports, not 0/0 readiness. **How It Works** explains Matching, Device Resources and
Saved Work. The 64 MiB reserve is grouped under Download Size; general information and
explicit file verification have distinct icons/styles. Cancel Download/Stop Indexing
are neutral tonal actions; removals have red labels, standard hover/focus, inline
confirmation and current busy/empty-data guards. Confirmation cancel/restoration keeps
focus on a surviving control if removal hides the original panel.

Header activity remains reachable from reader/search while native work runs; it opens
setup without starting work. It announces phases rather than every document tick,
respects reduced motion and does not promise work after process exit/mobile suspension.
Completed/cancelled indexing notices last eight seconds; failure/unavailable-final-state
Review remains until opened or a new accepted action. Closing Settings detaches its
listeners/timers, not the App-owned controller or native work. Native status recovery
and generation checks preserve late-completion and duplicate-action guards.
