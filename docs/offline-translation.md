# Offline Translation Roadmap And Status

Papercut now has an in-progress offline translation pipeline. This branch’s desktop build configuration enables CTranslate2 and llama.cpp translation features so pinned OPUS-MT and HY-MT2 models use the same model-download, job, cache, validation, and storage pipeline. Linux builds include optional Vulkan acceleration for HY-MT2; Android and other GPU backends remain roadmap work.

The goal is high-quality offline translation for long-form HTML and EPUB books, primarily into English, while keeping the app responsive on desktop and mobile. The feature should feel like audiobook saving: the user starts a long-running job, the backend performs bounded native work, progress is visible, results are cached, and the finished output becomes durable user data.

The latest [Bergamot/OPUS-MT evaluation](#bergamot-versus-opus-mt-evaluation--2026-10-18) records actual desktop inference, markup failures and conditional mobile feasibility. It does not approve a production engine replacement.

## Current Implementation Status

- Translation accepts **HTML and EPUB only**, enforced in both the UI and native source loader. PDF and other formats remain unsupported.

- Desktop builds on this branch enable `native-tts-shared,experimental-search-embeddings,native-translation-ctranslate2,native-translation-llama` through `npm run desktop`.
- `npm run desktop:no-translation` keeps shared TTS and conceptual search while disabling translation for packaging/debug isolation.
- Spanish -> English and French -> English OPUS-MT CTranslate2 manifests are pinned and installable.
- HY-MT2 1.8B Q8 is a pinned, installable quality model for Arabic, German, Spanish, French, Russian, and Chinese translation into English. Desktop CPU inference remains the default. On Linux, the workbench offers Vulkan acceleration only when llama.cpp reports a real integrated or discrete Vulkan GPU.
- Installed models can be removed independently to reclaim storage. Removal is serialized against model installs and active translation jobs, while translated documents and reusable segment caches remain available.
- The translation workbench lists only pinned, supported models; future quality-model candidates remain informational.
- Translation jobs run through the native engine boundary, emit progress/cancel events, reuse segment cache entries, run first-pass quality gates, and persist successful output as derived uploaded documents.
- Fixed-pair OPUS-MT models use their actual source language; the UI and backend no longer imply automatic language detection.
- OPUS-MT exposes no quality, glossary, or repair controls because that engine cannot honor them; direct requests using those options are rejected.
- Translation controls, normal progress states, confirmations, counts, language names, and expected native failures use the app's eight locale resources. Translation commands return stable error codes while retaining the original Rust diagnostic; unexpected failures still fall back to that detail.
- Concurrent starts for the same document and translation settings are rejected before they can write the same cache or variant.
- Job progress distinguishes model loading, job and batch preparation, active
  batch inference, completed batches, final validation, and storage. Before
  the first measurable batch completes, the UI shows indeterminate activity
  instead of an apparently idle empty progress bar.
- OPUS-MT jobs use both a conservative 900-character planner cap and an engine-local tokenizer split before CTranslate2 inference, so long Spanish/French prose can be subdivided below Marian's 512-position limit without changing public cache segment ids.
- Translated variants are separate durable documents that can be opened, searched, deleted, and later used by the normal TTS flow.
- HTML/EPUB rendering uses the sanitized reader HTML where possible and preserves links, ids, images, tables, and EPUB-rewritten assets conservatively.
- Historical Stage 5B validation, before the latest main merge, covered local desktop and packaged `.deb` model-install, translation, open/search/delete, cancellation, restart recovery, cache-assisted retry, removal, and reinstall smoke tests. It does not validate the merged build; current smoke and release validation remain pending.
- Android and iOS translation are not supported yet; native runtime packaging, memory, thermals, and model-size behavior must be validated separately from desktop.
- Chapter-level repair has not started. Qwen remains research-only.

## Product Goals

- Translate imported HTML and EPUB documents offline.
- Preserve the original document unchanged.
- Produce a translated document variant that can be opened, searched, and used for TTS.
- Support long-running translation jobs with progress, cancellation, cache-assisted retry, and clear failure states.
- Use model catalogs, verified downloads, and app-data caches like native TTS.
- Prioritize translation quality for books and textbooks, not just short webpage snippets.
- Keep desktop and mobile model choices separate when needed.

## Non-Goals For The First Version

- Do not translate an entire book in one model request.
- Do not rewrite the original uploaded source.
- Do not require cloud translation.
- Do not attempt word-perfect bilingual highlighting in the first release.
- Do not add PDF translation to the first release; its page/text-layer locators need a separate translation design.
- Do not make one model serve every language pair if pair-specific models produce better results.

## Recommended Architecture

Translation should be a document-variant pipeline, not a reader-only overlay.

```text
Uploaded HTML/EPUB
  -> existing parser/sanitizer
  -> parsed sections and stable locators
  -> translation job
  -> translated safe HTML variant
  -> SQLite metadata + FTS index
  -> normal reader/search/TTS surfaces
```

This keeps the current contracts intact:

- `document_uploads` continues to own safe source extraction and section records.
- Translation consumes the parsed/stored document output instead of reparsing raw files in React.
- The reader opens a translated document variant the same way it opens normal imported documents.
- Search indexes the translated text as normal SQLite FTS rows.
- TTS generates audio from the translated document, so highlighting maps to translated DOM text instead of trying to align audio to a separate source-language DOM.

## Rust Module Shape

Add translation as a separate native feature area, parallel to native TTS:

```text
src-tauri/src/translation/
  cache.rs          # resume-safe segment cache and translation memory
  capabilities.rs   # capability/model-status reporting for the UI
  commands.rs       # Tauri command edge and event subscription plumbing
  config.rs         # size limits, chunk limits, feature constants
  ctranslate2.rs    # CTranslate2/OPUS-MT engine adapter
  dom.rs            # shared importer-backed DOM plan and source identities
  engine.rs         # shared engine trait and batch/segment contracts
  hash.rs           # stable non-cryptographic hashing for cache identity
  html.rs           # shared HTML parser boundary for DOM-preserving transforms
  hy_mt2.rs         # llama.cpp/HY-MT2 engine adapter
  inline_markup.rs  # marker-aware inline models and conservative phrase matching
  job.rs            # request validation, segmentation batching, cache keys
  model_install.rs  # download, verify, install, remove
  model_store.rs    # model manifests, install paths, model status helpers
  models.rs         # catalog metadata and language-pair support
  quality.rs        # output checks and repair hooks
  render.rs         # DOM-preserving translated document rendering
  runner.rs         # long-running translate/persist job orchestration
  segment.rs        # document/chapter/paragraph/sentence segmentation
  source.rs         # source-document reads from upload storage
  storage.rs        # translated variant persistence and provenance
  types.rs          # serde DTOs crossing the Tauri boundary
```

Keep dependencies pointing inward:

```text
commands -> jobs -> { engine, model, segment, storage, quality }
models/config/types have no app-state side effects
document_uploads stays format-focused and does not depend on translation
```

This prevents translation from leaking into EPUB/HTML parser code and keeps future PDF support clean.

## Frontend Module Shape

Use small React/API modules similar to TTS:

```text
src/translation/
  api/nativeTranslation.ts       # invoke commands and subscribe to progress events
  components/TranslationPanel.tsx
  components/TranslationPanel.css
  hooks/useTranslationManager.ts
```

The document list should show translated variants as durable documents, not transient UI state. A translated variant should carry visible metadata such as:

- source document title
- source language
- target language
- model name/version
- translation status
- created/updated time

## Long-Book Job Pattern

Long documents need bounded, resumable work. Recommended flow:

1. Validate selected source document and target language.
2. Load source provenance and a safe reader HTML/asset snapshot; derive text from the shared DOM plan.
3. Segment by chapter, heading, paragraph, sentence, and protected inline ranges. Current builds now carry translated segment fragments with normalized source offsets into rendering, and each readable block is extracted into a marker-aware inline model: prose text goes to MT, formatting spans stay as side data, and footnote/backlink markers are preserved as non-translatable inline codes. Inline rendering keeps exact carry-over emphasis for unique terms that survive translation unchanged and uses translated emphasized-phrase hints where available; hint matching is tolerant of MT probe noise (sentence casing, accent folding, wrapping quotes/punctuation) but still requires one unambiguous word-bounded target occurrence. Each projected span is treated independently so one unsafe range does not drop all nearby formatting, and distinct source spans that project onto overlapping target ranges are dropped rather than merged into fabricated combined emphasis. Proportional emphasis guesses were removed in the shared DOM foundation; uncertain spans now remain plain text. Full cross-language phrase alignment remains future work.
4. Future context-rich engines may build a document memory packet:
   - title
   - author, if known
   - heading hierarchy
   - recurring terms and named entities
   - user glossary entries
   - style preference, such as literal, fluent, or academic
5. Translate in bounded batches under the model context limit.
6. Cache each translated segment by source hash, model id, target language, glossary hash, and settings.
7. Run quality checks on each segment.
8. Assemble translated safe HTML with original anchors, footnotes, headings, images, and document structure preserved where possible.
9. Store translated variant metadata and source HTML in app data.
10. Index translated sections into SQLite FTS.
11. Surface completion in the Library and Search views.

This is the same mental model as audiobook saving, but the durable output is translated HTML plus index rows instead of WAV chunks plus timing data.

## Quality Strategy

Model choice alone will not make textbook translation good. Papercut should add book-aware quality controls from the start.

Required first-pass checks:

- Empty output detection.
- Wrong target language detection.
- Repeated-output loop detection.
- Length ratio sanity checks.
- Missing protected term checks.
- Broken anchor or tag checks.
- Chapter/section progress logging.

High-value quality upgrades:

- User-editable glossary per document or collection.
- Translation memory for repeated source sentences and phrases.
- Named-entity consistency across a book.
- Chapter-level repair pass for terminology and tone.
- Per-section regeneration.
- Side-by-side source/translation diff view for manual review.

## Performance Rules

- Never translate full books in one call.
- Keep model inference off the WebView thread.
- Download models on demand, verify checksums, and atomically promote complete installs.
- Batch sentences or paragraphs, but cap total tokens per request.
- Treat character caps as a first-pass guardrail only. OPUS-MT/Marian models can fail when tokenized input reaches the model's positional limit, so the CTranslate2 adapter must also enforce an installed-tokenizer source budget before inference. The current adapter translates subsegments internally and joins them back into the original cached segment output.
- Cache segment translations so resume work does not recompute finished sections.
- Persist job state after each section or batch.
- Stream progress events with current chapter, completed segments, total segments, elapsed time, and current model.
- Make CPU thread count configurable where the engine supports it.
- Prefer quantized models for default local use.
- Keep mobile model catalogs smaller and stricter than desktop catalogs.

Quality should not be sacrificed for speed by silently switching models. Expose a speed/quality choice only when the selected engine implements distinct modes; OPUS-MT currently does not.

## Model And Engine Recommendations

Papercut should support a catalog, not one universal model.

| Tier | Candidate | Engine | Best Use | Notes |
| --- | --- | --- | --- | --- |
| Fast MVP | OPUS-MT / Marian pair models | CTranslate2 | common pair translation, mobile-friendly baseline | small, fast, quality varies by pair |
| Quality desktop | HY-MT2 1.8B Q8 | llama.cpp via `llama-cpp-2` | multilingual book translation into English | pinned Apache-2.0 GGUF; CPU default with capability-gated Linux Vulkan |
| Context-rich fallback | Qwen3 8B or 14B | llama.cpp / GGUF | academic prose, Chinese-heavy work, glossary-aware prompts | strong but more generative and less task-bounded |
| Multilingual research | MADLAD-400 3B-MT | CTranslate2 or Candle spike | broad language coverage with Apache-2.0 license | heavier than pair models |
| HTML-aware reference | Bergamot | C++ or Wasm spike | tag alignment and sentence iteration ideas | not quality default for academic books |
| Avoid default | NLLB | CTranslate2 possible | broad research comparison only | CC-BY-NC and model card says not production/document translation |

### OPUS-MT And HY-MT2 System Requirements

| Concern | OPUS-MT CTranslate2 | HY-MT2 1.8B Q8 |
| --- | --- | --- |
| Installed model size | about 153-159 MB per language pair | 1.91 GB |
| Runtime | CTranslate2/Marian | llama.cpp |
| CPU use | practical default; optimized pair-specific inference | supported but expected to be materially slower |
| Memory | comparatively modest; benchmark before setting a formal floor | multi-GB model plus context/runtime overhead; benchmark before setting a formal floor |
| GPU requirement | none | none; compatible Linux Vulkan hardware is an optional acceleration path |
| Language coverage | one installed model per pair | one model covers the currently exposed six source languages into English |
| Mobile status | catalog candidate only; Android/iOS packaging is not yet supported | desktop only; mobile size, memory, thermals, and packaging are unvalidated |
| Best fit | quick routine translation | quality-focused translation where added time and storage are acceptable |

The table describes architecture and pinned artifact sizes, not a performance
promise. Record latency, peak memory, and quality on representative hardware
before defining minimum requirements. The Vulkan option is not a performance
promise and still needs representative integrated-GPU benchmarks. Do not
advertise CUDA, Metal, Android, or iOS support for HY-MT2 based only on
llama.cpp's upstream capabilities.

### Recommended Initial Language Pairs

Start with pairs that can be manually judged against real books:

- Arabic -> English
- Spanish -> English
- French -> English
- Russian -> English
- Chinese -> English
- German -> English

Each pair should have at least one short sample, one long chapter sample, and one textbook/academic sample in manual testing notes.

## Engine Evaluation Order

1. **CTranslate2 spike**
   - Best first implementation target.
   - Designed for optimized Transformer inference.
   - Supports Marian/OPUS-MT, NLLB, MADLAD-400, T5-style models, and quantization.
   - Native C++ packaging work is the main cost.
   - Start with OPUS-MT/Marian Spanish -> English and French -> English candidates.
   - Use `ct2rs` as the fastest desktop proof if it builds cleanly, but keep Papercut's `TranslationEngine` trait as the real boundary.
   - Before marking Android supported, validate whether `ct2rs` can package CTranslate2 cleanly with the Android NDK. If not, evaluate supported upstream bindings/build targets before introducing a maintained direct C++/FFI wrapper; see the latest Bergamot evaluation.
   - iOS should be treated as a later packaging validation target, not assumed from the desktop spike.

2. **llama.cpp / GGUF integration**
   - HY-MT2 1.8B Q8 is the first supported prompt-driven engine.
   - Good desktop ecosystem and quantization support.
   - Prompting must be tightly constrained to avoid paraphrase drift and hallucination.
   - Papercut uses the model's published translation-only prompt and bounded generation, while retaining the existing per-segment cache and quality gates.
   - CUDA, Apple Metal, and mobile are explicit follow-up targets rather than assumed support.
   - Qwen can still be compared for academic prose/context handling with stricter QA against paraphrase drift.

3. **Bergamot spike**
   - Valuable for HTML alignment and sentence iteration.
   - Do not assume Firefox-style browser translation quality is enough for academic books.
   - Use as a markup-preservation reference even if not the primary quality engine.

4. **Candle or ONNX Runtime spike**
   - Consider only if it reduces packaging complexity for a chosen model.
   - Avoid writing a fragile custom decoder unless a model clearly warrants it.

## Storage And Cache Identity

Translated variants need stable identity. Include:

- source document id
- source document content hash
- source language
- target language
- model id
- model version/checksum
- engine id/version
- translation settings
- glossary hash
- parser version
- segmenter version

Recommended tables, subject to migration review:

```text
translated_documents
  id
  source_document_id
  source_hash
  source_language
  target_language
  model_id
  engine_id
  settings_json
  glossary_hash
  status
  source_path or document_url
  created_at
  updated_at

translation_segments
  translated_document_id
  ordinal
  source_locator_json
  source_hash
  translated_text
  status
  quality_json
```

Translated variants reuse the existing uploaded-document section storage once promoted. The translation metadata table records provenance and the generated document URL; the upload/search tables own reader HTML, section text, and FTS rows.

Variant identity is deterministic from source document id plus translation settings (languages, model, quality mode, repair mode, glossary). Re-running the same document with identical settings replaces the stored variant in place - same id, same document URL - instead of accumulating duplicates. Replacement commits upload/search rows and translation provenance together, retains the previous files until the transaction succeeds, and recovers an interrupted promotion on retry. Deletion likewise removes provenance and shared upload/search rows in one transaction while the existing reversible filesystem flow protects the stored files. Changing a setting supported by the selected engine produces a separate variant; OPUS-MT accepts only its default quality with repair disabled and no glossary.

## UI/UX Requirements

First release should be plain and reliable:

- Add **Translate** action for uploaded HTML/EPUB documents.
- Keep document-level actions clear in the reader: users should choose whether they are saving an audiobook or starting translation instead of interpreting one overloaded save icon.
- Add a top-level **Translate** tab so long-running translation jobs, model downloads, translated variants, and diagnostics have a dedicated home like Audiobooks.
- Let the user choose source language, target language, and model.
- Show quality, glossary, or repair controls only for engines that implement them.
- Show model install state like TTS.
- Localize normal workflow copy and known command failures through stable error codes; do not parse or translate arbitrary native error prose in the frontend.
- Show progress by chapter/section, not an indeterminate spinner.
- Support cancellation and cache-assisted retry.
- Show translated copies in the Library.
- Show machine-translation caveat.
- Keep diagnostics under an advanced toggle.
- Never block reading the original while translation runs.

Mobile:

- Use smaller default models.
- Show storage size before download.
- Warn when model/job may be slow or battery-heavy.
- Avoid wide side-by-side translation UI until a translated variant exists.

## Build Modes

Desktop build feature selection deliberately keeps TTS and translation separate:

- `npm run desktop`: shared native TTS plus CTranslate2 and llama.cpp offline translation.
- `npm run desktop:static`: static native TTS plus CTranslate2 and llama.cpp offline translation.
- `npm run desktop:no-translation`: shared native TTS only, for isolating packaging or translation-runtime failures.

The build script composes the features at its final Tauri boundary:

```text
ttsFeature -> native-tts-shared or native-tts-static
features   -> ttsFeature + native-translation-ctranslate2
                         + native-translation-llama unless explicitly disabled
```

This keeps desktop builds useful for end-to-end translation testing without coupling translation diagnostics to TTS link-mode decisions.

Linux desktop builds need `cmake` because both native engines compile C/C++ code. Building the llama.cpp adapter also requires Clang/libclang, Vulkan development headers and loader, SPIR-V headers, and `glslc` for the Vulkan shaders. Papercut's declared Rust minimum is 1.85.

Android remains native-TTS-only for now. Do not add translation to `npm run android:apk:native-tts` until CTranslate2/`ct2rs` or a direct C++ wrapper has been validated with the Android NDK and package size/performance checks.

## Implementation Stages

Each stage should be easy to review and commit independently.

### Stage 1: Planning And Contracts - Done

- Add this document.
- Link it from README and document-upload docs.
- Add a Translation tab so the app has a dedicated navigation target.
- Add a distinct **Translate Document** reader action beside audiobook saving; route eligible uploaded HTML/EPUB documents into Translation setup.
- Decide branch name, feature flag name, and initial model candidates.
- Do not add translation model downloads, jobs, storage, or fake progress yet.

### Stage 2: Backend Skeleton - Done

- Add `src-tauri/src/translation/` with `types`, `models`, `config`, `commands`, and a stub engine.
- Register commands behind a disabled or stubbed feature.
- Return deterministic "translation unavailable" capabilities in browser/non-native paths.
- Keep planned model entries inert until checksum, license, required-file, and platform-gating review is complete.
- Surface manifest state, license notes, and size notes in the Translation tab so reviewers can distinguish candidate-only metadata, pinned file manifests, and future downloadable models.
- Add unit tests for model lookup and cache-key construction.

### Stage 3: Translated Variant Storage - Done

- Add SQLite metadata for translated document variants in the existing runtime upload/search database.
- Store generated safe HTML as derived upload documents under the existing upload/search contract, rather than a parallel translation-only document store.
- Add list/delete plumbing without model inference. Delete must remove only translated variant metadata/files and must not mutate the original uploaded document.
- Verify deleting a source document handles variants deliberately.

### Stage 4: Job Progress UI - Done

- Add React API, hook, and minimal Translation panel wired to the stub/storage commands.
- Load translation state lazily when the Translation tab is opened so normal Search/Library startup does not touch translation storage.
- Display capabilities, planned model metadata, translated-variant list/delete state, and clear unavailable messaging.
- Add selected-document controls for model/source/target/quality. Early builds used this as **Check Readiness** preflight; CTranslate2 desktop builds now run the real translation job through the same command path.
- Do not add fake progress; only report real install/job events.

### Stage 5A: Engine And Segmentation Contracts - Done

- Add a native translation engine boundary without pulling in CTranslate2 yet.
- Add source-document reads from the existing upload section tables by document URL.
- Add deterministic text segmentation with bounded segment sizes, stable ids, and unit tests.
- Add a job planner that validates request shape, batches bounded segments, and creates a deterministic settings cache key.
- Wire `translation_start` to run source/job planning on a blocking task. Non-translation builds return a clear unavailable response; CTranslate2 desktop builds continue into native inference.
- Keep this stage dependency-free so the branch stays easy to build/review before native packaging decisions.
- Treat segment context as quality hints only; translated output must map back by segment id, not by prompt text.
- Use per-segment content hashes in the future cache manifest before resume/regeneration ships; the current job key only separates incompatible settings.
- Document the CTranslate2 integration decision: Rust bindings exist, but choosing one affects native library packaging, Android support, tokenizer handling, and model cache layout.

### Stage 5B: CTranslate2 MVP - Desktop MVP Done

- Add native engine spike for OPUS-MT/Marian Spanish -> English and French -> English candidates.
- Keep `ct2rs` as the first desktop proof route, but do not couple storage/job code to it.
- Validate desktop and Android packaging before treating the CTranslate2 backend as supported. Evaluate upstream-supported packaging alternatives if the binding does not work on Android/iOS; do not assume a custom FFI fork is the default fallback.
- Add model manifest/cache plumbing before downloads:
  - Translation model files live under `<app-data>/translation/models/{model-id}`.
  - Future installer scratch work should live under the OS cache directory, then promote verified files into app data.
  - Candidate-only entries must stay non-installable until they have pinned source URLs, SHA-256 hashes, file sizes, required-file validation, license review, and platform gates.
- Pin the first file manifests and install them before native inference exists:
  - Spanish -> English: `michaelfeil/ct2fast-opus-mt-es-en` at revision `437f5ffc6c8544943c685ea405650e0d17cf6098`, 8 required files, 159,387,032 bytes total.
  - French -> English: `michaelfeil/ct2fast-opus-mt-fr-en` at revision `cb3b2d680bf35591a508d8479e2c99c44e281ef3`, 8 required files, 153,350,068 bytes total.
  - These Hugging Face repos advertise Apache-2.0 metadata, but keep OPUS-MT/Helsinki-NLP provenance and redistribution notes visible until final license review is complete.
- Implement model download/verify/install from the pinned file manifest:
  - Stream files with flat memory use.
  - Verify each file byte count and SHA-256.
  - Stage downloads in the OS cache directory.
  - Promote the complete verified folder into app data only after all files pass validation.
  - Emit model-install progress events for the Translation tab.
  - Keep the install command separate from `translation_start`, because having model files on disk does not mean the translation engine can run yet.
- Wire the Translation tab to model install state:
  - Load per-model status lazily with the Translation tab.
  - Show install buttons only for pinned file manifests.
  - Display download progress and installed badges in the candidate model cards.
  - Keep candidate-only models out of the runnable workbench selector until they have a pinned manifest and engine support.
- Wire installed models into translation preflight:
  - Reject unknown models and unsupported language pairs before reading large documents.
  - Require the selected pinned model to be installed before job planning.
  - Construct the matching native engine from the verified model directory.
  - In default builds, stop with a clear message when that model's native engine feature is not compiled.
  - Load either `ct2rs::Translator` or the pinned HY-MT2 GGUF behind the same `TranslationEngine` boundary used by stored jobs.
- Run the native engine in bounded batches, keep translated text in memory until all batches finish, and emit progress/cancellation events before durable writes begin.
- Preserve one translated section slot for every source section during assembly; blank outputs should fail validation with the source section ordinal instead of being dropped and reported as generic incomplete coverage.
- Do not cache empty segment translations; a native empty-output failure must be retried or reported, not reused on every later run.
- Plan text segments with offset-preserving Unicode sentence/word boundaries instead of splitting strings and searching for them again. This keeps URL-heavy reference blocks, citations, abbreviations, and repeated phrases from silently dropping source blocks before the native engine runs.
- Persist completed runs as separate derived upload documents:
  - Generate escaped safe HTML from translated sections.
  - Insert derived upload/source/section/FTS rows so the normal reader, Find, search, and TTS can consume the translated copy through the same contract as imported HTML.
  - Record translation provenance in `translated_documents`.
  - Delete translated variants without mutating the original uploaded document.
  - Refresh Library/Search state after translation create/delete so the generated document list stays in sync outside the Translation tab.
- Add retry-safe per-segment cache manifests:
  - Store completed segment translations under `<app-data>/translation/segment-cache/{cache-key}/segments.json`.
  - Key cache compatibility by model, language pair, quality mode, segment limits, and source text hash.
  - Reuse cached segments before calling the native engine so cancelled or failed large-book runs do not throw away completed batches.
  - Save the manifest after each completed batch to keep retries useful without waiting for the whole book to finish.
- Add tokenizer-aware source splitting inside the CTranslate2 adapter:
  - Load the same auto tokenizer family as the translator from the verified model directory.
  - Enforce a 448-source-token budget for OPUS-MT/Marian before calling CTranslate2.
  - Split oversized segments by sentence, then word, then character only as a last resort. Sentence boundaries use the same Unicode (UAX #29) rules as the planner, so abbreviation-heavy prose is not chopped mid-abbreviation before inference.
  - Rejoin translated subsegments under the original segment id so existing cache/progress/storage contracts do not change.
- Add staged writes/cleanup for translated-document persistence:
  - Write generated safe HTML through a staging directory before promoting it into upload storage.
  - Commit upload/search rows and translation provenance together.
  - Keep the previous generated variant available until a replacement transaction commits.
  - Recover interrupted replacements by comparing the promoted generation marker with SQLite.
  - Keep original source documents untouched throughout cleanup.
- Add visible resumed/cached segment counts to the Translation progress UI.

Status:

- Done: CTranslate2 feature flag, adapter, tokenizer-aware OPUS-MT source splitting, capabilities reporting, installed-model preflight, pinned manifests, model installer, install UI, fixed-pair source validation, source loading, bounded batches, duplicate-job rejection, cooperative cancellation, progress events, validation-before-storage state, segment cache with empty-output rejection, exact translation memory, staged writes, transactional/recoverable derived upload replacement and deletion, replace-on-rerun variant identity, document-list refresh, and visible cached/reused progress.
- Done: `npm run desktop` includes both `native-translation-ctranslate2` and `native-translation-llama` by default.
- Verified locally: desktop and packaged `.deb` model download/load, HTML/EPUB translation, translated variant open/search/delete, cancellation, restart recovery with cached-segment reuse, model removal/reinstall, and cache-assisted retry.
- Needs proof: release CI artifacts on supported desktop targets.
- Not done: Android translation packaging.

### Stage 6: Shared DOM Mapping And Asset Preservation — Foundation Complete

`translation::dom::DomPlan` is the engine-independent mapping boundary. It
uses the importer's leaf-block selector, so EPUB `div` blocks, nested lists and
blockquotes have the same source order during extraction, phrase probes and
rendering. SQLite supplies provenance; the sanitized reader snapshot supplies
text. Block identities use importer ordinals (gaps are allowed for protected or
marker-only blocks), and source text is checked again before rendering. Missing,
extra, reordered or stale mappings fail before durable writes, instead of
falling back to an HTML document that silently loses assets or links. Engine
batch replies must retain the exact requested segment identities and order.

Responsibilities:

- `document_uploads::html`: canonical block selection, sanitizer, section locators
  and search body ranges; the saved DOM is parsed once for its actual index rows.
- `translation::dom`: shared block identities, heading context and normalized
  inline models. Segment/source offsets remain **Unicode scalar indices**;
  target rendering ranges are **UTF-8 byte offsets**, with explicit conversion.
- `translation::inline_markup`: normalization, emphasis spans, footnote/backlink
  markers and conservative phrase matching. Structural word boundaries reuse the
  importer policy, including table cells/rows and line breaks; adjacent inline
  formatting does not split a word.
- `translation::render`: verifies the mapping and mutates the cloned reader DOM.
  Engine output is text, never trusted HTML. Existing block ids survive; links,
  notes and whole-block emphasis retain their existing conservative handling.
- `document_uploads::reader_assets`: snapshots only referenced canonical image
  names, deduplicates them, rejects missing/nonregular/symlinked/empty/oversized
  files and content-hash mismatches. Existing limits apply: 5 MiB per asset,
  100 MiB total. No hard links or source-directory pointers are persisted.
- `document_uploads::derived`: sanitizes and indexes the rendered HTML, stages
  its independently owned images with the HTML, and commits upload/search and
  translation metadata together. Failed replacements restore both old files
  and database rows. Stored size includes HTML plus unique referenced images.

There is **no proportional emphasis projection** in new output. Whole-block
formatting and unique carry-over/phrase-hint matches can survive; uncertain
partial emphasis becomes plain text. Phrase probes remain a best-effort aid,
not proof of semantic alignment. The cache alignment version is now **3**, so
old strategy caches are not silently reused. Variant identity is unchanged;
existing saved variants, original files, audio and highlighting are not migrated
or rewritten. Explicitly rerunning the same settings can replace that variant.

Limitations are deliberate: media/table-heavy blocks and blocks containing code,
protected inline content or nested non-link ids retain the original block and
receive translated prose beside it. Entire `translate="no"` blocks are not sent
to the engine. Protected inline text may still occur in the plain-text request;
the renderer preserves its original markup, rather than trusting a generated
replacement. Ordinary link labels may remain in the source language. Footnote
placement remains best effort. Exact inline translation, translated table-cell
mapping and provider annotations/alignments are future work, not guarantees of
this foundation.

Review hardening (2026-10-18) closes the three boundary gaps found after the
foundation commit:

- Both HTML and EPUB sanitizers retain canonical `translate="yes"`/`"no"`
  declarations and the narrowly allowed `doc-noteref`, `doc-backlink` and
  `epub:type="noteref"` annotations. Invalid values, unrelated roles, scripts and
  event handlers remain rejected. Marker recognition is shared with document
  metadata handling; this does not change the search auxiliary-range policy.
- HTML root/body declarations survive reader loading. EPUB root/body protection
  is copied to the generated chapter wrapper before those source elements are
  discarded. The nearest block declaration wins, so `translate="yes"` can opt a
  child block back in. Standard EPUB doctypes are supported without expanding
  authored entity declarations. Partial protected inline content keeps the
  conservative behavior described above.
- Table cells/rows and other structural elements now separate model-input words
  on entry and exit using the importer's existing rule. Punctuation and adjacent
  inline spans retain their prior behavior, including Unicode scalar offsets.
- Fragment rendering operates inside existing whole-block formatting wrappers.
  A bold paragraph with a partly italic phrase keeps its outer bold across one
  or multiple fragments; uniquely matched inner emphasis still survives, while
  uncertain inner emphasis stays plain. The normal planner-offset path, not
  only legacy fragments without offsets, is covered.
- Derived-document tests use the production preparation path before storing
  files and index rows. Sanitization is exercised on source reading and output
  preparation, not bypassed by every fixture.

Existing imports may already have lost these annotations under the previous
sanitizer. The new rules preserve annotations still present in stored sources;
they cannot reconstruct discarded publisher metadata. Reimport the original
HTML/EPUB when that protection matters. No migration or guessed reconstruction
modifies existing documents or audio.

Regression coverage includes shared leaf-block order, repeated text, line-break
boundaries, Arabic combining marks/emoji offsets, protected blocks, stale and
reordered mappings/replies, real French gold/Spanish king emphasis failures,
escaped model output, image snapshot tampering/limits, source deletion, actual
saved section/body-range rows, and replacement rollback. The hardening cases add
a real EPUB ZIP, safe metadata round trips, protected note-label extraction,
multilingual table input, mixed whole-block formatting with planner-generated
fragment offsets, and saved-reader/SQLite metadata checks.

Latest validation (2026-10-18): `cargo test --manifest-path src-tauri/Cargo.toml
--no-default-features --locked --offline --lib` passed **384 tests**, with
**4 existing ignored**. `cargo check` with the same no-default/locked/offline
configuration, Rust formatting and diff checks passed. The original three
review repros also passed against the actual sanitizer/mapper/renderer using
fixture translations, without running an engine. Feature-disabled native adapter
dead-code warnings remain. No runtime, dependency, provider, or app version
changed. Benchmarks, model inference/hardware comparisons, installed-app smoke
and the version bump remain deferred. Review of hardening commit `dea421a` on
2026-10-19 found no new blockers in the changed import, mapping and persistence
paths. A fresh run repeated **384 passed, 4 ignored**. This is code/regression
validation, not a claim about engine quality or packaged WebViews.

#### Provider Mapping Contract And Validation Slice (2026-10-19)

The next provider must fit the existing job and storage flow. Keep
`TranslationEngine::translate_batch` and its text-only OPUS-MT/HY-MT2 adapters
working until a real adapter needs a richer result. Do not introduce a provider
registry, second document store, copied Bergamot HTML implementation or parallel
renderer. The validation slice below implements reply checks and fixture-tested
emphasis mapping; the rest of this contract remains a requirement for future
integration. No new translation engine or HTML-aware capability is enabled.

| Owner | Responsibility |
| --- | --- |
| Rust job runner | Read the sanitized source snapshot, plan bounded batches, bind replies to the active attempt, manage cancellation and durable progress. |
| DOM/inline mapping | Own block identities, normalized source text, formatting identities and protected content; validate provider mappings. |
| Provider adapter | Translate complete sentence/segment context across inline tags; convert its own token/offset or HTML representation into validated text and mappings. |
| Shared renderer/storage | Change only authorized text/formatting locations in the cloned source DOM; sanitize, index and persist with independently owned assets. |

**Result binding and offsets.** One result belongs to one requested segment,
under the exact source snapshot, language pair, model artifact/runtime and
mapping strategy used for that attempt. Keep exact id/count/order checks before
cache writes. A source fingerprint identifies the snapshot; validate mappings
against the retained exact normalized source, not a returned hash alone. Repeated
sentences in different blocks are different locations even when their text is
identical. A worker bridge must additionally bind request, batch and attempt
identities: a late reply from a cancelled/restarted worker cannot satisfy a new
request. A mismatch is an error, not a reason to attach the result by position.

Use half-open `[start, end)` **Unicode scalar ranges** against the exact normalized
segment text for source and target mapping. Convert provider UTF-8 byte offsets,
UTF-16 code units or token positions explicitly in the adapter. Rendering converts
validated target ranges into UTF-8 byte offsets only at its existing boundary.
Do not normalize or trim text after establishing ranges without rebasing them.
Reject negative, reversed, out-of-bounds and non-boundary provider ranges; never
round an offset into a different character. Token alignment additionally needs
the provider's matching tokenization and normalization mapping to the original
segment, not offsets guessed from token string lengths.

**Text remains the baseline.** The existing adapters return text and may use the
documented conservative phrase hints. They do not claim semantic alignment.
Richer mapping is optional and must be established for the actual adapter/model
combination, not inferred from an engine option or model name. Normalized richer
results contain target text and target ranges associated with source-owned inline
identities. Provider HTML, if used by a supported API, is only an intermediate
adapter representation; it is never a replacement reader document.

**Validate before reconstructing markup.** Keep the authoritative tags and
attributes in Papercut's DOM plan. The adapter may associate translated text with
known inline identities, but cannot invent tags, URLs, ids, asset paths or note
targets. A protected-code HTML representation must preserve each required identity
and its allowed nesting/multiplicity; reject unknown, missing, duplicated or
changed codes and attributes. Sanitizing generated HTML alone is insufficient:
it can remove active content while still accepting the wrong footnote target or
dropping an image. Reconstruct from the source-owned DOM after validation.

Target order may differ from source order. For example, French
`Une bague en <strong>or</strong>.` may map that source emphasis to `gold` in
`A <strong>gold</strong> ring.`; it must not map by relative position to `ring`.
Nested ranges must form valid nesting, never crossing overlaps. An adapter may
report more than one disjoint target range for a source identity only when its
representation supports that case explicitly; do not fill the gap with invented
emphasis. Structural validity does not prove translation quality: reordered,
omitted and ambiguous words still need linguistic acceptance later.

**Protected content and failure behavior.** Links, assets, ids, code, note
identities and `translate="no"` content remain source-owned. A future rich path
must keep protected literal contents out of translatable payloads and validate
their unchanged restoration; an instruction to an LLM is not that guarantee.
The current partial-inline/code/media preservation fallback remains in force
until that path exists. If text is valid but emphasis mapping is unsupported or
ambiguous, retain the current safe plain-text/conservative rendering and report
the formatting limitation. If protected structure cannot be restored, retain
the source block with translated prose beside it or fail before saving; do not
silently discard content. Missing/empty text, wrong identities, stale replies or
invalid protected-code output fail the batch and leave the previous saved variant
intact. Do not silently select a different model or engine.

**Cache and worker lifetime.** Rich mappings are not reusable just because the
plain source string matches: their DOM identities must be rebound and validated
against the current plan. When implemented, include provider/model artifact,
normalization and mapping-strategy identity in cache compatibility; old text-only
cache entries cannot become aligned results. Preserve completed variants and
existing audio/highlighting. New variants derive indexing and future audio
positions from the actual saved DOM. For WASM, retain Rust job/storage authority;
use bounded messages and response deadlines, account for worker loss/WebView
suspension, discard late replies and resume only completed validated batches.
No full-book/model payloads as JSON/base64 and no detached worker may finish a
cancelled job by writing a variant. The synchronous native trait does not itself
provide this lifecycle bridge.

##### Implemented Validation Slice

- `translation::engine::validate_outputs` checks the whole reply for exact
  count/order, unique nonempty requested ids, exact source text and nonempty
  translated text before consuming any returned segment. Both normal batches
  and optional phrase probes use it. CTranslate2 and HY-MT2 retain their input
  text in the internal reply; this is adapter bookkeeping, not text generated by
  the model or authentication of an external worker. Invalid probe replies are
  nonfatal and contribute no new hints. Existing valid cached hints remain usable.
- `translation::inline_markup::provider` validates emphasis-only mappings against
  a retained `InlineSourceModel`. Each mapping references a source-local
  formatting-run index and a target scalar range. All requested runs must occur
  exactly once; unknown/missing/duplicate runs, empty/reversed/out-of-bounds ranges
  and all target overlaps are rejected. Target order may change, and nested styles
  use the source run's tag stack. This deliberately narrow representation cannot
  express multiple disjoint targets for one run or overlapping target runs, even
  when contained. Such mappings need an explicitly supported representation later,
  not guessed merging or expanded emphasis now.
- Target ranges convert strictly into UTF-8 bytes for the existing renderer.
  A separate strict UTF-8-to-scalar conversion rejects offsets inside an accent,
  emoji or combining-mark scalar. Offsets are scalar boundaries, not a claim of
  grapheme/linguistic alignment. UTF-16 and tokenizer adapters are not implemented.
- Mapping input has no tag, attribute, URL, asset or note-target fields. Tags come
  only from the retained source model; code cannot be remapped as emphasis. A
  renderer fixture verifies reordered `or` → `gold` emphasis and escaped
  HTML-looking output while original note/reference targets survive sanitization.
  This is not a provider-HTML or protected-code parser: structural code identity,
  nesting and multiplicity checks are still required before accepting such output.
- The emphasis boundary is compiled and fixture-tested but **not wired into a
  production engine or cache**. Current adapters still return plain text, and
  their existing conservative rendering/preservation paths remain active. There
  is no new UI capability, formatting-warning flow or rich-cache schema. Existing
  alignment epoch **3**, saved variants, originals, audio and highlighting remain
  unchanged. The ordinary text-only result DTO changes only inside Rust.

Validation: **392 Rust library tests passed, 4 existing ignored**, including
**115 translation tests**. New cases cover reply/source binding, a later empty
output in a batch, reordered and repeated emphasis, nested source tag stacks,
missing/duplicate/unknown runs, overlapping and invalid scalar/UTF-8 ranges,
plain-text/no-mapping input, code rejection and escaped HTML-looking output.
CTranslate2-feature `cargo check` with the locked/offline dependencies, Rust
formatting and diff checks passed. The combined native-feature compile check was
blocked by missing Vulkan headers required by HY-MT2 in this environment; that
feature still needs CI validation.
No model inference or new translation-quality result is claimed.

Next, prototype one supported Bergamot worker/interface using bounded text
requests through the current job lifecycle before enabling rich rendering. That
stage must establish attempt/batch identities, cancellation, timeout, worker loss
and late-reply handling; none is implemented by this synchronous validation
slice. Only then connect validated provider mappings and protected-content
restoration, cache provenance and capability reporting. Worker packaging, model
verification and real-device quality/performance remain acceptance work; no
default-model or mobile-support decision is made here. Benchmarks, installed-app
smoke and the app version bump remain deferred.

### Stage 7: Quality Upgrades - Mostly Done

- Add deterministic first-pass output sanity checks:
  - Reject all-empty translated output.
  - Reject extreme source/translation length-ratio failures that look like truncation or decoder loops.
  - Reject repeated long translated bodies across many sections.
  - Reject generated HTML with broken internal `#anchor` links before storage.
- Add exact-match translation memory through the existing segment cache:
  - Store translated text by source-text hash as well as segment id.
  - Reuse exact repeated source text across later batches and retries before calling the native engine.
  - Reuse inline phrase-probe translations through the same memory so repeated/resumed jobs do not re-translate every emphasized phrase.
  - Stamp caches with the inline-alignment strategy version so alignment-rule changes invalidate stale probe memory instead of mixing strategies.
  - Materialize reused translations back into positional segment entries so retries remain stable.
  - Keep this in the existing `serde_json` cache manifest; no new library is needed for exact-match memory.
  - Later optimize duplicate segments inside the same batch if benchmarks show repeated paragraphs are common enough to matter.
- Add glossary support scaffolding:
  - Accept glossary entries on translation job requests as source term, target term, and optional note.
  - Reject conflicting glossary mappings where one source term has multiple target terms.
  - Include glossary entries in job cache identity so changed protected terms do not reuse incompatible cached output.
  - Pass only exact source-term matches into each segment context to keep engine prompt/context payloads bounded.
  - Reject stored output when a section contains a glossary source term but misses the requested target term.
  - Store glossary-entry count and hash in translation provenance metadata.
  - Use standard string matching for now; add fuzzy matching libraries only if glossary miss rates justify the extra dependency.
  - Reject glossary entries for OPUS-MT because Marian pair models cannot consume them.
- Add named-entity consistency checks:
  - Current first slice treats glossary entries as explicit protected entities.
  - Future automatic NER should be model/library-backed; do not hand-roll broad multilingual NER with regexes.
- Add section regeneration foundation:
  - Keep quality failures internally structured by issue kind and optional source section ordinal.
  - Done: classify expected user-actionable failures at the Tauri command boundary, retain original diagnostic detail, and localize the stable codes in React.
  - Future retry loop can use those section ids to regenerate only failed sections instead of rerunning a whole book.
- Add optional chapter repair pass scaffolding:
  - Add `repairMode` to translation requests and cache identity.
  - Default remains `off`; no repair pass runs yet.
  - Carry repair mode through engine and persistence metadata so future quality models can add chapter-level repair without changing command shape.
  - Reject enabled repair mode for OPUS-MT until an engine implements the pass.

Status:

- Done: empty output, per-section empty output, length-ratio, repeated-output, broken-link, and glossary-target gates.
- Done: wrong-target-language detection via `whatlang` (statistical, deliberately conservative: long sections only, reliable detections only, and rejection requires a wrong-language majority across several sections or a unanimously wrong multi-section document, so foreign quotations never fail a job; unmapped target codes skip the gate).
- Done: exact-match segment cache and exact-memory reuse.
- Done: glossary request/cache/provenance scaffolding and conflicting glossary rejection.
- Done: structured quality issue internals and `repairMode` plumbing.
- Done: OPUS-MT rejects non-default quality, glossary, and repair options instead of creating misleading variants.
- Not done: automatic multilingual NER, section regeneration loop, actual chapter repair pass.

### Stage 8: HY-MT2 Quality Engine - In Progress

- Done: pin `tencent/Hy-MT2-1.8B-GGUF` at an exact revision, file size, and SHA-256.
- Done: add HY-MT2 to the existing installer and runnable workbench catalog.
- Done: add a CPU-first `llama-cpp-2` adapter behind `TranslationEngine`.
- Done: load the GGUF once per job, reuse a context within each batch, clear KV state between independent segments, and bound prompt/output tokens.
- Done: use HY-MT2's published translation-only prompt and sampler settings with a fixed seed.
- Done: retain the existing cache, glossary prompt hints, progress, cancellation-between-batches, validation, rendering, search indexing, and durable variant storage.
- Done: expose the model's 1.9 GB footprint and CPU-only performance warning
  in the workbench, and report model loading, first-batch preparation, active
  inference, batch completion, validation, and storage as distinct progress
  phases.
- Done: expose confirmed model removal with reclaimed-storage reporting while preserving completed translations and segment caches.
- Done: compile Vulkan into the normal Linux desktop llama.cpp runtime, detect real Vulkan GPUs, and expose an HY-MT2-only hardware acceleration toggle when supported.
- Done: keep hardware selection out of cache and translated-document identity so CPU and Vulkan execution reuse the same compatible segment results.
- Partial: the pinned model installed successfully and entered native inference on the current CPU-only development machine; cancellation also worked. Startup was too slow to complete a useful quality or throughput benchmark on that hardware.
- Verified: a packaged Linux `.deb` detects an Intel integrated Vulkan GPU and
  an accelerated HY-MT2 job engages it.
- Pending: run completed desktop quality and performance smoke tests on representative short/chapter/book samples using CPU and Intel integrated Vulkan hardware.
- Pending: confirm software-only Vulkan hides the option and the toggle-off
  path remains CPU-only in a packaged build.
- Deferred: NVIDIA RTX/CUDA smoke testing is unavailable on the current development machine. Do not claim CUDA support until a separate build feature, packaging pass, and quality/performance benchmark succeed on an RTX-class Linux machine.
- Deferred: Apple Metal, Windows packaging, Android, and iOS validation.
- Deferred: compare Qwen only if HY-MT2 quality has a demonstrated gap worth another runtime/model cost.

### Stage 9: Mobile Hardening - Not Started

- Benchmark memory, storage, speed, thermals, and battery.
- Limit model choices by platform capability.
- Add mobile-specific warnings and defaults.

## Manual Test Matrix

For every engine/model candidate:

- Short paragraph translation.
- Full chapter translation.
- Full long book translation.
- Resume after cancel.
- Resume after app restart.
- Delete translated variant.
- Re-run after source document delete.
- Search translated text.
- Generate TTS from translated text.
- Open original and translated variants independently.
- Verify internal links and footnotes still work.
- Verify RTL source documents and LTR translated output render correctly.
- For glossary-capable engines, verify the term is used consistently.
- Verify repeated terms/names are stable across chapters.
- On Linux with a physical Vulkan GPU, verify HY-MT2 shows the detected device and completes an accelerated job.
- Verify OPUS-MT never shows the hardware-acceleration option.
- Verify HY-MT2 with acceleration off follows the CPU path, and software-only Vulkan does not expose the option.
- Verify equivalent CPU and Vulkan runs reuse compatible cached segments rather than creating separate translation variants.

Language samples:

- Arabic -> English: undiacritized prose with names, footnotes, and Quranic/classical terms if relevant.
- Chinese -> English: academic prose and names.
- Russian -> English: philosophy/political terminology.
- German -> English: compounds and long sentences.
- French/Spanish -> English: high-resource baseline quality.

## Open Questions

- Should translated variants appear under the original document as children, or as separate documents with a source badge?
- Should translated variants be exportable/importable as part of future library backup?
- Should glossary be global, per collection, or per document?
- Does HY-MT2 Q8 meet the reviewed quality and latency floor on representative books?
- Which CTranslate2 model pairs have acceptable licenses and quality for the first supported languages?
- Should Android support translation in the first release, or only after desktop benchmarks?

## References To Recheck Before Implementation

- CTranslate2: https://github.com/OpenNMT/CTranslate2
- CTranslate2 Transformers support: https://opennmt.net/CTranslate2/guides/transformers.html
- ct2rs Rust bindings for CTranslate2: https://docs.rs/ct2rs
- HY-MT2 GGUF model card: https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF
- llama.cpp Rust bindings: https://github.com/utilityai/llama-cpp-rs
- Qwen3 8B model card: https://huggingface.co/Qwen/Qwen3-8B
- MADLAD-400 3B MT model card: https://huggingface.co/google/madlad400-3b-mt
- NLLB model card and limitations: https://huggingface.co/facebook/nllb-200-distilled-600M
- OPUS-MT Spanish -> English model card: https://huggingface.co/Helsinki-NLP/opus-mt-es-en
- OPUS-MT French -> English model card: https://huggingface.co/Helsinki-NLP/opus-mt-fr-en
- CTranslate2 Spanish -> English candidate: https://huggingface.co/michaelfeil/ct2fast-opus-mt-es-en
- CTranslate2 French -> English candidate: https://huggingface.co/michaelfeil/ct2fast-opus-mt-fr-en
- Bergamot / Firefox Translations docs: https://firefox-source-docs.mozilla.org/toolkit/components/translations/resources/03_bergamot.html
- ALMA / X-ALMA repository: https://github.com/fe1ixxu/ALMA

## Bergamot Versus OPUS-MT Evaluation — 2026-10-18

**Decision: keep OPUS-MT as the baseline and continue a bounded Bergamot
packaging/DOM-mapping prototype. Do not replace the production engine or promise
mobile support yet.** Bergamot is the stronger compact HTML-aware candidate,
but this evaluation found meaning errors and a protected-inline spacing defect.
It does not establish a translation-quality winner for books.

This supersedes earlier assumptions that a small model download implies low
runtime RAM, that the existing formatting projection preserves semantic
emphasis, or that an Android-friendly catalog entry proves mobile support.
Raw inputs, references, outputs, model hashes, configuration and measurements
are in [the evaluation evidence](offline-translation-evaluation.json). The JSON
is a research record, not a production catalog or a passing-quality oracle.

### Branch Integration And Validation

- Switched from `main` to `feature/offline-translation`; merge `834529d` has
  parents translation `0adce42` and main `871d702`.
- Preserved main's Ammonia 4.2.4, ZIP 9.0.4, ORT rc.13, current transfer/search
  work, Rust 1.88 minimum and the shared runtime setup. Translation keeps
  `ct2rs` 0.9.19 and llama.cpp binding/sys 0.1.153. Dependency resolution added
  translation transitives to main's lockfile; no Bergamot dependency was added.
- Reconciled changed document-source and parsed-document types and passed
  filename/PDF readiness metadata through the caller-owned transaction helper.
  Desktop build regression checks cover translation alongside conceptual
  search, the static alternative and disabling translation without disabling
  conceptual search.
- Passed: TypeScript build, eight-locale validation, frontend suite **373 passed,
  1 skipped**, Rust no-default-feature library suite **364 passed, 4 ignored**,
  and no-default-feature `cargo check`. Local socket/browser tests required
  running outside the restricted test sandbox; those retries passed.
- These checks do **not** validate a merged desktop native-translation release,
  Android/iOS packaging, installed-app smoke or a version bump. Those remain
  unclaimed/deferred. Original documents and existing audiobooks were not
  migrated or rewritten during this investigation.

### What Actually Ran

The isolated experiment used an Intel Core Ultra 5 325 Linux x86-64 host,
Node 24.21.0 and Python 3.12.14. No GPU was used. A synthetic diagnostic corpus
contains 12 French and 12 Spanish cases into English, including reordered bold,
nested emphasis, links, protected code/numbers, a table, negation, an idiom,
pronouns, technical vocabulary and a short multi-sentence library notice.
These are agent-authored references and an unblinded agent assessment, not
professional bilingual acceptance or a representative long-form book sample.

- **Bergamot:** official Firefox runtime `v0.6.0+eea6e5a8`, Remote Settings WASM
  3.0 and coherent fr→en/es→en model/lexicon/vocabulary version 2.0 sets.
  All downloaded artifacts were checked against their advertised SHA-256 and
  byte size. The stock runtime/glue and C++ were not patched. The upstream
  Mozilla Node worker test wrapper was used with Firefox's current **40 MiB
  initial WASM memory** instead of that test wrapper's older 234,291,200-byte
  setting; beam 1, workspace 128 MiB, soft alignment, one worker, cache disabled.
- **OPUS-MT:** exactly the branch's pinned model revisions and all eight verified
  files per direction. Official CTranslate2 **4.8.3 Python wheel**, CPU INT8,
  one inter/intra thread, beam 2 and `replace_unknowns=True`. SentencePiece
  source encoding appends `</s>`, matching `ct2rs`'s tokenizer. This is a model
  baseline through an official runtime, **not a measurement of the branch's
  compiled ct2rs/Ruy adapter**; wheel kernels and runtime versions differ.
- Both performed identical sequential plain-text requests, then five warm
  corpus passes. Timing excludes downloads, phrase probes, the Papercut planner,
  persistence and renderer. Different decoding settings are disclosed rather
  than presented as an equal-kernel engine comparison. Every case fits the
  branch's 448-token safety budget.
- Bergamot additionally translated the original HTML. A temporary Rust harness
  compiled the exact merged `render.rs`, `html.rs` and `inline_markup.rs` with
  storage DTO stubs and rendered actual OPUS output plus actual phrase-probe
  translations. That isolates rendering; it is not an end-to-end import/job test.
- The official Bergamot runtime also passed an HTML translation in a standard
  **Chromium 154 Web Worker**, with files served only on temporary localhost,
  no Firefox privileged APIs and no runtime patches. Transferable buffers were
  used. Node/Python inference ran after acquisition with network access blocked;
  it does not require a translation service connection.

### Size, RAM And Speed

| Measurement | Bergamot fr→en | OPUS-MT fr→en | Bergamot es→en | OPUS-MT es→en |
| --- | ---: | ---: | ---: | ---: |
| Model files, decimal MB | 37.20 | 153.35 | 37.01 | 159.39 |
| Fresh model to first short result, ms | 233.5 | 244.3 | 232.3 | 265.9 |
| Median warm 12-case pass, ms | 341.9 | 622.5 | 339.6 | 654.3 |
| Approx. source whitespace words/s | 418 | 230 | 421 | 219 |
| Whole-process peak RSS, MiB | 369.8 | 378.3 | 374.7 | 381.6 |

Bergamot also needs the shared 4,960,506-byte WASM and 100,187-byte JS glue.
Both language packs together are 74.21 MB before that shared runtime, versus
312.74 MB for both OPUS packs. Runtime packaging overhead for native CT2 is
not included in these model totals.

These are one fresh process per engine/language, with warm medians over five
passes, not repeated cold-start distributions. “Fresh” excludes interpreter
launch/imports and does not flush the OS file cache. Peak RSS includes Node
plus its worker or Python plus its libraries, not just tensors or incremental
Papercut memory. In this setup a four-times-smaller download did **not** produce
a four-times-smaller RAM footprint. No phone RAM, thermal or battery result is
available. Do not extrapolate these numbers to the ct2rs/Ruy build, whole books,
parallel jobs or a low-memory phone.

### Translation And Markup Findings

| Case | Bergamot observation | OPUS-MT / renderer at evaluation time |
| --- | --- | --- |
| `La bague en <strong>or</strong> est perdue.` | `The <strong>gold</strong> ring is lost.` | Text is correct; then-current renderer bolded **ring**. Spanish `oro` bolds **ring has**. |
| `La casa del <strong>rey</strong> es grande.` | Bolds **king's** correctly. | Text is correct; then-current renderer bolded **house**. |
| French size / nested emphasis | “king's house is big”; nesting retained. | “king's house is great”; French nested case loses italic and includes “a” in bold despite phrase probes. |
| French library notice | Changes “three books” to **“three pounds.”** | Retains “three books.” Both retain the broad notice structure. |
| Rain idioms | French becomes “raining ropes”; Spanish loses the rain meaning. | French also becomes “raining ropes”; Spanish retains rain but weakens the idiom. |
| Spanish possessive | Changes **her book** to **his book**. | Retains **her book**. |
| Link | Translates link text and retains `href="#note-1"`. | Isolated renderer retains the href but appends the original French/Spanish link text after the English sentence. |
| Protected `user_id` / `42` | Contents/attributes retained, but HTML yields **`user_idand`** with no separating space. | Plain output retains contents; renderer keeps `<code>` but drops `translate="no"` and the number's span. This fixture alone does not prove arbitrary protected text is safe. |
| Simple table | Retains the table/cell structure with English labels. | The text API returns English labels only. A synthetic bare-table request hits section fallback in the renderer; import/planner coverage for this shape was not tested. |

Keeping the same tags is insufficient: the emphasis must still refer to the
same meaning, protected text needs correct surrounding boundaries, and links
must keep their intended translated label. Bergamot's own alignment-based
restoration is materially better on the reorder examples, but still needs
validation. Generic attention or model-generated placeholders are not an
accuracy guarantee either. Neither engine's output is ready to be a golden
reference merely because it is fluent.

### Desktop And Mobile Feasibility

| Path | Evidence | Remaining gate |
| --- | --- | --- |
| Desktop Bergamot WASM | Actual Node and Chromium worker inference/HTML passed. One portable upstream runtime; no C++ fork needed. | Tauri CSP/asset access, verified model delivery without large JSON/base64 copies, cancellation, worker lifetime and native job integration; test actual Windows/macOS/Linux WebViews. |
| Desktop native Bergamot | Maintained source has native CMake targets and ARM/Ruy paths. | No native Bergamot build, Rust binding or all-desktop packaging was validated here. Source availability is not a supported turnkey mobile binary. |
| Android Bergamot WASM | Mozilla ships offline translation on Firefox Android; the Web Worker approach is plausible. | Firefox uses Gecko, Papercut uses Android WebView. SDK 26 alone does not establish SIMD support. Feature-detect on the installed WebView; test model load, offline operation, RSS/PSS, background/restart, cancellation, thermals and storage on a physical low-end device. |
| iOS Bergamot WASM | WebKit added WASM SIMD in **16.4**; current Papercut deployment target is **15.0**. | Do not silently raise deployment target. A WASM provider needs a runtime capability gate and an unavailable explanation on unsupported WebViews; test WKWebView/CSP, memory pressure and background suspension. No physical iOS result exists here. |
| OPUS-MT / CTranslate2 mobile | Upstream supports ARM64 and Ruy; the branch already selects Ruy. | Existing translation feature is still desktop-only. Neither Android NDK packaging nor iOS linking/lifecycle/performance is validated. The Python wheel benchmark does not fill those gaps. |

No connected Android/iOS device or `adb` was available for this investigation.
Mobile feasibility is therefore conditional; **mobile speed/RAM are unmeasured**.
A responsive desktop browser at a mobile viewport would not fill that gap.

### Next Bounded Work Before Production Integration

1. The shared DOM/asset foundation and hardening review are complete. The
   [provider validation slice](#provider-mapping-contract-and-validation-slice-2026-10-19)
   is now fixture-tested; production adapters remain text-only. Extend inline
   mapping only with validated annotations/alignments; do not reintroduce
   positional emphasis guesses.
2. Then prototype the supported WASM worker boundary before choosing a native
   Bergamot FFI. It runs today without a maintained C++ fork, but it cannot be
   dropped into the synchronous Rust `TranslationEngine` trait unchanged.
   Keep Rust job/storage authority, bounded requests and durable batch progress;
   explicitly handle WebView disappearance/suspension and late worker replies.
   Compare that small bridge with a supported upstream native build; avoid
   copying Bergamot's internal HTML C++ into Papercut.
3. Normalize provider results into that shared plan: annotated source/target
   spans or validated protected markup. `HTML::restore` needs token annotations
   and alignments; it is not a supported engine-independent HTML-only package.
   `ct2rs` 0.9.19 does not return attention through its FFI/result type even
   though it exposes the request option. Use an upstream-supported binding/API
   or a separately evaluated aligner if OPUS semantic inline mapping is retained.
   LLMs remain optional future providers, not trusted raw-HTML generators.
4. Asset ownership and actual-DOM indexing are now implemented and regression
   tested. Installed-app validation still needs complex EPUB images, semantic
   search after replacement, reader navigation and TTS/highlighting on new
   translated variants. Existing generated audio is not migrated.
5. Run a larger **blinded bilingual** French/Spanish book/technical corpus on
   both candidates, including idioms, numbers, omissions and semantic emphasis.
   Benchmark the actual packaged adapter on the weakest supported Android
   device and WKWebView, including cancellation, restart and sustained load.
   Only then select defaults. Test Arabic RTL/combining marks/reordering as a
   separate extension, not an inferred success from fr/es results.

Version cache/alignment semantics when mapping changes; preserve existing
variants, original documents and already generated audio/highlighting.
There is no Bergamot production integration, new model catalog entry, app
version bump or installed-app smoke result in this stage.

### Benchmark Deferral

The user deferred further benchmarks and model-default selection. The uncommitted
Pixel 7/Termux benchmark scripts, guide and generated package/cache were removed;
none is required by builds or tests. The committed evaluation JSON remains the
historical evidence behind the comparison above. No Pixel 7 speed/RAM or mobile
translation support is claimed. Reintroduce a bounded benchmark only when an
actual provider adapter is ready to evaluate.

### Reproducing Or Extending The Comparison

The evidence JSON includes the exact corpus, both plain-text outputs, Bergamot
HTML outputs, OPUS phrase probes/renderer outputs, raw warm timings, artifact
hashes and Remote Settings records. Fetch Mozilla attachments from the recorded
base URL plus each `attachment.location`; reject size/hash mismatches. Download
OPUS files from `https://huggingface.co/{repository}/resolve/{revision}/{path}`
and verify the recorded hashes. Use the recorded immutable Firefox JS revision and verify its hash as well as
WASM; a moving `main` URL is not sufficient to reproduce the runtime.

For Bergamot use the [upstream Node engine test wrapper](https://github.com/mozilla/translations/tree/69455acaecbe8650cdba988dbcf7c10ca20e7c48/inference/wasm/tests/engine),
placing official artifacts under `generated/` and model files under
`models/fren/` and `models/esen/`. Set its loader `INITIAL_MEMORY` to 41,943,040
as [current Firefox does](https://github.com/mozilla-firefox/firefox/blob/main/toolkit/components/translations/content/translations-engine.worker.js).
Construct `TranslationsEngine(lang, 'en')`, await `translate(case.text, false)`
for each language's cases, then repeat that sequence five times with cache zero.
Use `translate(case.html, true)` separately for markup and terminate the worker.
Record `performance.now()` and `process.resourceUsage().maxRSS` from the isolated
process. Adapting the wrapper to browser-native Worker globals verified the same
runtime in Chromium; no Emscripten/C++ build was needed.

For OPUS use Python `ctranslate2==4.8.3`, `sentencepiece==0.2.2` and the source
and target `.spm` files. The matching inference call is:

```python
translator = ctranslate2.Translator(model_dir, device="cpu", compute_type="int8",
                                   inter_threads=1, intra_threads=1)
source_tokens = source_sp.encode(text, out_type=str) + ["</s>"]
result = translator.translate_batch([source_tokens], beam_size=2,
                                    replace_unknowns=True)[0]
translated = target_sp.decode(result.hypotheses[0])
```

Use the same first pass/five-repeat sequence, `time.perf_counter()` and Linux
`resource.getrusage(resource.RUSAGE_SELF).ru_maxrss`. Do not include warm-up,
model acquisition or invented EOS settings in only one engine's comparison.
Exact wrapper/runner scratch files for this run are under
`/tmp/papercut-translation-investigation/`; they are disposable, not a production
build path. Durable evidence/configuration is retained in the linked JSON.

Primary feasibility references: [Firefox Android offline translation](https://support.mozilla.org/en-US/kb/android-translation),
[WebKit 16.4 SIMD](https://webkit.org/blog/13966/webkit-features-in-safari-16-4/),
[CTranslate2 hardware](https://opennmt.net/CTranslate2/hardware_support.html),
[CTranslate2 build/install options](https://opennmt.net/CTranslate2/installation.html),
[Bergamot HTML restoration](https://github.com/mozilla/translations/blob/69455acaecbe8650cdba988dbcf7c10ca20e7c48/inference/src/translator/html.cpp),
and [pinned ct2rs tokenizer](https://github.com/jkawamoto/ctranslate2-rs/blob/v0.9.19/ct2rs/src/tokenizers/sentencepiece.rs).
