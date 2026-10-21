# Offline Translation Roadmap And Status

Papercut now has an in-progress offline translation pipeline. This branch’s desktop build configuration enables CTranslate2 and llama.cpp translation features so pinned OPUS-MT and HY-MT2 models use the same model-download, job, cache, validation, and storage pipeline. Linux builds include optional Vulkan acceleration for HY-MT2; Android and other GPU backends remain roadmap work.

The goal is high-quality offline translation for long-form HTML and EPUB books, primarily into English, while keeping the app responsive on desktop and mobile. The feature should feel like audiobook saving: the user starts a long-running job, the backend performs bounded native work, progress is visible, results are cached, and the finished output becomes durable user data.

The latest [Bergamot/OPUS-MT evaluation](#bergamot-versus-opus-mt-evaluation--2026-10-18) records actual desktop inference, markup failures and conditional mobile feasibility. It does not approve a production engine replacement.

## Current Implementation Status

- **Current CTranslate2 path:** [boundary-based translation](#boundary-based-ctranslate2-translation--2026-10-20)
  retains source formatting by translating fragments separately, with an explicit
  cross-fragment fluency tradeoff. It reuses the shared document pipeline and
  does not load an aligner. The strict candidate remains disabled; the Stage 9
  investigations below do not block this separately approved boundary policy.

- Translation accepts **HTML and EPUB only**, enforced in both the UI and native source loader. PDF and other formats remain unsupported.

- Desktop builds on this branch enable `native-tts-shared,experimental-search-embeddings,native-translation-ctranslate2,native-translation-llama` through `npm run desktop`.
- `npm run desktop:no-translation` keeps shared TTS and conceptual search while disabling translation for packaging/debug isolation.
- Mobile build scripts now accept explicit `--translation-worker` opt-in; CI exposes a default-off `mobile_translation` input for `mobile`/`full` scope. This enables only the experimental Bergamot host, not native CTranslate2/llama.cpp; ordinary PR/release builds remain unchanged. See [mobile build opt-in](#worker-only-mobile-build-and-ci-opt-in-2026-10-19).
- Spanish -> English and French -> English OPUS-MT CTranslate2 manifests are pinned and installable.
- HY-MT2 1.8B Q8 is a pinned, installable quality model for Arabic, German, Spanish, French, Russian, and Chinese translation into English. Desktop CPU inference remains the default. On Linux, the workbench offers Vulkan acceleration only when llama.cpp reports a real integrated or discrete Vulkan GPU.
- Installed models can be removed independently to reclaim storage. Removal is serialized against model installs and active translation jobs, while translated documents and reusable segment caches remain available.
- The workbench selects only pinned models whose engine and WebView gates pass. Unavailable pinned models remain visible with an explanation; future model candidates remain informational. Existing native catalog ordering/defaults are unchanged.
- Translation jobs run through the shared Rust engine boundary, emit progress/cancel events, reuse segment cache entries, run first-pass quality gates, and persist successful output as derived uploaded documents.
- The [Stage 2 alignment contract](#stage-2-pure-alignment-contract--2026-10-20)
  has passed its [contract-only review](#stage-2-contract-review--2026-10-20)
  and pure validation/rejection tests. The internal stages below now consume
  these checks; public alignment admission remains disabled and formatting acceptance open.
- The [Stage 3 asset/tokenizer slice](#stage-3-asset-and-raw-tokenizer-admission--2026-10-20)
  verifies local candidate files and connects real raw-tokenizer evidence to the
  contract behind a default-off internal feature. Its [admission review](#stage-3-admission-review--2026-10-20)
  found no must-fix issue within this internal scope. That original slice added
  no downloads, model selector entry, inference or saved-document migration.
- A separate default-off [bounded ONNX session](#stage-3-bounded-onnx-session--2026-10-20)
  now loads verified graph bytes and returns checked CPU embeddings. Public
  translation starts do not call it; internal extraction/projection is recorded in Stage 5 below;
  its [session review](#stage-3-bounded-session-review--2026-10-20) found
  no must-fix issue within that internal scope. The candidate remains unavailable.
- The [redistribution/installer evaluation](#stage-3-redistribution-and-installer-admission-evaluation--2026-10-20)
  found a published Apache-2.0 permission path. [Notice payloads and shared
  installer hardening](#stage-3-notice-payloads-and-shared-installer-hardening--2026-10-20)
  are implemented. [Trusted auxiliary admission](#stage-3-auxiliary-manifest-and-notice-delivery--2026-10-20)
  now supports explicit install/status/remove behind the existing default-off
  alignment feature. Shipping flags and translation engine choices are unchanged;
  installing the auxiliary pack does not enable document alignment.
- The internal [two-model job lifecycle](#stage-3-two-model-job-lifecycle--2026-10-20)
  now holds translator/aligner ownership through the shared runner, bridges job
  cancellation to native alignment and retains the existing atomic save cutoff.
  Its alignment load hook remains internal and unused by public starts;
  [Stage 7](#stage-7-mapped-cache-resume-and-atomic-save--2026-10-20) connects
  explicitly loaded sessions to the shared runner/cache/writer.
- [Stage 4 exact-piece/literal-island planning](#stage-4-exact-pieces-and-literal-islands--2026-10-20)
  now retains DOM-owned identities, exact source/target ranges, whitespace joins
  and attachment ownership. Its internal CTranslate2 path admits both source
  tokenizers before inference and rejects target overflow/decoder cutoffs.
  Public starts do not opt into the strict path; internal cache/renderer
  integration is recorded in Stage 7. Public formatting acceptance remains open.
- [Stage 5 word links and conservative projection](#stage-5-word-links-and-conservative-projection--2026-10-20)
  now runs the fixed extraction rule over checked raw-tokenizer evidence and
  projects exact pieces against the retained source hierarchy. Missing links,
  conflicting ownership and declared scope blockers remain explicit rejections.
  The mandatory positive scope and runtime/quality/resource gates are unmet;
  the candidate remains disabled.
- [Stage 6 source-owned reconstruction](#stage-6-source-owned-mapped-reconstruction--2026-10-20)
  now converts checked scopes and lexical/punctuation attachment evidence into
  the existing protected renderer. This internal path preserves hierarchy,
  literal islands and source-owned assets; ambiguous attachment scopes reject.
  The [parent-boundary follow-up](#stage-6-leading-attachment-parent-boundaries--2026-10-20)
  preserves leading notes/images across equivalent whitespace positions and has
  passed its [focused review](#stage-6-parent-boundary-review--2026-10-20).
  Internal mapped cache/resume/save integration is recorded in Stage 7;
  public admission and packaged acceptance remain pending.
- [Stage 7 mapped-cache, resume and atomic saving](#stage-7-mapped-cache-resume-and-atomic-save--2026-10-20)
  now reuses the shared batch checkpoints, source-owned renderer and writer.
  Strict receipts and variant identities remain separate from legacy hints and
  saved results. Its [focused lifecycle review](#stage-7-lifecycle-review--2026-10-20)
  found no actionable issue within this internal scope. The candidate is still
  disabled for public starts.
- [Stage 8 capability/setup/progress UI](#stage-8-gated-formatting-setup-and-progress--2026-10-20)
  separates the auxiliary formatting pack from translation engines, adds explicit
  strict consent and localized progress/errors, and rejects forged native opt-in
  requests. The public acceptance constant remains **false**; ready UI fixtures
  and successful internal inference are not release acceptance. The
  [recovery follow-up](#stage-8-download-retry-and-terminal-progress-recovery--2026-10-20)
  fixes failed-download retry and terminal loading progress in the shared UI.
- The [Stage 9 semantic investigation](#stage-9-auxiliary-word-and-missing-link-investigation--2026-10-20)
  reproduces missing auxiliary/contraction and adjective links on actual OPUS-MT
  output. Neither FP32 nor the older diagnostic runtime resolves the real-output
  failures. No production repair or public enablement is justified by these checks.
- The [fine-tuned provider evaluation](#stage-9-fine-tuned-provider-evaluation--2026-10-20)
  tests an alternative Awesome-align checkpoint against the frozen cases. It
  repairs the Spanish adjective and fixed French future but leaves important
  actual-output gaps. No provider replacement is approved.
- [OmniAlign loading/export feasibility](#stage-9-omnialign-offline-loading-export-and-pack-feasibility--2026-10-20)
  now demonstrates a provisional 151.6 MiB four-bit pack and native CPU execution
  on the unchanged Linux runtime. Formatting quality, quantization effects and
  other platforms remain unqualified; the public candidate stays disabled.
- The [ownership-boundary evaluation](#stage-9-ownership-boundary-evaluation--2026-10-20)
  finds that a separate English clitic boundary view can represent all 52 frozen
  positive scopes without changing model tokens. Boundaries cannot establish
  semantic ownership. The subsequent [FP32/four-bit link evaluation](#stage-9-omnialign-fp32four-bit-link-evaluation--2026-10-20)
  fails mandatory positive scopes and accepts both fused negatives under the
  tested raw-input policy. **Do not integrate this OmniAlign candidate.**
- The [alignment strategy reassessment](#stage-9-alignment-strategy-reassessment--2026-10-20)
  recommends retaining the shared DOM contract while qualifying evidence per
  provider. A separate word aligner alone has not met auxiliary/fused ownership
  requirements. Native CTranslate2 evidence is a bounded research option, with
  binding and semantic gates still open; no architecture switch is implemented.
- The [official CTranslate2 access probe](#stage-9-official-ctranslate2-tokenattention-access--2026-10-20)
  confirms real token/attention access and exact frozen-target prefix evaluation
  with unmodified public APIs. Native Rust token parity passes, but its result
  still omits attention. Ownership quality and production delivery remain open.
- The [frozen attention ownership policy](#stage-9-frozen-ctranslate2-attention-ownership-policy--2026-10-20)
  fails: only **4/52** mandatory positive scopes are exact and admitted, despite
  rejecting both fused negatives. It also admits a wrong negation scope and an
  extra-punctuation scope. No native implementation or threshold tuning follows.
- The [pretrained alignment-aware model screen](#stage-9-pretrained-alignment-aware-translation-screen--2026-10-20)
  establishes no replacement. The original French model is already guided and
  its trained last-layer setting matches the existing pack. No new checkpoint
  or ownership signal is qualified; strict implementation remains blocked.
  The separately approved boundary policy does not depend on this research.
- Bergamot French/Spanish → English are now **experimental, capability-gated catalog choices**. The document path reuses one foreground worker, resumes completed batches, validates protected markup and confirms a live host before the shared Rust atomic writer saves. Explicit verified installation is required. Catalog availability is not packaged-device or translation-quality acceptance; see [catalog integration](#capability-gated-bergamot-catalog-and-ui-2026-10-19).
- Bergamot document planning keeps blocks that fit the 320-scalar cap intact, then uses sentence/clause boundaries for oversized blocks. This is an app-side mitigation for long-input omissions, not a model-token budget or a completeness guarantee; this worker mitigation is separate from the native boundary policy.
- Fixed-pair OPUS-MT models use their actual source language; the UI and backend no longer imply automatic language detection.
- OPUS-MT exposes no quality, glossary, or repair controls because that engine cannot honor them; direct requests using those options are rejected.
- Translation controls, normal progress states, confirmations, counts, language names, and expected native failures use the app's eight locale resources. Translation commands return stable error codes while retaining the original Rust diagnostic; unexpected failures still fall back to that detail.
- Concurrent starts for the same document and translation settings are rejected before they can write the same cache or variant.
- Job progress distinguishes model loading, job and batch preparation, active
  batch inference, completed batches, final validation, and storage. Before
  the first measurable batch completes, the UI shows indeterminate activity
  instead of an apparently idle empty progress bar.
- OPUS-MT boundary jobs retain bounded whole reader blocks for DOM identity; their actual MT fragments use the shared 900-scalar and native-token limits. Cache and saved-result identities are separate from the previous phrase-hint path.
- Translated variants are separate durable documents that can be opened, searched, deleted, and later used by the normal TTS flow.
- HTML/EPUB rendering uses the sanitized reader HTML where possible and preserves links, ids, images, tables, and EPUB-rewritten assets conservatively.
- Historical Stage 5B validation, before the latest main merge, covered local desktop and packaged `.deb` model-install, translation, open/search/delete, cancellation, restart recovery, cache-assisted retry, removal, and reinstall smoke tests. It does not validate the merged build; current smoke and release validation remain pending.
- Packaged Android/iOS translation support is not accepted yet. Experimental Bergamot choices can appear when their build and WebView gates pass, but mobile memory, thermals, lifecycle and actual-model quality still need device validation; native runtime packaging remains separate work.
- Chapter-level repair has not started. Qwen remains research-only.

## Engine-independent offline alignment plan — 2026-10-19

**Historical strict-formatting plan: use a separate offline word aligner (Option 7).
No alignment model is approved for production. The current deliverable is the
separately approved [boundary-based CTranslate2 path](#boundary-based-ctranslate2-translation--2026-10-20);
the strict candidate below remains disabled.**
This records the selected Option 7 plan, superseding the earlier attention-first
proposal below. Its sole-aligner assumption is now under the
[Stage 9 strategy reassessment](#stage-9-alignment-strategy-reassessment--2026-10-20);
the shared pipeline remains useful, but no tested model closes the semantic gate.
Further Bergamot upstream work is
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
The [Stage 2 pure contract slice](#stage-2-pure-alignment-contract--2026-10-20)
is implemented; model/runtime integration and semantic acceptance remain
**pending** as completed acceptance gates. The internal Stage 3 session below
does not imply production runtime integration. A stage with a deferred gate must
not be described as accepted.
Stage 1 now has a [feasibility receipt](#offline-aligner-stage-1-findings--2026-10-19):
runtime compatibility passed locally, but model/formatting acceptance has not.
The subsequent [scope/token policy evaluation](#auxiliary-scope-and-token-provenance-evaluation--2026-10-19)
defines conservative rejection rules; it does not fix the mandatory auxiliary
case or authorize enabling the candidate as formatting-preserving.

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

**Status 2026-10-20:** pure contract/rejection slice implemented and tested below.
No adapter consumes it. Actual literal isolation, attachment placement, discovery
of correct word links and renderer/cache integration remain later-stage work.

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

**Status 2026-10-20:** local asset/raw-tokenizer admission and a bounded internal
CPU session, notice-complete auxiliary installer admission and an internal
two-model job lifecycle are implemented below. Public alignment admission,
packaged notice/runtime qualification and resource/platform acceptance remain
pending; this stage is not complete or accepted.

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

**Status 2026-10-20:** internal exact-piece/literal-island planner and maintained
CTranslate2 adapter implemented and tested below. No public strict-path caller,
word links, projection or mapped-cache/save admission is enabled by this slice.

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

**Status:** internal implementation and synthetic/native regressions are recorded
in the [Stage 5 receipt](#stage-5-word-links-and-conservative-projection--2026-10-20).
This does not pass the mandatory semantic-scope or release acceptance gates.

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

**Status:** internal reconstruction and synthetic/native regression evidence are
recorded in the [Stage 6 receipt](#stage-6-source-owned-mapped-reconstruction--2026-10-20).
This does not close the Stage 5 semantic gates or enable public strict output;
Stage 7 internal mapped-cache/save admission is recorded below; public starts
remain disabled pending semantic acceptance.

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

**Status:** internal integration is implemented in the [Stage 7 receipt](#stage-7-mapped-cache-resume-and-atomic-save--2026-10-20).
Public admission remains disabled; this does not close the Stage 5 semantic gates.

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

**Status:** integration and regression evidence are recorded in the
[Stage 8 receipt](#stage-8-gated-formatting-setup-and-progress--2026-10-20).
Public capability remains unavailable; Stage 9 gates are still open.

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

The [Stage 1 findings](#offline-aligner-stage-1-findings--2026-10-19) give a
conditional technical go and no production go yet. The [policy evaluation](#auxiliary-scope-and-token-provenance-evaluation--2026-10-19)
sets the safe-rejection contract but leaves semantic-scope acceptance open.
Do not call that gate passed when implementing contracts → assets/runtime → exact planning →
alignment → reconstruction → lifecycle → UI, each with its own focused review
and evidence. Do not build the full integration around an unproven aligner.
Stop and revisit the design if it needs a maintained fork, training pipeline,
unacceptable resource use, silently misplaced formatting or mostly rejected
ordinary prose. This design moves the learned correspondence problem into a
pretrained model; Papercut still owns substantial offset, hierarchy and safety
engineering. It is not a guarantee of perfect semantic formatting.

## Offline aligner Stage 1 findings — 2026-10-19

**Decision: technically feasible without a runtime upgrade or engine fork;
not ready to approve production formatting parity.** The plan is committed in
`2ba1772`. This evaluation changes documentation only. All assets, synthetic
probes and results live outside the repository. No private article, sample
filename or sample-derived regression fixture is added to tracked files.

### Candidate, reference and maintenance scope

The first candidate is the published **eight-layer multilingual BERT INT8 ONNX
export**, with its matching FP32 export as the reference alternative. This is
pretrained mBERT, not evidence of an alignment-fine-tuned checkpoint. Two
extraction approaches were evaluated using the same encoder family, rather
than downloading an unrelated second model or training anything.

| Artifact | Pinned revision | Actual graph bytes |
| --- | --- | ---: |
| [cstr/awesome-align-onnx-int8](https://huggingface.co/cstr/awesome-align-onnx-int8/tree/e803a2f501474044099b539160aaca461976ecd0) | `e803a2f501474044099b539160aaca461976ecd0` | 149,709,873 |
| [cstr/awesome-align-onnx](https://huggingface.co/cstr/awesome-align-onnx/tree/5f35c9d8f7b24391f8dbbf04c4bdacc6b74de417) | `5f35c9d8f7b24391f8dbbf04c4bdacc6b74de417` | 595,783,899 |
| [Original mBERT reference weights](https://huggingface.co/google-bert/bert-base-multilingual-cased/tree/3f076fdb1ab68d5b2880cb87a0886f315b8146f8) | `3f076fdb1ab68d5b2880cb87a0886f315b8146f8` | 714,290,682 |

INT8 graph SHA-256:
`beb17c2e9520155f7da7bb94a1fb8e74f5e528acdbd8f4415854c99a3d84f429`.
FP32 graph SHA-256:
`960f81703cbb279235260457c39ab8d106224245d85bab5e59f6895096d94edc`.
Full published repository inventories are approximately 154 MB and 600 MB
respectively, including tokenizer/configuration; these are download sizes,
**not runtime RAM estimates**. Actual downloaded weights matched the Hub's LFS
hashes, and all probe assets received local hash receipts.

Model cards label the weights Apache-2.0. The original mBERT card also labels
Apache-2.0; model-card labels alone do not finish redistribution provenance and
notice review. [Awesome-align source](https://github.com/neulab/awesome-align/tree/5f150d45bbe51e167daf0a84abebaeb07c3323d1)
is BSD-3-Clause, with other upstream notices to retain if code is reused;
[SimAlign source](https://github.com/cisnlp/simalign/tree/ca1932a3655e54880f6f92b73c59100c171c31da)
is MIT. Their latest commits at evaluation were March 2022 and November 2023;
do not describe either as a currently maintained mobile SDK. The INT8 artifact
revision dates to August 2026 and FP32 to December 2025. Published export metadata
contains inconsistent size fields; derive install requirements from actual files
and inspect the graph instead of trusting those fields.

No library patch, custom export, training or production Python installation was
needed for this probe. Papercut would still maintain a bounded word-piece/offset
adapter, the selected matching rule, source-owned projection and failure policy.
That is smaller than maintaining a translation-engine fork, but is real NLP and
DOM engineering. The model does not return reader HTML or ready-to-render ranges.

### What actually ran

- **Isolation:** pinned public downloads plus local-only inference; synthetic
  source/target pairs were authored for this check, not generated by Papercut's
  translators. No private document was sent to a service or used as a fixture.
- **Reference:** CPU PyTorch `2.9.0+cpu`, Transformers `4.57.1`, original mBERT
  weights loaded through layer eight. The probe executes the unmodified pinned
  Awesome-align guide-head/mask code and compares its word links with the small
  NumPy adaptation of its bidirectional softmax rule at the upstream `0.001`
  threshold. No threshold tuning was performed.
- **Export check:** Python ONNX Runtime `1.23.2`, ONNX `1.19.1`, NumPy `2.2.6` and
  tokenizer `0.22.2`. Both graphs pass ONNX checking, use IR 8/opset 17, contain
  encoder layers 0–7, and need no external weight file. Inputs are int64
  `input_ids`/`attention_mask`; output is finite float32 `[batch, sequence, 768]`.
  The config still says 12 layers; it must not override the inspected graph.
- **Actual app-runtime compatibility:** an isolated Rust harness uses the app's
  pinned `ort = 2.0.0-rc.13`, `api-17`, `load-dynamic` and `tokenizers = 0.23.2`
  against the existing Sherpa `1.13.4` Linux shared-runtime artifact. That actual
  library reports ONNX Runtime **1.27.0**, SHA-256
  `026c7d5c609323fb16506dbc3cce801bcdffdd7566fdba49a50727e2e1e881ca`.
  All **64 graph/input runs** passed shape/finite checks; all 32 source/target
  encodings matched Python token IDs and word IDs. Rust/Python tokenizer offsets
  were also checked on Unicode controls, combining marks and emoji. C API version
  17 and ONNX opset 17 are separate compatibility facts, not interchangeable.
- **Limits:** the model's position limit is 512 tokens including special tokens.
  Real INT8 Python inference with 512 tokens returned finite `[1, 512, 768]`;
  the probe explicitly rejects 513 before inference, without truncation. These
  checks establish neither a safe mobile batch size nor a latency/RAM result.

### Alignment and formatting-scope results

The fixed set has **16 synthetic pairs: eight French → English, eight Spanish
→ English**. Each specifies source words to emphasize and exact expected target
words. It exercises adjective reordering, repeated names, negation, auxiliary
verbs, accents, numbers and quoted punctuation. This is a narrow correctness
probe, not a translation-quality benchmark or a full HTML/EPUB renderer test.

| Encoder and extraction | Exact expected emphasis scope |
| --- | ---: |
| Original mBERT / upstream Awesome-align softmax | 15 / 16 |
| FP32 ONNX / same softmax | 15 / 16 |
| INT8 ONNX / same softmax | 15 / 16 |
| Original or FP32 / cosine mutual argmax | 14 / 16 |
| INT8 / cosine mutual argmax | 13 / 16 |
| Original, FP32 or INT8 / pinned SimAlign IterMax, default two iterations | 13 / 16 |

The candidate card's cosine/mutual-argmax example is **not** upstream
Awesome-align's softmax extraction. SimAlign's pinned IterMax was executed as
the bounded alternative, with its default matching parameters and no positional
distortion. It widened some scopes onto unrelated words; it is not recommended
as an automatic coverage repair.

FP32 export word-link sets matched original-reference sets on **16/16** pairs
under both softmax and mutual argmax; maximum absolute embedding difference was
`1.15e-5`. INT8 matched original link sets on **14/16** under each rule, even
though softmax's selected emphasis scopes remained 15/16. Therefore the card's
general claim of identical INT8 links is not established. Against the actual
bundled Rust runtime, all Python/Rust link sets matched on this set for both
exports and both rules; FP32 embeddings differed by at most `1.34e-5`. INT8
embedding differences across the two runtime versions reached `0.314`, so do
not equate these observed link matches with cross-runtime numeric equivalence.

The shared unresolved case is inflection: French `reviendra` in `Il reviendra
demain.` maps only to `return` in `He will return tomorrow.`, while the fixture
expects emphasis on `will return`. Simply extending to the next unaligned word
is unsafe: IterMax instead included `tomorrow`. A valid nonempty link does not
prove complete semantic scope. The exact treatment of generated auxiliaries is
a projection-policy decision requiring explicit tests, not an HTML-validity fix.

Three negative probes make the trust boundary concrete. Unrelated French/English
and Spanish/English sentence pairs still receive word links. Two distinct emoji
both tokenize to `[UNK]` and link to each other. Directional/zero-width controls
may disappear from tokenizer output even though source offsets must retain their
ownership. Never infer completeness from link count or similarity alone, and
never use a hand-counted subword list in place of checked tokenizer word IDs.

### Desktop/mobile and resource decision

**Desktop:** CPU inference and the current Rust/shared-runtime combination are
proven for these isolated inputs. No dependency upgrade is justified. FP32 is a
useful reference but its ~600 MB download is unsuitable as an assumed low-power
default. INT8 is the candidate to retain for further evaluation, not a production
default; it needs its own alignment acceptance, not inherited FP32 approval.

**Android/iOS:** the graph has standard quantized CPU operators, including
`MatMulInteger`, `DynamicQuantizeLinear` and `DequantizeLinear`, but no device
inference or mobile operator/build acceptance ran. Reuse the existing shared
runtime and initializer if implementation proceeds; add translation-only feature
gates instead of pulling in search/TTS merely to enable it. The current
worker-only mobile translation option does not package native CTranslate2 or
this aligner. Native translator packaging, shared-runtime teardown and both
model sessions' combined memory remain independent gates. Pixel 7/Android 17
is the reference device available; a weakest-supported-device target is still
undefined. Apple/Windows packaged runtime acceptance is also pending.

Proposed initial engineering ceilings, fixed **before future performance tests**:
alignment pack ≤200 MiB, added aligner peak resident memory ≤512 MiB, and combined
translation/aligner inference ≤1 GiB. Start with one foreground job, one alignment
session, one pair at a time and a provisional 256-token-per-side admission cap
including special tokens; the graph's 512 maximum is not a recommended batch
budget. Target warm alignment p95 ≤1 second per such pair on desktop and Pixel 7,
measured separately from translation. These are admission targets, not measured
results or established low-end support. Record whole-app RAM and other loaded
models too; revise limits explicitly before tuning, never after a failed run
just to claim acceptance. Benchmarks remain deferred, so these gates are open.

For eventual held-out semantic acceptance, pre-register at least 100 annotated
formatted regions per language pair, with ordinary prose and separate adversarial
cases for morphology, repetition, disjoint/nested formatting and attachments.
Keep the material outside Git and unseen during rule tuning; blind the model
label during bilingual review. Require every mandatory positive/negative case
to behave as specified, no observed incorrectly accepted scope/identity/asset
placement, and at least 95% successful coverage on the predeclared ordinary
supported set. Report safe rejections separately, with counts and uncertainty;
zero observed errors is not proof of zero error risk. Do not remove failed cases
or relabel them unsupported after inspecting results. This protocol is recorded
now; no held-out evaluation or benchmark was run.

### Go/no-go and next bounded work

- **Pass:** pinned artifact availability, basic provenance/source review, real
  offline inference, FP32 reference parity, tokenizer IDs, CPU operator/shape
  compatibility and current Linux Rust/shared-runtime compatibility.
- **Not passed:** mandatory one-to-many semantic scope, general INT8/reference
  parity, confidence/unknown-token admission policy, full redistribution notices,
  held-out semantic acceptance, and all packaged-device/resource gates. Stage 1
  therefore has a completed evaluation receipt, **not unconditional acceptance**.
- **Recommendation:** keep Option 7 and the shared architecture, but do not add
  production assets/dependencies or claim formatting preservation yet. Next do a
  narrowly scoped policy/contract design check: define auxiliary/unaligned-word
  scope, explicit `[UNK]`/normalization rejection, and the errors that an aligner
  cannot detect. Retain FP32 as the reference, INT8 as the candidate and upstream
  softmax as the initial extraction rule. Do not add adjacency guesses, IterMax
  retries, a custom-trained model or an engine fork to force the failing case.
  Proceed to Stage 2 implementation only after this gate is resolved. Real engine
  output, nested DOM/notes and lifecycle tests are still later stages, not passes
  inferred from these hand-authored pairs.

The next evaluation is now recorded in the [auxiliary scope/token policy
receipt](#auxiliary-scope-and-token-provenance-evaluation--2026-10-19). It resolves
how to reject unsafe tokenization and incomplete scope, not how to make every
partial inline mapping succeed. The mandatory positive case remains open.

Reproduction receipt outside Git: `/tmp/papercut-aligner-feasibility/` contains
immutable asset/source metadata and hashes, dependency versions, `probe.py`,
the isolated Cargo harness, per-case links/offsets, `results.json`,
`runtime-comparison.json`, `simalign-results.json` and `negative-results.json`.
The files are disposable research evidence, not maintained application tooling;
the directory is not guaranteed to survive a reboot. Keep benchmarks, broader
smoke and the app version bump deferred. No production behavior changed.

## Auxiliary scope and token provenance evaluation — 2026-10-19

**Stage 1 findings committed as `7443bcc`; this follow-up is evaluation only.**
Use the same pinned FP32/INT8 artifacts and upstream softmax rule, without new
downloads, thresholds, dependencies or engine changes. Stage 2 implementation,
benchmarks, smoke and the app version bump remain deferred. All diagnostic
fixtures/scripts/results remain outside Git; only this documentation changes.

### Auxiliary-word scope: reject incomplete evidence, do not expand by adjacency

A word's grammatical role depends on context. [Universal Dependencies' auxiliary
definition](https://universaldependencies.org/u/pos/AUX_.html) distinguishes
grammatical contributions from the lexical verb and notes that auxiliary use
varies by language. An English stop-word list is not a source-scope mapper.
For example, bolding French `terminé` should not automatically bold English
`has` when the separate, unstyled source word `a` supplied that auxiliary.

The frozen follow-up set has **24 formatting-scope cases over 20 distinct
French/Spanish → English pairs**, plus two contraction/fusion diagnostics.
Contrasts cover future/progressive/perfect forms, explicit versus generated
auxiliaries, negation-only formatting, disjoint scope, a lexical use of `has`,
and the name `Will`. These are correlated synthetic correctness checks, not
held-out quality measurements. Both real ONNX variants ran locally. The 26
positive/fusion cases per encoder were also replayed through the same pinned
Rust binding and actual bundled Linux runtime as Stage 1: **104 tensor runs**,
with all **52 pair/model link sets and tested policy outcomes matching Python**.
Raw and pre-split token IDs were equal for these ordinary-prose inputs; that
does not establish equivalence on the control/special-token diagnostics below.

| Policy diagnostic | INT8 | FP32 |
| --- | ---: | ---: |
| Direct softmax links: exact specified scope | 18 / 24 | 18 / 24 |
| Add adjacent unlinked auxiliary/function words: exact scope | 17 / 24 | 17 / 24 |
| Conservative coverage/conflict guard: accepted cases | 15 / 24 | 15 / 24 |
| Exact scopes among those accepted, on this set only | 15 / 15 | 15 / 15 |

The expansion ablation fixes `reviendra` → `will return`, but changes correctly
mapped negation-only `not` into `is not` in both languages. It still cannot fix
disjoint `is … working` across `still`, or missing lexical material in `is
coming back`. Do not adopt it, switch to IterMax, widen a target-range hull or
translate a bold word separately to make these cases pass.

The conservative guard rejects a pair if a required styled source word has no
link, any target lexical word has no source link, or a styled target word also
links to source text with a different formatting ownership. It catches all six
wrong direct scopes here, but also rejects three cases whose selected scope was
already correct. Thus **15/24 is an observed acceptance count, not an acceptable
product coverage result**. Do not weaken the held-out 95% ordinary-input coverage
goal or reclassify the original failing positive fixture as a passing rejection.

Coverage is still not semantic proof. Four further deliberately wrong sentence
pairs per encoder, including unrelated noun/adjective substitutions, pass this
coverage guard with complete links. The aligner can pair `maison` with `cat` or
`rouge` with `blue`; a complete graph does not establish that the translation or
its correspondence is correct. These are counterexamples to a validation claim,
not measurements of Papercut's translation engines. Scores are not calibrated
confidence, and increasing a score threshold has not been evaluated as a fix.

Contractions add a representability issue: source future tense and separately
unstyled negation can merge into `won't`. Formatting the whole contraction adds
negation to the formatted meaning; formatting token fragments is not evidence
that the original scope survived. The two probes remain unresolved. Do not
rewrite target prose or split target word pieces into styled fragments to hide
this conflict in the mapping layer.

Recommended scope policy for the future shared contract:

1. **Whole-unit ownership:** if the source plan proves the same style stack owns
   the entire translation unit, inherit it over that unit's complete target.
   This includes generated auxiliaries without lexical guessing. Reuse the
   existing narrow whole-block renderer concept. It is not permission to expand
   a partly styled verb to the whole sentence, move notes/links or skip content
   checks; inner structures needing their own positions still require mapping.
2. **Partial inline ownership:** retain the full correspondence graph, required
   source identities, target range lists and unaligned/conflicting words.
   Missing or mixed-ownership evidence yields an explicit unresolved result.
   Never infer grammatical ownership from a word's spelling or its proximity.
3. **Separate assurances:** tokenizer admission, lexical coverage, DOM structural
   validity and semantic acceptance are different checks. No contract field
   should turn successful tokenization or a nonempty graph into a claim of
   semantic formatting completeness.

This determines safe failure behavior, **not a successful general solution for
generated auxiliaries**. The original `will return` expectation stays unchanged.
Any future contract-only work must represent unresolved outcomes and keep the
candidate disabled; Stage 5's mandatory positive mapping gate remains unmet.

### Unknown tokens and normalization: admission before alignment

The added diagnostic set has **23 tokenizer cases**. Python's pinned tokenizer
and the application's Rust `tokenizers 0.23.2` agree on all token IDs, admission
decisions and issue categories: **five admitted, 18 rejected**. Admitted cases
include composed/decomposed accents, known Arabic combining marks and permitted
whitespace normalization. This proves tokenizer behavior, not Arabic translation
or morphological alignment quality. The cases were defined outside Git and are
not a representative rejection-rate estimate.

Use raw exact pair text and the tokenizer's returned word IDs/offsets as the
authority. Manual regex pre-splitting changed token IDs in **13/23** diagnostics,
including source-authored special-token strings and text containing controls.
Therefore the previous research probe's external word splitting must not become
the production normalization contract. Its ordinary-prose inference results
remain evidence for those inputs, not proof of safe arbitrary input handling.

The [pinned BERT normalizer](https://github.com/huggingface/tokenizers/blob/v0.23.2/tokenizers/src/normalizers/bert.rs)
can remove controls, private-use characters, NUL and replacement characters.
Offsets are necessary but insufficient: `a` + NUL + `b` becomes the same tokens
as `ab`, while a token's offset envelope can still cover the deleted position.
Both the original and normalized views must be retained and checked. The
prototype also uses Unicode White_Space matching consistent with Rust's source
normalization, rather than Python `isspace()`; an explicit U+001C test covers
that distinction.

| Input condition | Required strict alignment admission |
| --- | --- |
| `[UNK]` for any source or target prose content | Reject the pair; equal token IDs are not equal source characters |
| Authored `[CLS]`, `[SEP]`, `[MASK]`, `[PAD]` or `[UNK]` recognized as reserved IDs | Reject; do not mistake content for inserted model framing |
| Source/target non-whitespace removed or changed without a checked reversible map | Reject; includes bidi controls, joiners, soft hyphen and internal NUL/replacement characters in these probes |
| Source/target non-whitespace has no corresponding token range | Reject even if no `[UNK]` was emitted |
| Invalid/ambiguous byte-to-scalar conversion, partial grapheme or overflow | Reject; no rounding, clipping, truncation or replacement decoding |
| Known text and normalization that obey the retained DOM/coordinate contract | Admit to alignment; this alone grants no semantic approval |
| A genuine source-owned code/literal subtree already excluded from inference | Preserve through its existing identity/island; no lexical alignment of its content |

Apply the checks to **both sides and the entire pair used as alignment context**,
not only the bold word. Unknown words elsewhere can affect contextual embeddings.
Reject rather than silently re-tokenizing, stripping marks, lowercasing names,
turning unknown prose into a literal, or replacing characters with placeholders.
An emoji may be copied only when its source-owned literal attachment is already
unambiguous; there is no new automatic emoji/unknown-name placement exception.

Only tokens proven to be postprocessor/padding framing may be excluded from
lexical coverage, using their insertion provenance, word/sequence identity,
offsets and masks. A mask alone is insufficient. All five authored reserved
strings in the probe were recognized as reserved IDs but had content offsets;
they are distinct from the automatically inserted zero-width framing tokens.
The [encoding API](https://github.com/huggingface/tokenizers/blob/v0.23.2/tokenizers/src/tokenizer/encoding.rs)
provides the relevant IDs, word IDs, masks, sequence information and offsets;
never rebuild that association by counting separately tokenized pieces.

Rejection stops the affected job before saving a variant; it does not modify
the original or discard earlier validated batches. Keep typed reasons and local
block/segment coordinates so the UI can explain the failure without including
private source text in diagnostic exports. This policy targets the future rich
alignment path, not a new restriction on today's plain translation or saved
documents. A whole-unit style shortcut is not an exemption for losing content.

### Stage 2 implications and next gate

**Ready as a design:** explicit token provenance/admission and unresolved
coverage/ownership outcomes. The shared contract should carry exact source and
target identities, source-owned style/attachment identities, many-to-many links,
disjoint target ranges, model/tokenizer/projection versions and typed rejection
reasons. Keep inference/model code out of this pure validation layer.

Mandatory contract tests should cover the 23 token-admission patterns, raw versus
pre-split differences, byte/scalar/grapheme boundaries, mixed-style links,
unaligned target auxiliaries, explicit-source auxiliaries outside emphasis,
negation-only scope, contractions, disjoint emphasis, literal exclusion and
source-owned whole-unit styling. Existing hierarchy, note, cache and pre-save
cancellation gates remain required. Tests using supplied gold links establish
contract behavior, not the candidate's ability to discover those links.

**Still not ready for production:** partial-inline auxiliary scope, a reliable
semantic admission criterion and acceptable ordinary-input coverage. Do not
enable a model/UI capability, add a parser/fine-tuning pipeline or lower the
earlier positive tests to advance the checklist. Stage 2 implementation was not
started in this evaluation. A bounded contract-only slice can follow, with the
candidate still disabled and these unresolved model gates explicit; that does
not mean Stage 1 or full formatting parity passed.

Evidence remains under `/tmp/papercut-aligner-feasibility/`: frozen
`policy-cases.json`, `policy_probe.py`, `policy-results.json`, contraction and
negative-guard results; `token-policy-cases.json`, Python/native admission probes
and matching result receipts. Both encoders were evaluated on 24 positive, two
fusion and four deliberately wrong pairs each. No new corpus/benchmark,
physical-device smoke, application build, dependency change or version bump ran.

## Stage 2 pure alignment contract — 2026-10-20

**Implemented as an unused, model-free validation boundary. Candidate disabled.**
The preceding policy evaluation was committed as `0361484`. This slice adds no
alignment model, download, engine option, feature flag, runtime dependency or
production call path. Existing CTranslate2, HY-MT2 and Bergamot behavior stays
unchanged; native cache epoch and protected-code identity version are unchanged.

### Code ownership and data flow

- `inline_markup/provider/contract.rs` binds and checks evidence at the existing
  provider-mapping seam. It does not infer links, emit HTML or write files.
- `provider/contract/tokens.rs` checks token provenance, raw UTF-8 coordinates,
  normalization/coverage and merged word boundaries without loading a tokenizer.
- `provider/contract/tests.rs` uses generic synthetic text, supplied token
  evidence/gold links and the real sanitized DOM/`ProtectedSource` path. No
  private file, sample-specific fixture or source-article reference is included.
- `ProtectedPlan` exposes only a read-only derived view of source code IDs,
  parent IDs, scalar ranges, repeatability and literal status. Its existing tree
  and identity retain attributes/literals; there is no competing identity parser.
  A size check bounds identity serialization before hashing large source payloads.

The trusted future planner supplies the retained source fragment, block/segment
identity, parent offsets, actual translated target and pinned version metadata.
The binding captures exact untrimmed source/target strings, source-plan identity,
source/target parent scalar ranges, translator/aligner/tokenizer identities and
normalization/extraction/projection policy versions. Offset addition is checked
for overflow; proving the real engine's subdivisions match parent text remains
Stage 4 work, not a claim made by this standalone boundary.

A future **verified raw tokenizer adapter**, separate from the aligner reply,
must supply token IDs, word IDs, byte ranges, inserted-token masks and normalized
text under the bound tokenizer profile. This pure layer checks consistency; it
cannot prove that fabricated token IDs came from model files. Profile verification
and actual tokenizer execution belong to Stage 3. No token evidence or tensors
are exposed over Tauri in this slice.

Replies contain binding echoes, many-to-many word links, source-ID → disjoint
target scalar range lists, and an optional known unresolved-scope reason.
Validation returns either a typed rejection or a privately constructed
`CheckedMapping` view of ranges. It returns no reader markup or semantic approval.
The earlier flattened-span helper remains for its existing renderer-boundary
tests; the new tree contract does not replace a production renderer here.

### Validation and explicit limits

The contract rejects stale/mismatched text, coordinates, source identity or
versions; unknown/duplicate IDs or links; invalid byte/scalar/grapheme boundaries;
out-of-order word pieces; missing prose coverage; and unsafe hierarchy changes.
Nested source ancestors must contain their mapped children. Unrelated source
siblings cannot overlap. Repeatable style nodes may have multiple disjoint
ranges; a single-use authored identity cannot be split or cloned.

Token admission covers the 23 diagnostic patterns from the evaluation, including
`[UNK]`, authored reserved IDs, false framing, normalization deletion hidden
inside an offset envelope, Unicode whitespace distinctions, accents/Arabic marks
and emoji. Subwords may split a combining sequence **within the same word**;
merged word and formatting boundaries may not split a grapheme. Unknown content
anywhere in either pair context rejects, not only unknown text inside emphasis.

For partial inline mappings, required source words must have links, every target
lexical word must be covered, mixed formatting ownership rejects, and declared
range scopes must equal the supplied graph. There is no adjacency expansion or
range-hull repair. A uniform whole-unit source wrapper may cover the entire
target without lexical guessing, but still undergoes binding/token/identity
checks. Known auxiliary, fused-meaning or attachment blockers always reject;
they cannot disappear merely because the graph is complete.

**Deliberate incompleteness:** a source literal still present in the prose pair
returns `LiteralRequiresIsolation`; zero-width notes/images/anchors return
`AttachmentRequiresEvidence`. This prevents treating attachment coordinates as
proven placement or silently translating a literal. Their source identities
remain retained. Correct isolation and quote/lexical attachment placement belong
to Stages 4–6. A complete but semantically wrong graph can pass structural checks;
a fused contraction without an explicit blocker may also look structurally
complete. Tests preserve these limits rather than pretending to detect meanings
the contract cannot infer. The model and its future UI capability remain off.

V1 limits are 32 KiB per text/normalized view, 128 source IDs (the existing
protected-plan cap), 512 tokens per side including exactly one leading/trailing
framing token, 4,096 links and 512 total target ranges. Identity hashing is
bounded to an eight-times-32-KiB source payload budget. This is an unpadded
single-pair evidence format, not an accepted mobile batch/runtime budget;
Stage 3 must apply the chosen model's lower limits where needed. Errors carry
categories/side only, with no private text or paths. All evidence is checked
before a `CheckedMapping` can be returned; there is no partial-success cache.

### Regression evidence and next step

**21 focused contract tests pass**, covering exact binding/version changes,
reordered nested/disjoint mappings, many-to-many supplied links, whole-unit
inheritance, missing auxiliaries, explicit auxiliaries outside emphasis, scope
fusion/negation conflicts, literals/notes/images, malformed identities/ranges,
both-side unknown context, 512/513-token limits and candidate catalog/installer
absence. Invalid UTF-8 offsets and `usize::MAX` word indices reject without
panics. A deliberately wrong complete correspondence remains only structural
evidence, never a model-acceptance fixture. These tests perform **no inference**.

Validation receipt: full base Rust library suite **466 passed, four existing
ignored**; focused contract suite **21 passed**. The broad suite initially hit
sandbox restrictions on existing loopback/D-Bus fixtures, then passed with local
test permissions. The translation-enabled suite passed **190 tests**, and the
frontend/CI-contract suite passed **93 tests**. The latter needed child-process
permissions for existing shell stubs.
Formatting and diff checks pass. No live models, packaged build, CI run,
benchmark or smoke was used to validate this slice.

The [contract-only review](#stage-2-contract-review--2026-10-20) is now recorded
below. Do not expose the candidate while semantic-scope, resource/device
and model-provenance gates remain open. Saved originals, variants, assets,
audiobooks and read-along data require no migration. Benchmarks, smoke and the
app version bump remain deferred.

### Stage 2 contract review — 2026-10-20

**Scope:** review `b352918` and its connected source-plan, provider, engine,
runner, renderer, cache, catalog and installer paths. Use the preceding scope/token
policy receipt as the expected behavior. Expected load is one foreground
document job, processing bounded pairs; no inference or production consumer
exists in this slice.

**Verdict: no must-fix findings for the contract-only slice.** This does not
approve the candidate, complete the full Stage 2 integration or establish
formatting parity. In particular:

- IDs, attributes and hierarchy remain source-owned. The new read-only view
  derives from the same protected tree rather than a second identity parser.
  Pair binding covers exact text, parent coordinates and version identities;
  malformed indices/ranges reject before constructing checked evidence.
- Token evidence is a separate trusted-adapter input, not an assertion supplied
  by the aligner. Unknown/reserved content and lossy normalization reject on
  both sides. Fabricated but internally consistent token IDs cannot be detected
  by this layer; verified files and actual raw tokenization are Stage 3 work.
- Partial styling uses supplied word links without expanding over an adjacent
  auxiliary or bridging disjoint ranges. Uniform whole-unit ownership is a
  source-plan shortcut only. Missing lexical coverage and mixed ownership reject.
  Complete wrong graphs and unreported fused morphology remain semantic limits,
  explicitly exercised in tests, rather than bugs claimed solved by validation.
- Literals and zero-width attachments reject explicitly pending later isolation
  and placement work. `CheckedMapping` contains ranges only; it is not HTML or a
  renderer/save admission token. The existing production validation, rendering,
  cache identities, atomic writer and installed-model lifecycle do not call it.
- Pair text, token/link/range counts and source identity payload are bounded.
  These bounds limit pure validation work; they are not measured mobile memory
  or latency acceptance, nor permission to load the candidate at its graph maximum.

**Fresh validation:** `cargo test --manifest-path src-tauri/Cargo.toml --locked
--offline --lib translation::inline_markup::` passes **57 tests**, including
the 21 contract tests and connected protected/source/provider regressions.
The same filter with `--no-default-features --features native-translation`
also passes **57 tests**. Provider-only filtering passes **27 tests** (a subset,
not additional coverage). Formatting and diff checks pass. No new tests were
needed for a docs-only review; these are supplied-evidence tests, not inference.
The previous full-suite receipt above is unchanged, not a new full CI run.

**Next bounded stage:** start Stage 3 with verified asset/provenance and real
raw-tokenizer admission, then the bounded local runtime. Preserve the distinct
alignment asset role; do not add the aligner as a selectable translation engine.
Pin/review weight and tokenizer revisions, hashes, sizes, configuration and full
redistribution notices before allowing any install. Connect actual raw token
IDs, masks, word IDs, original byte offsets and normalized views to this contract;
do not copy the earlier pre-split research probe into production. Test corrupt or
replaced assets and repeat the token-policy cases against the pinned tokenizer.
Reuse existing staged installation, shared-runtime initialization and model-use
guards when those pieces are wired; do not introduce a second model manager.

Keep the candidate unavailable throughout these internal steps. Real model load,
shape/output admission and lifecycle tests remain separate Stage 3 gates, and
mandatory auxiliary-scope success, held-out acceptance, resource budgets and
packaged-platform validation still block enabling the feature. Benchmarks,
broader smoke and the app version bump remain deferred. No source code,
dependency, asset, cache schema or application behavior changed in this review.

## Stage 3 asset and raw-tokenizer admission — 2026-10-20

**Internal, default-off implementation; candidate still unavailable.** Review
notes were committed first as `94ae85c`. This slice implements verified local
files and actual tokenizer evidence only. It does not download/install assets,
load weights, infer links, alter provider selection, expose a Tauri command or
change the existing document/cache pipeline. Benchmarks, broader smoke and the
app version bump remain deferred.

### Ownership, pins and admission

- `alignment.rs` owns tokenizer execution and adapter admission, separate from
  DOM/source ownership and the pure `inline_markup/provider/contract` validator.
  Its private tokenizer is constructed from a hash-verified snapshot; callers
  cannot mutate it or pass arbitrary tokenizer JSON through an application API.
- `alignment/assets.rs` retains a distinct alignment-role inventory, reusing
  `TranslationModelFile`. It is absent from the translation catalog and installer
  resolver, has no download URLs, and grants no release/install approval.
- `model_store/verified.rs` shares bounded regular-file/path/size/SHA-256 checks
  with the existing Bergamot asset reader. Small files return the exact snapshot
  whose hash passed; weights are streamed through a 64-KiB buffer and discarded.
  Symlinked roots/leaves, path escapes, missing files, wrong sizes and modified
  bytes reject. Typed internal failures contain no source text or private paths.

The pinned repository/revision is the previously evaluated
[`cstr/awesome-align-onnx-int8@e803a2f501474044099b539160aaca461976ecd0`](https://huggingface.co/cstr/awesome-align-onnx-int8/tree/e803a2f501474044099b539160aaca461976ecd0).
The [source inventory](../src-tauri/src/translation/alignment/assets.rs) records
full SHA-256 and exact size for **seven required files**, totaling **153,638,839
bytes**: graph, tokenizer JSON, model config, tokenizer config, special-token map,
vocabulary and model card. Local admission rechecks all seven, retaining only the
tokenizer JSON. The inventory is capped at 200 MiB, tokenizer JSON at 4 MiB.
The model config's 12-layer field still does not describe the eight-layer graph;
this code does not derive a runtime architecture from that field.

**Provenance/notice gate remains open.** The retained pinned Hub inventory has
no standalone `LICENSE` or `NOTICE`. The export and original mBERT cards label
Apache-2.0; that is recorded evidence, not completed redistribution approval.
No upstream extraction implementation is copied in this slice. Finish the full
base/export attribution and applicable notice inventory before installer or
release admission; the later [redistribution evaluation](#stage-3-redistribution-and-installer-admission-evaluation--2026-10-20)
clarifies published permission and the remaining packaging work. No model
weights, vocabulary, model card, private article or sample-derived fixture is
committed.

### Raw tokenizer evidence and bounds

`native-translation-alignment` enables the internal module and makes the already
resolved `tokenizers = 0.23.2` an optional direct dependency with default features
off and `onig` only. Cargo.lock adds only the app's direct dependency edge; no
package version or checksum changes. Normal desktop/mobile scripts and CI do
not enable this feature. Its dependency closure adds no ORT, search, TTS,
CTranslate2 or llama.cpp runtime; runtime work remains separate.

The adapter encodes exact raw source/target strings, not pre-split words. It
retains original UTF-8 offsets, word IDs, inserted-token masks and normalized
views; checks sequence/type IDs, attention masks and framing; then reuses the
pure contract's Unicode, unknown/reserved-token, normalization and coverage
checks. Whole-context admission applies to both sides. `check_pair` binds actual
pair strings to this evidence and the pinned tokenizer identity; a successful
result remains structural evidence, never semantic approval or renderer HTML.

Text is capped at 32 KiB and each side at **256 tokens including framing**.
Padding/truncating profiles and overflow reject; there is no clipping or retry
that silently loses text. The 512-token graph maximum does not override this
provisional engineering cap. Resource/performance acceptance is still unmeasured.

### Regression receipt and next gate

Fresh Rust library validation passes **469 tests with the feature off** (four
existing ignored) and **477 with it on** (four existing ignored plus the explicit
external-pack test). The slice adds ten ordinary regression tests: seven
alignment/pack tests and three shared-file tests. Existing Bergamot tests also
exercise the shared reader. One existing localhost downloader fixture initially
hit sandbox permissions; full suites passed with local test permissions.

The external-pack test was then explicitly run against all seven previously
downloaded pinned files and **passed**. The actual tokenizer gives the expected
**five admissions and 18 rejections** for the 23 generic policy patterns, checked
on both source and target. Known ordinary/accent/Arabic-mark token IDs match the
earlier feasibility receipt; original-byte/combining-subword and 256/257-token
boundaries pass. This is tokenization/integrity correctness, not graph inference,
translation quality, latency/RAM measurement, packaged testing or app smoke.

To repeat with an externally obtained exact pack (the test never downloads):

```sh
PAPERCUT_ALIGNMENT_ASSETS_DIR=/path/to/pinned-alignment-pack \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --no-default-features --features native-translation-alignment \
  pinned_external_pack_passes_real_tokenizer_admission_without_inference -- --ignored
```

Unit tests use small generated synthetic vocabularies and supplied mapping links,
with the real tokenizer library and sanitized source-plan path. They separately
cover malformed profiles, missing/corrupt/replaced files, symlinks/path escapes,
unknown/reserved strings, hidden normalization loss, both-side rejection,
graphemes and word IDs, exact pair binding and catalog/installer absence.
Feature dependency checks, Rust formatting and diff checks pass.

The [admission review](#stage-3-admission-review--2026-10-20) is recorded below.
The separate [bounded ONNX session slice](#stage-3-bounded-onnx-session--2026-10-20)
is now implemented; the following requirements remain the acceptance checklist.
That later slice must reuse shared-runtime initialization, validate actual graph
inputs/outputs and non-finite values, enforce tokens/tensors/threads/queue limits,
and test real model load independently of mocks. It must verify the bytes actually
loaded under a model-use lease: today's streamed file check is not a future
path-based load guarantee. Installer promotion/removal races and cancellation
also remain separate gates; do not add another model manager. Keep the candidate
unavailable while notices, auxiliary-scope success, held-out acceptance,
resource/device qualification and the remaining lifecycle work are unresolved.

### Stage 3 admission review — 2026-10-20

**Scope:** `259d8d1`, committed before this review. Read the asset inventory,
shared verifier, real-tokenizer adapter, pure contract and all new tests, plus
the connected Bergamot delivery/status, model-operation guards, catalog,
capabilities and cache/render admission paths. Expected load remains one
foreground document job with bounded pairs. There is no aligner application
caller or graph session to qualify yet.

**Verdict: no must-fix findings within the admission-only scope.** In particular:

- Extracting the shared verifier preserves Bergamot's file-size/hash/snapshot
  checks and 32-MiB limit. Its existing command holds the model-file lease;
  worker-side verification remains in place. The helper also checks the opened
  handle's file type/size. This review does not claim protection against every
  hostile concurrent filesystem mutation or guarantee a later path-based load.
- The alignment inventory is immutable and separate from translation selection.
  No unverified file, UI argument or environment variable constructs the adapter
  through a production entry point. The environment-selected pack is confined
  to the explicitly invoked test. Streaming verification retains no weight
  buffer or reusable graph-load authorization.
- The tokenizer is private after verified-snapshot construction. Its real raw
  encoding supplies offsets, word/sequence IDs and masks; there is no independent
  word splitting or caller-selected framing. Exact pair strings and the pinned
  tokenizer identity are checked before structural evidence can be returned.
  Unknown/normalization/scope rejection and semantic limitations remain explicit.
- Token/text/inventory limits bound this layer's work. The new feature is off in
  shipping scripts; its dependency closure adds the pinned tokenizer but no
  ONNX/search/TTS/translation-engine runtime. Existing saved data and cache
  identities require no migration. No additional parser, model manager or
  parallel lifecycle framework is needed for this slice.

**Fresh checks:** the translation test filter passes **192 tests with the feature
off**, and **200 with it on** (one external-pack test ignored in that ordinary
run). The external pinned-pack test was then explicitly rerun and **passed**,
including all 23 policy patterns on both sides, known token IDs and 256/257-token
admission. Existing Bergamot/shared-file, lifecycle, cache and reconstruction
tests are included in those filtered suites. Formatting, diff and feature
dependency checks pass. This is a fresh targeted review receipt, not a repeat of
the full 469/477-test suites or CI. No new tests or production code changes were
needed for this review.

**Next bounded slice:** implement the ONNX session behind an internal opt-in,
reusing the shared runtime initializer without depending on search or TTS. Bind
the actual graph bytes loaded to their pin under the existing model-use guard;
a checked filename from an earlier call is insufficient. Validate graph input
names/types, output shape/finite values and memory/token/tensor/thread/queue
bounds before exposing embeddings. Test session admission/release, rejected
loads, cancellation/error cleanup and real pinned-graph execution separately
from mocks. Keep extraction/projection, document-job integration and installer
promotion as their own later work; a passing session test does not establish
semantic alignment or full Stage 3 acceptance.

Redistribution notices, mandatory auxiliary-scope success, held-out coverage,
resource/device qualification and the remaining lifecycle gates still block
enabling the candidate. Benchmarks, smoke and the app version bump remain
deferred. No app/package build, fresh graph inference, platform-device test or
model download ran in this review.

## Stage 3 bounded ONNX session — 2026-10-20

**Internal CPU embeddings only; candidate still disabled.** The preceding
admission review was committed first as `efc55fc`. The new default-off
`native-translation-alignment-runtime` feature enables the tokenizer admission
feature plus the existing pinned `ort = 2.0.0-rc.13` with `load-dynamic`/API 17.
It does not enable search or TTS, add a model selector/download, change normal
build/CI flags, extract word links or enter the document writer. No dependency
version, weight, tokenizer or translation engine changes.

### Load ownership and execution

- `alignment/runtime.rs` reuses the existing `onnx_runtime::initialize()` and
  process-wide environment. Its compile gate now admits this independent
  consumer; previous search/TTS and linked initialization branches are unchanged
  when the new feature is off. No second runtime loader is introduced.
- `AlignmentSession::load` claims the existing exclusive model-file guard under
  an internal alignment key before admission. A second session on that shared
  application state fails immediately rather than queuing or loading twice.
  Removal is blocked until the session drops. Successful construction retains
  the guard; every rejected load releases it through normal Rust ownership.
- The pack is checked in one inventory pass. Runtime admission retains the exact
  bounded, SHA-256-verified graph snapshot and passes those bytes directly to ORT;
  there is no later path reopen. The snapshot is dropped after native creation.
  Tokenizer-only admission still streams/discards weights. Shared file reads can
  now check cancellation between 64-KiB chunks and never return partial bytes.
- The pinned signature must be exactly two dynamic rank-two int64 inputs,
  `input_ids`/`attention_mask`, and one dynamic rank-three float32 output,
  `last_hidden_state`, with width 768. The misleading 12-layer config field is
  not used to build an architecture. Unknown names/types/ranks/widths reject.
- Each session uses CPU only, its own pool (two intra-op threads, one inter-op
  thread), sequential graph execution, and no inherited environment execution
  providers. CPU arena and memory-pattern retention are disabled for this
  variable-length workload. These are engineering bounds, not a measured RSS cap.

`infer_pair` admits both exact raw strings before running either side, reuses one
session and runs one side at a time. It returns source/target encodings with
checked embeddings, never HTML, scores or semantic approval. Each side remains
limited to 32 KiB and 256 tokens including framing. Input tensors contain only
those admitted IDs and their full attention mask. Named outputs must be finite
float32 `[1, token_count, 768]` with the exact value count before they are copied.
At the maximum admitted length, each returned side contains 786,432 bytes of
float values (1.5 MiB for the pair). Native weights, loading copies, intermediates
and the rest of the app are additional memory; the resource acceptance gate is
still open.

### Cancellation and lifetime limits

A small job-scoped `Cancellation` handle records an irreversible flag and the
currently registered ORT run options. Registration and cancellation share a
mutex so a cancel cannot be lost between the flag check and run admission.
Another thread can request ORT termination without a polling thread or a queue.
Run registration and temporary results are released on success/error; known
cancellation discards even a completed native result. One mutable session admits
one pair at a time. Internal errors retain categories rather than private text,
paths, raw tensors or ORT diagnostic strings.

ORT offers no cancellation hook for `CreateSession`. Cancellation during graph
reads is cooperative; during native creation it is honored when that call
returns, at which point the new session and lease are discarded. Native run
termination is also cooperative, not a hard wall-clock guarantee. The caller
owns the reusable session and must drop it when a job ends; per-run rejection
does not silently unload it. This slice has no application job/cancel bridge,
save cutoff or translator-model lease integration yet. Existing session field
drop order releases ORT before releasing the alignment model guard.

### Regression receipt and reproduction

Fresh full Rust library suites pass **470 tests with the feature off** (four
existing ignored), **479 with tokenizer admission only** (five ignored), and
**483 with the runtime feature** (seven ignored). Six new ordinary tests cover
signature/output rejection, cancellation/failed-load cleanup, monotonic shared
cancellation, immutable graph snapshots and cancelled reads. Existing
Bergamot/file-validation, cache, hierarchy and job tests remain in these suites.

Two explicit external tests ran separately and **passed**:

1. The original pinned INT8 graph loaded from its verified snapshot through the
   existing official shared library. That previously inspected artifact reports
   ORT 1.27.0 (C API 17 consumer). Repeated French/Spanish synthetic pairs, maximum
   256-token input, 257-token/unknown-token rejection, registered-run native
   termination, reuse after rejection, invalid graph rejection, blocked second
   load/removal, drop and repeated job admission all passed. These test outputs
   establish shape/finite/lifecycle correctness, not numeric reference parity,
   semantic mapping, translation quality or measured performance.
2. In an isolated process with no discoverable shared library, loading returned
   `RuntimeUnavailable` and released the guard. Cargo initially reintroduced a
   runtime library search path, so that attempt did not satisfy this test's
   precondition. Directly invoking the built test binary with an isolated loader
   environment produced the intended missing-runtime case and passed.

The positive check uses externally obtained pinned files and a compatible
official shared library; it never downloads or exports models:

```sh
PAPERCUT_ALIGNMENT_ASSETS_DIR=/path/to/pinned-alignment-pack \
LD_LIBRARY_PATH=/path/to/official-shared-runtime/lib \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --no-default-features --features native-translation-alignment-runtime \
  pinned_graph_reuses_session_obeys_cancel_and_releases_lease -- --ignored
```

The loader variable above is a Linux test instruction, not an end-user setup
requirement or a new packaged runtime path. Run the missing-runtime test in a
separate process by invoking the test executable reported by Cargo directly,
with an isolated loader environment and the same external pack. Do not run all
ignored tests together: positive and missing-runtime tests require opposing
loader preconditions, and the shared initializer deliberately caches its result
per process.

The runtime-only build and the combined
`native-tts-shared,experimental-search-embeddings,native-translation-alignment-runtime`
library build check pass. The latter is compile compatibility, not fresh TTS or
search inference acceptance. Formatting/diff checks pass; Cargo.lock is unchanged.
All inputs are generic synthetic strings, with model files outside Git. No
sample-specific material, app/package build, CI, physical-device test, benchmark,
broader smoke or version bump is included.

### Stage 3 bounded session review — 2026-10-20

Reviewed implementation commit `b6e677a`, including its asset reader, tokenizer,
model-operation guards, shared runtime initializer, feature gates and tests.
Assumed load: one foreground alignment session on the shared application state,
with sequential source/target inference. **No concrete must-fix or should-fix
finding within this disabled internal slice.** This does not accept Stage 3 or
authorize exposing the candidate to users.

The review confirmed that rejected loads release the guard, native session
ownership ends before the lease, and graph loading consumes the verified byte
snapshot rather than reopening its path. Per-run cancellation registration is
cleared on every return, both raw strings are admitted before inference, and
output shape/count/finiteness checks precede copying. Existing search/TTS
callers still share the initializer; no production build or UI caller enables
alignment. The ORT binding's ordinary memory loader does not require retaining
the graph buffer as its separate borrowed-memory API would.

Fresh focused translation suites passed **193 tests with features off**, and
**206 with the runtime feature** (three explicit external tests ignored).
The real pinned-graph lifecycle test and the isolated missing-runtime test each
passed separately. The native termination check cancels a registered run from
another thread before native execution; it does not measure interruption during
a running kernel or prove a cancellation deadline. Formatting and diff checks
passed. Earlier full-suite and combined-feature build receipts above remain
applicable; this review did not rerun full CI, app packaging or device tests.

**Review recommendation, evaluated below:** resolve the alignment pack's
redistribution notices and asset-role/installer admission before adding
downloads. Reuse the existing model manager, staged promotion and removal guards; keep alignment out of the
translation selector. Then complete the application two-model job lifecycle and
cancellation/save integration, with failed-install, removal-race and repeated-job
tests, before Stage 4's exact-piece/literal-island planning. Actual word-link
extraction/projection, mandatory auxiliary-scope success, held-out coverage,
combined RAM/latency and platform/device qualification remain open. Benchmarks,
smoke and the version bump stay deferred; the candidate stays unavailable.

## Stage 3 redistribution and installer admission evaluation — 2026-10-20

**Decision: prepare a notice-complete auxiliary pack and harden the existing
installer; do not add another downloader or translation engine entry.** Published
permission is stronger than the earlier card-only receipt: Google's original
release explicitly covers its models. Missing standalone Hub license files are
a packaging task, not by themselves a reason to fork, re-export or wait for an
upstream change. This evaluation adds no production code, notice payloads or
download URLs to the application. Stage 3 and formatting acceptance remain open.

### Verified provenance and notice scope

| Component | Evidence checked | Admission consequence |
| --- | --- | --- |
| Selected INT8 export | [Pinned Hub inventory](https://huggingface.co/api/models/cstr/awesome-align-onnx-int8/revision/e803a2f501474044099b539160aaca461976ecd0) and [original card](https://huggingface.co/cstr/awesome-align-onnx-int8/blob/e803a2f501474044099b539160aaca461976ecd0/README.md), revision `e803a2f501474044099b539160aaca461976ecd0`, declare Apache-2.0 and mBERT ancestry; no standalone `LICENSE`/`NOTICE` | Retain the unmodified card, exporter identity and pinned source. Seven existing runtime files total 153,638,839 bytes; legal payloads add to this inventory and its accounting. |
| Original mBERT model/tokenizer | [Hub snapshot](https://huggingface.co/google-bert/bert-base-multilingual-cased/tree/3f076fdb1ab68d5b2880cb87a0886f315b8146f8) declares Apache-2.0. Google's [original release README](https://github.com/google-research/bert/blob/eedf5716ce1268e56f0a50264a88cafad334ac61/README.md#pre-trained-models) explicitly licenses models under the same terms as its [Apache-2.0 license](https://github.com/google-research/bert/blob/eedf5716ce1268e56f0a50264a88cafad334ac61/LICENSE). | Record Google/BERT and Hugging Face card attribution. This observed base snapshot is evidence, not a claim that the exporter used that exact checkpoint revision. |
| Awesome-align research/source | [Pinned BSD-3-Clause license](https://github.com/neulab/awesome-align/blob/5f150d45bbe51e167daf0a84abebaeb07c3323d1/LICENSE), copyright Zi-Yi Dou; its README identifies additional borrowed code. | Research attribution does not change the weight license. Current Rust code copies no extraction implementation; if later code is adapted, retain applicable source licenses/notices at that point. No upstream Python package ships. |
| Existing native runtime/binding/tokenizer | Existing pinned ORT Rust binding and tokenizers, plus the already-used official shared runtime; [ORT runtime license](https://github.com/microsoft/onnxruntime/blob/v1.27.0/LICENSE) and [third-party notices](https://github.com/microsoft/onnxruntime/blob/v1.27.0/ThirdPartyNotices.txt) are separate from model terms. | Reuse the runtime, without dependency upgrades. Check actual packaged binaries and their existing notice delivery before platform release; the alignment pack's license does not cover those components. |

The export's freshly fetched card matches the existing pin exactly: SHA-256
`e358416e1e88ba918f506b4988ad6ec29d85c0964bd988d9e6cd9473c24df9fa`.
Both inspected Hub inventories lack a standalone `NOTICE`; the original BERT
repository's complete tree at the linked revision contains `LICENSE`, no
`NOTICE`. This is the inspected scope, not a claim about every historical
dependency or training-data license.

[Apache-2.0 section 4](https://www.apache.org/licenses/LICENSE-2.0)
requires license delivery, preservation of relevant notices and identification
of modifications. Carrying upstream `NOTICE` attribution is conditional on one
being supplied; do not invent an upstream notice or require special permission
merely because none exists. Keep copyright/attribution separate from optional
paper citations and do not relicense the model under Papercut's MIT license.

For this pack, deliver a full `LICENSE-APACHE-2.0.txt` and a Papercut-maintained
`ATTRIBUTION.md`, alongside the pinned upstream card. The canonical license
checked from BERT and Apache is 11,358 bytes, SHA-256
`cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`.
Pin that file through the installer's existing explicit-URL support. Bundle the
small app-owned attribution payload and copy/verify it during staging; do not
create an external hosting service for it. Both legal files must be required,
available offline with the installed pack and included in storage accounting.

Attribution must identify the base, exporter, immutable artifact revision,
license and first-eight-layer INT8 export, and state that Papercut uses the
upstream bytes unchanged. The exporter card's specific eight-layer description
and metadata conflict with its generic assertion of no architecture change.
Record the specific observed artifact and this limitation; do not claim it is
an untouched twelve-layer mBERT or a reproducible conversion from a known base
commit. That uncertainty does not erase the published license, establish
numeric/reference parity or justify changing the graph/tokenizer pins.

### Installer fit and gaps to address before admission

The current [manifest resolver](../src-tauri/src/translation/model_store.rs)
derives every entry from `find_planned_model`; install, status and removal use
that resolver. Adding the aligner to `models.rs` solely to get a download would
incorrectly make an auxiliary dependency look like a translator. The runtime's
existing lease key is `alignment-awesome-align-int8`; future installer/removal
claims must use that exact identity.

Reuse the current file manifest, pinned HTTPS URL construction, streaming size/
SHA-256 verification, progress events, work-directory cleanup and state guards.
Add an explicit auxiliary lookup behind the default-off alignment feature; keep
translation engine selection restricted to the translation catalog. The caller
must obtain role and paths from those trusted lookups, never from a user-supplied
role/path/URL. Reuse the current install/status/remove command plumbing with
role-aware status messages; installed auxiliary files must not imply engine or
formatting readiness. No new public selector, automatic download or shipping
build flag is needed in this stage.

Reusing the installer also requires these concrete corrections:

- **Re-admission:** native installed status currently checks file presence only
  (Bergamot additionally verifies its pack). Thus an existing corrupt native
  directory can trigger install's early-success path. The alignment path must
  verify the complete runtime/notice inventory before reporting installed or
  skipping a repair. Runtime still rechecks the bytes it actually loads.
- **Promotion:** current download staging is under app cache and final storage
  under app data. The installer removes the old final directory before renaming
  staged files. A rename failure, including separate cache/data filesystems, can
  leave no final install. Stage on the destination filesystem, preserve the old
  directory until verified replacement, and recover interrupted replacement.
  Use a bounded staging/previous-directory sequence, not a claim that replacing
  a nonempty directory is one portable atomic rename. Keep this correction in
  the shared installer rather than duplicating a safer alignment-only version.
- **Space/cleanup:** there is no free-space preflight in this installer. Reserve
  space for a full new pack while retaining the old install. Reuse
  `library_transfer::ensure_available_space`, already used by search, with its
  64-MiB reserve; account for every retained copy if cache copying remains.
  Check the destination volume and propagate write failures; preflight cannot
  reserve space against other applications. Cleanup/recovery must not delete a
  valid old pack or touch originals, variants, audiobooks or segment caches.
- **Lifecycle:** reuse install/read/remove claims under the same alignment key.
  The application job still needs ownership of both translator and alignment
  resources, cancellation and the existing save cutoff. Installer work alone
  does not complete that later integration. No new pause/resume download API is
  implied; interrupted-install cleanup/recovery is required.

### Next implementation and completion checks

1. Prepare the small pinned license/attribution payloads and auxiliary manifest
   from the existing asset pins, with no duplicate graph/tokenizer inventory.
   Keep actual download admission disabled until the following checks pass.
2. Harden shared re-admission/promotion/cleanup and destination-space checks
   before connecting the auxiliary resolver. Tests must use synthetic bytes and
   temporary directories, not model downloads or private articles.
3. Require regressions for missing/changed legal and runtime files, wrong hash/
   size, symlink/path rejection, interrupted downloads, insufficient space,
   failed final rename, recovery at each replacement boundary and retention of
   the previous valid pack. Exercise a successful install/repair/status/remove
   round trip through the real installer, with injected I/O failures.
4. Test matching install/read/removal keys, contention and guard cleanup;
   feature-off/unknown IDs must reject and the engine selector must never list
   the pack. Regression checks must preserve existing OPUS-MT/HY-MT/Bergamot URL
   pins and behavior. Do not claim an interrupted native load was preempted.
5. Review this bounded installer slice, then finish the two-model application
   lifecycle before Stage 4. Semantic/auxiliary-scope success, held-out coverage,
   numeric reference parity and resource/platform acceptance remain independent
   gates; neither notice completion nor a successful install enables alignment.

**Evaluation validation:** rechecked published immutable inventories/card bytes,
the original BERT license/model release and upstream notice scope. Read the
connected installer, status/catalog, path and state-guard code. Existing focused
translation regressions passed **206 tests**, with three external-pack tests
ignored; this includes the real downloader's synthetic localhost size/hash
checks. There are currently no promotion-failure/space/recovery regressions in
this installer; the checklist above is required future coverage, not a claim
that it passed. No weights were downloaded or redistributed, upstream contacted,
production code changed, or benchmark/smoke/version-bump work performed.

## Stage 3 notice payloads and shared installer hardening — 2026-10-20

**Implemented after evaluation commit `2e73f7a`; auxiliary download admission is
still disabled.** This slice prepares the legal payloads and corrects the
existing installer's filesystem lifecycle. It does not register an alignment
manifest, add a selector/command/build flag, change model pins or integrate
alignment into document jobs. Stage 3 is still partial.

### Prepared notices

The [notice directory](../src-tauri/src/translation/alignment/notices) contains
the unchanged canonical Apache-2.0 license and a separate Papercut-owned
attribution file. Their exact bytes are regression-checked:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `LICENSE-APACHE-2.0.txt` | 11,358 | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| `ATTRIBUTION.md` | 2,235 | `071418670638baed08e3932a375dec40fb374ec4073c076dc95db1b9f625b45f` |

Attribution identifies the base/exporter/revision, eight-layer INT8 artifact,
unchanged downstream bytes, research/source distinction and the source-revision
uncertainty recorded above. These files are prepared source assets, not yet
installed or automatically packaged by a new application path. The existing
seven-file runtime inventory remains unchanged. A later notice-complete pack
would total 153,652,432 bytes with these two files; its admission must require
both, alongside the original upstream card, before downloads are enabled.

### Shared installer behavior

- `model_install.rs` retains command admission, leases, HTTP download and event
  handling. Its `transaction.rs` owns destination-volume staging, verification,
  recovery and removal; tests live separately. No second downloader or generic
  filesystem framework was introduced.
- Install/repair validates the manifest before filesystem work and streams every
  required SHA-256 before its early-success return. Same-sized corrupt files no
  longer skip repair. Downloaded bytes are still size/hash checked, then the
  staged inventory is independently verified and synced before promotion.
- Routine native status remains a cheap regular-file/type/size check, not a
  hash-based integrity certificate. Bergamot retains its additional verified
  status path. Large native models are fully hashed when explicitly installed
  or repaired, without adding multi-gigabyte hash scans to status polling.
- Staging is `<models>/.{id}.installing`, beside `<models>/{id}`; the previous
  directory is `<models>/.{id}.previous`. Promotion retains the previous pack
  until the new rename succeeds. Failed promotion restores it; failed rollback
  retains it for recovery rather than deleting the last copy.
- On the next install attempt, recovery keeps a verified new final directory,
  otherwise restores the previous directory, and discards interrupted staging.
  This precedes the early-success check. It is retry recovery, not background
  startup work or a portable guarantee against power loss. Explicit removal
  reclaims final/staging/previous directories under the existing model guard.
- Install and explicit removal also reclaim the old per-model cache staging
  root, so a pre-upgrade interrupted download does not consume retry space.
  New downloads check the destination volume using the existing
  `ensure_available_space` helper (new pack plus 64 MiB). Preflight does not
  reserve disk space; write/flush/sync errors still abort with cleanup.
- Manifest escapes, duplicates, invalid hashes and size overflow reject before
  downloading. Model storage accounting rejects symlinks/special files instead
  of following unrelated data or cycles. Unexpected installer root types fail
  safely. Original documents, translated variants, audiobooks and segment cache
  roots are outside all these cleanup paths.

### Regression receipt and next gate

Ten new ordinary tests cover notice identity, disabled auxiliary/unknown IDs,
install/repair/status/remove, missing legal/runtime files, corrupt hashes and
sizes, preflight refusal, interrupted downloads, rename/rollback failures,
process-interruption recovery, invalid inventories, symlinks and an actual HTTP
downloader-to-transaction round trip with model lease contention/cleanup. The
existing downloader test additionally checks short and interrupted responses.
All fixtures are tiny generic files in temporary directories; no model or
private-document test files were added.

Full Rust library suites passed **478 tests with features off** (four ignored),
**489 with tokenizer admission** (five ignored), and **493 with the internal
runtime feature** (seven ignored). Failure injection exercises the production
filesystem transaction, not a duplicate implementation. It simulates space and
rename failures; it does not fill a real disk, kill the app or qualify Windows/
Apple filesystems. Ordinary tests never download weights.

**Next:** review this notice/installer slice, then wire the trusted auxiliary
manifest and verified notice delivery through existing install/status/remove
admission. Keep it out of the translation engine catalog. Two-model application
job ownership, cancellation/save integration and the semantic/resource/platform
gates remain later work. Benchmarks, broader smoke and version bump remain
deferred; no alignment candidate is enabled by this commit.

## Stage 3 auxiliary manifest and notice delivery — 2026-10-20

**Trusted auxiliary install/status/remove admission is now wired; translation
alignment remains unavailable.** The existing default-off
`native-translation-alignment` feature admits only
`alignment-awesome-align-int8` through the current commands. Feature-off builds
and unknown keys reject it. `models.rs`, capability model choices, native engine
defaults, frontend, shipping scripts/CI flags and dependency versions are
unchanged. Translation-start admission still requires the translation catalog,
so this auxiliary key cannot be used as a translator.

### Manifest and delivery boundary

- `alignment/assets.rs` supplies the trusted manifest and bundled legal bytes.
  Its installer inventory references the existing seven runtime pins and adds
  `LICENSE-APACHE-2.0.txt` plus `ATTRIBUTION.md`: nine required files totaling
  **153,652,432 bytes**. The repository/revision, source URLs, graph/tokenizer
  hashes and runtime's `alignment-awesome-align-int8` lease key are unchanged.
- The common manifest resolver checks the feature-gated auxiliary lookup before
  the translation catalog. Role comes from this compiled lookup; callers cannot
  supply a role, directory, URL, filename or notice body. No new manifest/file
  source framework or IPC DTO was introduced.
- The seven upstream files use the current pinned-revision HTTP downloader.
  Both legal payloads are compiled into alignment-enabled builds and copied
  locally during staging. They never reach network delivery. Inventory
  validation verifies their exact compiled length/hash before I/O; the staged
  and installed files are independently hashed with the rest of the pack.
- The shared transaction's recovery, storage preflight, promotion, cleanup and
  removal rules apply unchanged. Totals include legal files, and the notices
  remain available offline beside the installed model. Native translation
  manifests cannot accidentally resolve bundled alignment notices.
- Auxiliary status verifies the full nine-file inventory, including same-sized
  notice or graph corruption, before reporting installed. Its diagnostic says
  asset verification is separate from translation/formatting readiness.
  Install progress also identifies auxiliary assets. Native metadata-only and
  Bergamot verified status behavior remain unchanged. This explicit auxiliary
  status query hashes about 150 MB; no polling loop or selector entry was added.

The standalone raw-tokenizer/runtime engineering probes still accept the
original seven pinned files outside installed storage. That does not bypass
nine-file command/status admission or grant document-formatting approval.
The application two-model job path has not been connected in this slice.

### Regression receipt

- Synthetic transaction tests keep the real auxiliary identity/legal pins while
  substituting a tiny generic graph. They verify first install, verified reuse,
  bundled delivery without notice HTTP calls, accounting, same-sized graph and
  notice corruption, missing notices, repair, removal and rejection of mismatched
  compiled notice size/hash before preflight or download.
- Catalog/resolver tests run with the feature off/on and prove the auxiliary key
  never enters translation choices. Manifest tests check revision, all seven
  reused runtime pins, nine-file totals and no bundled-notice lookup for native
  models. Existing native/worker URL and lifecycle regressions remain intact.
- A separate explicit external-pack test **passed** using the cached original
  pinned files in a temporary mock-app data root. It exercises actual command
  verified reuse, status, altered/missing notice rejection, install/remove
  contention under the runtime lease key, removal and preservation of a nearby
  synthetic document. It copies files locally and does not download weights;
  HTTP proxy endpoints were set to an unavailable local endpoint during the run.
  This is command/storage integration, not a WebView/device smoke test.

Repeat the external test with the original pinned pack outside the repository:

```sh
PAPERCUT_ALIGNMENT_ASSETS_DIR=/path/to/pinned-alignment-pack \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --no-default-features --features native-translation-alignment \
  pinned_auxiliary_commands_verify_reuse_notices_and_remove_under_the_runtime_lease_key \
  -- --ignored
```

Ordinary test runs leave it ignored and use no model downloads. Full Rust
library suites passed **478 tests with features off** (four ignored), **490 with
tokenizer/auxiliary admission** (six ignored) and **494 with the internal runtime
feature** (eight ignored). The explicit external command test above passed
separately. Formatting/diff checks passed; no new dependency, feature default,
model-catalog pin, packaged build, CI or device qualification is included.

**Next:** review auxiliary admission and notice delivery, then implement the
application's two-model ownership/cancellation/save lifecycle. Exact-piece
planning, word-link extraction/projection, mandatory auxiliary-scope success,
held-out semantics and resource/platform qualification remain separate gates.
Benchmarks, broader smoke and the app version bump remain deferred.

## Stage 3 two-model job lifecycle — 2026-10-20

**Internal ownership/cancellation integration, not formatting enablement.** The
shared runner now owns a job scope in `runner/lifecycle.rs`. Normal native and
Bergamot starts still load only their translator. With the default-off
`native-translation-alignment-runtime` feature, an internal loader can attach
one installed alignment session to that same job. No command argument, model
selector entry, shipping flag, cache format, dependency or version was added.
Public starts do not call that alignment hook until strict mapping admission is
implemented; merely compiling the feature or installing the pack changes no
document's formatting path.

### Ownership and admission

- The existing cache/job claim and translator file lease remain held throughout
  source planning, native loading, batches, final validation and saving. The
  optional alignment session owns the same auxiliary lease as install/remove.
  Contention rejects immediately; failed auxiliary admission unwinds its claim
  without releasing the translator early. Any failed overall load releases the
  whole job scope, allowing retry.
- Installed alignment admission reads the full nine-file inventory, including
  both notices, under the auxiliary lease. Graph/tokenizer bytes come from the
  verified snapshots used by the existing runtime. Raw seven-file developer
  probes remain separate. No metadata-only status check authorizes a load.
- The loader may reuse the attached session within a job. The native translator
  is destroyed before the scope; the alignment session is destroyed before its
  auxiliary lease and cancellation registration, then the translator lease is
  released. Cancellation cleanup precedes release of the cache/job claim, so an
  exiting attempt cannot clear a same-cache retry's cancellation.

### Cancellation and save boundary

- Alignment registration, cancellation and save admission use the existing
  cancellation-then-activity lock order. Cancellation before registration
  rejects it; cancellation after registration reaches that job's monotonic
  native handle. Other jobs' handles are unaffected. Native termination runs
  outside the application locks; there is no polling thread or waiting queue.
  Registration drops on every load failure and job exit; retries get a fresh
  handle rather than stale native termination state.
- The runner checks cancellation before/after model loading and after native
  batch inference. A completed late batch is discarded before validation,
  caching or rendering. A cancellation after the legacy phrase-probe phase also
  prevents starting the main batch. This does not introduce preemption inside
  native translator calls or rewrite the legacy phrase-probe algorithm.
- The final renderer/validation callback still checks the engine's live-owner
  admission and then calls `claim_save`. Cancellation accepted before that
  cutoff prevents durable writes, including a fully cached retry. Once saving
  is admitted, cancellation is rejected and does not terminate the aligner.
  Existing atomic writer/rollback behavior and saved variant identities remain
  unchanged. ORT session creation is still uninterruptible; a cancellation
  during creation discards the new session when the call returns.

### Regression receipt and limits

Ordinary synthetic tests exercise the real shared loader/runner/cache/renderer
and SQLite writer with a deterministic translator. They cover failed loads,
cancellation during loading and inference, pre-save rejection, a failing SQLite
transaction, successful saving, model claims during engine destruction, late
reply cache rejection and clean retry admission. State tests cover both
registration/cancel orderings, concurrent registration/cancellation, duplicate
registration, job isolation and the save-first cutoff. Missing/busy auxiliary
loads release their registrations and leave translator ownership intact.

An explicit ignored integration test uses the externally cached pinned INT8
graph and official shared ORT library, copying the pack plus bundled notices to
a temporary app root. A deterministic translator runs through the real writer
while the actual aligner loads, infers and is reused. It checks both model
leases, translator load failure after aligner admission, late load/batch
cancellation, pre-save cancellation, saved-HTML byte stability, retry without
duplicate variants and missing-notice rejection. This is lifecycle/storage
integration, not actual translation quality, word-link/projection correctness,
packaged-device smoke or a performance benchmark. No model weights or private
sample documents are committed or downloaded by these tests.

The existing explicit native-session test additionally sends cancellation
through application state to registered ORT run options and verifies native
termination plus rejected save admission. It cancels before execution; it does
not establish mid-kernel cancellation latency. Both explicit external tests
passed locally with the pinned INT8 pack and official ORT **1.23.2** shared
library; the application test used unavailable local HTTP proxies to prevent
unexpected network delivery.

Full Rust library suites passed **480 tests with features off** (four ignored),
**492 with tokenizer/auxiliary admission** (six ignored) and **499 with the
internal runtime feature** (nine ignored). The two external tests above passed
separately; ordinary runs skip model/runtime-dependent tests. Formatting/diff
checks passed. No fresh packaged build, CI or device qualification was run.

Reproduce the application lifecycle test using an external pinned pack and the
official shared runtime:

```sh
PAPERCUT_ALIGNMENT_ASSETS_DIR=/path/to/pinned-alignment-pack \
ORT_DYLIB_PATH=/path/to/official/libonnxruntime.so \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --no-default-features --features native-translation-alignment-runtime \
  installed_two_model_runner_reuses_session_cancels_and_preserves_saved_variants \
  -- --ignored
```

**Next:** review this lifecycle slice, then Stage 4 exact-piece/literal-island
planning. Word-link extraction, strict source-owned projection, mapping/cache
integration, mandatory auxiliary-scope success, numeric/held-out semantics and
runtime redistribution/resource/platform qualification remain open. Candidate
enablement, benchmarks, broader smoke and the app version bump stay deferred.

## Stage 4 exact pieces and literal islands — 2026-10-20

**Internal bounded planning and provider execution, not formatting enablement.**
`inline_markup/provider/pieces.rs` reuses `ProtectedSource`/`ProtectedPlan` and
the stable public segment ID, source block ordinal and scalar start. It does
not introduce another DOM parser, identity tree, translation selector, model
manager, serialized reply/cache field, dependency or build flag. Public runner
starts keep their current text/worker path; this new planner has no command or
writer caller. Stage 5 projection/mapping admission is still required.

### Exact source ownership and model pairs

- One retained source plan owns all original tags, attributes, literals and
  parent relationships. Exact scalar ranges partition its text into prose,
  immutable literal islands and whitespace joins. Emphasis alone never causes
  an inference boundary. A fitting sentence stays whole across nested styles.
- Literal content is copied through the retained plan and never enters either
  tokenizer or translation inference. No invented placeholders or marker echo
  are used. This intentionally reduces model context across a literal. A
  literal-only unit needs no model call and grants no prose-mapping approval.
- Zero-width notes/images/empty identities retain source ID, parent, source
  offset and exactly one unit owner. A marker on a cut belongs to the preceding
  unit; a marker at source start belongs to the first. Interior positions retain
  their exact source coordinate. This is **source ownership**, not a guessed
  target placement; semantic/quote/punctuation attachment evidence and actual
  source-owned DOM reconstruction remain later gates.
- The planner tests the actual translator and aligner admission functions before
  model inference. Only oversized prose is split, preferring Unicode sentence,
  then clause, then whitespace boundaries. Balanced cuts bound repeated
  tokenization. A word/indivisible prose unit that cannot fit rejects; it is not
  chopped into characters. Authored identities remain in the single retained
  tree across internal pieces. A supplied outer segment that already cuts an
  authored ID or literal rejects rather than clipping it.
- Boundaries are grapheme-safe, with exact half-open scalar coordinates. Source
  whitespace joins are explicit units, not reconstructed from sentence counts.
  Leading/trailing whitespace and decoded targets are retained exactly. The
  returned translation borrows its producing source plan and records one exact
  target range per source unit, including copied literals and joins. Providers
  translate each admitted input once; there is no hidden subdivision/join or
  independent splitting of an oversized target against unrelated source text.

Engineering bounds are 32 KiB source/combined target, 128 total units including
joins, 900 scalars per prose piece, at most 256 planning probes per prose island
and 16 inference inputs per call. Existing protected-tree/payload bounds also
apply. The actual tokenizer limits can be lower. Every source is re-admitted
immediately before inference; every target is checked afterward. Unknown or
normalization failures reject, while only size overflow is eligible for source
subdivision. A bad count, empty/oversized target, grapheme-crossing join or
cancellation returns no completed translation. No cache/write occurs here.

### CTranslate2 and alignment tokenizer seam

The two-method `ExactPieceTranslator` seam owns source-token admission and one
target per actual source input. CTranslate2 implements it only for the two pinned
French/Spanish OPUS-MT IDs. Future providers must implement their own framing,
decoder-completion and target-limit checks; HY-MT2 is not implicitly admitted.

The adapter now uses the maintained `ct2rs::sys::Translator` API with the existing
tokenizer. This exposes actual generated token sequences before decoding and
removes the high-level wrapper's duplicate tokenizer. Normal jobs retain their
existing splitter, native defaults, unknown replacement, decoding and output
joining. The new strict path includes source EOS in the existing 448-token
budget, disables native input truncation, requests returned EOS and rejects
missing EOS, empty output or generation beyond Marian's 512-position bound.
It removes only the verified terminal EOS before decoding; output token counts
are not guessed by re-tokenizing translated text. Actual aligner target
admission then applies its own 256-token limit including framing.

`CandidateTokenizer::text_fits` treats only size overflow as splittable. It
preserves unknown/reserved-token and normalization/provenance rejection. An
existing `AlignmentSession` exposes the same tokenizer check without creating
another graph/session. This slice does not implement word-link extraction or
make the unresolved auxiliary-word scope case pass.

### Regression receipt and reproduction

Synthetic regressions cover complete source/target coverage, stable public IDs,
unclipped identity ownership, nested inline styles, literal exclusion,
literal-only input, notes exactly on a cut, batching/work limits, token-dense
source, both framing budgets, the 900/901-scalar boundary, Unicode scalar versus
byte offsets, combining/Arabic marks, whitespace joins, cancellation, malformed
reply counts, target overflow and native EOS/position-limit rejection. Actual
tokenizer-library tests include protected unknown literals and strict rejection
of unknown prose; they are not semantic alignment tests.

An explicit native regression passed with the existing pinned French and
Spanish packs. Every file was verified against the current manifest; packs were
downloaded only to `/tmp` for correctness validation, outside the repository.
The normal adapter produced exactly the same short-sentence outputs as the
previous maintained high-level adapter with identical options. Strict outputs
also matched; both real tokenizers admitted the literal/note planning cases.
This is native correctness evidence, not a quality, speed/RAM benchmark or app
smoke test. The native backend's existing float16-to-float32 CPU fallback warning
was unchanged; low-power resource qualification remains open.

Reproduce with externally cached packs in model-ID subdirectories and the
original seven-file pinned alignment pack:

```sh
PAPERCUT_CT2_MODELS_DIR=/path/to/ct2-models \
PAPERCUT_ALIGNMENT_ASSETS_DIR=/path/to/pinned-alignment-pack \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --no-default-features \
  --features native-translation-ctranslate2,native-translation-alignment-runtime \
  pinned_native_legacy_parity_and_exact_piece_outputs -- --ignored
```

Ordinary tests skip this external-pack case and download no models. Private
sample documents and weights are not committed. No new benchmark tooling was
added. Fresh CI, packaged/mobile acceptance and wider model-quality acceptance
are not claimed.

Final full Rust suites passed **491 tests with features off** (four ignored),
**512 with the alignment runtime** (nine ignored), and **511 with CTranslate2
plus alignment runtime** (ten ignored). The explicit pinned-model parity and
planning test above passed separately for both language pairs. The native-only
translation suite and combined-feature compile check also passed. Formatting,
diff and documentation-link checks passed. Existing feature-off unused-import
and native CPU compute-type warnings are not new acceptance failures.

**Next:** review Stage 4, then Stage 5 reference-compatible word-link extraction
and conservative projection over these exact pairs. Stage 6 attachment/DOM
reconstruction and Stage 7 mapped-cache/save integration remain separate work.
Mandatory auxiliary-scope semantics, numeric/held-out acceptance and packaged
runtime/resource/platform qualification still block public alignment enablement.
Benchmarks, broader smoke and the app version bump remain deferred.

## Stage 5 word links and conservative projection — 2026-10-20

**Internal implementation; public formatting remains disabled.** This extends
Stage 4's exact pairs, without changing engines, model pins, dependencies,
shipping flags, commands, cache schemas or saved documents. It does not complete
the Stage 5 mandatory positive scope gate, reconstruct HTML or enable the
candidate. Benchmarks, broader smoke and the version bump remain deferred.

### Extraction and scope ownership

- `alignment/extract.rs` independently implements the pinned
  [Awesome-align guide-head rule](https://github.com/neulab/awesome-align/blob/5f150d45bbe51e167daf0a84abebaeb07c3323d1/awesome_align/modeling.py):
  unscaled embedding dot products, stable row/column softmax, strict `> 0.001`
  intersection, then sorted/deduplicated tokenizer word IDs. No cosine scaling,
  argmax replacement, threshold tuning, adjacency repair, retries or fallback
  engine is added. The reference BSD-3-Clause notice is retained beside the
  Rust module; no Python implementation/package is shipped or added as a
  dependency. The model pack's existing nine-file notice inventory is unchanged.
- Graph inputs still include framing. Extraction excludes framing from both
  softmax denominators and returned word indices. Actual raw-tokenizer evidence
  is checked before arithmetic and before constructing `WordLinks`; source and
  target borrows retain the exact strings. Shapes, finite values, token counts,
  word IDs, link indices/duplicates and the existing 4,096-link ceiling are
  checked. Overflowing dot products reject. Cancellation is checked during
  extraction as well as before/after the existing native calls.
- `contract/projection.rs` derives minimal target word runs and reuses the
  existing source coverage, mixed-ownership, whole-word, hierarchy and identity
  checks. Only whitespace between adjacent owned words joins a run. Unlinked
  punctuation receives no guessed style; disjoint runs never become a broad
  hull. Whole-unit uniform ownership can include generated auxiliaries without
  guessing individual grammatical scope. Partial ownership still requires
  complete evidence. Extraction and projection have separate policy identifiers
  for later compatibility admission; no mapping is persisted in this stage.
- `pieces/projection.rs` aligns each actual prose pair sequentially, then checks
  the combined ranges against the single retained source tree. Piece-local
  coordinate views do not clone DOM attributes or authored identities. Copied
  literal/whitespace units use their exact source-owned offsets and never enter
  either model. The combined result is capped at 512 ranges before the final
  pairwise hierarchy check. Authored identities must remain representable once;
  a disjoint style can repeat only when the source permits it.
- `AlignmentSession::project_translation` reuses one job session and drops each
  pair's embeddings before the next. The final projection borrows the exact
  translation/source plan; no completed projection escapes on partial failure
  or cancellation. Public starts still do not invoke this path. Stage 6 must
  supply attachment/quote evidence and source-owned DOM reconstruction; any
  zero-width attachment currently returns `AttachmentRequiresEvidence` before
  alignment. Stage 7 still owns mapping-aware cache, resume and save admission.

Unknown/provenance failures, missing styled-source links, unaligned lexical
targets, mixed/fused ownership, unsupported identity splitting and hierarchy
conflicts remain typed rejections. A declared `Auxiliary`, `FusedMeaning` or
`Attachment` blocker rejects before inference. Absence of a declared blocker,
complete word coverage and a structurally valid projection are **not semantic
certification**; the deliberately wrong complete-graph counterexample remains
a regression. No grammatical detector, confidence calibration or semantic
completeness claim has been introduced.

### Regression and native-reference receipt

Ordinary synthetic tests cover two test translation providers yielding the same
shared mapping, nested/reversed/repeated words, many-to-many and disjoint scopes,
explicit versus generated auxiliaries, mixed ownership, partial words, composed
and combining Unicode, punctuation, copied literals, exact subdivisions, one
authored identity, stale/wrong pair binding, invalid graphs, range/link bounds,
non-finite arithmetic and cancellation. Small raw-tokenizer/embedding tests prove
framing exclusion and subword-to-word merging without loading a graph.

An explicit eight-pair French/Spanish test compares actual extracted links with
the unmodified pinned Python guide head on the **same native tensors**, then
checks separately annotated target scopes. Inputs are minimal synthetic prose,
not private documents. The cached INT8 pack is hash-admitted. This run used the
existing bundled Sherpa library reporting **ORT 1.27.0**, SHA-256
`026c7d5c609323fb16506dbc3cce801bcdffdd7566fdba49a50727e2e1e881ca`.
All eight reference link sets match. **Five intended scopes map; three remain
unsupported and reject safely:** the two generated-subject/auxiliary examples,
and `El libro rojo` → `The red book`, where this runtime omits the adjective
link. Intended target scopes remain in the test even for rejected cases;
safe rejection is not counted as positive semantic acceptance.

The isolated Python ORT **1.23.2** comparison retained that Spanish adjective link
on this short input. That version exists only in a disposable research environment,
not as a second application runtime. The current Linux app uses its one shared
bundled ORT **1.27.0**; Stage 5 adds no runtime dependency or upgrade. Future
runtime upgrades must coordinate the shared bundle and validate TTS, Arabic
preprocessing, conceptual search and alignment together rather than giving the
aligner its own runtime. Applying the pinned guide head to the application's tensors
confirmed inference/runtime variation rather than a Rust extraction-rule
mismatch. This adds a concrete runtime-qualification gap; do not tune thresholds
or claim cross-runtime INT8 equivalence to hide it. Existing wider auxiliary,
numeric and held-out semantic gates remain unmet.

A second explicit regression uses both pinned CTranslate2 packs, their actual
strict generated targets (`A Book`/`A book` on these minimal French/Spanish
inputs), the real alignment tokenizer/graph and the shared projection path.
Both map the book word to scalars `2..6`; this native test also checks an
already-cancelled call. It does not exercise late cancellation; the deterministic
regressions in the follow-up below cover that boundary separately.
These are correctness checks, not quality/performance benchmarks or app smoke.
Ordinary tests download no weights. External probes/tensor dumps remain in
`/tmp`, with no private article, sample-derived fixture or custom build script
added to the repository.

Reproduce both explicit tests using the existing bundled shared runtime and
externally cached, verified packs:

```sh
PAPERCUT_CT2_MODELS_DIR=/path/to/ct2-models \
PAPERCUT_ALIGNMENT_ASSETS_DIR=/path/to/pinned-alignment-pack \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --no-default-features \
  --features native-translation-ctranslate2,native-translation-alignment-runtime \
  translation::alignment::extract::tests:: -- --ignored
```

The frozen native link receipt is specific to the runtime above; a different
runtime must be evaluated rather than silently changing its expected links.
The application uses its shared initializer's `libonnxruntime` lookup; setting
`ORT_DYLIB_PATH` alone does not override that explicit initializer.

Final full Rust suites passed **503 tests with features off** (four ignored),
**528 with the alignment runtime** (ten ignored), and **527 with CTranslate2
plus alignment runtime** (twelve ignored). The two explicit native tests above
passed separately using cached assets and unavailable network proxies. Formatting,
diff, new documentation anchors and removal of temporary probe hooks were checked.
No fresh CI, packaged/device build, quality/resource benchmark or smoke is claimed.

**Next:** review this internal slice. Then Stage 6 can consume structurally
checked scopes for source-owned hierarchy/literal/attachment reconstruction,
while keeping the unresolved cases explicit. Do not enable public strict
formatting or build release integration around an assumed semantic pass.
Mandatory positive scope, held-out/numeric acceptance, runtime notices and
packaged resource/platform qualification still block enablement.

### Stage 5 late-cancellation regression follow-up — 2026-10-20

The Stage 5 review found that the native cancellation test cancelled before
projection began. It could not catch removal of the post-work guards. This
follow-up corrects that evidence claim and tests the actual late boundaries.

The existing typed cancellation check now passes through the shared
`Translation::project` path. It checks before work, between units and after final
hierarchy validation, before constructing the completed projection. The native
adapter supplies its existing `Cancellation` handle to this same path; there is
no separate test implementation, timer, thread-local hook or new cancellation
service. Extractor arithmetic, token admission, policies and native sessions
remain unchanged.

Four deterministic ordinary regressions use synthetic text and the same valid
tokenizer/embedding fixtures as the successful controls:

- Cancel after the first extraction row and return no partial word links.
- Cancel after every row and the softmax intersection, discarding completed links.
- Cancel after one successful pair and verify the next pair is never aligned.
- Cancel as the final pair returns valid links, then discard the projection after
  all ranges and the authored identity have been assembled and validated. The
  same input succeeds without cancellation and retains one enclosing identity.

These tests need no weights or shared runtime. The existing explicit native
tests still verify the adapter wiring and real French/Spanish output separately.
Removing each corresponding row, extraction-final, between-unit and
projection-final guard in isolation made its new regression fail. All guards
were restored before final validation; the temporary mutation driver/logs live
only in `/tmp`, not repository tooling.

Final full Rust suites passed **505 tests with features off** (four ignored),
**532 with the alignment runtime** (ten ignored), and **531 with CTranslate2
plus alignment runtime** (twelve ignored). The two explicit cached native tests
passed separately with unavailable network proxies. Formatting, diff and guard
restoration checks passed. No fresh CI or packaged/device acceptance ran.

Known unresolved scopes, attachment admission, public capability gates, caches,
saved documents, runtime/model versions and shipping flags remain unchanged.
Benchmarks, smoke and the version bump remain deferred.

### Stage 6 source-owned mapped reconstruction — 2026-10-20

**Scope.** An internal `Projection::restore` consumer now reconstructs checked
exact-piece output through the existing `ProtectedPlan` source renderer. No
public command, selector, runner, persisted fragment, cache epoch, app version,
model file or native runtime changes. The auxiliary candidate remains disabled
for public translation; HTML/EPUB remain the supported document formats.

**Separation of concerns.** `WordLinks::attachment` derives bounded lexical
boundary evidence. Piece projection combines those point mappings with copied
literal/whitespace coordinates and the original hierarchy. The new
`protected/mapped.rs` adapter converts validated scalar ranges into escaped,
opaque transport; it never accepts provider tags or attributes. The existing
protected renderer alone emits retained source tags, attributes and literal
payloads. Its byte budget and strict identity validation are shared with worker
reconstruction. Strict maps bypass the legacy worker note-repositioning and
best-effort formatting paths entirely.

**Hierarchy and literals.** The complete source tree is checked again before
reconstruction. A boundary sweep opens/closes only source-owned identities;
disjoint style-only ranges may repeat, while links, authored IDs and literals
cannot reopen or clone. An outside attachment may interrupt repeatable emphasis
without becoming its child; an unrepresentable identity split rejects. Copied
literals must exactly match their retained source scalar slice, are replaced by
their original DOM payload once, and never enter either model. Opaque tables
retain their original cells and nested assets: this does **not** add translated
table-cell planning. Source block order/context and external owned asset bytes
remain the existing DOM/writer's responsibility, not an aligner output.

**Attachment policy.** Zero-width notes, backreferences, images and required
empty ancestor wrappers retain their one Stage 4 owner at piece cuts. In a
translated piece, the nearest lexical words on each side must have unique
reciprocal target anchors. The target gap must contain the same punctuation
scope, allowing documented opening/closing quote glyph equivalents and closing
quote/terminal-punctuation movement. Quote containment takes precedence when a
period moves across a closing quote, as in the existing punctuation rule.
There is no sentence-count zipping, label matching, relative-position estimate,
retry or phrase-probe fallback. Missing/fused anchors, reversed boundaries,
extra target words, changed punctuation, inside-word positions and incompatible
parents remain explicit `AttachmentRequiresEvidence`/hierarchy rejections.
Attachments in copied units use their exact unchanged coordinates.

**Text and resource contract.** The exact target already contains copied literal
text; source-owned note labels and image markup are excluded from its normalized
reader text. After reconstruction, the shared reader extractor must equal that
target exactly. A target whose whitespace/control characters normalize to a
different reader string rejects rather than silently changing mapped offsets.
The adapter caps escaped synthetic transport at eight times the 32 KiB text
bound, reuses the 512-range/128-code bounds, and the final renderer charges all
repeated source bytes against its existing expansion allowance. Source payloads
are never amplified without charge. Cancellation is checked before and after
reconstruction; a late result is discarded. Cooperative checks do not promise
native-call preemption or replace Stage 7's atomic save guard.

The initial Stage 6 policy identity was
`word-ownership-v2:whole-unit-or-complete-links:no-hull:unique-anchor-gap-v1:source-dom-v1`.
The parent-boundary follow-up below supersedes it for strict maps.
The worker mapping identity and existing stored variants are unchanged. Stage 7
must bind this strict policy and source/target identities into mapped cache
admission and use the retained source block's parsing context when inserting
fragments. It must not route strict output back through worker note adjustment
or a legacy formatting fallback.

**Regression evidence.** Synthetic gold links/maps (not model-quality evidence)
cover nested/disjoint styles, reordered source links, compact authored identities
across actual pieces, copied code/whitespace, forward/reverse/nested quote notes,
duplicate note labels/source order at a cut, an empty styled link around an
owned image, standalone image/line-break/backreference restoration, and an
opaque mixed-content table with literal code and an owned image. Assertions
check exact styled words, source ancestor chains, exactly-once IDs, unchanged
URLs/asset keys and readable text. Target strings resembling tags/attributes
remain escaped text and cannot become reader markup. Rejections cover
stale/malformed mappings, crossings, ID splits, changed literals,
grapheme cuts, unresolved attachment evidence, excessive output/attribute
expansion and reader-text normalization changes. New reconstruction cancellation
tests cover both entry rejection and suppression after successful work.

The existing explicit cached native regression suite now also reconstructs
the actual OPUS-MT French/Spanish minimal outputs with the existing aligner on
the bundled runtime. This is a correctness check of those cases, not a broader
quality/resource benchmark. The three previously recorded unmet native semantic
scopes remain unresolved; no extraction threshold or runtime was changed.

**Validation:** full offline Rust library suites passed **519 tests with features
off** (four ignored), **546 with the alignment runtime** (ten ignored), and
**545 with CTranslate2 plus alignment runtime** (twelve ignored). The two
explicit cached native regressions passed separately with unavailable network
proxies, including exact reconstructed French/Spanish emphasis and casing.
Use `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib
--no-default-features`. Add `--features native-translation-alignment-runtime`
or `--features native-translation-ctranslate2,native-translation-alignment-runtime`
for the other suites; the native asset/environment invocation remains in the
Stage 5 receipt. Formatting
and diff checks passed. No fresh CI, packaged/device acceptance or quality/RAM/
speed benchmark is claimed.

**Next:** review this internal slice, then implement Stage 7's mapping-aware
batch cache/resume and atomic save integration while keeping public capability
disabled until semantic, numeric/held-out, runtime notices and packaged
resource/platform gates pass. Invalid strict output currently returns no
reconstructed result and has no cache/writer caller; durable failure/no-save
acceptance belongs to Stage 7. Benchmarks, broader smoke and version bump remain
deferred.

### Stage 6 leading attachment parent boundaries — 2026-10-20

The Stage 6 review reproduced a false `Hierarchy` rejection with
`Un <span id='phrase'><a role='doc-noteref'>1</a>deux</span>` and exact one-to-one
links to `UN DEUX`. Pending source whitespace left the note at scalar position
2, while its nonempty parent's lexical map started at 3. A leading image inside
a nonempty styled link hit the same conflict. Both new positive regressions
failed against the original implementation before this fix.

The shared punctuation-gap helper now returns inclusive scalar **positions**
within the same whitespace run after the checked punctuation boundary.
Word-link validation still requires unique reciprocal lexical anchors and the
same punctuation/quote evidence. A range of positions is not permission to move
across punctuation, quotes, words or literal contents. DOM text/style ranges
remain half-open scalar ranges.

Piece projection retains this bounded evidence until all nonempty parent scopes
are assembled, including parents that start in a later piece. Source preorder
then resolves empty parents before their children. An attachment selects the
earliest position in the intersection of its checked gap and its source-owned
parent's mapped range. No intersection or more than one parent occurrence
rejects; there is no nearest-parent guess, hull expansion, ID cloning or changed
word formatting. Root attachments retain their previous preferred position.
Copied-unit attachments retain their exact single position. The final combined
hierarchy/range validation and late cancellation checks remain mandatory.

Synthetic regressions exercise leading notes in compact/nested authored wrappers
and leading images in nonempty styled links, with both a styled caption and an
empty style around the image. They cover an intact piece and an actual split,
Latin/Unicode/emoji prefixes, exact readable text, first-child placement,
unchanged note/link URLs and asset keys, and exactly-once IDs. A contradictory
gold graph places the parent outside the attachment gap and must still reject
for both notes and images, rather than moving either marker across punctuation
or target words. Existing quote, literal, disjoint-style and cancellation tests
remain in the shared suite. These are structural regressions, not new model
quality evidence.

**Validation:** full offline Rust library suites passed **522 tests with features
off** (four ignored), **549 with the alignment runtime** (ten ignored), and
**548 with CTranslate2 plus alignment runtime** (twelve ignored). The two
explicit cached native regressions passed separately with unavailable network
proxies. Formatting and diff checks passed. No fresh CI, packaged/device
acceptance or quality/resource benchmark is claimed.

The strict policy identity is now
`word-ownership-v3:whole-unit-or-complete-links:no-hull:parent-aware-unique-anchor-gap-v2:source-dom-v1`.
Public runner/selector/cache/writer paths, worker mapping version, model/runtime
versions, saved variants and owned asset files remain unchanged. The candidate
is still disabled; the previously unmet semantic scopes and all public
acceptance gates remain open. The focused review below completed the review
handoff to the Stage 7 implementation recorded below.
Benchmarks, smoke and the app version bump remain deferred.

### Stage 6 parent-boundary review — 2026-10-20

Reviewed commit `2c5b33b`, its callers, source-plan/piece ownership, checked
word-link gap evidence, combined hierarchy validation and source-owned renderer.
**No actionable findings in this fix.** The expected load is one foreground
translation job, with existing code/range/target-size limits. The review traced
leading attachments across intact and split pieces, empty and nonempty parent
chains, copied literals, Unicode positions and contradictory punctuation scopes.
The allowed positions stay inside the checked whitespace gap and one mapped
parent occurrence; the final hierarchy and rendered-text checks still apply.
Legacy worker note movement and public cache/writer paths are unchanged.

Fresh offline translation suites passed **245 tests with features off** and
**271 with CTranslate2 plus alignment runtime** (eight ignored). The two cached
native French/Spanish alignment regressions passed separately with unavailable
network proxies. Formatting and diff checks passed. Synthetic ownership tests
and these small native cases do not establish general formatting/model quality.

**Handoff:** the Stage 7 implementation below follows this review. Stale
translated text is not a validated map. The previously unmet semantic scopes
and public acceptance gates remain open. No fresh CI, packaged/device acceptance,
benchmarks, broader smoke or app version bump is claimed by this review.

### Stage 7 mapped cache, resume and atomic save — 2026-10-20

The internal exact-piece path now reaches the existing job runner, batch cache,
DOM renderer and SQLite/filesystem writer. A job whose internal load closure
explicitly calls `Job::load_alignment` uses its one retained auxiliary session
and cancellation registration. Public native and worker starts do not call this
hook; build flags, model selectors and public formatting claims are unchanged.
No second engine, retry ladder or storage pipeline was added.

**Batch admission and resume.** Each strict batch rebuilds source-owned plans
from the current reader DOM, then translates, aligns and reconstructs all its
segments before any strict record is checkpointed. An unresolved or
late-cancelled segment leaves the entire current batch uncommitted; earlier
validated batches remain reusable. The shared runner checks cancellation around
native work, reconstruction, checkpoint admission and the existing atomic save
guard. Native calls still need to return before their late results/resources can
be discarded. Completed checkpoints may survive a later job cancellation;
cancellation before the save cutoff prevents a durable variant.

Strict records live separately from legacy segment/probe memory and worker HTML.
They contain bounded source/target text, exact source units and decoder target
ranges, checked node maps and version identities, never embeddings or tensors.
Admission binds the source structure/literals, block parsing context, segment
location, exact target, translator pins/decoder adapter, aligner revision/runtime
build, tokenizer, normalization and extraction/projection policy. The pure
binding contract is now version **2** to include the block context. The worker's
`protected-codes-v6` transport/cache identity is unchanged.

Receipt checksums detect accidental local corruption; they are not an
authentication boundary against someone who can edit the app's data directory.
Every cache hit rechecks bounds, exact bindings, piece partitions/tokenizer
admission, copied literals and source-owned reconstruction. A valid exact
translation can be reused when only alignment versions change, but its map must
be recomputed. Changed translator/normalization/source structure/context or
incompatible piece boundaries requires fresh exact translation. Legacy text or
phrase hints never certify strict output. Corrupt receipts are discarded.

**Rendering and storage.** Strict fragments are rebound to the cloned source
DOM and parsed in that block's HTML context. The save request declares its
strict profile independently of its fragments, so losing every receipt cannot
downgrade it to legacy output. A missing/mixed/changed receipt
fails before save admission; worker note repositioning, phrase probes and
plain-text fallback are bypassed. The existing quality checks, save cutoff,
transaction/rollback and owned-asset writer still apply. Installed-model/runtime
identities are recorded in `settings_json.alignment`. Strict variants include
these identities in their derived ID: retries replace only the same strict
profile, while legacy/differently aligned variants retain their IDs, files and
search rows. No migration rewrites originals, older variants, audio or read-along
metadata. Tables retain the existing importer/opaque-island behavior; this is
not a new table-cell translation feature.

**Regression coverage.** Synthetic gold links exercise HTML/EPUB reader plans,
real cache files and SQLite/file writes: nested/leading notes, images inside
styled links, unique authored IDs, literal code, list context and owned asset
bytes; cache-only resume; policy/tokenizer/aligner versus translator/source/context
invalidation; corrupt targets/maps/piece boundaries; partly failed batches;
late translation/alignment cancellation, cached pre-save cancellation and the
shared save cutoff; missing/mixed receipts; transaction failure and retry;
legacy variant and existing audio/timing-file preservation. Existing shared
worker-loss and model ownership/removal tests remain applicable.

**Validation:** full offline Rust library suites passed **531 tests with features
off** (four ignored), **558 with the alignment runtime** (ten ignored), and
**557 with CTranslate2 plus alignment runtime** (thirteen ignored). Four explicit
cached native regressions passed separately with unavailable network proxies:
the reference extraction/scopes, actual French/Spanish piece projection,
installed two-model lifecycle and installed French/Spanish runner/save/resume.
The latter also verifies that ordinary command starts remain translator-only
when the auxiliary is compiled and installed. Formatting and diff checks passed.
These small native cases and gold ownership graphs do not close general
semantic/resource acceptance. No fresh CI or packaged/device acceptance is claimed.

**Handoff:** the focused review below completes Stage 7's review before Stage 8
truthful capability/progress and setup UI work. Keep public strict admission disabled: the three previously
unmet native semantic scopes, mandatory positive formatting gate and held-out,
numeric, runtime-notice, resource/platform acceptance remain open. Benchmarks,
broader smoke and the app version bump remain deferred.

### Stage 7 lifecycle review — 2026-10-20

Reviewed commit `8592a1f`, the connected runner/load closures, exact-piece and
receipt admission, protected DOM reconstruction, cache read/write paths,
cancel/save ownership and the shared derived-document transaction/rollback.
**No actionable findings within the internal Stage 7 scope.** Expected load is
one foreground translation job with the existing token, code, range and target
bounds; this is not a measured large-book/mobile resource acceptance.

The review traced exact-text reuse versus mapping invalidation, changed
source/context/version bindings, corruption and incomplete-batch rejection,
late native results, model ownership through save/failure, and complete receipt
loss. The independent strict request profile prevents a missing receipt from
selecting legacy rendering. Strict variant identities preserve legacy and
different-profile results; same-profile retry and cancellation retain the
existing writer cutoff and rollback semantics. Root parsing context is retained
without changing worker transport identities. Ordinary public load closures do
not request alignment, including when the auxiliary is compiled and installed.

**Fresh validation:** offline translation suites passed **254 tests with features
off** and **280 with CTranslate2 plus alignment runtime** (nine ignored). The two
explicit installed-model regressions passed separately with unavailable network
proxies: French/Spanish native runner/save/resume plus the two-model lifecycle
and cancellation probe. The latter uses identity decoding to isolate lifecycle;
it is not translation-quality evidence. Formatting and diff checks passed.

**Handoff:** the Stage 8 integration below follows this review. The strict
capability remains unavailable until semantic and public acceptance gates pass;
test-only ready fixtures do not approve the candidate. No new CI, packaged/device
acceptance, benchmarks, broader smoke or app version bump was claimed by this review.

### Stage 8 gated formatting setup and progress — 2026-10-20

**Scope.** Reuse the existing workbench, installer/status/removal commands and
job lifecycle. No new engine, aligner picker, automatic download, runtime upgrade
or shipping feature flag is introduced. The trusted auxiliary remains outside
`capabilities.models` and the engine selector. Its status includes verified
weights/tokenizer/notices; possession alone does not advertise formatting support.

**Admission and setup.** Capabilities now include a separate `formatting` payload
(`available`, reason, optional auxiliary model ID and download bytes). Compiled
CTranslate2 plus the alignment runtime on desktop are prerequisites; the shared
`FORMATTING_ACCEPTED` constant remains **false**. Default and worker-only builds
explain runtime unavailability; combined internal builds explain pending
acceptance. Ready capability fixtures exist only in tests. An older backend
without this payload stays conservatively unavailable.

The workbench offers the named pack's explicit install and confirmed removal via
existing handlers when admitted, explains its separate download/storage cost and
never installs automatically. Previously installed packs can still be removed
while admission is closed. Missing, corrupt or installing packs cannot enable
strict consent. The consent is bound to the selected French/Spanish OPUS-MT →
English pair, resets on an explicit engine/language change, and cannot quietly
downgrade to plain output when assets/capability disappear. The user can clear
the choice to continue with ordinary translation. Bergamot never receives this
auxiliary checkbox and is not selected as a fallback.

**Native boundary and lifecycle.** The optional `preserveFormatting` request
field defaults to false. Both native and Bergamot document commands enforce the
release/provider/pair gate before loading resources or opening a worker; crafted
true requests currently reject even with both packs installed. Internal runner
tests exercise the true flag through verified auxiliary loading, the existing
two-model ownership and strict receipts. Strict jobs use a distinct cache key;
plain keys and saved variants retain their existing identities. No aligner is
loaded by an ordinary public request.

Loading the auxiliary, translating, aligning and reconstructing now report their
actual phases through the existing event stream. Cached mappings still reconstruct
but do not pretend to run alignment again. Progress publication is followed by a
cancellation check before the next native/reconstruction operation; a cancelled
batch retains the last complete cache and saved variant. The UI localizes the
new phase labels, active messages and actionable release/pair, asset verification,
runtime, unsupported-unit and unsafe-mapping failures in all eight locales.
These errors add no private source-content logging. Strict final rendering
failures retain their own mapping error category before any write.

**Fresh validation.**

- Full frontend suite: **493 passed, one existing skip**; the final translation
  subset also passed **94 tests** after presentation cleanup. Tests cover disabled
  release/worker capabilities, absent/corrupt/installing/ready fixtures, explicit
  setup/removal confirmation, install/removal errors and retry, lost-pack blocking,
  engine changes, native cancellation/retry and localized progress.
- Full offline Rust library suites: **536 passed/four ignored** with features off
  and **562 passed/13 ignored** with CTranslate2 plus alignment runtime. The
  cancellation regression additionally covers each new phase notification.
- Worker-only native capability suite: **two passed**; it does not claim native
  alignment availability. Separate cached-asset tests: **two native runner/lifecycle
  regressions passed** (French/Spanish strict load/save/resume and cancellation),
  plus **one auxiliary command/status regression passed**, including absent,
  installing, verified, tampered/missing notice, in-use and removed states.
  These run with unavailable network proxies and pinned local assets; the
  identity-decoding lifecycle probe is not semantic-quality evidence.
- TypeScript, targeted ESLint, eight-locale parity, Rust formatting and diff
  checks passed. Full suites were rerun with local subprocess/socket permission
  because sandbox restrictions initially blocked unrelated existing loopback,
  CI-wrapper and Chromium layout tests.

**Handoff:** the focused review found two shared UI recovery defects, addressed
in the follow-up below. The existing auxiliary-word/missing-link semantic blockers,
numeric and held-out acceptance, runtime redistribution/platform delivery and
measured resource gates remain open. Compiled-feature detection is not proof that a packaged ORT runtime loads;
that remains a prerequisite to opening release admission. No new CI, packaged
WebView/mobile acceptance, benchmark, broader smoke or app version bump is
claimed. Private documents and sample-specific tests remain outside tracked code.

### Stage 8 download retry and terminal progress recovery — 2026-10-20

The focused review of `bb39949` found two P2 defects in the shared UI state
handling: a download that emitted progress before failing left its pack marked
installing, and a failed/cancelled model load could leave an animated loading
bar after the command had ended. Both also affected ordinary translation models.

**Fixes.** Failed installs now re-query the affected model's native status before
releasing the UI operation. This retains a verified older pack restored by an
unsuccessful repair. If status verification itself fails, the UI releases retry
but treats that pack as unverified; it never enables strict formatting from stale
readiness. The original install error remains visible. Finished installs ignore
queued progress, with suppression reset on explicit retry. The UI can still
observe an already-running native installation after refresh.

Translation command rejection now changes the matching job's progress to failed
or cancelled, even when a loader emitted no final event. Last checkpoint counts
and percentages remain intact; late progress cannot restart the ended animation.
Failed progress uses an existing translated label in all eight locales. Retry
still receives a new job ID and no stale progress. Unknown-outcome errors retain
their existing Library refresh and explanatory message; the UI makes no promise
that output was not saved.

**Regression evidence.** The hook tests now render the real workbench. They cover
progress followed by download failure, enabled retry clicked through the actual
button for both the auxiliary pack and ordinary model, renewed retry progress,
late events, failed status verification, restored valid assets and observing an
existing native install. Loading-model/loading-alignment each cover failure and
cancellation without a terminal native event, stopped animation, accessible
localized status, fresh retry and rejection after partial batch progress.
These use generic synthetic fixtures only.

**Fresh validation:** full frontend suite **503 passed/one existing skip**;
translation subset **104 passed**. TypeScript, targeted ESLint, locale parity and
diff checks passed. Two focused offline native admission regressions passed
separately with features off and CTranslate2 plus alignment runtime compiled.
Broader Rust/native integration counts in the earlier receipt are historical;
no Rust implementation changed in this follow-up.

**Next:** Stage 9 qualification starts with the investigation below. The native
release constant remains **false**; test-only ready UI fixtures do not enable
production formatting. No runtime, installer transaction, cache/storage format,
model catalog, dependency or app version changed. Benchmarks, smoke, packaged
acceptance and version bump remain deferred; no fresh CI result is claimed.

## Stage 9 auxiliary-word and missing-link investigation — 2026-10-20

**Evaluation only; no implementation or acceptance change.** The three Stage 5
unresolved examples are reproducible, but are not three instances of one bug.
The DOM renderer cannot recover correspondence absent from the alignment graph.
The current extraction rule matches its reference; its output is insufficient
for the required partial-formatting coverage. Keep `FORMATTING_ACCEPTED = false`.

### Method and limits

- Re-ran both existing ignored native extraction/projection tests against the
  current checkout: **two passed**, including the expected safe rejections.
  These exercise actual production session/extraction/projection code. A test
  passing because the expected rejection occurred does not pass its intended
  positive formatting requirement.
- Reused the previously frozen **24 scopes over 20 French/Spanish pairs**,
  without changing their fixed English targets or annotations. Added two
  synthetic short-adjective controls, one per language. All inputs and
  investigation tooling remain outside Git; no private article was consumed.
- A disposable native harness imported the current `ctranslate2.rs` unchanged
  and invoked `translate_exact`, retaining natural EOS admission and exact
  decoded output. It used the pinned `ct2rs 0.9.19` CPU adapter and existing
  OPUS-MT packs: French revision `cb3b2d680bf35591a508d8479e2c99c44e281ef3`,
  Spanish revision `437f5ffc6c8544943c685ea405650e0d17cf6098`. All eight files
  per pack were independently size/hash-checked against the compiled manifests.
  This harness is not a public job, renderer or packaged-app test.
- Recorded **104 pair/model runs**: 26 scope cases × fixed/actual target ×
  INT8/FP32. Native graph loading used the application's CPU, independent-pool,
  two-intra/one-inter-thread, sequential, memory-pattern/arena-disabled settings
  and shared ORT **1.27.0** library (SHA-256 remains
  `026c7d5c609323fb16506dbc3cce801bcdffdd7566fdba49a50727e2e1e881ca`).
  The two aligner graph/tokenizer inventories were checked against their earlier
  pinned receipts. FP32 is a diagnostic control, not a new installable pack.
- Applied the unchanged pinned guide head to these tensors and separately to
  Python ORT **1.23.2** tensors for the exact same strings/token IDs. Raw Rust
  and Python tokenizer IDs/word IDs agreed; no `[UNK]` occurred and the longest
  alignment sequence was 11 tokens, including framing. These failures are not
  evidence of token-budget truncation or unknown-token admission problems.
  Python remains disposable research tooling, not a second application runtime.

### What fails and where

| Case | Observed evidence | Conclusion |
| --- | --- | --- |
| French future, styling `reviendra` | Fixed `He will return tomorrow.` links the verb only to `return`, leaving `will` unlinked. Actual OPUS-MT gives `He'll be back tomorrow.`; `be`/`back` link to the verb but `ll` does not. Both precisions and runtimes retain this gap. | A translation-dependent correspondence gap, not a dropped DOM node. Expanding adjacent words would guess ownership. |
| Spanish implicit subject, styling `Volverá` | Fixed `She will return tomorrow.` links `will`/`return` but not `She`. Actual output is `He'll be back tomorrow.`: INT8 links the verb to `He`/`be` but misses `ll`/`back`; FP32 links `He`/`ll`/`be` but misses `back`. Both reject. | Generated subjects and lexical paraphrases need more than auxiliary insertion. The different English subject is context-dependent translation choice, not an aligner repair opportunity. |
| Spanish short adjective, styling `rojo` | Fixed `El libro rojo` → `The red book` has no adjective link on native INT8; Python 1.23.2 INT8 retains it. Actual OPUS-MT returns `The Red Book`, and **both runtimes and precisions omit the adjective link**. French `Le livre rouge` → `The Red Book` retains its adjective link. | The earlier runtime difference is real but narrow. A downgrade, precision switch or case normalization is not a demonstrated general fix. Exact output/casing matters. |
| Negation-only formatting | Actual French/Spanish `She's not coming.` under INT8 links the selected negation to `not` correctly, but the contracted auxiliary `s` has no source link. FP32 additionally misses French `ne`. | The sentence-wide lexical coverage guard rejects even this locally correct INT8 scope. That is a known conservative coverage cost; it must not be reported as wrong negation mapping. |
| Spanish modal, styling `salir` | Actual `He can get out.` links `salir` only to `out`, leaving lexical `get` unlinked. | Missing links include lexical paraphrase components, not just a finite list of auxiliary words. |

The raw tokenizer treats `ll` and `s` as lexical word IDs alongside a separate
apostrophe. They cannot be discarded as punctuation. Some contractions do link
successfully (for example, `She's done.`); globally expanding or rewriting
contractions would change successful cases and exact target/cache coordinates.

On the original 24 scopes, the fixed-target graph screen reproduces **18/24
exact direct scopes** and **15/24 without missing-source, missing-target or
mixed-ownership blockers**, for each precision. On actual OPUS-MT output,
**12/24 have those blockers**: six French and six Spanish. The two extra
adjective controls are excluded from these denominators. These are diagnostics
of the word graph and existing guard conditions, **not full DOM reconstruction
passes or a semantic acceptance score**. Actual-target scopes were not frozen
before observing output, so this is not a held-out quality evaluation. A graph
without these blockers can still be semantically wrong.

The two runtimes produce identical word-link sets on **all 26 actual targets**
for each precision. Their sole link-set difference in the 104 comparisons is
the fixed lowercase Spanish adjective pair under INT8. The result narrows the
earlier runtime concern; it does not establish general runtime equivalence or
qualify a shared runtime change for TTS, preprocessing or conceptual search.

### Recommended next step before implementation

1. **Keep the current candidate disabled and the exact-pair/DOM contract.**
   There is no demonstrated extraction, tokenizer or reconstruction defect to
   patch from these failures. Whole-unit uniform formatting already inherits
   the whole target safely; it cannot justify widening a partial styled verb,
   negation or noun to its sentence.
2. **Evaluate a better word-link provider against these frozen blockers before
   another production slice.** Require the French future, Spanish implicit
   subject/short adjective, negation-only, disjoint and lexical-paraphrase cases
   to succeed alongside the existing unsafe/fused-ownership rejections. Keep
   the current shared runtime and provider-independent contract as constraints
   first; a candidate needing another runtime or a fork needs a separate decision.
   Freeze intended scopes on new actual-output cases before evaluating that
   next candidate, and report French/Spanish coverage separately.
3. Do not lower the softmax threshold, add nearest-word/auxiliary expansion,
   lowercase exact model output, drop unlinked lexical targets, or split at
   style boundaries merely to turn these cases green. Prior adjacency trials
   already style negation incorrectly; missing `back`/`get` shows why an
   auxiliary allowlist is insufficient. Removing the global coverage guard
   would admit the French future with `will` left unstyled. Any future explicit
   unformatted/null ownership mechanism needs independent evidence and its own
   contract tests; absence of a link is not that evidence.
4. If no maintainable provider meets the mandatory positives and unsafe-case
   gate, stop and revisit the promised strict-formatting coverage with the user.
   Do not silently substitute safe rejection for the ≥95% coverage goal, expose
   a mostly rejecting strict mode, or describe legacy formatting hints as parity.

**Evidence and validation.** Disposable inputs, native harness, exact outputs,
tokenizer receipts, tensors and comparison results are in
`/tmp/papercut-stage9-investigation/`; logs are
`/tmp/stage9-{native-recheck,probe,analysis}.log`. They are local investigative
evidence, not durable repository tooling or release artifacts. Production code,
dependencies, model pins, saved documents, candidate gate and app version are
unchanged. Only this roadmap is updated. Benchmarks, broader/device smoke,
packaged acceptance, fresh CI and version bump remain deferred.

## Stage 9 fine-tuned provider evaluation — 2026-10-20

**Decision: do not implement a provider swap.** The least disruptive alternative
improves a few examples but fails the mandatory auxiliary/disjoint-scope cases.
The preceding investigation was committed as `00e6085` before this evaluation.
Production code and `FORMATTING_ACCEPTED = false` remain unchanged.

### Candidate, provenance and reproducible method

Tested [aneuraz/awesome-align-with-co](https://huggingface.co/aneuraz/awesome-align-with-co/tree/777756717e1fa9556e304d4d5db173ee386b9c16),
revision `777756717e1fa9556e304d4d5db173ee386b9c16`, rather than retraining the
current raw-mBERT export or changing its extraction threshold. The publisher's
model card supplies the same layer-8, softmax-intersection `> 0.001` recipe and
attributes the checkpoint to [Awesome-align](https://github.com/neulab/awesome-align).
Its language metadata includes French but not Spanish; Spanish performance must
be established independently. The publisher labels it BSD-3-Clause; complete
weight provenance and redistribution notices were not qualified for app delivery.

- Downloaded only the pinned checkpoint/config/tokenizer/card into `/tmp`.
  The 1,086,292,375-byte `pytorch_model.bin` SHA-256 is
  `9f9d19adbab07c9f5ec2d74fe5701a66c123919feb35d05fce29969b6a85adb9`,
  checked against repository LFS metadata. All downloaded files have local
  size/hash receipts. This is the full research checkpoint, including unused
  heads/layers, **not** a measured deployment-pack requirement.
- Reused installed research Torch **2.9.0+cpu** and Transformers **4.57.1** with
  `weights_only=True`, `trust_remote_code=False`, local files and unavailable
  network proxies during inference. No repository Python model code, custom
  model class, library patch, training or new app dependency was used. Standard
  `BertModel` loaded the first eight layers without a pooler/MLM head, matching
  the documented eighth hidden state. The checkpoint's raw fast tokenizer
  supplies word IDs/offsets; its vocabulary hash matches the existing candidate.
- Before inference, froze **58 synthetic cases** outside Git: the original 24
  fixed-target scopes plus two adjective controls; those same 26 scopes on
  previously captured exact native OPUS-MT targets; two existing fused-negation
  controls; and four existing deliberately wrong translation pairs. Required
  lexical scopes on the actual targets were annotated **before seeing the new
  provider's results**, without rewriting the targets. The frozen JSON SHA-256 is
  `8c0a50b23bf51d088e8728991b740b754217a3cc7aaa8a8ccb72686a042ebad5`.
  These familiar diagnostic inputs are not a new held-out acceptance corpus.
- Ran the same pinned, unmodified guide head and missing-source/lexical-target/
  mixed-ownership screen over exact pairs. No threshold/layer sweep, alternate
  matching rule, case normalization, contraction expansion or extra source
  context was tried. Shape/finite/token/annotation checks passed for all cases.
  No private article or sample-derived fixture was read or added to Git.
- Re-ran the same 58 cases with the cached original mBERT safetensors using the
  same Torch/Transformers, eighth layer and extraction setup. Its scope/blocker
  totals reproduce the earlier native INT8 totals below. Individual link sets
  need not be identical across precision/runtime; this same-software FP32
  control supports the limited scope improvements, not ONNX equivalence.

### Results

The original 24-case denominators exclude the two added adjective controls and
the six negative controls. Baseline results below are the preceding native INT8
receipt; the new checkpoint is **PyTorch FP32 research inference**, not a
production ONNX/quantized equivalence result.

| Original scope set | Existing candidate: exact direct scope / no graph blockers | Fine-tuned checkpoint: exact direct scope / no graph blockers |
| --- | --- | --- |
| Fixed targets, French (12) | 8 / 7 | 9 / 8 |
| Fixed targets, Spanish (12) | 10 / 8 | 10 / 8 |
| Actual targets, French (12) | 7 / 6 | 8 / 7 |
| Actual targets, Spanish (12) | 7 / 6 | 8 / 6 |

Thus the fixed-target totals improve **18 → 19 exact scopes** and **15 → 16
without graph blockers**, out of 24. Actual-target totals improve **14 → 16
exact scopes** and **12 → 13 without graph blockers**, out of 24. No positive
scope admitted by this screen was incorrect within this small annotated set.
These counts describe lexical ownership only, not full HTML/attachment
reconstruction, semantic completeness or a population-wide success rate.

- **Improved:** fixed French `reviendra` now owns `will return`; Spanish short
  `rojo` owns `red`/`Red` on both fixed and actual targets. Actual French lexical
  `a` in `She's got a book.` now owns the auxiliary `s` and `got` together.
- **Still blocked:** actual French future `He'll be back tomorrow.` owns `ll`
  and `back` but misses `be`. The problem changes shape rather than disappearing.
  French progressive/disjoint and French/Spanish negation cases still lack
  auxiliary links. Spanish implicit future now owns `ll be back` correctly but
  leaves generated `He` unlinked, so the unchanged global coverage guard rejects.
  Spanish `salir` → `get out` now owns `get` instead of `out`; the other lexical
  component is still missing. No auxiliary allowlist would repair that case.
- **Safety limits:** both fused `won't` controls still have missing lexical
  evidence and reject under the screen. All four deliberately wrong translation
  pairs still have complete graphs and pass that screen (for example, `rouge`
  aligns to `blue`). As before, correspondence coverage cannot certify MT
  meaning; those are negative semantic counterexamples, not successful outputs.

The mandatory positive gate remains unmet even before considering unknowns,
attachments, identities, hierarchy, held-out quality or resource acceptance.
There is no reason to spend a production slice exporting/quantizing/installing
this checkpoint hoping that quantization will repair its FP32 scope failures.

### Other providers screened, not executed

| Provider | Evidence and disposition |
| --- | --- |
| [WSPAlign](https://github.com/qiyuw/WSPAlign) | Both inspected base-model cards and the code declare CC-BY-NC-SA-4.0. This is not the existing permissive redistribution path; it would need a separate permission/distribution decision. Do not add a new span-prediction adapter or download these weights merely on the strength of published accuracy. No frozen-case result is claimed. |
| [BinaryAlign](https://github.com/ubisoft/ubisoft-laforge-binaryalign) | The actual [license file](https://github.com/ubisoft/ubisoft-laforge-binaryalign/blob/main/license.txt) specifies CC-BY-NC-ND-4.0, beyond the README's shorter noncommercial description. This also needs a separate permission/distribution decision. No weights downloaded or quality result claimed. |
| [AccAlign](https://github.com/sufenlp/AccAlign) | Published recipe relies on LaBSE and an adapter checkpoint with legacy `adapter-transformers 4.16.2`/Torch 1.9.0 training requirements. It is not an already verified encoder pack for the shared app runtime. No port, dependency downgrade or frozen-case result claimed. |
| [OmniAlign](https://github.com/MilkDargon/OmniAlign) | A distinct Apache-2.0 candidate whose published language list includes French/Spanish. Metadata at `WPS-Qingqiu/OmniAlign@eaeeb279332156479b9092641a6b9e189e83a6d8` lists a 610,751,248-byte safetensors file plus custom GTE/NEW architecture Python modules, with no ONNX artifact. The authors describe approximately 305M parameters. Its existing published payload is outside the 200 MiB pack target; a smaller export's size/quality and shared-ORT compatibility remain unknown. No quality, RAM, speed or mobile result is claimed. |

**Next bounded decision:** keep the existing provider-independent contract and
defer production alignment enablement. If further provider research is pursued,
OmniAlign is the remaining distinct permissively labelled lead from this screen:
first evaluate its reviewed offline loading/export feasibility and ability to
meet the unchanged pack budget, before any app adapter. Do not assume that a
larger model or a published alignment score solves grammatical ownership, and
do not silently raise the pack/resource budgets. If that path cannot fit without
a maintained fork or materially larger delivery, revisit the strict-formatting
scope with the user rather than introduce ownership guesses or an open-ended
sequence of model swaps. No model is approved for replacement by this evaluation.

**Evidence/validation:** all 116 local correctness probes completed (58 per
checkpoint); receipts,
frozen inputs, comparison assertions and results are under
`/tmp/papercut-stage9-provider-evaluation/`, with logs
`/tmp/stage9-provider-{fetch,evaluation,baseline}.log`. These are disposable, outside-Git
research files. No ONNX export, quantization, shared-runtime run of the new
checkpoint, installer/catalog change, public admission, saved-document change,
fresh app/CI build, benchmark, broader/device smoke or version bump was performed.

## Stage 9 OmniAlign offline loading, export and pack feasibility — 2026-10-20

**Offline/export/size feasibility demonstrated; no production integration.**
Offline loading, an ONNX export and a pack below the existing 200 MiB target are
demonstrated without a library fork or runtime upgrade. This does **not** show
that OmniAlign fixes the frozen formatting failures. The previous provider
findings were committed as `dc7b4cd` before this investigation.

### Immutable inputs and offline loading

- Used [WPS-Qingqiu/OmniAlign](https://huggingface.co/WPS-Qingqiu/OmniAlign/tree/eaeeb279332156479b9092641a6b9e189e83a6d8)
  revision `eaeeb279332156479b9092641a6b9e189e83a6d8`. The actual safetensors
  file is **610,751,248 bytes**, SHA-256
  `e3177c8f3a75d81ad5947bd6e2dfada53f8fd0124de3186d58eb62c23794df1a`,
  verified against publisher LFS metadata. Config, tokenizer, source and card
  files have separate pinned size/hash receipts. All research files stay in
  `/tmp`; no private article or sample-derived input was used.
- Plain `AutoConfig(..., trust_remote_code=False)` rejects this custom model.
  Reviewed the pinned `configuration.py` and `modeling.py`, then imported their
  explicit local `NewConfig`/`NewModel` classes. Loaded safetensors with
  `local_files_only=True`, `weights_only=True`, CPU FP32 and the provided eager
  attention implementation. No upstream file was patched, remote code fetched
  during loading, or xformers/CUDA dependency added. Offline environment flags
  and unavailable proxies were set for loading/export/inference. This is a
  reviewed local research loader, not permission for the app to execute newly
  downloaded Python code.
- Used the authors' [pinned inference example](https://github.com/MilkDargon/OmniAlign/blob/bf2687ef7172801b2b5c84df7b1fdfd4d1642bfe/example/example.py)
  default **`hidden_states[7]`**. Loaded the full 12-layer checkpoint first;
  retaining its first seven layers produces that same tensor. Verified exact
  full-model/layer-prefix equality on seven synthetic inputs before export.
  This removes computations after the requested output, not trained weights
  from within the seven-layer representation. No layer search or training ran.

### Export and unchanged-runtime checks

The existing research environment uses Torch **2.9.0+cpu**, Transformers
**4.57.1**, ONNX **1.19.1** and Python ORT **1.23.2** tooling. No project
dependency was added or changed.

1. Standard `torch.onnx.export(..., dynamo=False, opset_version=17)` produced
   FP32 `input_ids`/`attention_mask` → `[1, sequence, 768]` embeddings with a
   dynamic sequence dimension. This currently supported legacy exporter emits
   a deprecation warning; the newer dynamo exporter was not qualified here.
   The two upstream position-cache tracing warnings concern expansion beyond
   8,192 positions. This probe stays within the existing **256-token** bound;
   it makes no long-context export claim.
2. Used ORT's installed **`MatMulNBitsQuantizer`** with the documented
   [four-bit weight-only recipe](https://onnxruntime.ai/docs/performance/model-optimizations/quantization.html#quantize-to-int4uint4):
   RTN, symmetric weights, block size 128, accuracy level 4, QOperator format,
   constant `MatMul`/`Gather` weights and axes 0/1 respectively. The resulting
   graph has **28 `MatMulNBits` and two `GatherBlockQuantized`** nodes, ONNX
   opset 21 and `com.microsoft` domain version 1. These are ORT-supplied contrib
   operators; no Papercut kernel or custom native extension was built.
3. Both graphs pass `onnx.checker`. An external Rust harness used the project's
   existing **ort rc.13 / API 17** binding and actual shared Linux **ORT 1.27.0**
   library with the same explicit CPU, independent pools, two-intra/one-inter
   threads, sequential execution and disabled arena/memory-pattern settings.
   No second app runtime, `ORT_DYLIB_PATH` override or new runtime build was used.
4. Both graphs loaded and returned finite FP32 tensors for all **seven inputs
   per graph**: French, Spanish, accented/inverted-punctuation text, synthetic
   lengths 2/23/256 and a masked-padding case. FP32 output matches the Torch
   reference at predeclared `rtol=1e-4, atol=5e-5`; maximum absolute error across
   these checks is about **2.77e-5**. This is representation/export correctness,
   not a translation or formatting acceptance test.

The existing Rust **tokenizers 0.23.2** also parses the new tokenizer and agrees
with the research Python tokenizer on IDs, word IDs and decoded UTF-8 offset
slices for five ordinary Unicode/contraction inputs. This verifies basic native
loading, not the unknown-token/normalization admission policy for this family.
The authors' separate whitespace/NLTK pre-split word-alignment pipeline was not
adopted; these checks exercise encoder representations and raw tokenizer evidence.

Four-bit outputs are **not numerically equivalent** to FP32. Maximum element
differences on the three ordinary text probes are approximately **0.85–1.14**;
the repeated-token/padding probes differ more, including unused masked positions.
Finite outputs and a small file
do not establish preserved word links. No extraction threshold was adjusted,
no formatting case was relabelled and no semantic-quality result is claimed.

### Pack-size result

Sizes are uncompressed bytes, matching installer accounting; no compression
ratio or measured RAM/latency is substituted for the pack budget.

| Payload | Bytes | Disposition |
| --- | ---: | --- |
| Published full BF16 safetensors | 610,751,248 | Research input; not a delivery pack. |
| Exported layer-7 FP32 graph | 1,037,046,783 | Too large for the pack target. |
| Optimistic seven-layer all-parameter INT8 plus tokenizer files | 275,251,111 | Already exceeds 209,715,200 bytes before scales, graph and notices; this is a tensor-inventory lower bound, not an INT8 export measurement. |
| Actual four-bit graph | 141,774,895 | 135.2 MiB; self-contained ONNX file. |
| Four-bit graph + config/tokenizer/special-token files + reviewed source/card | **158,957,415** | **151.6 MiB**, leaving about 48.4 MiB below the target before final notices/manifest. |

The four-bit graph SHA-256 is
`cb9e655cdda6374a22c4f3faee418bc155c47be5569084eef5db0a966f1e904b`.
The provisional sum deliberately includes the research source files; production
inference would consume the ONNX graph and need not ship a Python loader. It
excludes the full safetensors and FP32 build inputs. Complete redistribution
notices, immutable hosted export provenance and installer admission remain open;
this is not an approved installable manifest.

### Next gate and maintenance implications

- **Resolve the word-boundary representability question before integration.**
  Raw OmniAlign tokenization groups all of `He'll` into one word ID (and groups
  trailing punctuation with the preceding word). A frozen actual-output scope
  needs `ll` styled while `He` remains unstyled. The unchanged whole-word
  contract cannot represent that `3..5` span inside the `0..5` word, regardless
  of embedding quality. `won't` is likewise one word and its fused-negation
  rejection must remain intact. Do not fabricate new word IDs, re-split/rewrite
  the target or silently remove that required positive to claim compatibility.
  The next evaluation must establish whether a principled ownership-boundary
  design can meet the frozen requirements without weakening the unsafe cases;
  otherwise this provider cannot meet them under the current contract.
- **Then qualify FP32 versus four-bit frozen-case semantics**,
  using the same exact French/Spanish targets, mandatory scopes, negation/fusion
  controls and wrong-translation counterexamples. Quantization may change word
  links, so passing the FP32 model alone cannot approve the small pack. If that
  fails, stop before implementing installer/UI/provider integration.
- This is **not** a drop-in replacement for the current mBERT auxiliary pack.
  OmniAlign uses an XLM-R tokenizer with different reserved IDs/normalization;
  the current BERT-specific `CandidateTokenizer` intentionally rejects it.
  Any later approved adapter needs a verified tokenizer profile, unknown/lossy
  normalization tests, new immutable graph/tokenizer/extraction identities and
  separate cache compatibility. Keep the source-owned DOM contract and explicit
  unresolved ownership; do not weaken old admission or reuse old model IDs.
- A later delivery path should publish a verified export produced from pinned
  unmodified sources with a reproducible build-time recipe. Do not download code
  and export models on users' devices or vendor a patched upstream architecture.
  The export-tool deprecation and ORT contrib-operator availability still need
  maintained tooling and packaged-platform qualification. The Linux result says
  nothing about Android/iOS/macOS/Windows kernel availability or low-power speed.

**Validation/evidence:** pinned-source/weight hashes, full/prefix parity, both
ONNX checker results, 14 finite native inferences, five native tokenizer parity
checks, FP32 tolerances and pack-size
arithmetic passed. Artifacts and disposable scripts are under
`/tmp/papercut-omnialign-feasibility/`; logs are
`/tmp/omnialign-{fetch,export,quantize,native-fp32,native-q4}.log`.
Only this document changes. Production code/dependencies/runtime bundles, model
catalog/installer/cache, saved documents, the disabled candidate and app version
are unchanged. Frozen-case OmniAlign quality, benchmarks/RAM/thermals, broader
smoke, packaged/device acceptance, fresh CI and version bump remain deferred.

## Stage 9 ownership-boundary evaluation — 2026-10-20

**Representability is feasible; semantic safety remains unqualified.** Committed
the previous loading/export findings as `859f007` before this evaluation. No
production code changed. Keep the public candidate disabled and evaluate actual
link evidence next, rather than implementing a tokenizer workaround now.

### Frozen inputs and method

Preserved the existing **58-row** synthetic set byte-for-byte, SHA-256
`8c0a50b23bf51d088e8728991b740b754217a3cc7aaa8a8ccb72686a042ebad5`:
26 fixed-target positive scopes, the same 26 on previously captured exact
CTranslate2 output, two fused-negation negatives and four deliberately wrong
translation pairs. The 26 include the original 24 scopes plus two short-adjective
controls. Kept the original lexical annotations; the old tokenizer locates their
exact source/target scalar ranges, not new desired boundaries fitted to OmniAlign.

Added ten separate synthetic diagnostic controls before comparing policies:
straight/curly apostrophes, partial and whole fused contractions, lost future
meaning, a combining-mark cut, an ordinary subword fragment, whole-unit style
inheritance and valid unrelated formatting beside negation. These are not new
acceptance data or private sample-derived fixtures. All research stays outside
Git. The combined inputs contain **63 distinct exact strings**.

Used the pinned OmniAlign tokenizer with native **tokenizers 0.23.2** to retain
actual token IDs, word IDs and UTF-8 offsets. The existing
**unicode-segmentation 1.13.3 / Unicode 17.0.0** supplies word and grapheme
boundaries. No input rewrite, pre-split model inference, fabricated word ID,
divided token vector or new dependency was used. A proposed independent boundary
view is representable only if each raw content token fits wholly within one cell
and every requested cut is a grapheme boundary.

Compared four diagnostic views:

| Boundary view | Fixed scopes / 26 | Actual-output scopes / 26 | Total / 52 |
| --- | ---: | ---: | ---: |
| Raw OmniAlign word IDs | 13 | 8 | 21 |
| Unicode default word boundaries | 26 | 15 | 41 |
| Every raw model token independently ownable | 26 | 26 | 52 |
| Unicode boundaries with English clitic cells | 26 | 26 | 52 |

These counts measure **span geometry only**. They do not measure inferred links,
correct grammatical ownership, identity/hierarchy reconstruction, translation
quality or held-out coverage. Some frozen annotations cover multiple lexical
spans; representing them is not proof that a complete DOM identity can safely
move there. No OmniAlign inference or new translation generation ran this round.

### What the counterexamples establish

- Raw word IDs conflate ownership with tokenizer grouping: `He'll` is one word,
  and punctuation may belong to its preceding word. Unicode default boundaries
  separate punctuation but keep the contraction together. Neither view can
  express the required `ll` span at scalar range `3..5` while leaving `He`
  unstyled. Unicode segmentation permits documented tailoring; it does not
  determine grammatical ownership. See [UAX #29, Unicode 17](https://www.unicode.org/reports/tr29/tr29-47.html).
- Making every subword independently ownable overcorrects. It admits both `ll`
  and the unsafe `international` fragment of `internationalization`, plus `won`
  or `t` inside `won't`. All four views reject the combining-mark cut. Grapheme
  safety alone therefore does not supply word/meaning safety.
- The evaluated English-only view separates a Unicode word's final apostrophe
  and `ll/re/ve/m/s/d` into stem, punctuation and suffix cells, preserving the
  raw evidence. It handles straight/curly apostrophes and all 52 frozen positive
  scopes while keeping `won't` atomic and ordinary subwords indivisible. This
  is a proposed geometry policy, **not** an approved morphological parser;
  ambiguous suffixes such as `s/d` receive no inferred grammatical meaning.
  [Treebank tokenization](https://www.nltk.org/api/nltk.tokenize.treebank.html)
  illustrates established contraction splitting, but its complete algorithm was
  neither installed nor adopted and would not itself prove ownership.
- Keeping `won't` atomic still cannot establish safety. For both frozen fused
  negatives, the three alternative views allow a fully covered but wrong graph:
  unstyled negation owns the whole contraction while the styled future verb owns
  only `return`. Every target cell has a link, every styled source cell has a
  link and no target cell mixes owner stacks, yet future meaning is lost from
  the style. Raw word IDs also allow this French counterexample; the Spanish
  one rejects only because its source verb shares a word ID with the period.
  That incidental punctuation rejection is not a fused-meaning detector.
- A diagnostic whole-piece veto on English `n['’]t` for partial source ownership
  rejects both fused negatives and preserves uniform whole-unit inheritance.
  It also wrongly rejects styling only `book` in `She won't read the book.`
  This trades unsafe acceptance for avoidable rejection; it is not approved as
  a fix or evidence of ordinary-input coverage. Selecting only affected owners
  would require independent evidence, not adjacency or a larger style hull.

The current contract already tests this limitation explicitly in
`rejects_fused_contractions_mixed_ownership_and_unaligned_required_source`:
a covered graph can pass until `ScopeIssue::FusedMeaning` is supplied. A frozen
negative's expected label cannot serve as that detector during evaluation.
Likewise, the four wrong-translation controls remain reminders that a complete
graph is structural evidence, not semantic certification; no new rejection
claim is made for them here.

### Recommendation and next gate

1. **Continue only the outside-repo FP32/four-bit evidence evaluation.** Freeze
   the raw-token link extraction and cell aggregation rules before inference;
   compare both exports on the unchanged positive and fused-negative cases.
   Report missing links, mixed owners and wrong-but-covered ownership separately.
   Do not supply expected blockers to make negatives pass or tune thresholds,
   split inference at style boundaries, or fill links by proximity. Keep the
   wrong-translation controls as explicit limits on what the result establishes.
2. **If that passes, separate ownership cells from inference provenance.** The
   smallest viable design retains immutable raw IDs/word IDs/offsets and derives
   a separate, versioned, language-scoped cell table with raw-token membership.
   Reject a token straddling a cell; do not duplicate its vector. Preserve the
   current unknown/normalization/coverage, grapheme, mixed-owner, literal,
   attachment and hierarchy guards. A new tokenizer family/policy needs explicit
   identities and cache compatibility, not replacement of genuine word IDs.
3. **Stop before production integration if mandatory scopes or fused negatives
   fail.** Surface boundaries alone cannot repair missing semantic evidence.
   Neither an English clitic rule nor a broad contraction veto earns public
   acceptance; later held-out coverage and complete DOM tests are still required.

**Validation/evidence:** native boundary collection, immutable input/control
hashes, all table counts, token/cell containment and explicit counterexample
graphs passed assert-based checks. The existing focused Rust contract suite
passed **30 tests**, offline and without default features; this validates current
guards, not an implemented new policy. Research scripts/results are in
`/tmp/papercut-ownership-boundary-evaluation/`, including `verification.json` and
`graph-counterexamples.json`; logs are `/tmp/ownership-boundaries-{native,evaluation,verification,contract}.log`.
Only this document changes. The candidate remains `FORMATTING_ACCEPTED = false`;
model/runtime dependencies, installer/catalog/cache, saved documents and app
version are unchanged. Actual OmniAlign link quality, benchmarks, broader/device
smoke, fresh CI and version bump remain deferred.

## Stage 9 OmniAlign FP32/four-bit link evaluation — 2026-10-20

**No-go for the tested raw-input provider policy.** Both exports fail mandatory
positive scopes and accept unsafe fused-negation mappings. A larger FP32 pack
does not resolve these failures. Committed the preceding boundary findings as
`df4e426` before inference; no production implementation followed this evaluation.

### Fixed policy and execution

- Preserved all **58 frozen cases** and exact previously captured CTranslate2
  outputs; frozen SHA-256 remains
  `8c0a50b23bf51d088e8728991b740b754217a3cc7aaa8a8ccb72686a042ebad5`.
  No private sample, new translation generation or modified expected scope was
  used. The cases contain **59 unique source/target strings**.
- Froze the extraction/aggregation specification before native inference,
  SHA-256 `a7c3fc0a413d5ae96c191b3c0415a2e01129a65033c79fea7d0d374a8b5144d3`.
  Used the existing layer-7 FP32 and four-bit exports of the pinned OmniAlign
  checkpoint. Raw exact strings retain their actual token IDs, word IDs and
  offsets; framing is excluded from the alignment scores. No truncation,
  pre-split inference, target rewriting or changed tokenizer profile was used.
- Each export ran all 59 strings through the **unchanged native Linux runtime**
  used in the feasibility check: ort rc.13 / API 17, shared ORT 1.27.0, CPU,
  independent two-intra/one-inter thread pools, sequential execution, disabled
  arena/memory pattern. All **118** inferences returned finite `[1, n, 768]`
  tensors, and native tokenizer IDs matched the frozen inputs.
- Applied the pinned [upstream example's](https://github.com/MilkDargon/OmniAlign/blob/bf2687ef7172801b2b5c84df7b1fdfd4d1642bfe/example/example.py)
  four unmodified tensor assignments to those native embeddings: FP32 unscaled
  dot products, row/column softmax and intersection **strictly above 0.001**.
  Executed only these assignments in the existing research environment, not
  the upstream CLI's imports, sentence processing or pre-split tokenization.
  An independent NumPy implementation agrees on all **116 case/export graphs**.
- Aggregated genuine token links by union into the previous Unicode source /
  English clitic target cells. Each token must fit wholly in one cell. Required
  source coverage, lexical target coverage and mixed-owner checks remain
  conservative. No proximity repair, threshold/layer sweep, broad contraction
  veto or expected `FusedMeaning` label was supplied to make a case reject.

This tests the proposed raw-input adapter with upstream extraction math, **not**
the authors' complete pre-split CLI or every possible OmniAlign configuration.
It is a fixed correctness probe, not a speed/RAM benchmark or held-out quality
measurement. Cell geometry and lexical scopes do not replace complete DOM,
identity, hierarchy, literal or attachment acceptance.

### Results

The numbers are identical for FP32 and four-bit; the underlying graphs are not.

| Frozen positive group | Cases | Exact scope | Exact with no screen blocker | Screen rejects | Wrong scope with no screen blocker |
| --- | ---: | ---: | ---: | ---: | ---: |
| Fixed targets | 26 | 20 | 17 | 9 | 0 |
| Actual CTranslate2 targets | 26 | 14 | 9 | 16 | 1 |
| Total | **52** | **34** | **26** | **25** | **1** |

“Exact scope” compares all owned scalar spans against the unchanged annotations.
Also checking lexical-only spans produces the same counts here; punctuation
leniency does not explain or repair these failures. Screen rejection means
missing required source/lexical target links or mixed owners, not a successful
translation of a required positive.

- **Missing auxiliaries persist.** French future `reviendra` owns only `return`
  in the fixed target and only `back` in `He'll be back tomorrow.`; the required
  `will` or `ll/be` links are missing. Actual Spanish future and perfect scopes
  likewise lose auxiliary components. Spanish `salir` owns only `out` while
  `get` is unlinked. These remain failures even when the main verb is linked.
- **Coverage can hide wrong ownership.** For `Ella no viene.` →
  `She's not coming.`, styling `viene` owns only `coming`. The `s` cell is linked
  to unstyled `Ella`, so there is no missing target or mixed-owner blocker, but
  the required auxiliary is outside the style. Both exports admit this wrong
  scope; complete coverage is insufficient.
- **Both fused negatives are unsafely admitted by both exports.** For the frozen
  French/Spanish future-plus-negation cases, the styled verb owns only `return`.
  The whole `won't` cell is assigned to unstyled negation. All required coverage
  checks pass, with no mixed owners, while future meaning falls outside the
  style. This is actual inferred evidence of the previous counterexample, not
  merely a hypothetical graph. Neither negative is a successful rejection.
- **All four deliberately wrong translation pairs pass the graph screen** for
  both exports, including the changed-colour controls. This confirms the stated
  limitation that alignment coverage cannot certify translation correctness;
  it is not a new promise that this structural contract detects arbitrary bad
  translations.

Quantization changes token-link sets in **7/58** case rows and cell-link sets in
**4/58**, including the French fused negative. It does not change the aggregate
scope counts or the two unsafe fused admissions. Therefore the result is neither
numerical equivalence nor evidence that quantization caused the core failures.

### Decision and next work

Stop this OmniAlign integration path **under the evaluated policy** before
changing production contracts, tokenizer admission, model manifests, installers
or UI. The provisional small pack and successful offline inference do not meet
the formatting gate. Do not implement the clitic boundary view as a fix for
missing grammatical evidence, or disable coverage to increase the pass count.

The next step is a Stage 9 design decision, not another production stage:
continuing the engine-independent approach needs demonstrably better ownership
evidence for auxiliaries and fused morphology. Any proposed maintained provider
or different documented inference path must first meet the same frozen scopes
and unsafe-negative gate outside the app. The existing results do not justify
an upstream fork, a grammar repair heuristic or lowering the user's formatting
requirements. Keep the present candidate disabled while deciding that path.

**Validation/evidence:** input/policy/model/tokenizer/runtime hashes, all native
outputs, 116 independent extraction comparisons, exact scope counts and negative
admissions passed assert-based receipt checks. Research scripts/results remain
outside Git in `/tmp/papercut-omnialign-link-evaluation/`, including
`frozen-policy-receipt.json`, `summary.json` and `verification.json`; logs are
`/tmp/omnialign-links-{prepare,fp32,q4,evaluation,verification}.log`.
Only this document changes. Production code/dependencies/runtime bundles, model
catalog/install/cache, saved documents and app version are unchanged, with
`FORMATTING_ACCEPTED = false`. No new app/CI build, broader/device acceptance,
benchmark, smoke or version bump ran. The prior 30-test contract receipt remains
current; this docs-only update did not rerun that unchanged suite.

## Stage 9 alignment strategy reassessment — 2026-10-20

**Recommendation: preserve the shared DOM architecture; stop assuming a single
post-translation word aligner supplies sufficient ownership evidence.** Committed
the previous native link findings as `052cf4f`. This is a reassessment and a
proposed next research gate, not authorization to change the selected Option 7
implementation or enable any candidate. Formatting remains essential; the
solution must still support future engines without a second renderer or silent
plain-text fallback.

### The missing evidence

Three questions must stay separate: can the exact target span be represented,
which source meaning contributes to it, and can that map be reconstructed with
the original identities/hierarchy? The boundary probe answered only the first.
The existing contract/renderer address structural validation; neither discovers
semantic contributors that an evidence provider failed to report.

| Required case | Needed ownership evidence | Why a covered word graph is insufficient |
| --- | --- | --- |
| Styled finite verb → English auxiliary + lexical predicate | Both target components belong to the styled source predicate under the frozen policy. | A lexical aligner can link only the main predicate and leave the auxiliary missing or assign it to the subject. |
| Styled negation alone | Polarity belongs to the negation; the auxiliary must not automatically inherit it. | Adding nearby auxiliaries fixes some verb cases but corrupts correct negation-only scopes. |
| Styled future verb, unstyled negation → atomic `won't` | The target cell depends on both source owners; mixed ownership must reject. | A single negation link can make the cell appear fully covered while hiding its future contribution. |
| Unrelated styled noun beside a contraction | The noun has independent owners and remains usable. | Rejecting every partially styled sentence containing `n't` unnecessarily rejects this positive. |

These are the frozen product rules, not a claim that linguistic annotation has
one uniquely correct formatting answer. Generated subjects, paraphrases and
inflected features need explicit policy and reviewed examples. Do not change
their expected scopes after seeing model output. Whole-unit uniform style
inheritance remains the existing narrow safe case; it does not solve ordinary
partial emphasis or qualify the failed providers.

### Reassessed paths

| Path | Benefit | Cost / limitation | Disposition |
| --- | --- | --- | --- |
| Another post-hoc word aligner or larger precision | Preserves arbitrary engine choice and the existing two-model shape. | Tested base/fine-tuned mBERT and raw-input OmniAlign still miss required evidence; FP32 does not repair the core failures. | No further model search without a documented reason it addresses these ownership cases. |
| Ensemble, score threshold or missing-link expansion | May change individual graph decisions. | Agreement can share the same blind spot; nonzero scores or coverage are not contributor evidence. Extra sessions/downloads do not establish safety. | No automatic union, voting or adjacency repair. |
| Morphology/dependency analysis plus lexical links | Could identify auxiliaries, polarity and inflection before relating predicates. | Requires reliable source and target analyses, cross-language composition, clitic-to-surface mapping and ambiguity handling. Arabic adds a separate qualification problem. No suitable small maintained pack/runtime has been qualified. | A real NLP subsystem, not an approved small parser patch. |
| Evidence from the producing translation model | Relates actual generated target tokens to their source; potentially avoids a second encoder. | Generic attention is not semantic attribution; exact offsets, fusion detection, bindings and platform support still need proof. Engine-specific availability. | Best next bounded research option for existing OPUS-MT, not a production go. |
| Alignment-trained/tag-aware model or custom training | Can learn correspondence jointly with translation rather than repair it afterward. | Requires suitable artifacts, licensing, quality and resource evidence; own training is a continuing data/model maintenance commitment. | Consider maintained pretrained artifacts only; no training project or upstream fork approved. |

Universal Dependencies documents [auxiliary relations](https://universaldependencies.org/u/dep/aux_.html),
[word-level tense features](https://universaldependencies.org/u/feat/Tense.html)
and [multiword tokens](https://universaldependencies.org/format.html). Its tense
guidance explicitly notes that periphrastic tense may not be directly expressed
by an individual word's feature. **Inference:** parsing both sides would provide
useful observations, but would not itself map cross-language semantic ownership.
It must not become an auxiliary allowlist disguised as a dependency adapter.

### What existing APIs actually offer

CTranslate2's maintained [translation API](https://opennmt.net/CTranslate2/python/ctranslate2.Translator.html)
supports `return_attention`; [TranslationResult.attention](https://opennmt.net/CTranslate2/python/ctranslate2.TranslationResult.html)
contains a matrix for each generated hypothesis. This establishes access to
decoder evidence, not a formatting guarantee. OpenNMT distinguishes
[raw attention-derived alignments from supervised alignment training](https://opennmt.net/OpenNMT-py/FAQ.html#can-i-get-word-alignments-while-translating).
Exposing attention cannot retroactively make the installed OPUS models
alignment-trained or prove they handle fused ownership.

The local **ct2rs 0.9.19** result exposes hypotheses and scores only; attention is
commented out at both the Rust FFI and C++ conversion. The inspected public
[ct2rs result documentation, version 0.10.1](https://docs.rs/ct2rs/latest/ct2rs/sys/struct.TranslationResult.html)
also exposes only hypotheses and scores. Do not upgrade dependencies merely to
enable the existing request flag: a maintained result path is still unproven.
No newer binding was installed or built during this review, and no comprehensive
search of every alternative binding is claimed.

The official Python API is suitable for an isolated feasibility check, **not** a
new packaged Python service. `translate_batch` attention belongs to its own exact
hypothesis. The documented `score_batch` interface does not expose a
`return_attention` option. Therefore a supported gold-target attention path for
arbitrary frozen targets is not established. Forcing a target prefix, using
attention from a different translation or relabelling a changed hypothesis must
not be assumed to solve that evaluation problem.

### Proposed next gate, before further coding

1. **Check evidence access and exact-pair feasibility outside Git.** Using an
   official CTranslate2 API and the already evaluated model artifacts, determine
   whether it can return source tokens, actual hypothesis tokens and bounded
   attention without changing decoding. Independently establish whether frozen
   fixed targets can be evaluated through a supported API. Missing access is a
   recorded blocker, not permission to patch C++ or ship another runtime.
2. **Only if access is viable, freeze an extraction/ownership policy before
   testing it.** Preserve exact text/token provenance and the same mandatory
   positive/fused scopes. A regenerated actual output must match its frozen
   target to count as that row; otherwise report it as not comparable. Do not
   use a different output as a passing replacement. Any separately declared new
   corpus is additional evidence, not closure of failed frozen gates.
3. **Require both complete positive ownership and safe fused rejection.** Report
   missing links, wrong-but-covered scopes and unrelated-near-contraction
   positives separately. Continue reporting wrong-translation controls as limits
   on structural confidence. Do not select the strongest attention edge or an
   arbitrary mass cutoff and call it semantic proof. If the path still needs
   guessed grammatical links or rejects ordinary supported prose, stop.
4. **Resolve maintained Rust delivery before production work.** A successful
   research probe would still need a supported binding/result API and actual
   target/source offset mapping. No local binding fork, bespoke C++ wrapper,
   dependency upgrade or model/UI change is approved by this reassessment.
   If that route is unavailable, pause this strict-formatting path and revisit
   provider feasibility; do not continue infrastructure work around missing data.

### Architecture retained and conditional impact

Engine independence belongs at the **checked mapping contract**, not in a promise
that every engine uses one identical aligner. This is a proposed refinement to
Option 7: a separately qualified aligner may still serve future text-only models;
a producing engine may supply qualified evidence directly. Neither qualifies
another engine automatically. No auto-fallback, aligner picker, general plugin
framework or second DOM implementation is needed.

Retain source-owned DOM/asset planning, exact pieces, mapping validation,
reconstruction, cancellation/resume and atomic saving. Current candidate-specific
runtime/extraction and installer code stays disabled; do not delete it or migrate
user data as part of a docs-only decision. If a native-evidence route qualifies,
the affected seams would be:

- `ctranslate2.rs`: retain the source and hypothesis tokens/evidence from the
  producing call before decoding; legacy translation behavior remains stable.
- `inline_markup/provider/{pieces,contract}.rs`: bind evidence and contributor
  ranges to the exact pair. Any new ownership units remain separate from real
  tokenizer IDs. Existing structural checks cannot be represented as semantic
  detectors. Do not design a broad new schema before the evidence is proven.
- `runner/aligned.rs`, cache and state: lease only models actually used; avoid
  mandatory auxiliary downloads for an engine-native route. Preserve late-cancel,
  pre-save and atomic batch behavior; version new evidence/cache semantics.
- Capabilities/UI: qualify each provider, language pair and evidence policy.
  An engine without qualified evidence cannot start strict formatted jobs or
  silently save degraded output. Preserve originals, older variants, audiobooks
  and read-along data.

**Validation/limits:** reviewed the connected engine, piece/contract, extraction,
runner and capability code against the frozen receipts and prior decisions;
checked the primary API/UD documentation above. No new inference, model download,
parser, training, production code or dependency change ran. This is a design/API
assessment, not new semantic acceptance. Only this document changes;
`FORMATTING_ACCEPTED = false` remains intact.
Benchmarks, smoke, fresh CI/app builds and the version bump remain deferred.

## Stage 9 official CTranslate2 token/attention access — 2026-10-20

**Research access is viable; no formatting or production go.** Committed the
strategy reassessment as `9529c94` before this probe. Official APIs return usable
token evidence and full attention matrices for the frozen cases without a local
library patch. This closes an access question, not the auxiliary/fused ownership
gate. No attention-to-style algorithm was selected or tested this round.

### Runtime, assets and fixed probe

Used official prebuilt **CTranslate2 4.7.2** and **SentencePiece 0.2.1** Python
wheels in an isolated `/tmp` environment, with Python **3.12.14** and NumPy
**2.2.6**. CTranslate2 4.7.2 matches the core version bundled by the app's pinned
**ct2rs 0.9.19**. The wheel and Rust/Ruy build are different binaries; matching
version numbers alone do not establish numeric or platform equivalence.
No Python runtime or new dependency was added to Papercut.

Used the existing French/Spanish OPUS-MT packs outside app data. All **16 files**
matched the sizes/hashes in the trusted app manifest before and after inference.
The saved weights are float16; both CPU builds report conversion to **float32**
for inference. This is the library's normal load behavior, not a rewritten pack
or a change to app compute defaults.

Preserved the frozen 58-row set, SHA-256
`8c0a50b23bf51d088e8728991b740b754217a3cc7aaa8a8ccb72686a042ebad5`.
It contains **27 unique language/source inputs** and **47 unique exact pairs**.
The probe records its policy before the frozen-case run, SHA-256
`523ce6fb4708a031491175a8f722ac0047a5f1f6c60701a63bede85dd216d0e5`.
No private sample or relabelled expected target was used.

Matched the strict adapter's decoding configuration: beam size 2, one hypothesis,
unknown replacement enabled, no input truncation, maximum decoding length 512
and returned EOS. Used one CPU worker/two intra threads, without measuring speed
or RAM. The maintained SentencePiece wrapper appends an explicit source EOS;
its position is part of the supplied source-token list.
With unknown replacement enabled, the core already requests attention internally;
the off/on check tests returned-result behavior, not whether attention is computed
or what resources it costs.

### API results

The official [translation API](https://opennmt.net/CTranslate2/python/ctranslate2.Translator.html)
accepts `return_attention` and `target_prefix`; its
[result](https://opennmt.net/CTranslate2/python/ctranslate2.TranslationResult.html)
exposes per-hypothesis matrices. Actual checks distinguish access from ownership:

| Check | Observed result |
| --- | --- |
| Free decoding with attention off versus on | **27/27** identical hypothesis-token lists. |
| Free output versus frozen actual CTranslate2 rows | **26/26** exact decoded matches. Only 3/26 fixed-target rows naturally match, so free attention is not substituted for other frozen targets. |
| Full frozen target as `target_prefix`, without EOS in the prefix | **58/58** rows return exactly the gold token list plus one EOS, with full attention; 47 unique pair calls. Includes both fused negatives and all four wrong-translation controls. |
| Prefixing the model's own free hypothesis | **27/27** identical tokens and bit-identical attention matrices to free decoding. Forced-prefix rows are not empty placeholders in these checks. |
| `score_batch` attention access | Returned scoring results have no attention; passing `return_attention=True` raises `TypeError`. |
| Including EOS inside a complete prefix | The diagnostic returns a second EOS. This must not be silently stripped and counted as an exact single-EOS result. |

Every retained matrix is finite, nonnegative and has rows summing to one within
`atol=1e-5`. Its rows match the **actual returned target-token count**, including
EOS, and columns match the supplied source-token count, including explicit EOS.
Future mapping must exclude framing according to verified provenance, not treat
EOS as a source owner. Shape/row-sum checks are not calibrated confidence tests.

The complete non-EOS prefix supplies an observed public-API route for the frozen
gold-target probe. It is **constrained decoding**, not evidence that the model
would naturally produce those targets, nor a promise that every arbitrary
prefix ends without continuation. Reject a non-exact returned hypothesis rather
than clipping it. This supersedes the earlier “gold-target access not established”
assessment for these tested pairs. It does not prove semantic ownership: even a
deliberately wrong forced translation receives a valid attention matrix.

### Tokens, offsets and native parity

The [SentencePiece Python API](https://github.com/google/sentencepiece/blob/v0.2.1/python/README.md)
provides immutable proto evidence with piece, ID, surface and offsets. Source
encoding retains offsets into the original string. Decoding the **actual
generated pieces** into a proto supplies target offsets without re-encoding a
possibly different tokenization of the decoded text. Target proto pieces matched
the actual hypotheses on all 27 free outputs.

An outside-repo Rust harness used the unmodified pinned `ct2rs` and existing
**sentencepiece 0.13.2** API, with normal Cargo builds and no custom native patch:

- Source token strings, SentencePiece IDs and surfaces match Python on **34**
  inputs: all 27 free-decoding sources plus seven Unicode/normalization/unknown
  controls. Rust reports UTF-8 byte spans; Python proto offsets are scalar
  positions in these checks. Verified both against actual source slices before
  comparing. SentencePiece IDs are not automatically CTranslate2 vocabulary IDs.
- Native decoding with its attention request flag off/on yields the same token
  lists on all **27** sources. Those lists and decoded outputs also match the
  official Python wheel. The Rust wrapper still returns no matrix: its request
  option alone does not expose the underlying attention.
- The controls retain important limits: leading/repeated whitespace collapses
  during decode; decomposed accents normalize; a zero-width-space control
  changes; an Arabic control has an empty leading token span; a private-use
  character is an actual unknown token despite retaining a raw surface.
  Therefore available offsets do not establish unknown/normalization admission,
  grapheme-safe ownership or Arabic quality. Do not round or invent ranges.

The pinned Rust SentencePiece wrapper offers generated-text decoding as a string,
not the Python decoder-proto result used above. A supported native target-offset
path remains to be qualified alongside attention exposure. No binding fork or
ad-hoc FFI is approved. Attention numeric parity across the wheel and native
build also remains untested because the Rust result does not expose it.

### Decision and next gate

Proceed only to an **outside-repo ownership-policy qualification**, if continuing
this research path. Freeze a documented extraction/contributor/rejection rule
before testing the same mandatory positive and fused-negative scopes on these
exact matrices. A new rule must account for auxiliaries linked to subjects,
unaligned paraphrase components and a fused cell's multiple source contributors;
argmax, nonzero attention, row normalization or a mass cutoff alone is not proof.
Keep the wrong-translation controls as explicit limits on semantic claims.

Do not implement native delivery or invest in a binding change until that probe
shows useful ownership evidence. If it fails, stop this policy rather than adding
grammar guesses or weakening the frozen gate. If it passes, independently resolve
maintained Rust attention and target-offset APIs, evidence/cache identities and
platform qualification before changing the app. Other engines remain separate
capability decisions; this is no automatic formatting fallback or parity claim.

**Validation/evidence:** assert-based checks cover policy/frozen/model hashes,
all access counts, exact target completion, matrices, source offset conversion
and native/Python token parity. Artifacts/scripts remain outside Git under
`/tmp/papercut-ct2-attention-evaluation/`, including `receipt.json`, `results.json`,
`native-results.json` and `verification.json`; logs are
`/tmp/ct2-attention-{install,probe,native,verification}.log`.
Only this document changes. No production code, dependency, runtime bundle,
installer/catalog/cache, saved document or app-version change was made.
`FORMATTING_ACCEPTED = false` remains intact. Benchmarks, smoke, fresh app/CI
builds and version bump remain deferred; no new semantic acceptance is claimed.

## Stage 9 frozen CTranslate2 attention ownership policy — 2026-10-20

**No-go for this policy.** Committed the API-access findings as `45535f8` before
this evaluation. The official matrices are accessible, but the single frozen
conservative contributor policy rejects most mandatory positives and still
admits a lexical ownership error. Do not implement it or invest in its native
binding/target-offset delivery on the strength of API feasibility alone.

### Policy fixed before scope evaluation

Used the saved **CTranslate2 4.7.2 / OPUS-MT** attention and genuine SentencePiece
evidence from the preceding probe. No new translation, model inference,
dependency, parser or download was needed. All **58 cases** remain byte-identical,
SHA-256 `8c0a50b23bf51d088e8728991b740b754217a3cc7aaa8a8ccb72686a042ebad5`.
The policy JSON was frozen before deriving ownership, SHA-256
`3114d1e06695d34de638571a39f2952a26d70e705ea8fcb97e16c1f19e13ed19`.
The matrices and expected cases were already available from earlier work; this
is a predeclared diagnostic, **not** blinded or held-out validation.

The rule is deliberately explicit and falsifiable:

1. Reuse Unicode 17 source cells and the previously evaluated English clitic
   target cells, keeping `won't` atomic. Preserve original token IDs/surfaces and
   exact scalar spans. Ignore only leading/trailing whitespace when assigning a
   token to a unique cell; never split or duplicate its vector across cells.
   Unknowns, lossy source normalization or crossing-token geometry reject.
2. For each actual target subword, sum its stored FP32 source-token weights in
   FP64 into source cells. EOS and empty/whitespace-only source tokens form an
   **unresolved control bucket**. Do not mask that mass and renormalize the rest.
3. Sort source-cell masses descending and retain enough to reach **0.90 of the
   raw attention mass**, including every exact tie at the cutoff. This nucleus
   is a hypothesized contributor set, not a calibrated confidence interval or a
   published guarantee of semantic attribution. No cutoff sweep was performed.
4. Union the contributors of all actual subwords in a target cell. Control
   support or incompatible selected/unselected source owners leaves it
   unresolved. Otherwise it inherits their common owner. The frozen source
   selections provide one diagnostic style class versus unstyled text; they
   do not encode a complete authored identity/hierarchy tree.
5. Require every lexical target cell to resolve and every styled source cell
   to participate in a resolved mapping. Unresolved punctuation may remain
   unstyled under the existing lexical-coverage rule. Project only owned cell
   ranges; do not widen hulls, add auxiliaries by spelling/proximity, rewrite
   targets, or inject expected fused-meaning blockers.

Whole-unit uniform inheritance remains the existing separate narrow shortcut;
these frozen cases exercise partial scopes. Exact scoring includes punctuation;
a lexical-only diagnostic cannot silently replace the frozen expected spans.
All 26 actual-output rows use matrices bit-identical to their free-generation
ones. Other rows use the exact gold-prefix matrices, without treating forced
targets as naturally generated translations.

### Observed results

| Positive group | Cases | Exact scope | Exact and admitted | Rejected | Non-exact but admitted |
| --- | ---: | ---: | ---: | ---: | ---: |
| Fixed targets | 26 | 4 | 2 | 23 | 1 |
| Actual CTranslate2 targets | 26 | 4 | 2 | 23 | 1 |
| Total | **52** | **8** | **4** | **46** | **2** |

Only six positives pass the policy screen, all French; four have the required
exact scope. No Spanish positive is admitted. This is failure on the mandatory
set, not an estimate of general-corpus quality or ordinary-input coverage.
There are **zero geometry/unknown/normalization rejections** in this frozen run;
the poor result is not explained by an incompatible ownership coordinate system.

- Control support leaves lexical cells unresolved in **15/26 fixed** and
  **17/26 actual** cases. Mixed owners occur in **10/26 fixed** and **8/26 actual**
  cases. These categories overlap. Removing control columns after observing
  these failures would change the policy, not validate the original one.
- The admitted fixed French negation-only case owns **`is not`**, where only
  **`not`** is required. Complete required coverage and a common selected owner
  still produce a wrong lexical scope. The conservative mass rule is not a
  semantic ownership detector.
- The admitted actual French perfect-whole case owns **apostrophe + `s done`**,
  while the frozen span selects **`s done`**. This is a punctuation-only exact-span
  failure, not evidence of lost auxiliary meaning. Report it separately from
  the negation error; do not relabel it as an accepted exact result.
- **Both fused negatives reject without expected labels or a contraction
  veto.** In French, `won't` has contributors `ne`, `reviendra`, `pas`, exposing
  mixed styled/unstyled ownership. In Spanish, `won't` includes unresolved
  control support; that is generic uncertainty rejection, not demonstrated
  recognition of fused future/negation. Neither rejection repairs the failed
  positive gate.
- Three wrong-translation controls reject, but French changed-colour
  **`rouge` → `blue`** is admitted. The other rejections arise from control/mixed
  evidence, not an independently established mistranslation detector. Attention
  consistency remains distinct from translation correctness.

### Decision and next work

Stop this policy before production coding. Successful matrix access, both fused
rejections and a few exact examples are insufficient when mandatory ordinary
scopes fail and one lexical error is admitted. The result rejects **this fixed
policy on these matrices**; it does not prove that all decoder attention or every
possible policy is incapable of useful alignment.

Do not respond with a threshold/head sweep on the same examples, an auxiliary
allowlist, control-mass deletion or relaxed exact spans. No additional native
integration work is justified for this policy. Keep the shared DOM/asset,
contract, cache and atomic writer work intact and the candidate disabled.

Further research needs an explicitly different, documented ownership signal
with a credible path through the same auxiliary/fused cases, followed by
independently annotated held-out checks. Prefer evidence designed or trained
for alignment over more infrastructure around unqualified weights. No such
replacement is selected here; no custom training, morphology subsystem,
upstream fork, automatic fallback or change to formatting requirements is
approved. Pause strict-path implementation while that feasibility decision is
open. Native attention and target-offset delivery remain later gates only if
a useful policy is first demonstrated.

**Validation/evidence:** helper self-checks cover cutoff ties, same-owner unions,
mixed/control rejection and invalid masses. An independent NumPy implementation
recomputes **291 target-token support rows**, cell ownership, coverage and all
58 outcomes from the immutable saved matrices. Frozen inputs, policy and
attention-file hashes and the exact/punctuation distinction are checked.
Scripts/results remain outside Git under
`/tmp/papercut-ct2-ownership-evaluation/`, including `frozen-receipt.json`,
`policy.json`, `summary.json` and `verification.json`; logs are
`/tmp/ct2-ownership-{freeze,evaluation,verification}.log`.
Only this document changes. No production code/dependency/runtime, install,
catalog/cache, saved document or app-version change was made.
`FORMATTING_ACCEPTED = false` remains intact. Benchmarks, smoke, fresh app/CI
builds and version bump remain deferred; no new semantic acceptance is claimed.

## Stage 9 pretrained alignment-aware translation screen — 2026-10-20

**No replacement selected; keep production work paused.** Committed the frozen
attention-policy findings as `3cbb018` before this screen. Reviewed official
model cards, training metadata, converter source and deployment examples for a
pretrained signal that differs meaningfully from the failed policy. This was a
metadata/source feasibility screen, not new inference or a quality benchmark.

### What the available artifacts establish

| Lead | Verified evidence | Decision for this branch |
| --- | --- | --- |
| Existing French OPUS-MT | The [pinned card](https://huggingface.co/Helsinki-NLP/opus-mt-fr-en/blob/c4aed37b318c763fd177aa449b44e3b783cc6c02/README.md) identifies `transformer-align`. The original archive's training log confirms guided alignment, MSE cost, weight `0.1`, last decoder layer and eight heads. | Already alignment-trained upstream; changing the label/provider does not introduce a new signal. Its tested ownership policy still fails. |
| Existing Spanish OPUS-MT | The [pinned card](https://huggingface.co/Helsinki-NLP/opus-mt-es-en/blob/c96e2c5399ebfae4fc43d9669556b9afa74bb69d/README.md) identifies a `transformer` checkpoint from Tatoeba `spa-eng`, dated 2020-08-18. | No positive evidence here of a supervised alignment head; the project-wide statement that most OPUS models are guided cannot establish this checkpoint's training. |
| Larger French OPUS-MT | [Pinned `opus-mt-tc-big-fr-en`](https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-fr-en/blob/5fa3b3c9fa3bcd65fbf31206c3ec7419c9d9cd7d/README.md) identifies `transformer-big`, French → English, CC-BY-4.0. Its safetensors artifact is **461,429,130 bytes**; config has 1,024 model dimensions and 16 heads. | A translation-quality candidate, not a documented new ownership signal. No matching Spanish big checkpoint was verified. Larger size alone does not justify another attention-policy trial. |
| Multilingual OPUS-MT | [Pinned `opus-mt-ROMANCE-en`](https://huggingface.co/Helsinki-NLP/opus-mt-ROMANCE-en/blob/e9ca9975e3972afd80732f08ce01d3a1339f47f8/README.md) covers French and Spanish, labels itself `transformer`, and tags Apache-2.0. Published PyTorch artifact: **312,087,009 bytes**. | Covers the language pairs, but no specifically trained ownership signal is documented in the inspected card/config. Do not confuse coverage with formatting qualification. |
| OpenNMT / joint alignment training | [OpenNMT-tf](https://opennmt.net/OpenNMT-tf/alignments.html) supports supervised alignment; [OpenNMT-py](https://opennmt.net/OpenNMT-py/options/train.html) exposes alignment loss/layer/head settings. The inspected [pretrained index](https://opennmt.net/Models-py/) does not identify a guided French/Spanish → English pair. The [fairseq joint-alignment example](https://github.com/facebookresearch/fairseq/blob/main/examples/joint_alignment_translation/README.md) is an English → German training recipe. | A training capability is not a ready checkpoint. No suitable, licensed pair with documented trained heads was established from these sources. Do not start custom training or add another runtime on this evidence. |
| TransIns | Its [README](https://github.com/DFKI-MLT/TransIns) and [pinned download script](https://github.com/DFKI-MLT/TransIns/blob/09c41c1907d94e44d02598cb85312071867608a8/download-opus-models.sh) use existing OPUS models for German ↔ English and German ↔ French, with a Java/Marian/Python server deployment. | Useful markup-reinsertion research, not a new mobile-ready FR/ES → EN alignment checkpoint. Adopting its document pipeline would duplicate the shared DOM work and would not establish our auxiliary/fused-ownership gate. |

Artifact byte counts above are repository metadata, **not** converted INT8 pack
sizes, peak RAM or phone performance. No new weights were downloaded. The two
guessed Spanish/Romance `tc-big` repository names returned HTTP 401 from the
public API; that does not prove that no suitable checkpoint exists elsewhere.
The old original Spanish ZIP linked by its card returned HTTP 404. Retain the
existing verified app pack; neither response is a reason to change a download
URL, claim loss of all Spanish weights, or invent a replacement manifest.

### Conversion does not provide a missing ownership guarantee

Inspected unmodified **CTranslate2 4.7.2** source matching the app's bundled core:

- The [Marian converter](https://github.com/OpenNMT/CTranslate2/blob/v4.7.2/python/ctranslate2/converters/marian.py)
  reads `transformer-guided-alignment-layer` from the embedded model config and
  selects one head. This is an official conversion path, not a reason to patch
  upstream C++ or introduce another inference runtime.
- The [Transformers Marian loader](https://github.com/OpenNMT/CTranslate2/blob/v4.7.2/python/ctranslate2/converters/transformers.py)
  uses the specification defaults: last layer, one head. The original French
  training log explicitly says `last`; this screen found no layer-setting
  mismatch that would justify a different layer or a head sweep.
- Read the two **existing** verified `model.bin` files using the pinned official
  v6 serialization layout, including bounds, scalar dtype, aliases and EOF
  checks. Both contain `decoder/alignment_layer = -1` and
  `decoder/alignment_heads = 1`. The [decoder source](https://github.com/OpenNMT/CTranslate2/blob/v4.7.2/src/layers/transformer.cc)
  interprets these as the last layer's first head. Neither pack was rewritten;
  their SHA-256 hashes still match the earlier access receipt.
- The [OpenNMT-py converter](https://github.com/OpenNMT/CTranslate2/blob/v4.7.2/python/ctranslate2/converters/opennmt_py.py)
  reads the trained alignment settings when `lambda_align` is nonzero; otherwise
  it also defaults to the last layer's first head. Converter support is a
  delivery possibility only if an actual qualified checkpoint is identified.
  Full-context alignment behavior, if required, needs its own parity check.

This corrects a possible misunderstanding of the preceding recommendation:
“prefer alignment-trained evidence” does **not** imply that our French model was
untrained for alignment, that conversion discarded its selected layer, or that
an official attention result is a complete source-contributor graph. Guided
word alignment and Papercut's exact semantic style ownership are distinct
acceptance tasks. A low alignment-error rate or normalized matrix alone cannot
certify auxiliary scope, mixed-owner contractions or translation correctness.

The French original ZIP includes a **CC-BY-4.0 license**, whereas its Hugging Face
card tags Apache-2.0. Record both provenance layers; do not treat a card tag as
clearance to omit original notices. This screen changes no existing notice
payload and makes no new redistribution approval. Any future replacement must
resolve the artifact-specific notice/provenance requirements before admission.

### Next decision and completion gates

There is no justified next production implementation stage from this screen.
Keep `FORMATTING_ACCEPTED = false`, the shared DOM/source-owned renderer, asset
preservation, pure contracts, cache and atomic writer intact. Keep Bergamot
upstream work shelved. Do not resume the failed policy with threshold/control
changes, train an NLP subsystem, replace the existing translator by size or
popularity, or silently downgrade formatting requirements.

The following follow-up was recommended at the time of this screen. The user
subsequently declined maintainer outreach and approved the boundary path below.
No questions were posted.

The proposed follow-up was a **checkpoint/evidence clarification**, not
more infrastructure: prepare a short maintainer question asking for an existing
FR/ES → EN checkpoint with its supervised head/layer, extraction method,
auxiliary/null-link behavior and redistribution terms. The French original
settings are already known; ask about contributor coverage rather than asking
whether attention can be returned. No issue/message was posted here.

If a concrete new signal is identified, the next bounded probe must:

1. Pin the checkpoint, license/notices, tokenizer, trained head/layer and official
   offline extraction path. Establish a plausible converted pack/runtime route
   before downloading large weights; no binding work merely to expose a matrix.
2. Predeclare the ownership/extraction policy without tuning it on the 58 known
   cases. Preserve their targets, expected spans and fused-negative semantics;
   record wrong-translation controls separately from alignment correctness.
3. Pass the mandatory exact positive scopes and fused-negative rejections, with
   real token offsets and explicit abstentions, then check separately annotated
   held-out cases. A few repaired examples do not qualify a provider or engine.
4. Only after useful ownership evidence passes, evaluate maintained Rust
   delivery, identity/cache provenance, pack admission and platform execution.
   French/Spanish success would not establish Arabic support.

**Validation/evidence:** versioned cards/configs/API metadata are saved with
source URLs, revisions, lengths and hashes under
`/tmp/papercut-alignment-provider-screen/`. The bounded HTTP range probe read
only the French ZIP directory and small documentation/config/license/log files;
it did not fetch the compressed NPZ weights. Local attribute inspection leaves
both existing pack hashes unchanged. Source/metadata assertions, documentation
anchors, disabled-capability check and `git diff --check` pass. Only this document
changes. No inference, production code/dependency/runtime, catalog/install,
cache/storage or app-version change was made. Benchmarks, smoke, fresh app/CI
builds and version bump remain deferred; no new semantic acceptance is claimed.

## Boundary-based CTranslate2 translation — 2026-10-20

**Current decision:** French/Spanish → English CTranslate2 jobs now translate
source-owned fragments instead of translating whole passages and guessing
where their emphasis belongs. The user accepted the potential fluency loss at
formatting boundaries. This supersedes the previous prohibition on independent
fragment translation for this path. Maintainer outreach is cancelled; no draft
was posted. The strict alignment candidate remains disabled and unqualified.

### Architecture and limits

- Keep the existing HTML/EPUB reader snapshot, DOM identities, owned assets,
  model leases, job cancellation, progress, cache and atomic writer. No model,
  runtime, dependency, download manifest or provider selector is added.
- Plan one segment per normalized reader block, capped at **32 KiB UTF-8**.
  This avoids splitting authored links/IDs at an arbitrary outer 900-character
  cut. Protected transport is independently capped at 32 KiB; existing depth,
  128-code, 128-unit and reconstruction-work limits remain enforced. Oversized
  or unsupported structures fail before saving rather than flattening markup.
- The shared exact-piece planner gains a separate boundary policy. Cuts come
  from every source shell endpoint and note/media attachment, with source
  whitespace copied explicitly. Compatible adjacent text within one scope is
  already coalesced by the DOM model; distinct source elements retain their
  identities. A uniformly styled sentence remains one MT input when it fits.
- Reuse the existing native tokenizer budget and complete-decoder checks.
  Each actual prose input is at most **900 Unicode scalars** and fits the
  OPUS-MT **448-token** source budget including framing. Over-budget prose is
  split at sentence/clause/whitespace boundaries. Calls contain at most 16
  inputs; source and target grapheme boundaries and the combined target size
  are checked. The unmodified CTranslate2 core performs inference.
- Code, `translate=no`, note labels and media payloads are copied, never sent
  to translation. Boundary extraction can traverse table/figure prose inside
  a selected reader block while retaining cells, captions and media shells.
  The existing reader's block selection still determines coverage: layout
  outside those selected blocks stays unchanged. This is not a new promise
  to translate every text node in arbitrary HTML.
- Each translated unit returns to the same source parent and position. Target
  word order may change **within** a unit. Source code IDs/order, literals and
  hierarchy are rechecked before cache admission and again during rendering.
  The source-context HTML fragment parser preserves table/list structure.
  Worker quote/note repositioning and legacy phrase matching are bypassed;
  translated text is escaped rather than accepted as model-generated HTML.
- `ctranslate2-source-boundaries-v1` separates job/cache and saved-variant
  identities from legacy phrase-hint output. Boundary cache entries bind
  source structure and fragment context, require validated markup and never
  reuse plaintext phrase memory. A whole batch must succeed before any new
  entries are checkpointed. Existing variants, assets, audio and timing files
  remain intact; a successful retry replaces only its own boundary variant.
- Cancellation is checked around planning, each bounded native call, cache
  admission and before the shared writer claims saving. A failed/late-cancelled
  batch produces no partial saved document. A fully cached run still reaches
  the same pre-save validation and cancellation cutoff.

### Product behavior and tradeoff

This is the default document path for the two existing CTranslate2 OPUS pairs;
it needs no auxiliary formatting pack. `preserveFormatting=true` still means
**strict semantic alignment**, and public requests for it remain rejected by
`FORMATTING_ACCEPTED = false`. The UI labels that separate option “Strict
Formatting (Experimental)” and explains the boundary tradeoff in all eight
locales. Shared piece errors in a boundary job do not advise users to install
an alignment pack.

For a synthetic example, `<strong>viendra</strong>` can retain a complete
translated fragment such as `<strong>will come</strong>` without discovering
word links. This is an illustration, not an asserted model output. A mixed
sentence split across unstyled text, a link and emphasis cannot reorder those
fragments together; articles, agreement, negation and punctuation may translate
less naturally. Fine-grained/intra-word markup is especially risky for fluency;
a boundary through a grapheme is rejected. No semantic ownership or translation
quality guarantee is inferred from valid HTML or passing synthetic tests.

Bergamot transport/planning and HY-MT2's legacy path retain their policies.
The earlier alignment experiments and contracts remain historical/internal
work, not an automatic fallback, an enabled public feature or a requirement
for using the boundary path. No upstream fork, custom training, maintained
Python service or new alignment model is introduced.

### Regression coverage and remaining acceptance

Synthetic provider tests cover uniform/nested emphasis, repeated words,
one-to-many translated fragments, leading notes/images, copied literals,
links/IDs, escaped text, table context, >900-character authored wrappers,
grapheme-safe rejection, malformed/reordered/duplicate/missing markup and invalid
native reply counts/empty/oversized targets. HTML/EPUB lifecycle tests exercise
both model IDs through the real cache, renderer and writer: failed batches,
cache-only resume, missing-markup rejection, late native cancellation,
pre-save cancellation, separate legacy variants, owned assets and existing
read-along/audio files. UI tests verify the model-specific guidance and closed
strict checkbox. A separately opt-in native regression uses only externally
cached, manifest-verified FR/ES packs and synthetic text; it downloads nothing.

Validation passed:

- `cargo test --manifest-path src-tauri/Cargo.toml --lib --offline`: **547 passed,
  4 ignored**. Existing socket/D-Bus regressions require local socket permission.
- The same suite with `--features native-translation-ctranslate2,native-translation-alignment-runtime`:
  **573 passed, 14 ignored**, including the still-disabled strict candidate's
  internal regression coverage.
- Explicitly ran the ignored native boundary regression with
  `PAPERCUT_CT2_MODELS_DIR` pointing to the existing manifest-verified packs:
  **1 test passed across four synthetic FR/ES fixtures**, covering uniform and
  mixed formatting. No downloads or alignment session were involved. The
  existing Linux CPU backend reported its expected FP16 → FP32 compute fallback;
  no RAM/speed claim follows from this correctness check.
- Translation UI/hooks: **105 passed** across six Vitest files. TypeScript,
  eight-locale validation, Rust formatting and `git diff --check` passed.

Logs are `/tmp/translation-boundary-{base,strict,inference,ui}.log` for this local
run. These are engineering correctness checks, not release acceptance.
Model-quality review on representative HTML/EPUB documents, platform packaging
and device behavior remain future work. Benchmarks, app smoke, fresh CI and
version bump remain deferred. No existing audiobook was played or regenerated;
file/identity preservation is regression-tested without changing audio code.

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
