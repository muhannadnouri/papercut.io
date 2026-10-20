# Offline Translation Roadmap And Status

Papercut now has an in-progress offline translation pipeline. This branch’s desktop build configuration enables CTranslate2 and llama.cpp translation features so pinned OPUS-MT and HY-MT2 models use the same model-download, job, cache, validation, and storage pipeline. Linux builds include optional Vulkan acceleration for HY-MT2; Android and other GPU backends remain roadmap work.

The goal is high-quality offline translation for long-form HTML and EPUB books, primarily into English, while keeping the app responsive on desktop and mobile. The feature should feel like audiobook saving: the user starts a long-running job, the backend performs bounded native work, progress is visible, results are cached, and the finished output becomes durable user data.

The latest [Bergamot/OPUS-MT evaluation](#bergamot-versus-opus-mt-evaluation--2026-10-18) records actual desktop inference, markup failures and conditional mobile feasibility. It does not approve a production engine replacement.

## Current Implementation Status

- Translation accepts **HTML and EPUB only**, enforced in both the UI and native source loader. PDF and other formats remain unsupported.

- Desktop builds on this branch enable `native-tts-shared,experimental-search-embeddings,native-translation-ctranslate2,native-translation-llama` through `npm run desktop`.
- `npm run desktop:no-translation` keeps shared TTS and conceptual search while disabling translation for packaging/debug isolation.
- Mobile build scripts now accept explicit `--translation-worker` opt-in; CI exposes a default-off `mobile_translation` input for `mobile`/`full` scope. This enables only the experimental Bergamot host, not native CTranslate2/llama.cpp; ordinary PR/release builds remain unchanged. See [mobile build opt-in](#worker-only-mobile-build-and-ci-opt-in-2026-10-19).
- Spanish -> English and French -> English OPUS-MT CTranslate2 manifests are pinned and installable.
- HY-MT2 1.8B Q8 is a pinned, installable quality model for Arabic, German, Spanish, French, Russian, and Chinese translation into English. Desktop CPU inference remains the default. On Linux, the workbench offers Vulkan acceleration only when llama.cpp reports a real integrated or discrete Vulkan GPU.
- Installed models can be removed independently to reclaim storage. Removal is serialized against model installs and active translation jobs, while translated documents and reusable segment caches remain available.
- The workbench selects only pinned models whose engine and WebView gates pass. Unavailable pinned models remain visible with an explanation; future model candidates remain informational. Existing native catalog ordering/defaults are unchanged.
- Translation jobs run through the shared Rust engine boundary, emit progress/cancel events, reuse segment cache entries, run first-pass quality gates, and persist successful output as derived uploaded documents.
- Bergamot French/Spanish → English are now **experimental, capability-gated catalog choices**. The document path reuses one foreground worker, resumes completed batches, validates protected markup and confirms a live host before the shared Rust atomic writer saves. Explicit verified installation is required. Catalog availability is not packaged-device or translation-quality acceptance; see [catalog integration](#capability-gated-bergamot-catalog-and-ui-2026-10-19).
- Bergamot document planning keeps blocks that fit the 320-scalar cap intact, then uses sentence/clause boundaries for oversized blocks. This is an app-side mitigation for long-input omissions, not a model-token budget or a completeness guarantee; native provider planning stays unchanged.
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
- Packaged Android/iOS translation support is not accepted yet. Experimental Bergamot choices can appear when their build and WebView gates pass, but mobile memory, thermals, lifecycle and actual-model quality still need device validation; native runtime packaging remains separate work.
- Chapter-level repair has not started. Qwen remains research-only.

## Engine-independent offline alignment plan — 2026-10-19

**Decision: use a separate offline word aligner (Option 7). This is a plan, not
an implemented capability; no alignment model or runtime has been selected.**
This is the current forward plan for native rich formatting, superseding the
earlier attention-first proposal below. Further Bergamot upstream work is
shelved. Existing experimental Bergamot code is retained without extending or
patching its engine. Benchmarks, broader smoke and the app version bump remain
deferred; their eventual acceptance gates are explicitly listed below.

The first supported target is CTranslate2/OPUS-MT French/Spanish → English on
desktop. The alignment boundary must also accept HY-MT2 and future providers'
plain text, without depending on translation-engine attention. This does not
make those providers, Arabic or mobile accepted automatically. Inputs remain
HTML/EPUB only. Preservation means the semantics and assets in **sanitized reader
HTML**, not pixel-identical reproduction of arbitrary source CSS or scripts.

### Architecture and ownership

```text
Stored HTML/EPUB reader DOM + owned assets
  -> shared DOM/source-identity plan
  -> bounded prose units with exact source offsets
  -> selected translation engine -> exact target text + actual piece boundaries
  -> independent local word aligner -> source/target word links
  -> shared projection + strict mapping validation
  -> source-owned DOM reconstruction
  -> validated batch cache -> final validation -> existing atomic variant writer
```

- **DOM planning owns meaning-bearing structure:** block order, hierarchy,
  emphasis, links, authored identities, immutable literals, note attachment and
  asset references. Models never own reader tags, attributes or file paths.
- **The translation engine owns translated prose.** It preserves whole-sentence
  context across bold/italic boundaries and reports any internal subdivisions.
  Do not translate each inline text node separately or add phrase-probe calls.
- **The aligner owns lexical correspondence only.** It reads the exact source
  and actual target, returning word links and any available scores. It cannot
  rewrite either text, create markup, access storage or choose another engine.
- **The shared mapper owns projection and validation.** It combines source
  identities, word links and explicit punctuation/attachment rules. Structural
  validity and a high model score are not proof of semantic correctness.
- **The renderer/writer own durable output.** Reuse the source-owned protected
  reconstruction path; do not add another renderer. Preserve originals, existing
  variants, generated audiobooks and read-along data. New audio continues to use
  the translated variant's own reader DOM.

Reuse `dom.rs`, `segment.rs`, `inline_markup/{protected,provider}.rs`, `render.rs`,
`engine.rs`, `ctranslate2.rs`, `runner.rs`, `cache.rs`, `state.rs`, the model
installer/store and current UI capability flow. The existing provider validator
maps each flattened formatting run to one target range; it is a starting seam,
**not** the complete tree/notes/disjoint-range contract. Add a small translation
alignment module for inference and token/offset adaptation, separate from DOM
projection. No general plugin framework, parallel job system or second model
manager is needed. Non-obvious functions must explain intent, coordinate units,
ownership and failure behavior in comments.

### Non-negotiable behavior

1. Canonical mapping coordinates are half-open Unicode scalar ranges over exact
   retained strings. Tokenizer normalization/word-piece offsets require a checked
   conversion; UTF-8 bytes, scalars and grapheme boundaries are not interchangeable.
   Reject unmappable boundaries rather than rounding into another character.
2. Repeatable emphasis may map to multiple disjoint target ranges. Authored IDs,
   anchors and protected literals retain their source identity and parent chain;
   never clone them to make an impossible target mapping fit. Reordering prose is
   allowed, changing source block order or link targets is not.
3. Notes/images are source-owned attachments, not translated words. Record their
   owner, side and quote/punctuation scope. Internal links and backreferences
   must still resolve exactly once where their source identity requires it.
4. Keep code and other non-translatable literals out of inference as immutable
   islands. Initially translate contiguous prose around them and reassemble
   through the source plan; this can reduce context across a literal. Inline
   emphasis alone must never create such a boundary. Do not rely on the model
   echoing invented placeholders as proof of preservation.
5. Missing, ambiguous or contradictory required mappings stop the job with a
   useful error before saving. No fallback to positional estimates, phrase
   guesses, unformatted output, or source-plus-translation duplication on this
   strict path. Previously validated progress remains resumable. Legacy text
   output is not retroactively certified as formatting-preserving.
6. All inference runs locally after explicit, verified downloads. Bound tokens,
   tensor sizes, pair counts, model sessions and reconstruction work. No silent
   token truncation, unbounded retry tree, remote service or document-content
   logging. Alignment cannot repair a mistranslation or prove completeness.

### Stages and completion gates

Run these stages in order, reviewing each bounded change before proceeding.
Every implementation stage needs focused regression tests, relevant existing
suites and a documentation receipt distinguishing mocks from real inference.
All implementation stages below are **pending**. A stage with a deferred gate
must stay pending rather than being described as accepted.

#### 1. Prove a viable model/runtime path before building around it

**Tasks.** Evaluate one pretrained multilingual candidate first, with at most
one fallback. [Awesome-align](https://github.com/neulab/awesome-align) extracts
word links using multilingual BERT; [SimAlign](https://github.com/cisnlp/simalign)
uses multilingual embeddings and matching. Both publish research-oriented Python
implementations, not ready-made Rust/mobile HTML components. Treat them as
candidates, not selected dependencies. Check maintenance, code and weight
licenses separately, provenance, pinned artifacts, tokenizer behavior, extraction
algorithm, supported languages and supported export/inference tooling. Prefer
an existing compatible ONNX artifact and Papercut's existing ORT integration;
do not assume its pinned API/operator set or mobile packaging can load it.

Use an isolated, reproducible synthetic correctness probe outside the repo to
compare supported reference output with the proposed runtime. No training,
local library fork, copied C++ implementation, custom exporter or Python runtime
shipped with Papercut. If a small matching/offset adapter is necessary, record
exactly what code Papercut would maintain before choosing it. Export/quantization
is a separate feasibility decision, not an implicit maintenance commitment.

**Tests/gate.** Record exact model/tokenizer/runtime revisions and licenses;
load a real model offline and reproduce reference tokenization/link decisions,
with numeric tolerances documented for model scores, on French/Spanish synthetic
gold pairs. Include reordering,
repeated words, one-to-many links and punctuation; test meaningful successful
alignments as well as rejection cases. Inspect file sizes, tensor/token limits
and runtime compatibility. Set explicit desktop/mobile RAM and latency budgets
and a held-out semantic acceptance protocol before tuning the mapper; measured
performance remains deferred, not assumed. Stop here if a maintainable artifact,
reference-compatible adapter or plausible resource envelope is unavailable.
No production dependency or catalog entry is added merely to complete this probe.

#### 2. Define the source, pair and mapping contracts

**Tasks.** Extend the existing provider mapping seam to retain source tree
identities, parent relationships, style ranges and note/literal attachments.
Derive flattened formatting views from that source plan rather than maintaining
a second competing identity tree.
Separate translation piece metadata from alignment results. A bounded text pair
must identify its source block/segment, exact source/target strings and offsets
into their parent strings. An alignment result must bind to that pair and pinned
aligner/tokenizer/algorithm versions, with checked word/subword ranges and
many-to-many links. Projection produces source-owned nodes mapped to target
range lists or an explicit unresolved reason. Do not expose raw tensors through
Tauri DTOs or let generated HTML bypass validation.

**Tests/gate.** Pure tests cover exact source/target binding; unknown/duplicate
identities; missing links; invalid, empty and out-of-bound ranges; repeated words;
disjoint/nested emphasis; and legal reorderings. Distinguish valid nested ranges
from impossible crossing hierarchies. Verify byte/scalar conversions for emoji,
decomposed accents, Arabic combining marks, RTL text, whitespace and tokenizer
normalization. Arabic tests here prove coordinate safety, not translation quality.
The contract must represent all mandatory cases without inventing target words
or widening a span over unrelated prose.

#### 3. Add verified assets and a bounded local alignment runtime

**Tasks.** Extend the current installer/manifest with a distinct alignment asset
role, not an item in the translation-engine selector. Pin weights, tokenizer,
configuration, hashes, file sizes and notices. Reuse staged install/promotion,
storage accounting, errors and removal guards. Validate assets at runtime
admission, including replaced/corrupt files. The alignment feature must not
depend on enabling experimental search or native TTS. Check compatibility with
other users of the shared ONNX runtime before changing any dependency version.

Load at most one aligner session per foreground job, reuse it across batches,
and include the translation model in the peak-memory budget. Bound threads,
input tokens, tensors and queued work; serialize competing alignment work.
Coordinate asset leases for both models under existing state admission so
install/remove cannot race with use. Release resources on error or cancellation.

**Tests/gate.** Installer tests reject wrong hashes, missing tokenizer/config,
path escapes and interrupted installs without promoting partial data. Runtime
tests reject wrong shapes/types, oversized inputs, non-finite output and missing
runtime support. Lifecycle tests exercise two job starts, both model-removal
races, cancellation during load and repeated jobs without leaked leases.
Feature-on/off builds pass without changing search/TTS runtime behavior. A real
model load test must pass separately from mocked inference tests.

#### 4. Carry exact translation pieces through bounded planning

**Tasks.** Reuse the shared DOM's normalized text and stable block ordinals.
Retain CTranslate2's actual token-bounded source pieces and corresponding target
pieces before joining them; today the adapter discards those boundaries.
Preserve public segment IDs and record join separators in target offsets.
Check both runtimes' source-token limits before inference, including special
tokens, and check actual target-token limits after translation. An oversized
target fails explicitly; do not arbitrarily split it against unrelated source
pieces. Do not pair source/target sentences by
naively zipping sentence counts after translation, and do not truncate long
inputs or clip authored identities to satisfy a token limit.

Keep ordinary sentences whole when both runtimes permit it. Split oversized
prose at DOM-safe sentence/clause boundaries; assemble enclosing identities
only once after their pieces are mapped. If an indivisible unit cannot be
represented within the selected models' limits, report it as unsupported.
Literal islands and zero-width attachments retain explicit boundary ownership.

**Tests/gate.** Verify long blocks around the current 900-scalar planner boundary,
token-dense text, tokenizer subdivisions, leading/trailing whitespace, joined
punctuation, inline boundaries and a note exactly at a cut. Assert complete
source coverage, exact target reconstruction, stable segment IDs and one owner
per attachment. No lost/duplicated characters or changed sentence context merely
because a word is bold. Include over-limit rejection, not just successful splits.

#### 5. Implement alignment and conservative projection

**Tasks.** Run the selected aligner on each actual source/target piece after
translation. Convert model tokens to checked word ranges without losing the
relationship to either original string. Implement the reference-compatible
extraction/matching rule established in Stage 1; treat scores as evidence, not
calibrated probabilities. Project formatted source words to target ranges with
explicit handling of many-to-many, unaligned words and punctuation. Reject
conflicts or unsupported empty mappings; never replace them with proportional
positions. Do not expand a disjoint mapping into one broad hull. No inference
retries or model switching are part of this stage.

**Tests/gate.** Golden token/link tests cover subwords, contractions, repeated
tokens, merged/split words, phrase reversal, omitted source words and unrelated
targets. Require exact expected target scope on the mandatory synthetic markup
suite; every ambiguous/invalid case must fail safely. Run real French/Spanish
inference against reference links and manually annotated target scopes. A
test-only second provider supplying the same texts must produce the same
mapping, proving there is no hidden CTranslate2 dependency. Passing by rejecting
all formatted inputs is not success: mandatory positive cases must render.

#### 6. Reconstruct hierarchy, links and attachments through the shared renderer

**Tasks.** Feed validated mappings into source-owned protected reconstruction.
Allow repeated style-only wrappers for disjoint emphasis; retain source-owned
attributes and exactly-once identities. Parent/child containment must remain
representable; reject crossings instead of reparenting a link or cloning an ID.
Place notes from explicit lexical ownership and punctuation/quote scope; do not
reuse positional note estimates. Preserve immutable literals and owned assets,
including images inside styled links, tables and EPUB-relative references.
Route strict mapped output around all legacy best-effort formatting fallbacks.

**Tests/gate.** DOM assertions check the exact styled words, ancestor chains,
unique IDs, unchanged URLs/backreferences, code/literal content and asset bytes.
Cover nested/disjoint styles, compact ID wrappers, forward/reverse/nested quote
notes, duplicate labels, standalone images and mixed-content tables. Reject
injected tags/attributes, changed identities, bad attachment scopes and excessive
reconstruction expansion. Assert rendered readable text equals the expected
target plus source-owned literals/markers under the documented text contract.
An invalid mapping must yield no saved variant, never a degraded fallback.

#### 7. Integrate job lifecycle, resume and atomic saving

**Tasks.** Use the existing runner and cache: translate → align → validate the
whole batch → mark progress reusable. Add alignment/model/tokenizer/projection
versions and source-plan/target identities to cache compatibility. Reuse valid
translated text when only alignment changes, but never treat an old text cache
or legacy hint as a validated map. Store only bounded reconstruction metadata,
not tensors. Persist translated provenance with the existing variant metadata.

Check cancellation between native calls, after alignment and before the current
save-admission guard. Do not promise preemption inside an uninterruptible native
call; suppress its late result and release its resources when it returns.
Preserve the writer's current cancel-versus-save race semantics and rollback.

**Tests/gate.** Use the real HTML/EPUB → runner → cache → renderer → SQLite/file
writer path with deterministic test engines/aligners. Verify resume skips valid
work, an aligner/tokenizer/target change invalidates mapping, corrupt cache is
rejected, a partly failed batch is not marked complete, and retries create only
one derived variant. Exercise cancellation/load/inference/save races, storage
failure, host shutdown and model removal. Originals, older variants, audiobook
files and read-along metadata remain byte-stable; no orphaned assets or FTS rows.

#### 8. Expose truthful capabilities and useful progress/errors

**Tasks.** Reuse the current model setup and translation panel. Offer one
verified alignment pack for eligible native models; do not add an aligner picker
or hidden automatic download. Explain the separate download/storage requirement.
Only advertise strict formatting support when the provider, pair, aligner and
runtime gates pass; enforce the same rules in Rust for direct command calls.
Keep the new formatting capability gated off pending Stage 9 acceptance;
integration tests can supply ready capabilities without implying release support.
Report loading/alignment progress and actionable missing-model, unsupported-unit
and mapping failures without logging private content. Existing experimental
Bergamot protected output must not be aligned a second time or silently selected
as a fallback.

**Tests/gate.** Frontend/native tests cover absent, installing, corrupt,
unsupported and ready assets; engine/pair changes; download/removal failures;
cancellation and resume. Controls have stable accessible names, keyboard support
and localized labels/errors across the eight locales. Plain documents remain
usable according to their actual capability; formatted documents cannot bypass
strict admission through stale UI state or a cached legacy result. Verify the
worker-only mobile build does not accidentally claim native-aligner support.

#### 9. Qualify real output and platforms before calling the feature complete

**Tasks.** Record automated fidelity, independently reviewed semantic placement,
successful-input coverage and safe-rejection rates separately. Formatting scope
must be judged on actual engine output, not only prewritten translations.
Evaluate French and Spanish separately on held-out material; numbers, negation,
omissions and terminology also need review because alignment cannot fix them.
Use the budgets and protocol fixed in Stage 1; if they are unmet, keep capability
disabled rather than lowering the bar after seeing results.

**Tests/gate.** All mandatory synthetic positive cases have exact expected
markup/attachment scopes; all mandatory unsafe cases reject, with zero observed
identity/asset loss. Real-output acceptance records both errors and coverage,
not only HTML validity. Full relevant Rust/frontend/feature-matrix CI passes.
Packaged offline HTML and EPUB flows cover install, translate, resume, cancel,
restart, open/search, links/images, deletion and new TTS/highlighting. Measure
combined peak RAM and sustained latency/thermals on supported platforms. Pixel 7
(Android 17) is a known test device, not proof of low-end suitability; native
CTranslate2 mobile packaging and iOS acceptance remain separate prerequisites.
Arabic requires its own model, tokenizer, semantic and RTL acceptance before
exposure. A second real engine must pass the shared-contract integration gate
before claiming that engine has full formatting parity.

**Deferred now:** benchmarks, physical-device/broader smoke and release/version
work. They may be listed as pending while earlier implementation stages finish;
they cannot be counted as passed or omitted from eventual release acceptance.

### Delivery order and stop conditions

Next work is **Stage 1 only**, ending in a go/no-go decision with pinned candidate
and adapter scope. Then implement contracts → assets/runtime → exact planning →
alignment → reconstruction → lifecycle → UI, each with its own focused review
and evidence. Do not build the full integration around an unproven aligner.
Stop and revisit the design if it needs a maintained fork, training pipeline,
unacceptable resource use, silently misplaced formatting or mostly rejected
ordinary prose. This design moves the learned correspondence problem into a
pretrained model; Papercut still owns substantial offset, hierarchy and safety
engineering. It is not a guarantee of perfect semantic formatting.

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

The initial slice below is historical. The later
[protected rich-markup integration](#protected-rich-markup-integration-2026-10-19)
now wires the optional contract through the Bergamot adapter, cache and shared
renderer; OPUS-MT/HY-MT2 remain text-only.

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

##### Validation Slice Review — 2026-10-19

Reviewed commit `14440d4` and its connected adapter, runner, cache, source-model
and shared renderer paths. **No actionable findings in this slice.** The expected
load is local book translation using the existing 900-scalar segment and
16-segment batch defaults, with at most eight phrase probes per segment.

Confirmed that both batch paths reject mismatched or empty replies before
accepting returned text; probe ownership follows the validated reply order.
Mapping range checks precede indexing/slicing, known source-run identities are
used exactly once, and sorting rejects target overlaps before rendering. Source
DOM extraction supplies the formatting tag stacks. The renderer fixture keeps
HTML-looking target text inert and preserves original note/backlink targets.

The emphasis module's fixture-only status and narrow run representation are
intentional. They do not claim semantic alignment, protected-HTML restoration,
worker authentication or asynchronous lifecycle safety. Those requirements must
be implemented before a new provider can use the boundary in production.

Fresh validation repeated **392 passed, 4 existing ignored**, including
**115 translation tests**; the locked/offline CTranslate2 compile check, Rust
formatting and diff checks passed. The HY-MT2/Vulkan feature still needs CI
validation because the local headers are unavailable. No model inference,
installed-app or mobile/WebView validation was added by this review.

##### Bounded Bergamot Worker Bridge — Prototype (2026-10-19)

At this initial slice, the text-only bridge was **not selectable in the Translation
panel or used by `translation_start`**. It proved the bounded runtime/transport
boundary before verified asset delivery and the full-document integration
described below. The model picker still excludes Bergamot; HTML-aware engine
rendering is not enabled.
It does not add a second document writer or a benchmark kit.

Responsibilities:

- `translation::bergamot`: one active, main-WebView-owned worker attempt,
  single-slot reply mailbox and a `TranslationEngine` implementation. The backend
  reuses `TranslationState` job admission/cancellation and `validate_outputs`.
  The thin `translation_bergamot_batch` / `translation_bergamot_reply` commands
  pass text and control messages through a Tauri channel. The prototype returns
  validated text only; it never opens document files or writes cache/variant rows.
- `src/translation/bergamot/nativeBridge.ts`: starts the worker, transfers binary
  files, relays bounded requests/replies, handles AbortSignal, worker/message
  errors and pagehide/hidden-document events, and terminates/cleans up on every
  exit. It waits for Rust's command result rather than treating a worker message
  as durable completion. This helper is an internal prototype entry point, not
  an advertised app capability.
- `public/translation/bergamot/worker.js`: verifies binary size/SHA-256 pins,
  initializes the stock runtime and calls its public `BlockingService.translate`
  API with **HTML and alignment disabled**. Source echoes come from the request,
  not model-generated text. Per-call vectors/responses are released; the host
  terminates the worker to release model memory on completion or interruption.

The only vendored engine code is the **unmodified 100,187-byte Mozilla loader**
at [Firefox revision 48d55cf7ec80093903e2ef7f58b61a84a22ef716](https://github.com/mozilla-firefox/firefox/blob/48d55cf7ec80093903e2ef7f58b61a84a22ef716/toolkit/components/translations/bergamot-translator/bergamot-translator.js),
SHA-256 `faff1ef6285b0d26f01787776fd49299dfb756ecb9688aa990c250e66797b47d`.
Its MPL-2.0 license is included alongside it. The adapter follows the
[pinned Mozilla WASM test API](https://github.com/mozilla/translations/blob/69455acaecbe8650cdba988dbcf7c10ca20e7c48/inference/wasm/tests/engine/translations-engine.worker.mjs);
there are no engine/C++ patches or custom engine build scripts. A regression
checks the loader hash and all WASM/model pins against the retained evaluation
JSON. Only the worker, loader and license are allowlisted in Git; WASM and model
weights are not bundled or downloaded automatically.

| Boundary | Enforced limit/behavior |
| --- | --- |
| Work in flight | One Rust worker attempt and one reply mailbox; no unbounded request queue. |
| Text request | 1–16 segments, at most 900 Unicode scalars per segment; unique nonempty ids, at most 128 UTF-8 bytes each. Prototype pairs are fr→en and es→en only. |
| Binary startup | Four distinct, nonempty ArrayBuffers, at most 64 MiB total. Worker checks exact coherent WASM 3.0 and model/lexicon/vocab 2.0 size/hash pins before loading the runtime. |
| Reply | At most 256 KiB across ids, source and translated text; exact requested count/order/source and nonempty text. |
| Identity | Rust-generated 128-bit random attempt nonce, checked batch counter and invoking WebView ownership. Stale/duplicate/other-window replies cannot fill a new mailbox. |
| Deadline | Independent 120-second Rust and host deadlines include initialization and inference. Stale messages never renew them. Rust polls cancellation at most every 25 ms while waiting and checks it again before accepting a reply. |
| Failure | Timeout, cancellation, lost channel/worker, bad output and engine failure close the attempt and release its slot. Explicit worker errors/lifecycle events notify Rust; if a WebView disappears without notification, Rust's deadline still releases the attempt. |

The future installer must supply verified local binary buffers to
`runBergamotBatch(segments, files, signal)`. These are transferred to the worker
outside JSON/base64. Transfer consumes the buffers; a failed/cancelled call may
already have detached them, so reload buffers for a retry. The loader is a fixed
app-owned same-origin script, not an IPC URL.
No CSP relaxation, remote script origin, native filesystem permission or model
download was added. Existing WASM/worker CSP directives are retained. Secure
WebCrypto/worker/WASM support is required; unsupported runtimes fail closed.

Validation includes **7 Rust bridge regressions** (one real Tauri mock-runtime
command/channel round trip), **20 frontend regressions**, TypeScript/lint checks,
and the Vite asset build. Cases cover bounded inputs/assets, source binding,
other-window/old-attempt/old-batch/duplicate replies, cancellation winning a
result race, timeout/channel loss, cleanup, hidden/closed pages, late requests
after frontend cancellation, corrupt assets and generic errors without runtime
stacks. Full suites passed **399 Rust tests, 4 existing ignored** and
**393 frontend tests, 1 existing skipped**. CTranslate2-feature compilation also
passes; the existing HY-MT2/Vulkan-header CI gap is not waived.

The exact new worker also ran one actual French→English and one Spanish→English
request using the stock loader, verified cached WASM/model files and Node worker
threads with browser worker globals supplied by a temporary test harness. Both
returned nonempty English output with exact source/id binding. This is an API
integration check, **not a benchmark, quality comparison, installed-app smoke or
proof of Tauri/WebView/mobile packaging**. The harness and model files are not
checked in. Existing originals, translated variants, audio and highlighting are
unchanged; rich mappings are still fixture-only and no cache epoch changed.

The worker-boundary review and verified asset-delivery slice below are complete.
The delivery review's page-lifecycle finding is addressed below. The
prototype command is deliberately one bounded batch, not a way to bypass native
document preflight or persistence. Full-book worker reuse/resume, packaged
WebView/CSP behavior, cancellation before durable saving and platform support
need that integration. Only then enable validated inline/protected-content
restoration and rich-cache provenance. No default-model or mobile-support
decision is made here. Benchmarks, installed-app smoke and the app version bump
remain deferred.

##### Worker Bridge Review (2026-10-19)

Reviewed commit `e4df3bc` with Ponytail against the connected Tauri commands,
translation job admission/cancellation, shared output validator, frontend host,
worker, pinned loader, tests and current CSP. Assumed one local foreground
worker attempt, 1–16 segments of at most 900 scalars each and the documented
fr→en/es→en assets. **No actionable findings for this prototype; it can advance
to asset delivery.** This is not approval to enable full-document or mobile
Bergamot translation yet.

Checked main-WebView ownership, random attempt/batch binding, stale/duplicate
reply rejection, single-slot admission, independent deadlines, cancellation
before result acceptance, worker loss and terminal cleanup. Asset verification
precedes runtime initialization; only fixed app-owned code is loaded. Existing
document jobs still own their persistence, and the prototype has no source-file,
cache or variant write path. The one-batch frontend entry point and absence of
catalog/UI integration are deliberate limits of this stage.

Fresh review validation passed **399 Rust tests, 4 existing ignored**, including
the command/channel and lifecycle regressions, and **20 frontend bridge/worker
tests**. TypeScript, changed-file ESLint, Rust formatting, diff checks and the
locked/offline CTranslate2-feature compile check passed. The earlier full
frontend suite, Vite asset build and two real Node-worker inference checks are
implementation evidence, not fresh runs in this review. HY-MT2/Vulkan compilation
still needs CI; packaged WebView/CSP/mobile behavior was not checked. Benchmarks,
installed-app smoke and the app version bump remain deferred.

The review recommended reusing `model_store`, `model_install`
and capability reporting for the pinned runtime/model files. Deliver only
catalog-owned, bounded binary assets to the worker; do not add arbitrary path
reads, JSON/base64 model transport, broader filesystem scopes or a second
installer. Keep install status distinct from runtime readiness, and test corrupt,
missing and unsupported assets plus worker capability failures before advertising
document translation. Full-book reuse/resume and validated rich mapping follow
as separate stages.

##### Verified Asset Delivery And Capability Gates (2026-10-19)

The internal worker entry point can now load installed files through Rust.
It still does not enable Bergamot in the Translation panel, `translation_start`
or document persistence. The shared document capability and native model picker
remain unchanged.

- `translation::bergamot::assets` owns two pinned asset manifests:
  `bergamot-fr-en-worker` and `bergamot-es-en-worker`. Each contains WASM 3.0
  plus that pair's model/lexicon/vocabulary 2.0, using the exact Mozilla attachment
  URLs, sizes and SHA-256 values in the retained evaluation JSON. Each install
  owns its own WASM copy so installation/removal needs no shared-file registry.
- Existing `translation_install_model`, `translation_model_status` and
  `translation_remove_model` accept these internal ids. Downloads still require
  the existing `native-translation` Cargo feature and explicit installation;
  inference never downloads files. The existing installer stages and verifies
  complete sets. An explicit per-file attachment URL supplements the existing
  revision-based Hugging Face URLs; OPUS-MT/HY-MT2 pins do not change. Oversized
  responses now fail before writing bytes beyond the pinned size.
- Resolving an existing Bergamot install verifies all four files, so presence
  alone cannot make a corrupt install appear valid or skip reinstallation.
  Status reports **assets verified**, separately from worker readiness or
  document support. Status hashing and the install fast path run on blocking
  tasks rather than the UI thread.
- `translation_bergamot_asset` accepts only the invoking main WebView, `fr`/`es`
  and an enum naming `wasm`, `model`, `lex` or `vocab`. It accepts no path or URL.
  A translation job claim serializes asset reads; an exclusive model-file lease
  also blocks installation/removal in either ordering while a read is active.
  The read rejects symlinked roots/leaves,
  non-regular files, unexpected locations/sizes and checksum mismatches. It
  reads at most the pin plus one byte, with a 32 MiB per-file ceiling, and returns
  a raw Tauri binary response. Papercut does not JSON/base64-encode model bytes
  or widen filesystem/CSP scopes. The worker checks the resulting snapshot again.
- `src/translation/bergamot/assets.ts` exposes
  `runInstalledBergamotBatch(segments, sourceLanguage, signal)`. It validates text
  before reading files, serializes frontend admission, loads four ArrayBuffers
  sequentially under a 64 MiB combined ceiling and hands them to the existing
  bounded bridge. A 120-second absolute host budget includes asset loading and
  the worker batch. Cancellation/timeout releases frontend admission even if a
  binary IPC reply is lost; native bounded disk reads finish independently.
- Capability checks require a secure context, Worker, WebCrypto and WASM SIMD,
  using a minimal `v128.const` module rather than OS-version guesses. The actual
  downloaded WASM is validated before loading model files. Worker initialization
  remains the final runtime check; passing feature probes does not assert enough
  RAM, successful CSP loading or packaged-device support. The byte ceilings are
  transport limits, **not a bound on WASM/workspace or total app memory**.

New regressions cover pin/URL coherence, native-model URL preservation, invalid
selectors, missing/corrupt/truncated/oversized/non-regular/symlinked files, raw
response type, window ownership and read/mutation claims. Frontend tests cover
capability failures before reads, actual-WASM rejection, binary-only replies,
size limits, sequential delivery, cancellation, a lost-IPC deadline and retry.
The actual downloader is tested against tiny loopback HTTP responses for valid,
bad-hash and oversized bodies; no model download or inference is needed. CI now
runs that feature-enabled downloader regression with `provider_inference` off.

Validation: **404 Rust tests passed, 4 existing ignored**; the downloader-enabled
translation suite passed **128 tests**; **408 frontend tests passed, 1 existing
skipped**, including **35 bridge/worker/delivery tests**. TypeScript, changed-file
ESLint, the Vite asset build, Rust formatting/diff checks and the locked/offline
CTranslate2-feature compile check passed. CI YAML parsing also passed. This is
local regression evidence, not a new CI or installed-WebView acceptance result.
Header-only requests to all seven pinned Mozilla attachments returned HTTP 200
and the expected sizes; no model weights were downloaded for this check.

The delivery review found one page-lifecycle gap, fixed in the following slice.
The full-document slice below integrates worker lifetime, verified model identity,
resume and pre-save cancellation into the Rust runner. Protected rich mapping
and packaged desktop/mobile acceptance remain separate work. Benchmarks,
installed-app smoke and the app version bump remain deferred; no originals,
existing variants, audio, highlighting or cache epoch are migrated.

##### Asset Loading Lifecycle Hardening (2026-10-19)

Review of `cf2d9cd` found that hiding the page during a pending asset read missed
the worker's later lifecycle listeners. After the read completed, the old path
could finish loading and create a worker while the page was already hidden.

Both the installed-assets entry point and direct worker bridge now reject
already-hidden starts before allocating work. The installed entry point also
handles `pagehide` and hidden `visibilitychange` from before the first read
through worker completion, forwarding cancellation through its existing
AbortController. All listeners and timers are removed on every terminal path.
An already-running bounded native disk read may finish independently; its late
reply cannot trigger another read or create a worker after cancellation.

Ten new regressions exercise the actual asset loader and worker bridge together
with mocked IPC/Worker boundaries: hidden admission through either entry point,
both lifecycle events during the first and final pending reads, late replies,
successful retry, cancellation after worker handoff and listener cleanup on
success/failure. Six cases failed against the previous implementation before
the fix. Targeted tests now pass **45 cases**; full frontend validation passed
**418 tests, 1 existing skipped**. TypeScript, changed-file ESLint and diff checks
passed. Rust/installer code is unchanged and was not rerun for this frontend fix.

This closes review finding 1 for the bounded prototype. The following slice
implements full-document reuse/resume, cancellation before saving and text-cache
provenance; protected rich mapping remains separate work. Packaged desktop/mobile lifecycle behavior is not
claimed by these mocked-boundary regressions. Benchmarks, installed-app smoke
and the app version bump remain deferred.

##### Full-Document Worker Jobs, Resume And Save Admission (2026-10-19)

`startTranslationJob` routes the two internal `bergamot-{fr,es}-en-worker` IDs
through `runInstalledBergamotDocument` and the main-WebView-only
`translation_bergamot_document` command. The backend rejects mismatched models
and language pairs, non-balanced quality, repair, hardware acceleration and
glossary options that the adapter cannot honor. This is an internal job path,
not a new advertised model/default or a mobile-support claim.

The native and worker adapters now share the same runner: source provenance and
HTML/EPUB admission, DOM block planning, bounded phrase probes, progress,
segment memory, whole-document validation, asset ownership and atomic derived
upload/SQLite/FTS persistence. One model-file lease prevents same-model install
or removal while the job uses its verified snapshot. No second installer,
document writer, engine fork, dependency, schema migration or public cache epoch
was added. Stored metadata uses the actual engine ID instead of always saying
`ctranslate2`.

The worker initializes its stock runtime once per document attempt. Sequential
main/probe batches share its random nonce and strictly increasing request IDs;
only one request is outstanding. Phrase probes are split to the same 16-input
limit instead of sending up to 128 at once. Each Rust request and each valid
host request has an independent 120-second deadline; stale/duplicate replies
cannot extend one. Asset acquisition has a separate 120-second budget. A whole
book can therefore take longer than two minutes without being killed by a
single prototype-wide timer. Workers/listeners are cleaned up on every exit.

Only completely validated batches are checkpointed in the existing segment
cache. A new attempt reuses completed segments/probes and translates the rest;
there is no restored worker heap or concurrent batch replay. Bergamot caches
also bind to all four asset pins, the shipped stock JS loader, worker adapter
bytes and the `bergamot-text-v1` strategy. Missing/changed provider identities
invalidate worker caches; legacy native caches remain compatible. Stored
Bergamot metadata records that identity. Bump the strategy when introducing
alignment/protected-markup semantics; text caches must not imply rich mappings.

After rendering and quality checks, Rust requests an owner/nonce/sequence-bound
`confirm` acknowledgement **even for an entirely cached job**. A hidden,
aborted, closed or lost host cannot approve saving. Cancellation flags survive
batch completion and validation and are cleaned up when the job exits. A shared
lock arbitrates the final cancellation check and save admission. Before that
boundary, cancellation prevents all durable variant writes; after it, the
atomic writer finishes and cancellation returns an “already being saved” error.
The document host allows Rust's final command result to settle after a late
page/abort event rather than incorrectly reporting a saved book as cancelled.
The lifecycle hardening below bounds that wait when the final reply is lost.
Completed resume-cache batches may remain after cancellation.

Regression coverage uses the real planner, on-disk cache, DOM renderer and
SQLite/file writer with a deterministic worker transport: failure on a later
batch, retry of only unfinished work, engine provenance, all-cache owner loss,
cancellation during final validation/confirmation, preservation of an existing
variant/original files, 128 bounded phrase probes with cache reuse, and the
native adapter's default save checkpoint. Bridge tests reject wrong-owner,
stale and wrongly shaped confirmation replies. Frontend regressions use the
actual loader/bridge with fake IPC/Worker boundaries to verify one initialization,
multiple long batches, per-request deadlines, all-cache cancellation/hidden/pagehide,
late-save success, sequence rejection, cleanup and retry. These tests do not
measure translation quality, WASM memory or real WebView suspension.

Validation: **412 Rust tests passed, 4 existing ignored**; the downloader-enabled
translation suite passed **136 tests**; **427 frontend tests passed, 1 existing
skipped**, including **54 bridge/worker/delivery tests**. TypeScript, changed-file
ESLint, Rust formatting/diff checks, Vite asset bundling and the locked/offline
CTranslate2-feature compile check passed. The latter retains three existing
HY-MT2 dead-code warnings when llama.cpp is disabled; Vite retains its existing
large-chunk warning. These are local checks, not a CI or physical-device result.
The HY-MT2/Vulkan build still needs CI because local Vulkan headers are absent.

The following recovery slice addresses two frontend findings from this
integration's review. Validated protected rich mapping through the shared
provider contract is still required before exposing Bergamot in the catalog.
Packaged desktop/mobile acceptance, benchmarks, installed-app smoke and the app version bump remain
deferred. Original documents, existing variants and generated audio/highlighting
are not migrated.

##### Full-Document Lifecycle Recovery (2026-10-19)

Review of `87f6ebf` confirmed two P2 frontend gaps: a lost final command reply
left document admission busy indefinitely, and the manager's Cancel action
never reached the asset loader's cancellation signal. Both are now addressed.

After a worker timeout, page lifecycle interruption or abort, the document host
gives Rust **30 seconds** to return its known outcome. A successful save reply
within that grace period still wins, including after cancellation was refused
because the atomic writer already owned the commit. If no final reply arrives,
the host returns `translation-outcome-unknown`, releases its frontend admission
and removes its timers/listeners. The existing 120-second asset/request budgets
and support for books taking longer than two minutes remain unchanged. Stale
requests/results cannot extend the grace period or affect a new attempt.

An unknown outcome means the variant **may already have been saved**. The
manager refreshes the existing generated-document/Library list and keeps an
explicit instruction to check the Library before retrying; a refresh is not
proof that a still-running native job finished. Late progress cannot overwrite
that ended job's error/unknown message or a newer job's state. Rust retains its
own model/job claims and atomic-save guards, so releasing frontend admission
does not authorize a second writer or roll back a committed variant. A retry
may still be rejected while the original native operation owns those claims.

The manager owns one AbortController for its current Bergamot job and passes
its signal through `startTranslationJob` to the existing asset loader/worker
host. Cancel during a pending binary read releases frontend admission promptly;
that bounded native read may finish, but its late reply cannot trigger later
asset reads, create a worker or start a document job. Before a Rust document job
exists, no cancellation command is sent for a nonexistent job. After worker
handoff, the existing host forwards cancellation to Rust and observes the same
save cutoff/grace period. Native OPUS-MT/HY-MT2 jobs continue to use their existing
Rust cancellation command.

Nine added regressions cover missing final replies after timeout/abort/pagehide,
a known save reply just before the grace deadline, stale messages and retry
ownership, cancellation during the first/final asset read, manager signal
ownership/cleanup, native cancellation and Library refresh/late-progress
handling after an unknown outcome. They exercise the actual loader/bridge/API
and a mounted React manager with mocked native/Worker boundaries. The deadline,
manager-signal and late-progress checks failed before their respective fixes.

Validation: **436 frontend tests passed, 1 existing skipped**, including
**63 worker/delivery/manager cases**; changed-file ESLint and the complete build
(i18n check, TypeScript, Vite assets and Pagefind) passed. Vite retains its existing
large-chunk warning. Rust, native inference and packaging checks were not rerun
for these frontend-only fixes; the previous backend results above remain
historical evidence. No dependencies, stock engine/loader, model weights,
source-format scope, cache identity/epoch, stored variants, original documents,
audio/highlighting or app version changed.

The recovery review found no further issues. Its protected rich-mapping follow-up
is implemented below; catalog exposure and packaged desktop/mobile acceptance
remain pending. Benchmarks, installed-app smoke and the version bump stay deferred.

#### Protected Rich-Markup Integration (2026-10-19)

The shared engine contract now has an optional retained `ProtectedPlan` input
and protected-markup output. `supports_protected_markup` opts an adapter into
this path; it does not infer support from a model name. The Bergamot document
adapter opts in. Existing OPUS-MT/HY-MT2 adapters keep their text-only behavior,
conservative formatting and phrase probes. No provider registry, second writer,
new dependency, native engine patch or copied Bergamot HTML implementation was
added. Translation remains HTML/EPUB-only.

**Ownership and flow.** `inline_markup/protected.rs` derives bounded fragment
plans from the same normalized source DOM/scalar offsets used by the planner.
The runner extracts each source block once and retains only the current block's
scalar table, so long paragraphs are not rescanned for every bounded segment.
Rust retains the actual tags, attributes, URLs, ids and literal subtrees. The
worker receives only escaped prose and synthetic `<span data-pc="…">` codes;
code, `translate="no"` content, notes and image/media subtrees become opaque
`translate="no"` tokens, without sending their text, paths or targets. Whole
sentence context across inline tags is retained within each existing segment.
The stock worker uses the supported response options `html: true` and
`alignment: true` for these requests; the plain batch prototype remains text-only.
The vendored loader, runtime and model pins are unchanged. This call shape follows
Mozilla's [pinned engine wrapper](https://github.com/mozilla/translations/blob/69455acaecbe8650cdba988dbcf7c10ca20e7c48/inference/wasm/tests/engine/translations-engine.worker.mjs).

**Validation and reconstruction.** Reply owner/attempt/sequence and exact
transport-source checks still precede consumption. The adapter and shared runner
validate all rich results before caching any result from the batch. A strict
parser requires every retained identity under its original parent. Ordinary
formatting without an authored id/name may occur in several disjoint target
ranges: `<strong>El perro</strong> azul` can map to
`<strong>The</strong> blue <strong>dog</strong>`. This repeats formatting, not
the intervening word or the source content. Links, generic containers, authored
identities and opaque subtrees remain single-use. Siblings may reorder, but
crossing/reparented/missing/unknown codes, forbidden duplicates, changed opaque
tokens, unexpected tags/attributes/namespaces and malformed markup fail.
Zero-width notes/images use empty opaque spans instead of invented `PC…` words;
nonempty literal codes still require their exact placeholder. Provider HTML
never replaces the reader document. The renderer
rebuilds only source-owned shells and opaque subtrees, escapes translated text,
and revalidates fragment/source/text coverage against the cloned DOM. Existing
sanitization, independent asset ownership, indexing, quality checks and atomic
save/cancellation admission remain the final shared boundaries.

For example, `La bague en <strong>or</strong> est perdue.` can become
`The <strong>gold</strong> ring is lost.` through its source identity rather
than a proportional range or independent phrase guess. A link keeps its source
`href`/id while its label is translated; note markers retain their exact source
subtree. Zero-width notes at a segment boundary belong to the preceding fragment
and occur exactly once. Opaque literals retain source word-boundary spaces even
if the provider HTML alignment loses a separator.

**Resume and provenance.** Rich cache entries store the raw code response plus
an identity of the exact source plan, literals and `protected-codes-v6` strategy.
They are location-bound and revalidated on every read. Plain translation memory
cannot satisfy a rich request; missing/corrupt/mismatched mappings trigger fresh
inference. Provider identity is now `bergamot-protected-codes-v6:<hash>`, retaining
the existing exact asset/worker/loader hash binding. Old Bergamot text and v1/v2/v3/v4/v5
rich caches are incompatible; native text caches and alignment epoch **3** are
unchanged. Existing
completed variants, originals, generated audiobooks and highlighting are not
migrated or rewritten by this change. A failed new attempt keeps the previous
completed variant intact.

**Deliberate bounds and limits.** Main source segments remain at most 900 Unicode
scalars, with at most 16 per request. Each source block supports at most 128 codes
and depth 64; encoded input and output have a 32 KiB per-fragment ceiling, with
the existing 256 KiB aggregate transport reply cap. A protected literal or an
authored inline id/name that crosses a planned segment boundary fails safely before
saving instead of duplicating an id or translating half a literal. Formatting
without an authored id may span fragments. Provider XML must be well formed
(including escaped entities); unsupported output fails instead of being repaired
into a different identity tree. Broader structural/splitting support requires
its own tests, not a permissive fallback. Structural validation does not prove
semantic placement or translation quality, and actual stock-model rich output
has not been accepted on packaged devices in this stage.

**Spanish smoke follow-up (2026-10-19).** A user-built Linux AppImage failed with
`Changed, duplicated or reparented protected identity` on a 234-segment article.
The shipped worker and pinned Spanish model reproduced disjoint emphasis in
56 segments; blanket duplicate rejection was too strict. Replaying the remaining
results exposed a second issue: synthetic note text could move outside its
opaque span. V3 addressed these transport failures without relaxing single-use
marker, attribute or hierarchy checks. At that stage, all **234** corrected
stock-worker responses passed the structural source-plan validator. This was a
local correctness reproduction, not an installed-AppImage or quality benchmark.
The loader, WASM, model pins and native engines are unchanged. Rebuild and repeat
the packaged translation before marking that
acceptance complete; general smoke, benchmarks and the app version bump remain
deferred.

The same user's successful CTranslate2 translation still omitted some emphasis.
That adapter does not return word alignment: emphasis relies on unique target
phrase matches, with isolated phrase probes capped at 120 source characters.
Context-dependent wording, longer phrases and ambiguous repeats can remain plain
text. This is an existing limitation, not fixed by Bergamot's transport changes;
complete native-engine formatting needs supported alignment through the shared
provider contract, not proportional positioning or relaxed HTML validation.

Regression checks: **432 Rust library tests passed, 4 existing ignored**;
**156 worker-enabled translation tests passed**; **118 translation frontend and
shipping/CI tests passed**. New cases cover disjoint nested emphasis, the observed
Spanish punctuation split, empty zero-width codes, and rejection of duplicate
notes/links/authored identities, reparenting and injected attributes. The existing
HTML/EPUB runner test now carries split formatting through saving and a cache-only
resume, while preserving owned images, notes, the original and a previous valid
variant after failure. V2 worker resume entries invalidate under v3; native text
caches and completed documents/audio/highlighting are untouched. Rustfmt and diff
checks passed. No benchmark tooling or user article was added to the repository.

**Reconstruction and note-boundary hardening (2026-10-19).** V4 adds the two
follow-ups from the v3 review. One reconstruction buffer checks every append
before growth, across all nested elements. Its allowance is the retained source
shells/literals once plus six times the 32 KiB transport ceiling for worst-case
escaped text. Thus a large original literal can survive once, while repeated
formatting cannot multiply arbitrarily large source attributes. A 26 KB reply
that previously expanded an 8 KiB title into 8.2 MB of reader HTML now fails
before that expanded HTML is built or parsed.

The separate `inline_markup/protected/boundaries.rs` module owns known note
attachment. Strict XML and source identity/attribute checks run before DOM
movement; the result is checked again before source-owned HTML restoration.
Notes at a parent edge or a Unicode sentence boundary are placed at that target
boundary, preserving whether they precede or follow its terminal punctuation.
The module uses the same Unicode sentence segmentation library as the planner,
never relative text lengths or guessed word positions. It can split repeatable
formatting to retain the note's original parent, but cannot split a link,
literal or authored identity. Shared boundary markers retain source order.
Changed sentence counts, ambiguous split parents, incompatible punctuation and
unsafe identity splits fail before cache or document promotion. Mid-clause
references that do not identify a known boundary retain stock engine alignment;
this is not a guarantee of their semantic attachment or of emphasis quality.

At v4, the supplied article's recorded responses were replayed without more inference:
**230 of 234** pass the stronger checks, and the reported note-2 placement is
corrected. Four responses (`b18:s2`, `b33:s2`, `b40:s1`, `b57:s1`) fail because
their required terminal punctuation is missing or changed; at least the first
also truncates the source thought. Do not equate the earlier 234 structural
passes with accepted translation quality, or weaken the boundary checks to
force completion. A new packaged run still needs acceptance; investigating
these model-output/completeness failures led to the focused v5 follow-up below.

Regressions cover amplification across sibling/nested output, preservation of a
large source payload once, misplaced parent/sentence-end notes, Unicode/entities,
split formatting parents, source-order markers/images and rejection of unsafe
link splitting or ambiguous boundaries. The real HTML/EPUB job test repairs a
misplaced note through saving and cache-only resume; a bounded reconstruction
failure leaves the previous variant and cache unchanged. V4 invalidates only
older worker resume entries. Native caches, completed documents, originals and
audio/highlighting remain unchanged. No engine/loader/runtime/model changes.

Validation: **436 Rust library tests passed, 4 existing ignored**;
**160 worker-enabled translation tests passed**; **118 frontend translation and
shipping/CI tests passed**. Rustfmt and diff checks passed. The 234-response replay
above used cached model responses in a temporary local harness, removed afterward;
it did not run new inference, benchmark throughput or modify installed app data.
The rebuilt AppImage, mobile acceptance, broader smoke and version bump remain
unverified/deferred. These are the v4 results; the focused v5 follow-up below
records the newer targeted evidence without claiming packaged acceptance.

**Bergamot input planning and quote-aware notes (2026-10-19).** The four v4
failures were not equivalent: `b18:s2` and `b33:s2` omitted closing clauses;
`b40:s1` reached its ending but moved a period inside an English closing quote;
`b57:s1` both omitted quoted material and changed punctuation placement. Plain-text
probes reproduced the omissions before DOM reconstruction. Increasing the output
length factor or beam size did not resolve them. Changing `max-length-break`
also had no effect: the [pinned upstream WASM constructor](https://github.com/mozilla/translations/blob/1de4a085d3a7afb625c51a60aabb5ad298e4059f/inference/src/translator/text_processor.h#L61)
does not initialize its wrapping limit from that option. This merits an upstream
report, not a local C++ or generated-loader patch; it is not proof that this one
upstream defect caused every omission.

V5 selects Bergamot planning from the catalog-owned worker manifest in `job.rs`.
The shared scalar-range segmenter keeps complete sentences separate, caps their
parts at **320 Unicode scalars** (or a smaller caller limit), and prefers comma,
semicolon or colon boundaries followed by whitespace in oversized sentences.
Oversized clauses retain the existing word/scalar fallback. Every non-whitespace
source character remains covered in order with deterministic block/segment ids.
This is a conservative app-side policy informed by the reported reproduction,
not an established optimal threshold, token limit or translation-quality promise.
Native OPUS-MT/HY-MT planning and the worker transport's existing size ceilings
are unchanged. The engine, worker, generated loader, WASM and model pins are
unchanged too.

The existing `ProtectedSource::fragment` still owns all clipping, ancestor
retention, source attributes and asset/note identities. Serialized HTML is never
split with text rules. Large indivisible literals or authored identities that
would cross a smaller segment boundary fail safely instead of being cloned or
partially translated; this remains a supported-input limitation.

Known footnote boundaries now distinguish adjacent closing quotes from terminal
punctuation. For example, source `«phrase»[note].` may become target
`“phrase.”[note]`: staying outside the quoted material takes precedence over
remaining before a period that English moved inside the quote. An inside-quote
note remains inside. Quote-style/order changes are accepted only with the same
adjacent closing-quote count and non-quote punctuation sequence; missing periods,
changed question/exclamation marks, ambiguous sentence counts, identity splits
and the reconstruction bound still reject. Exact supported source suffixes keep
their prior placement. Mid-clause attachment still depends on engine alignment.

A nonempty source formatting identity must have some non-whitespace target
content across its disjoint copies. Individual empty split shells remain legal
when another copy carries content; an entirely empty mapping fails before cache
promotion. This catches the observed lost emphasis/quotation case without
pretending that all omissions can be detected structurally. Matching sentence
counts and valid markup are not semantic-completeness checks.

Targeted reproduction used the **unmodified pinned Spanish worker/model** on only
the four failing inputs after shared DOM fragmentation. All **14** resulting
responses passed the current Rust restoration checks, with the previously missing
closing clauses and quoted material present. Outputs still show wording,
capitalization and spelling problems (including the tiny authored split within
“doctrina”); this is not full-article or quality acceptance. The temporary private
harness and article are not repository fixtures. No speed/RAM comparison,
packaged application or broader smoke was run.

Regressions cover provider isolation, sentence/clause planning, scalar source
coverage and stable ids, Unicode/URLs/unpunctuated fallbacks, nested formatting,
indivisible identities, before/after/inside/outside-quote notes, missing/changed
punctuation and empty target mappings. The long HTML/EPUB job regression covers
owned assets and a misplaced quote note through real saving, cache-only resume
and cancellation before saving; the original and previous variant stay intact.
V5 mapping identity and the actual 320-scalar plan invalidate older worker resume
data. Native caches, completed variants, originals, audiobooks and highlighting
remain unchanged. Benchmarks, broader smoke and the app version bump stay deferred.

Validation for this slice: **442 Rust library tests passed, 4 existing ignored**;
**166 worker-enabled translation tests passed**; **118 frontend translation and
shipping/CI tests passed**. Rustfmt and diff checks passed. The private 14-response
probe was also checked against actual full-block planning and restoration; no
temporary test harness remains in production or committed tests. Latest-head CI
and rebuilt-AppImage acceptance remain follow-ups.

**Compact authored IDs and reverse quote order (2026-10-19).** V6 fixes the
follow-up review cases in the shared paths. A normalized block already within
its caller/provider scalar cap stays whole, even if it contains several
sentences. Thus `<span id="authored">Bonjour. Merci.</span>` no longer splits a
15-scalar identity into two invalid fragments. The 320 cap and sentence/clause
policy still apply to oversized Bergamot blocks; native planning is unchanged.
Planning of larger blocks remains text-based: an indivisible identity crossing a
required cut still fails safely, even if that element itself is short. The guard
is not weakened to clone IDs; supporting those crossings requires protected-range
cut selection in a later stage.

Quote containment now takes precedence in both punctuation directions. Source
`«Bonjour.[note]»` with target `“Hello”.` becomes `“Hello[note]”.`, rather than
moving the note outside. A marker between nested closing quotes also stays
between them. The quote-order fallback still requires matching punctuation
inventories, so it changes attachment without removing, inventing or accepting
changed terminal punctuation.
The source relationship to closing quotes wins when preserving both that
relationship and before/after-period placement is impossible.

Regressions cover short multi-sentence span/strong/link identities, reverse
inside-quote placement, nested and between-quote notes. A real HTML/EPUB job
combines a short authored ID and the reversed period/quote order through the
bridge, shared renderer, file/SQLite writer and cache-only resume. It asserts
unique IDs, intact note targets, stable saved HTML, no second inference and an
unchanged original. V6 mapping identity invalidates older worker resume results;
native caches and completed documents/audio/highlighting are unchanged.

No engine, worker, loader, WASM, model pins, dependencies or app version changed.
The prior 14-response stock-engine reproduction remains historical evidence;
this follow-up used deterministic regressions and a narrow local code probe,
with no new model inference. Latest-head CI and rebuilt-AppImage acceptance remain
pending. Benchmarks, broader smoke and the version bump remain deferred.

Validation: **445 Rust library tests passed, 4 existing ignored**;
**169 worker-enabled translation tests passed**; **118 frontend translation and
shipping/CI tests passed**. Rustfmt and diff checks passed. The short-ID and
reverse-quote unit regressions failed before the fixes and pass afterward.

**Formatting plan superseded.** The later decision selects a separate local
aligner rather than translation-engine attention or a best-effort formatting
capability. Follow the [engine-independent alignment plan](#engine-independent-offline-alignment-plan--2026-10-19)
for current architecture, tasks, safety rules and stage gates. The existing
shared DOM/cache/renderer/writer remains the foundation; neither prompts nor
generated HTML alone establish semantic formatting fidelity.

Regression coverage includes reordered/repeated/nested formatting, French and
Unicode/Arabic combining-mark/emoji text, literal/code/image/note preservation,
namespace/attribute/tag/code injection, missing/duplicate/reparented identities,
strict bounds, fragment clipping and a note exactly at a segment boundary. Actual
Rust worker-mailbox tests verify that source literals do not cross IPC. The real
runner/cache/SQLite/file writer is exercised for HTML and EPUB source snapshots:
a malformed later result caches none of the batch, valid markup and owned image
bytes survive saving, a second attempt resumes without inference, and invalid
replacement leaves the previous variant and original document intact. Worker and
host tests verify exact opaque payloads and the HTML API/transport limits without
claiming model inference.

Validation: **424 Rust library tests passed, 4 existing ignored**;
**148 translation tests passed with `native-translation` enabled**, including the
real loopback downloader regression; **439 frontend tests passed, 1 existing
skipped** (63 worker/delivery cases). The complete production build (i18n,
TypeScript, Vite, Pagefind), changed-file ESLint, Rustfmt/diff checks and a locked,
offline CTranslate2 feature check passed. Vite retains its existing large-chunk
warning; the CT2 check retains three existing unused HY-MT2 helper warnings.
Sandboxed full-suite attempts initially hit existing loopback/Chromium permission
restrictions; permitted reruns passed. No model inference, new benchmark, installed-app smoke or app version change was
performed. HY-MT2/Vulkan packaging and packaged WebView behavior remain unverified.
##### Hierarchy Review Fixes (2026-10-19)

The review found two P2 failures on valid source HTML: whitespace normalization
could move a formatting parent's start past a leading note/image/line break, and
fragment selection could retain a boundary note while omitting its parent when
the parent started exactly at the 900-scalar cut. Both failed safely before a
variant was saved, but could stop a complete book job.

The shared protected planner now restores parent containment after normalization
and retains each selected node's full ancestor chain before clipping. Source
identities are assigned in preorder, so a bounded reverse pass handles nested
grandparents as well as immediate parents. Anonymous formatting wrappers may be
empty in the preceding fragment to carry its boundary child; the child still
occurs exactly once. Existing indivisible id/name/literal checks, strict reply
validation, source ownership, transport bounds and atomic persistence remain in
force. No permissive fallback or second rendering path was added. Function-level
comments explain the empty-wrapper and ancestor-selection behavior.

Three new regressions cover normalized containment and round trips with nested
notes/images/line breaks; the actual 900-scalar segmenter, translated fragments
and shared renderer together (ASCII and non-ASCII byte offsets); and rejection
of id/name ancestors or literals that would be split. The boundary cases verify
one restored child under all original formatting ancestors and keep unknown or
reparented output rejected. Both positive regressions failed on the reviewed
implementation before the fixes. The source-owned line break intentionally
contributes a normalized space, so its second segment begins at scalar 901
while the first ends at 900. The review's independent 48-case source matrix now
passes completely, including the original two reproductions.

Mapping strategy **v2** deliberately invalidates the initial v1 rich resume
entries; a compatibility regression covers that change. The app version,
native text-cache epoch, completed variants, originals, generated audio and
highlighting are unchanged. New worker provenance records v2 only for new jobs.

Validation: **427 Rust library tests passed, 4 existing ignored** and
**151 translation tests passed with `native-translation` enabled**, including the
real loopback downloader fixture. The locked/offline CTranslate2 feature check,
Rustfmt and diff checks passed; CT2 retains three existing unused HY-MT2 helper
warnings. Frontend tests/build were not repeated for this Rust-only fix; their
439-test/build results above are historical implementation evidence. No model
inference, packaged-device test, benchmark or app version change was performed.
The subsequent hierarchy review found no actionable remaining issue. The next
[catalog/UI slice](#capability-gated-bergamot-catalog-and-ui-2026-10-19) is implemented below. Benchmarks, installed-app smoke, real model/device
acceptance and the app version bump remain deferred; no default-model or
mobile-support claim is made.

##### Capability-Gated Bergamot Catalog And UI (2026-10-19)

`bergamot-fr-en-worker` and `bergamot-es-en-worker` now appear as experimental
French/Spanish → English choices. They reuse the exact existing pinned four-file
manifests, verified installer/status/removal, raw binary asset delivery and full
worker document bridge. No runtime/model/dependency pins, cache strategy, app
version, platform deployment target or default model changed. The canonical
stored engine id stays `bergamot`, preserving prototype provenance; catalog
resolution replaces the old out-of-catalog fallback.

Availability has two owners:

- Rust reports per-model `available` from its compiled engine feature and pinned
  manifest state. `native-translation` enables the Bergamot host/download path
  without requiring CTranslate2 or llama.cpp; each native model still requires
  its own compiled engine. Qwen stays candidate-only even with llama.cpp enabled.
- The frontend narrows worker availability using the existing secure-context,
  Worker, WebCrypto/randomUUID and real WebAssembly SIMD probe. An unsupported
  WebView disables only Bergamot choices, supplies a localized explanation and
  leaves compiled native engines usable. Native feature gates cannot be
  overridden by a successful browser probe. The full runtime module and asset
  integrity checks still run at job start; a feature probe does not prove that
  CSP, memory pressure or packaged-device inference will succeed.

Desktop translation build scripts already enable the base feature transitively.
Standard Android/iOS builds still omit `native-translation` by default; their
rows explain the missing engine. The subsequent [mobile opt-in slice](#worker-only-mobile-build-and-ci-opt-in-2026-10-19) adds an explicit build/CI switch without enabling native
CTranslate2/llama.cpp or claiming packaged acceptance.
The existing no-inference CI translation step now runs the complete
`native-translation` suite, covering the true build gate as well as downloads;
the ordinary Rust suite also covers disabled gates.

The picker retains native ordering and falls back only to runnable models. A
worker-only build selects the first compatible Bergamot entry. Unavailable
choices cannot be selected or downloaded from the UI; existing files can still
be removed and saved variants can still be viewed. Model counts do not label
unavailable downloads as installable. Run requires installed files and an
HTML/EPUB source; missing installation explains the disabled action and never
starts an automatic download. Fixed source/target selectors and the existing
balanced request contract do not expose unsupported glossary, repair or hardware
options for Bergamot. The eight locales include foreground/experimental guidance
and unavailable-engine/WebView explanations.

`startTranslationJob` continues to dispatch worker ids through
`runInstalledBergamotDocument`, with the manager owning cancellation, job progress,
Library refresh and unknown-save-outcome handling. The native-only command now
rejects worker catalog models before file access; the worker document runner also
rejects builds without `native-translation`. Source validation, strict rich reply
checks, v2 rich cache binding, save admission and atomic persistence are unchanged.
Original documents, completed variants, audio and read-along data are not migrated.

Regression coverage includes real component selection/install/run handlers for
both source formats, fixed language pairs, worker-only selection, absent files,
unsupported WebViews, stale selection fallback, removal while unavailable, and
opening a saved variant with no runnable engine. API tests use the real feature
probe, preserve native dispatch and verify worker document/signal routing. Rust
checks each compiled engine gate, candidate exclusion, unchanged default ordering,
exact retained asset pins, native-command rejection, disabled worker entry and
unchanged `bergamot` writer provenance. No tests claim model quality or device RAM.

Validation: **429 Rust library tests passed, 4 existing ignored**;
**153 translation tests passed with `native-translation` enabled**, including
the actual loopback downloader. **455 frontend tests passed, 1 existing skipped**
(82 translation tests, including 16 new API/component cases). The complete
production build (eight-locale validation, TypeScript, Vite, Pagefind),
changed-file ESLint, Rustfmt/diff checks and locked/offline CTranslate2 feature
check passed. Vite retains its large-chunk warning; CT2 retains three existing
unused HY-MT2 helper warnings. These are automated checks, not a packaged-device
or inference benchmark. No new dependency, model download, app version change or
installed-app smoke was involved.

Manual acceptance remains pending (not run in this stage):

1. In a translation-enabled packaged build, install either experimental pair
   explicitly, disconnect the network and translate an HTML and an EPUB containing
   nested emphasis, links, notes and owned images. Check source order, rendering,
   search/indexing, Library replacement, and TTS/read-along on the new variant.
2. Cancel during asset loading and during inference; retry the same document and
   verify completed work resumes without duplicate variants. Switch away from the
   foreground WebView and verify safe interruption/resume, not background success.
3. On an unsupported WebView, confirm both worker choices explain why they are
   unavailable, no worker download/run starts, compiled native models still work,
   and old variants/model removal remain usable.
4. Compare a worker-only `native-translation` build and a translation-disabled
   build: only the former may offer runnable worker choices. Repeat on packaged
   desktop/Android/iOS before claiming platform support.

The catalog/UI review found no blocker and one dead-branch cleanup, addressed
in the mobile opt-in slice below. Packaged acceptance, blinded quality/RAM/speed
comparisons, default selection, installed-app smoke and the version bump remain
deferred.

##### Worker-Only Mobile Build And CI Opt-In (2026-10-19)

The existing Android/iOS entry scripts accept `--translation-worker` and add only
Cargo's base `native-translation` feature. They retain the previous build defaults,
custom target/features/runner arguments and native-TTS/search setup. Wrapper
flags stop at `--`, so runner tokens cannot silently enable translation or be
consumed as wrapper flags. No native
CT2/llama translation runtime, new dependency, model weights, deployment-target
change or automatic download is introduced. The iOS device-link helper accepts
the same flag so simulator and generic iPhoneOS validation agree. Using this
build-only option during project initialization fails before SDK work.

Use the existing commands; no extra build script is needed:

```sh
# Linux/macOS/Windows with Android SDK/NDK: shipping TTS/search plus worker host.
npm run android:apk:native-tts -- --translation-worker

# macOS/Xcode: unsigned arm64 simulator archive and generic iPhoneOS link check.
npm run ios:ci -- --native-tts --translation-worker
npm run ios:ci:device -- --translation-worker
```

For a worker-only Android build without preparing native TTS libraries, explicit
custom Tauri arguments still work:

```sh
npm run android:apk -- --translation-worker android build --apk --debug --target aarch64
```

In **CI → Run workflow**, select the current branch, `validation_scope: mobile`
(or `full`), **`mobile_translation: true`**, and **`provider_inference: false`**.
The new boolean defaults off, is independent of provider inference and rejects
non-mobile scopes instead of silently ignoring an enabled checkbox. It forwards
the flag to APK packaging and both iOS link checks. The opt-in APK artifact is
`papercut-android-native-tts-translation-worker-apk`; default APK naming and the
release workflow remain unchanged. Do not enable provider inference merely to
compile translation: that separate option downloads search-provider test models
and runs their inference checks, not a Bergamot quality test.

These builds only make the host available. Runtime secure-context/worker/
WebCrypto/SIMD checks still gate the model picker; actual WASM integrity/module
checks and protected-markup validation still guard jobs. Install the chosen
French/Spanish model explicitly before offline use. No CPU/GPU/mobile quality,
peak-RAM or background-execution claim follows from compiling the host.

The catalog review's obsolete native-description branches were removed: when
the base host feature is absent, both native engine features are necessarily
absent too. The unavailable description retains the same limits and wording.

Regression coverage executes the real build entrypoints with only their native
command/download boundaries mocked, and executes the actual CI Bash blocks with
an npm stub. It covers unchanged defaults, standalone/combined worker opt-ins,
custom features/targets/runner arguments, invalid initialization, default-off
release isolation, independent inference toggles, scope validation and feature
propagation to Android/iOS simulator/device commands. The active worker-only
Cargo graph excludes `ct2rs`, `llama-cpp-2` and `llama-cpp-sys-2`.

Validation: **429 Rust library tests passed, 4 existing ignored**;
**153 worker-enabled translation tests passed**, including the real loopback
downloader; **481 frontend/script tests passed, 1 existing skipped**. The
36 focused shipping/CI cases include 26 new opt-in/scope/runner-boundary cases.
TypeScript, eight-locale validation, changed Node scripts' ESLint recommended
rules, Node syntax checks, CI YAML parsing, Rustfmt and diff checks passed.
The worker-only active Cargo dependency graph was checked without native engine
features. Existing build/CT2 checks from the preceding catalog stage are historical;
this build-script slice does not claim a new Vite or mobile bundle result.

Actual Android target/NDK compilation and macOS/Xcode linking have not been run
here, nor have packaged acceptance, model inference, benchmarks or installed-app
smoke. Run the opt-in CI build before claiming a cross-platform compile/link
result. Next: focused review, then that CI build; smoke, benchmarks and version
bump stay deferred.

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

This list records the earlier Bergamot integration sequence. Its alignment/API
recommendation is superseded by the [engine-independent alignment plan](#engine-independent-offline-alignment-plan--2026-10-19).
Further Bergamot upstream work is shelved; its historical tests do not establish
native-provider formatting parity or mobile acceptance.

1. The shared DOM/asset foundation and hardening review are complete. The
   [provider validation slice](#provider-mapping-contract-and-validation-slice-2026-10-19)
   and protected-code hierarchy fixes have passed focused review. The experimental
   capability-gated catalog/UI slice is implemented; review its admission and
   selection flow passed focused review; worker-only mobile build/CI opt-in is
   now implemented. Review the build slice and run opt-in CI next. Do not
   reintroduce positional emphasis guesses.
   OPUS-MT/HY-MT2 remain text-only.
2. The bounded worker, verified delivery, lifecycle fix and full-document
   reuse/resume/save-admission slice are implemented and reviewed above.
   Packaged WebView suspension and memory behavior still need device acceptance,
   despite experimental catalog exposure. No native FFI or copied
   Bergamot HTML C++ is required.
3. Bergamot now normalizes protected markup into the shared plan. Further
   providers need annotated source/target spans or the same validated codes.
   `HTML::restore` needs token annotations
   and alignments; it is not a supported engine-independent HTML-only package.
   `ct2rs` 0.9.19 does not return attention through its FFI/result type even
   though it exposes the request option. Use an upstream-supported binding/API
   or a separately evaluated aligner if OPUS semantic inline mapping is retained.
   LLMs remain optional future providers, not trusted raw-HTML generators.
4. Asset ownership and actual-DOM indexing are now implemented and regression
   tested. Installed-app validation still needs complex EPUB images, semantic
   search after replacement, reader navigation and TTS/highlighting on new
   translated variants. Existing generated audio is not migrated.
5. When benchmarks resume, run a larger **blinded bilingual** French/Spanish book/technical corpus on
   both candidates, including idioms, numbers, omissions and semantic emphasis.
   Benchmark the actual packaged adapter on the weakest supported Android
   device and WKWebView, including cancellation, restart and sustained load.
   Only then select defaults. Test Arabic RTL/combining marks/reordering as a
   separate extension, not an inferred success from fr/es results.

Version cache/alignment semantics when mapping changes; preserve existing
variants, original documents and already generated audio/highlighting.
Bergamot now has experimental capability-gated catalog entries and the shared
document integration. This does not change defaults or establish packaged-device
acceptance. No app version bump or installed-app smoke result is claimed.

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
