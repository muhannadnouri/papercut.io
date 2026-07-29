# Offline Translation Roadmap And Status

Papercut now has an in-progress offline translation pipeline. The production desktop build includes CTranslate2 and llama.cpp translation features so pinned OPUS-MT and HY-MT2 models use the same model-download, job, cache, validation, and storage pipeline. Android and GPU-accelerated LLM translation remain roadmap work.

The goal is high-quality offline translation for long-form HTML and EPUB books, primarily into English, while keeping the app responsive on desktop and mobile. The feature should feel like audiobook saving: the user starts a long-running job, the backend performs bounded native work, progress is visible, results are cached, and the finished output becomes durable user data.

## Current Implementation Status

- Desktop production builds now compile with `native-tts-shared,native-translation-ctranslate2,native-translation-llama` through `npm run desktop`.
- `npm run desktop:no-translation` keeps the desktop build on native TTS only for packaging/debug isolation.
- Spanish -> English and French -> English OPUS-MT CTranslate2 manifests are pinned and installable.
- HY-MT2 1.8B Q8 is a pinned, installable quality model for Arabic, German, Spanish, French, Russian, and Chinese translation into English. The first supported path is desktop CPU inference through `llama-cpp-2`; NVIDIA/CUDA has not been smoke-tested yet.
- The translation workbench lists only pinned, supported models; future quality-model candidates remain informational.
- Translation jobs run through the native engine boundary, emit progress/cancel events, reuse segment cache entries, run first-pass quality gates, and persist successful output as derived uploaded documents.
- Fixed-pair OPUS-MT models use their actual source language; the UI and backend no longer imply automatic language detection.
- OPUS-MT exposes no quality, glossary, or repair controls because that engine cannot honor them; direct requests using those options are rejected.
- Translation controls, normal progress states, confirmations, counts, language names, and expected native failures use the app's eight locale resources. Translation commands return stable error codes while retaining the original Rust diagnostic; unexpected failures still fall back to that detail.
- Concurrent starts for the same document and translation settings are rejected before they can write the same cache or variant.
- Job progress now distinguishes final validation from storage, so a translation can reach 100% segment completion and still fail with a specific quality issue before any derived document is promoted.
- OPUS-MT jobs use both a conservative 900-character planner cap and an engine-local tokenizer split before CTranslate2 inference, so long Spanish/French prose can be subdivided below Marian's 512-position limit without changing public cache segment ids.
- Translated variants are separate durable documents that can be opened, searched, deleted, and later used by the normal TTS flow.
- HTML/EPUB rendering uses the sanitized reader HTML where possible and preserves links, ids, images, tables, and EPUB-rewritten assets conservatively.
- Stage 5B has passed local desktop model-install, translation, open/search/delete, and cache-assisted retry smoke tests. Packaged release artifacts still need CI/release validation.
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
  engine.rs         # shared engine trait and batch/segment contracts
  hash.rs           # stable non-cryptographic hashing for cache identity
  html.rs           # shared HTML parser boundary for DOM-preserving transforms
  hy_mt2.rs         # llama.cpp/HY-MT2 engine adapter
  inline_markup.rs  # marker-aware inline models and span projection
  job.rs            # request validation, segmentation batching, cache keys
  model_install.rs  # download, verify, extract, install
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
2. Load source section metadata and stored safe HTML.
3. Segment by chapter, heading, paragraph, sentence, and protected inline ranges. Current builds now carry translated segment fragments with normalized source offsets into rendering, and each readable block is extracted into a marker-aware inline model: prose text goes to MT, formatting spans stay as side data, and footnote/backlink markers are preserved as non-translatable inline codes. Inline rendering keeps exact carry-over emphasis for unique terms that survive translation unchanged and uses translated emphasized-phrase hints where available; hint matching is tolerant of MT probe noise (sentence casing, accent folding, wrapping quotes/punctuation) but still requires one unambiguous word-bounded target occurrence. Each projected span is treated independently so one unsafe range does not drop all nearby formatting, and distinct source spans that project onto overlapping target ranges are dropped rather than merged into fabricated combined emphasis. Full cross-language phrase alignment remains future work.
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
| Quality desktop | HY-MT2 1.8B Q8 | llama.cpp via `llama-cpp-2` | multilingual book translation into English | pinned Apache-2.0 GGUF; CPU supported first, GPU validation pending |
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
| GPU requirement | none | none for the initial CPU path |
| Language coverage | one installed model per pair | one model covers the currently exposed six source languages into English |
| Mobile status | catalog candidate only; Android/iOS packaging is not yet supported | desktop only; mobile size, memory, thermals, and packaging are unvalidated |
| Best fit | quick routine translation | quality-focused translation where added time and storage are acceptable |

The table describes architecture and pinned artifact sizes, not a performance
promise. Record latency, peak memory, and quality on representative hardware
before defining minimum requirements. In particular, do not advertise CUDA,
Metal, Android, or iOS support for HY-MT2 based only on llama.cpp's upstream
capabilities.

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
   - Before marking Android supported, validate whether `ct2rs` can package CTranslate2 cleanly with the Android NDK. If not, keep the Rust API and replace the internals with a thin direct C++/FFI wrapper.
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

Linux desktop builds need `cmake` because both native engines compile C/C++ code. Building the llama.cpp adapter also requires Clang/libclang for generated bindings. Papercut's declared Rust minimum is 1.85.

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
- Validate desktop and Android packaging before treating the CTranslate2 backend as supported. Use direct C++/FFI if Rust binding packaging is not good enough for Android/iOS.
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
- Verified locally: desktop model download/load, HTML/EPUB translation, translated variant open/search/delete, cancellation, and cache-assisted retry.
- Needs proof: packaged release artifacts and app-restart recovery under real interrupted jobs.
- Not done: Android translation packaging.

### Stage 6: HTML/EPUB Preservation - Mostly Done

- Preserve document order and heading shape from the existing section data:
  - Carry source section ordinals into translated output.
  - Render translated heading blocks as headings instead of duplicating original-language headings.
  - Add stable translated-section anchors and source metadata attributes.
- Add first-pass DOM transform rendering:
  - Use the sanitized reader `view_html` as the source of truth.
  - Use the existing HTML parser stack (`kuchikiki`) to replace mapped readable text in place.
  - Keep inline emphasis span collection/projection in `translation::inline_markup` so future phrase alignment or placeholder repair can replace the projection strategy without rewriting DOM rendering.
  - Extract readable blocks into a marker-aware inline model before translation: text is translated, footnote/backlink markers are preserved as inline codes, and formatting spans are carried as side data.
  - Collect render blocks with the same descendant-skipping behavior as the importer so nested endnote `<li><p>...</p></li>` structures do not shift section mapping.
  - Preserve simple block attributes, existing ids, links, images, tables, and EPUB-rewritten assets from the cloned safe DOM.
  - Preserve whole-block inline emphasis when one safe formatting wrapper owns the entire source block, such as `<strong>...</strong>` or nested `<em><strong>...</strong></em>`.
  - Carry translated segment fragments and normalized source offsets through storage-time rendering so mixed inline emphasis is projected inside sentence/segment windows before falling back to block-level projection.
  - Prefer exact carry-over matches for unique emphasized phrases that survive translation unchanged before using proportional projection.
  - Translate small emphasized source phrases as best-effort repair probes and store them with their segment fragments, so renderer can match the translated phrase rather than only proportional position. Probe translations are reused through the segment cache's translation memory, and probe hints are only attached when reader-DOM blocks align one-to-one with planned source blocks.
  - Match hint/carry-over phrases exactly first, then with case/accent folding and wrapping-punctuation trimming, always requiring a unique word-bounded occurrence in the translated text. Accent folding uses Unicode canonical decomposition, so all combining-mark Latin diacritics fold consistently across language pairs.
  - Project safe partial inline emphasis spans onto translated text by relative text position snapped to word boundaries (mid-word starts snap to the nearest word edge), retaining safe non-overlapping spans independently instead of dropping a whole fragment when one span cannot be trusted; overlapping projections are dropped, never merged.
  - Fragment hints also feed block-level projection when fragment-to-source pairing fails, so probe work is not lost with the fragment path.
  - Preserve footnote/noteref anchors and ordered-list endnote structure when replacing translated text; marker labels are stripped from MT input and reinserted near their translated anchor word, with word-boundary fallback to avoid markers landing inside words. Block-final markers (such as endnote backlinks) are placed after the whole translated text, including MT-added trailing punctuation.
  - Keep inline text extraction free of invented whitespace: punctuation text nodes directly after a formatting element stay attached to the preceding word so MT input and fragment matching see natural prose.
  - Insert translated fallback text beside blocks that contain media/table content instead of destroying assets or table structure.
- Next preservation work requires a stronger section locator layer:
  - Add phrase/word alignment for translated phrases that reorder substantially across languages.
  - Map translated segments to exact DOM text nodes rather than only block order when source fragments are split across complex nested markup.
  - Add coverage for complex nested links, footnotes, reordered phrases, and tables.
- Add fixtures for footnotes, links, RTL text, images, and tables.
- Add first-pass quality checks for broken internal links and empty translated output before storing translated variants.
- Later quality checks should add language detection, repeated-output detection, length-ratio checks, protected term checks, and richer tag/anchor validation.

Status:

- Done: DOM-preserving render path uses sanitized `view_html`; parser details are centralized in `translation::html`; inline markup collection/projection is isolated in `translation::inline_markup`; render block collection now matches importer block units; simple block text is replaced in place; real translation jobs carry source/target segment fragments plus normalized source offsets into translated sections; readable blocks are extracted into marker-aware inline models; whole-block inline emphasis is preserved when structurally unambiguous; mixed inline emphasis spans are projected inside segment windows before block-level fallback; exact carry-over emphasized phrases are preserved when one unique target match exists; small emphasized phrases are translated as best-effort repair hints for exact target phrase matching; safe non-overlapping inline spans survive even when nearby spans cannot be placed; footnote anchors and ordered endnote list items survive replacement without entering MT prose; media/table-heavy blocks keep source markup and insert translated fallback text nearby; generated output carries source ordinals and stable translated-section anchors.
- Done: first-pass broken internal-link and empty-output validation, including source-section-specific errors when one section returns blank after all segments finish.
- Done: focused renderer regressions cover reordered emphasis placed through translated phrase hints, nested internal-link markup, and internal links retained inside preserved table blocks.
- Still needed: general phrase alignment for reordered translations without a reliable hint, real-world complex-document fixtures, and richer tag/anchor validation.

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
- Done: expose the model's 1.9 GB footprint and CPU-only performance warning in the workbench, and report model loading separately from translation progress.
- Partial: the pinned model installed successfully and entered native inference on the current CPU-only development machine; cancellation also worked. Startup was too slow to complete a useful quality or throughput benchmark on that hardware.
- Pending: run completed desktop quality and performance smoke tests on representative short/chapter/book samples using suitable hardware.
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
