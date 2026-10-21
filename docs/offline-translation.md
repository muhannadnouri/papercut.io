# Offline Translation

Current implementation and next work, reconciled against code on 2026-10-21.
This is the maintained guide; historical experiments and per-stage validation
receipts live in [Git history](#history-and-evidence). Passing structural tests
is not representative language-quality or packaged-device acceptance.

## Supported scope and admission

Translation operates on imported **HTML and EPUB** reader documents. PDF and
other formats are rejected by both UI admission and the native source loader.
Models are installed explicitly; installed translation runs offline without a
cloud service. Original documents remain available alongside generated variants.

| Provider | New public jobs and downloads | Current role |
| --- | --- | --- |
| OPUS-MT French → English / Spanish → English, CTranslate2 | Admitted when the native engine is compiled; jobs also require an installed pack | Desktop whole-sentence translation with explicit formatting-review results |
| Bergamot French → English / Spanish → English, WebView worker | Gated with `acceptance-pending` | Internal experiments, installed-file status/removal, existing saved results |
| HY-MT2 1.8B Q8, llama.cpp | Gated with `acceptance-pending` | Internal experiments, installed-file status/removal, existing saved results |
| Retired auxiliary alignment pack | No new install/repair or document inference | Legacy inventory, verified notices and removal only |

The OPUS model IDs are `opus-mt-fr-en-ctranslate2` and
`opus-mt-es-en-ctranslate2`. Source and target are fixed per model. OPUS accepts
only default quality, no glossary and no repair; unsupported direct requests
fail rather than pretending those controls work. It exposes no GPU selector.
Runtime availability, catalog platform recommendations and installed bytes do
not independently grant product admission or certify translation quality.

Catalog-owned admission also guards native starts, worker starts/asset reads and
new downloads. Stale UI or crafted commands cannot bypass the acceptance gate.
Saved-result access and explicit model removal remain available without a source
selection or runnable provider. Partial and zero-byte installations are removable.

The strict semantic-alignment experiment is **retired**, not an opt-in feature.
Its runtime, token contracts, projection, receipts and Cargo features were removed.
Legacy `preserveFormatting: true` requests reject; the internal `Strict` policy is
only a rejection/cache-namespace sentinel. Normal OPUS jobs need no aligner.
Existing translations, source documents, audio and highlighting are not migrated.

## Formatting: what is preserved and what needs review

Sentence meaning takes priority over guessing which translated words inherit
partial bold, italics or link scopes. Translating each styled fragment separately
was rejected because it can change negation and word order despite valid HTML.

1. The shared DOM plan reads the sanitized reader snapshot in source order,
   retaining source-owned tags, attributes, IDs, links and owned assets.
2. OPUS translates whole natural sentences across inline emphasis/link boundaries.
   Code and `translate=no` literals, note labels and media payloads stay outside
   inference. Over-budget sentences may need bounded clause/whitespace cuts.
3. Complete natural-sentence scopes or exact copied content can retain validated
   source wrappers. A token-limit cut or nearby whitespace is not semantic
   evidence that a partial style belongs on particular target words.
4. If a style or attachment lies inside a translated sentence and cannot be
   restored safely, retain the entire affected formatted original block once,
   followed by a clearly labelled, escaped translation. Mark the document
   **Saved — Formatting Needs Review** (`needs-review`), rather than ordinary
   completion (`ready` in saved metadata, `stored` in the job result).
   Ordered-list containers and numbering remain source-owned.
5. Decoder cutoff/empty output, malformed markup, invalid mapping, cancellation
   and save errors remain failures. Review is a planned formatting outcome,
   never a catch-all recovery or permission to save partial output.

For example, `Je <em>ne</em> sais <strong>pas</strong>.` enters MT once as
`Je ne sais pas.` Its formatted original is retained for review beside the
translation. `<strong><em>Bonjour le monde.</em></strong>` can keep both wrappers
around the translated whole sentence. Neither case proves linguistic accuracy.

Review originals and generated labels remain visible with usable source links.
The validated `data-papercut-translation-review="original|label"` markers exclude
those nodes from backend search/section text, reader Find, narration, locators
and read-along highlighting. Those consumers use translated content, including
when a highlight chunk spans blocks. Saved English documents use English review
labels; app setup/status explanations use the eight UI locales.

Preservation concerns **sanitized reader HTML**, not arbitrary original CSS or
scripts. Import sanitization can remove style-based bold before translation;
do not weaken that security boundary to recover appearance. Exact semantic
emphasis on target words remains unsolved for general partial-sentence markup.
Bergamot's own HTML restoration does not remove its output-completeness blockers.

### Bounds and explicit failures

- Default planning uses 900 Unicode scalars per segment and 16 segments per batch.
  OPUS retains whole reader blocks up to 32 KiB UTF-8 for DOM identity, then uses
  sentence/literal planning and the installed tokenizer's actual input budget.
- CTranslate2's Marian adapter reserves an EOS-inclusive safe source budget of
  448 tokens below the 512-position limit. Decoder admission distinguishes empty
  output, token-limit cutoff, missing natural end and overflow. Character caps
  alone are insufficient; oversized indivisible text fails explicitly.
- Exact-piece work is bounded separately: 128 translatable prose pieces and 641
  total metadata units. Protected transport HTML has a 32 KiB bound and at most
  128 source codes, plus hierarchy, identity, literal and depth checks. Restored
  reader HTML has a separate expansion budget that accounts for source-owned
  attributes/literal payloads; it may legitimately exceed the transport bound.
- Internal Bergamot planning caps inputs at 320 scalars with sentence/clause cuts.
  This is not a completeness guarantee, a product-admission bypass or a fix for
  its alignment failures. Worker replies remain bounded and validated.

These limits guard CPU, memory and reconstruction. Do not increase one budget
without checking the connected decoder, batch, cache and renderer limits.

## Architecture and ownership

Keep shared source/data safety separate from provider inference. Formatting
policy is selected once from the admitted request and carried through planning,
cache admission and saving; never infer policy from cached markup's presence.
No engine fork, generic plugin framework or second lifecycle owner is needed.

All Rust paths below are under [src-tauri/src/translation](../src-tauri/src/translation).

| Boundary | Owner and responsibility |
| --- | --- |
| IPC and admission | `commands.rs`, `models.rs`, `capabilities.rs`, `formatting.rs`: window/request/build/pair checks, provider gate and policy |
| Source and shared DOM | `source.rs`, `dom.rs`, `html.rs`: provenance from uploaded-document URL, safe reader snapshot, ordered blocks and owned assets |
| Bounded input | `segment.rs`, `job.rs`, `boundary.rs`, `inline_markup/pieces.rs`: source-owned whole blocks, sentences/literals and exact decoder budgets |
| Protected reconstruction | `inline_markup/protected.rs` and children: source hierarchy, opaque identities, literals, attachments and restoration validation |
| Native engines | `ctranslate2.rs`, `hy_mt2.rs`, `engine.rs`: inference adapters and output contract |
| Worker transport | `bergamot.rs`: one window/owner, nonce, sequence, bounded mailbox, deadlines, stop/cancel and lifetime |
| Worker mapping adapter | `bergamot/adapter.rs`: input admission, opaque HTML, reply binding/restoration and pre-save live-owner confirmation |
| Installation | `model_store.rs`, `model_store/verified.rs`, `model_install.rs`, `model_install/transaction.rs`, `bergamot/assets.rs`: trusted pins, legal payloads, verified delivery and recovery |
| Retired-pack management | `model_store/legacy_alignment.rs`: inventory/notices/removal, never inference |
| Job lifecycle | `runner.rs`, `runner/provider.rs`, `runner/lifecycle.rs`, `state.rs`: job/model claims, validated sequential batches, cancellation and save ownership |
| Resume | `cache.rs`: compatible, source/DOM-bound checkpoints; separate from durable results |
| Rendering | `render.rs`: coverage/order, policy dispatch, protected/review rendering, escaping and serialization |
| Legacy projection | `render/legacy.rs`: private best-effort text/probe fallback for the old provider path; cannot recover invalid sentence/protected claims |
| Durable result | `quality.rs`, `storage.rs`: output sanity checks, upload-backed atomic variant write/list/delete |

The frontend lives under [src/translation](../src/translation):

- `App` mounts `hooks/useTranslationManager.ts` above the conditional tab. This
  remains the single owner of native commands, progress listeners, terminal
  outcomes, worker abort controller, download retry and Library refresh.
  Its `enabled` flag controls listener subscription; a presentation extraction
  must not move the hook into a tab child.
- `components/TranslationPanel.tsx` coordinates selection, capability/busy state,
  model management and job presentation. `components/TranslatedDocumentsPanel.tsx`
  owns the saved list, its disclosure/delete-confirmation/dismissal state and the
  existing callbacks. Confirmation resolves its ID against the latest rows.
  Both views share language/status labels in `utils/presentation.ts`; saved
  `ready` and job `stored` use the existing localized completion label. Dialogs
  retain the house component, accessible labels and focus handling.
- `api/nativeTranslation.ts` and `utils/errors.ts` normalize capabilities,
  WebView support and stable command errors.
- `bergamot/assets.ts` delivers verified binary assets; `bergamot/nativeBridge.ts`
  owns one worker per document attempt, sequence/abort/visibility handling and
  the authoritative Rust save outcome. Stock runtime/glue live in
  [public/translation/bergamot](../public/translation/bergamot).

### Installation, status and removal

Models live in `<app-data>/translation/models/{model-id}`. Trusted manifests bind
paths, byte sizes and SHA-256 pins. Install streams and verifies every file,
checks destination free space, stages beside the destination and promotes with
rollback/recovery. Legal notice payloads are verified with the pack. Failed
retry must re-admit the resulting files; progress alone cannot certify install.

Ordinary native status uses cheap required-file/type/size checks, not a complete
hash scan every render. Auxiliary legacy inventory verifies its pins/notices;
worker delivery verifies immutable byte snapshots and WebCrypto hashes before
inference. These are different trust boundaries, not interchangeable status flags.

`hasStoredFiles` is independent of `installed`: empty/partial/corrupt packs can
still be removed. A known stored size of zero displays zero rather than a full
download estimate. Removal uses trusted model paths and is serialized against
install and active jobs. It does not delete saved translations or segment caches.
Terminated operations cannot be relocked by late progress events.

### Job, cancellation and atomic-save contract

- One attempt owns its job/cache identity and model lease throughout loading,
  inference, validation and save admission. Concurrent identical starts reject.
- Batches run sequentially. Validate the **whole batch** before checkpointing;
  an accepted first result cannot leak into cache if a later result is invalid.
- Cancellation is checked around planning, loads, native calls, cached outcomes,
  checkpoints and save. A native load/inference call may be uninterruptible;
  discard its late result rather than claiming immediate native interruption.
- Worker ownership must still be live at persistence, including fully cached
  retries. Wrong/stale window, nonce, sequence or oversized replies reject;
  each exchange has one absolute deadline. Background/visibility interruption
  cannot masquerade as a completed foreground worker job.
- An atomic save claim serializes cancellation with persistence: cancellation
  before the claim prevents writes; once saving owns the claim, cancellation
  is refused and Rust's result is authoritative. An unknown frontend outcome
  refreshes Library rather than asserting that saving failed or was cancelled.
- Shared upload replacement/delete transactions and recovery protect existing
  variants, indexes and assets on write failure. Model removal is independent
  of document deletion. No existing audio or timing files are rewritten.

Translation deletion validates the exact raw upload ID before storage access and
requires a matching provenance row before touching files. Ownership deletion is
held in an immediate SQLite transaction through staged-file deletion and commit.
Unknown valid IDs are harmless, even if unowned live/staged files exist. The
existing audiobook-reference lock covers the pending/completed reference check
and deletion, so source-reference publication cannot race past it. Failed DB
deletion restores staged files and rolls back provenance/index changes.
Derived replacement uses the same short lock through file recovery, promotion,
DB commit/rollback and backup cleanup. Parsing and inference stay outside it.
Library deletion validates the requested source kind against stored metadata and
returns the stored canonical URL. Audiobook checks compare upload IDs, so query/
fragment aliases in either requests or existing manifests cannot bypass them;
bundled document references retain their exact-URL behavior.
Deletion requires exact stored ID/directory case and rejects case-only aliases
or collisions before staging. Existing mixed-case legacy IDs remain valid;
no URLs, directories or metadata are lowercased or migrated. Audiobook reference
checks conservatively ignore ASCII ID case across supported filesystems.

## Storage, cache and compatibility

There are two distinct forms of storage:

| Data | Current implementation |
| --- | --- |
| Durable variant | Existing upload database/files, shared sections/assets/FTS and a `translated_documents` provenance row; translation schema version `1` in `upload_schema_metadata` |
| Retry/resume | JSON at `<app-data>/translation/segment-cache/{cache-key}/segments.json`, cache version `1`, inline strategy version `3` |

There is **no production `translation_segments` SQLite table**. Metadata records
source URL/nullable source ID, languages, model/engine, quality, settings/glossary,
status, path and timestamps. Deleting the source sets the provenance FK to null;
it does not delete the derived upload. A new job still needs a readable source.
Saved translations open without loading the model that created them.

Both translation and Library single/batch/folder deletion remove the variant's
provenance, upload, section and FTS rows in one transaction. Shared deletion checks
for the optional translation table, so existing version-1 databases need no schema
migration or prior visit to the translation screen. Deleting an original retains
its independent variants and their owned assets. Missing variant files do not
prevent removal of owned metadata; repeated deletion returns not found safely.
Without provenance, translation deletion never cleans up unowned files, including
staging left after a committed deletion whose filesystem cleanup failed. Such
cleanup failures remain explicit errors; this stage adds no orphan-file collector.

Successful saved rows use `ready`; successful job responses/progress use
`stored`. Both use `needs-review` for retained-original results. Listing returns
the saved row's status unchanged, so presentation/tests must distinguish these
contracts rather than supplying a fabricated `stored` row for every success.

Cache keys bind document URL, languages, model, quality, repair, formatting policy
and ordered trimmed glossary entries. Resume additionally checks segment text
hashes, DOM/location/context identity, planning settings, provider identity and
inline strategy. Plain phrase memory cannot supply protected mappings or review
outcomes. Old `aligned` records are ignored. The sentence/review policy tag is
`ctranslate2-sentences-review-v1`, distinct from retired fragment policies.
Native cache identity does not automatically fingerprint every engine binary;
review compatibility deliberately when changing runtime/model semantics.

Saved variant IDs bind source document ID, languages, model, quality, repair,
glossary and the sentence/review policy. They exclude job ID, timestamps and
hardware toggles. Identical settings atomically replace the same variant;
changed supported settings create a separate variant. Source changes invalidate
incompatible segments but do not themselves create a new variant ID.

Settings retain provenance, provider identity, source format, job information,
policy and review counts. Existing saved HTML/metadata/audio are not retrofitted
when cache policy changes. Do not confuse retry receipts with reader documents.

## Builds and CI

Use the existing entrypoints in [package.json](../package.json). Native C++
translation requires CMake and the platform toolchain; the Linux llama/Vulkan
build also needs Clang/libclang, Vulkan headers/loader, SPIR-V headers and `glslc`.
A missing `cmake` executable is a build prerequisite failure, not a model failure.

```sh
npm run desktop
npm run desktop:no-translation
npm run desktop:static
```

`desktop` builds release bundles with native TTS, conceptual search and both
native translation engines. Provider admission still allows only the OPUS pairs.
`desktop:no-translation` retains native TTS and conceptual search. Static builds
use static TTS and omit the shared-build conceptual-search feature; translation
is still compiled unless explicitly disabled. Launch the rebuilt artifact
separately (AppImage on Linux); compiling is not an installed-app smoke test.
Browser/Vite fallback is not native inference acceptance.

### Worker-Only Mobile Build And CI Opt-In (2026-10-19)

This heading preserves the link from [Apple distribution](apple-distribution.md).
Android/iOS default builds do not add the translation host. Explicit opt-in adds
only `native-translation`, without native CTranslate2/llama.cpp, model downloads
or deployment-target changes. **With today's provider gate, a worker-only build
has no runnable public document translator.** It retains management/saved results
and internal host tests; do not install a gated provider as an acceptance shortcut.

```sh
# Android SDK/NDK: existing native TTS/search build plus worker host.
npm run android:apk:native-tts -- --translation-worker
# Custom worker-only APK build without preparing native TTS libraries.
npm run android:apk -- --translation-worker android build --apk --debug --target aarch64
# macOS/Xcode: unsigned simulator and generic iPhoneOS link checks.
npm run ios:ci -- --native-tts --translation-worker
npm run ios:ci:device -- --translation-worker
```

Wrapper flags precede `--`; tokens after that separator belong to the runner.
Worker opt-in during project initialization rejects before SDK work. Existing
custom targets/features/runner arguments remain supported. For Apple setup and
distribution requirements, use the linked distribution guide.

In **CI → Run workflow**, choose the current branch, `validation_scope: mobile`
or `full`, `mobile_translation: true` and `provider_inference: false` for host
compilation. Mobile opt-in defaults off and rejects non-mobile scopes. The APK
artifact is `papercut-android-native-tts-translation-worker-apk`; ordinary release
and default artifact behavior stay unchanged. `provider_inference` runs separate
**search-provider** inference checks, not translation quality validation.

A compiled worker host still requires a secure context, Workers, WebCrypto and
WASM SIMD; verified modules and protected output checks remain mandatory. Build
success does not qualify phone RAM, latency, background execution or translation.

## Validation and acceptance

Use synthetic regression fixtures in tracked tests. Private articles/repros stay
outside tracked code, tests and documentation; do not add named-corpus references
or benchmark helpers to keep them alive. The immutable evidence JSON below is
retained because production asset-pin tests consume it.

Useful existing checks (Cargo `--offline` requires cached dependencies):

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib translation::
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --no-default-features --features native-translation --lib translation::
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --features native-translation-ctranslate2,native-translation-llama --lib translation::
npm test -- src/translation
npm test -- scripts/shipping-builds.test.js scripts/mobile-translation-ci.test.js
npm run build:typecheck
npm run check:i18n
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git diff --check
```

Native-feature checks require the native toolchains. Ignored real-inference tests
are not model-quality passes. A source refactor should run the affected suites;
a documentation-only change need not rebuild every runtime. Historical counts
are bound to their commits in the archive, not fresh CI for the current head.

Focused review of consolidation commit `8213dd9` on 2026-10-21 traced admission,
source/render/cache/save ownership, model verification and build/CI commands.
Assumed one foreground HTML/EPUB job with sequential bounded batches.
Checks passed: 221 base Rust translation tests, 121 frontend translation tests,
36 mocked shipping/CI tests, local/inbound/archive links, unchanged evidence
bytes, command/test-path checks and diff whitespace. Ten CI-script cases first
hit sandbox subprocess `EPERM`; all 11 cases passed outside that sandbox.
No production code changed. This does not add inference, full CI, language-quality
or packaged-device acceptance. The saved-status presentation gap identified
here is addressed by the saved-document extraction below.

### Acceptance cases to retain

This matrix consolidates distinct regression and manual checks. It records
required behavior, **not completed smoke**. Evaluate complete translation quality
separately from HTML fidelity and lifecycle success.

| Area | Required checks |
| --- | --- |
| Scope/admission | HTML and EPUB; PDF/other-format rejection; fixed FR/ES → EN pairs; unsupported quality/glossary/repair rejected; gated downloads/commands reject even with installed files; no OPUS hardware control |
| Offline setup | Explicit model install, disconnect network, translate; no cloud dependency; missing/partial/corrupt pack, hash/size/path/symlink rejection, low space, failed install/retry/rollback and reinstall |
| Management | Confirm/cancel removal, failed removal/retry, zero-byte/partial pack removal and honest size; active-job/install contention; saved documents/cache/audio survive removal; no runnable engine still allows saved access |
| Meaning | FR/ES paragraphs, chapters and long books; negation, adjective/word order, repeated terms/names and terminology across chapters; human review of omissions, hallucinations and awkward prose |
| Whole-sentence styles | Nested bold/italic covering whole sentences restored; partial styles retain the formatted original once with labelled translation and durable `needs-review`; never silently flatten or guess target emphasis |
| Source structure | Headings, paragraphs, tables, nested scopes, original IDs/internal links, ordered lists/numbering, literal islands and copied punctuation/Unicode; source remains independently openable |
| Attachments | Leading/trailing notes, backlinks, images inside styled parents, quotes in either order and inline identities; correct source hierarchy/order, no duplicated labels or lost targets |
| Owned assets | HTML images and EPUB-rewritten chapter assets stay usable after save/reopen; missing/unsafe paths fail safely; translation cannot introduce arbitrary asset URLs or executable markup |
| Bounds/errors | Dense markup, 900-scalar segmentation, real tokenizer limits, long indivisible words, grapheme cuts, empty/cutoff/missing-end output and reconstruction limits; explicit failure, no partial saved variant |
| Progress | Model-loading versus inference states, batch/segment/cache counts; cancellation/failure remain terminal rather than false 100% success; late events cannot erase terminal/review state or relock retry |
| Cancel/restart | During loading, inference, between batches, cached completion and immediately before save; retry after cancel/restart reuses only valid complete checkpoints; duplicate starts reject |
| Resume integrity | Same-URL changed source/DOM, settings/provider/policy mismatch, invalid cache and retired alignment receipts; mixed cache/fresh source order; invalid later batch row cannot checkpoint earlier rows |
| Save/delete | Disk/write/transaction failure and recovery; identical rerun replaces one variant, supported changed settings separate; delete variant leaves original intact; source deletion leaves saved variant readable but new source jobs reject |
| Reader/search/audio | Open original/variant independently; search/Find/narration/read-along include translated text only, exclude review originals/labels across chunks; links remain usable; existing audiobooks/timings unchanged; new TTS uses variant text |
| Presentation | Eight locales, disabled/build/acceptance reasons, narrow layout, light/dark theme, keyboard/focus/dialog labels; saved-result access independent of selection/runtime; job and delete status dismissal independent |
| Resource/platform qualification | Sustained long-book memory/thermals and cancellation latency; packaged desktop and physical-device checks, foreground interruption/background limits; compile success alone is insufficient |

Deferred qualification retains these distinct cases when their providers become
eligible: Arabic undiacritized prose/names/classical terms with RTL source and
LTR target; Chinese academic names, Russian terminology, German compounds;
glossary consistency and chapter repair for engines that actually support them;
HY-MT2 physical Vulkan detection/on/off, software-only Vulkan rejection and
CPU/GPU-compatible cache/variant identity. These are **not current supported
language or mobile quality claims**. Benchmarks, broader smoke and version bump
remain deferred.

## Saved-document presentation — implemented 2026-10-21

`TranslatedDocumentsPanel` now owns presentation only. The parent passes rows,
model names and existing open/delete callbacks; `App` and `useTranslationManager`
still own native operations, worker lifetime, job results and Library refresh.
No new store, listener, IPC, dependency, CSS or localization key was introduced.
Disclosure/dismissal state survives prop refreshes while the view is mounted;
local presentation state resets when the view unmounts. No inference, provider
admission, cache, metadata, original document, audio or timing behavior changed.

Panel integration regressions cover actual saved `ready` versus job `stored`,
`needs-review` and existing `stored` handling; access without a source/runtime or
model; model-name fallback; delete confirmation/cancel/focus restoration; target
removal or metadata refresh while confirming; failed deletion/retry; collapse
across updates and empty refresh; independent job/delete status dismissal and
new outcomes. Removing `ready` localization makes the new real-row test fail.

Validation: 132 frontend translation tests passed, including 39 Panel cases;
TypeScript, changed-file ESLint, all eight locale checks and diff checks passed.
Evidence JSON is unchanged. These are synthetic interaction/lifecycle checks,
not browser-layout, native-inference or packaged-device acceptance.

Focused review of `f4cbc27e` traced both views, shared label callers, the parent
manager/App wiring, deletion API/storage boundary and house dialog/disclosure.
Assumed one foreground job and a local saved library. No actionable correctness,
security, data-loss, performance or unnecessary-abstraction finding was identified
in this extraction. The saved list/dialog JSX is retained, label logic differs
only by localized `ready`, and no second async owner or per-row command was added.
Fresh checks passed: 137 translation/dialog tests, TypeScript, changed-file lint,
eight-locale validation, local/inbound/history links, evidence identity and diff
checks. Disclosure/dismissal resets on view unmount are presentation-only;
manager-owned jobs and results survive conditional tab presentation as before.

## Branch integration/readiness review — 2026-10-21

Reviewed `4570dbf6` against local `main` (`8ebc9d0f`), using merge base `871d7026`:
118 branch-changed files, including tests, stock worker glue and retained evidence.
Main has one additional README-only commit; no merge or remote fetch was performed.
Assumed one foreground HTML/EPUB translation job with a local library;
crafted IPC requests still need
native validation. Traced import/sanitization, shared DOM/assets, sentence/review
planning, batch/cache admission, cancellation/save ownership, derived upload
storage, reader/search/TTS consumers, model management and build/provider gates.
No new inference, private corpus, provider re-admission or production changes.

The current separation is useful: OPUS keeps sentence context with explicit
review originals; shared source/cache/writer guards remain connected; worker
transport and mapping are separate; gated providers cannot become runnable from
installed files. Search/Find/narration exclude review originals/labels. Desktop
builds compile native engines; mobile opt-in compiles only the worker host and
still has no admitted public provider. Passing these paths' existing tests is
not a language-quality or packaged-platform acceptance result.

### Deletion-safety corrections — 2026-10-21

The integration review's three confirmed findings are addressed in native storage:

1. **P1 — Unchecked identity/ownership:** validate raw hexadecimal IDs before any
   storage bootstrap. The derived deleter requires provenance deletion to affect
   a row inside its immediate write transaction before staging files. Original
   HTML/EPUB/PDF IDs and unknown IDs cannot remove source files or index rows.
2. **P1 — Audiobook dependency bypass:** derived deletion uses the existing
   process-wide source-reference lock and checks the canonical variant URL.
   Pending and completed references reject deletion without changing reader,
   asset, index, provenance, audio or highlighting data.
3. **P2 — Ghost saved translations after Library deletion:** shared row deletion
   removes optional translation provenance atomically with upload/search rows.
   Single, batch and folder commands reuse it; source deletion still nulls only
   the source FK and preserves independent variants. No schema migration is needed.

Permanent synthetic IPC regressions live in
`src-tauri/src/translation/deletion_tests.rs`, registered through the existing
admission-test fixture. They use the real SQLite/reader/asset writer and actual
commands, without loading a model. Coverage includes malformed/unknown/original
IDs, both deletion screens plus batch/folder, zero/missing files, conflicting
staging, transaction failure/rollback, interrupted pre-commit staging/retry,
repeat deletion, pending/completed/unrelated audio references, serialization
with pending-reference publication, and original-source deletion semantics.
The original six regression tests were run before the production fix: five
failed, reproducing all three findings; the source-retention case already passed.

Validation after the fix: 509 base Rust library tests passed (4 ignored), 230
native-feature translation tests passed (2 ignored), 231 worker-only translation
tests passed, and 535 frontend/script tests passed (1 skipped). TypeScript,
eight-locale validation, Rustfmt, links/evidence and diff checks passed. Existing
immutable evaluation evidence is byte-identical. This is storage/lifecycle
validation, not language-quality, packaged-platform or new inference acceptance.
Cross-platform CI/linking, installed-app behavior, current dependency advisories
and real-device resources were not revalidated here.

### Save/delete serialization and URL identity — 2026-10-21

The [focused deletion review](https://github.com/muhannadnouri/papercut.io/blob/30d2344e0cef78f83b500acd1e520a92c2a60d49/docs/offline-translation.md#deletion-safety-review--2026-10-21)
reproduced two remaining P1 gaps: a save/delete overlap could leave a saved row
without live files or backup, and Library URL aliases/wrong extensions bypassed
canonical audiobook references. Both are addressed in shared native storage:

- `document_uploads/derived.rs` reuses the existing source-reference lock for
  the complete file/SQLite replacement, including recovery and rollback. Both
  deletion routes already hold this lock. Connections are opened before saving,
  without retaining a write transaction while waiting for the lock; mutation
  takes the reference lock before its SQLite transaction. No new job manager,
  lock registry, provider gate, schema, cache identity or dependency was added.
- `document_uploads/pipeline.rs` resolves stored upload metadata before deletion,
  rejects HTML/PDF kind mismatches and returns the canonical stored URL. When
  metadata is already gone, canonical identity still permits staged-file retry.
  `native_tts/references.rs` compares upload IDs on both sides of the reference,
  protecting pending/completed sources even with legacy query/fragment aliases.
  Existing bundled references still use exact matching; audio/timings are untouched.

Four new regression tests extend the existing synthetic native fixture. Controlled
metadata callbacks coordinate both operation orders and save/delete failures;
assertions cover HTML/assets, source preservation, provenance/index rows and
staging/backup cleanup. IPC cases cover 48 HTML/PDF × reference/request alias ×
single/batch combinations, wrong-kind rejection after removing audio, successful
canonical responses and protected staged retries without metadata. Existing
reference tests also cover alias manifests and unchanged bundled matching.
The overlap and aliased-request assertions failed against the prior production
code before the fix. No private samples, model weights or production test hooks
were added; concurrency checks live in `translation/mutation_tests.rs`.

Validation: 513 base Rust library tests passed (4 existing ignores), 234
native-feature translation tests passed (2 existing ignores), 235 worker-only
translation tests passed, and 535 frontend/script tests passed (1 existing skip).
TypeScript, eight locales, Rustfmt, documentation links/evidence and diff checks
passed. Evaluation evidence is byte-identical. The explicit post-commit cleanup
limit above is unchanged; this does not add inference, language-quality or
packaged-platform acceptance.

### Exact-case deletion identity — 2026-10-21

The [case-alias review](https://github.com/muhannadnouri/papercut.io/blob/03c12ccc7058ac9c59f307ee6124a21f4aaeae2a/docs/offline-translation.md#mutation-and-identity-follow-up-review--2026-10-21)
identified uppercase IDs bypassing exact DB/reference matching while a filesystem
could resolve them to lowercase storage. The shared destructive-boundary rule is
now **unique, exact stored case**, rather than globally lowercasing existing IDs:

- `store::validate_upload_deletion_id` rejects another upload ID differing only
  by ASCII case before either translation or Library deletion proceeds. This
  includes ambiguous case-only metadata collisions, even when an exact row exists.
  Syntax validation/read parsing and transfer restoration retain legacy ID case.
- Shared staged deletion checks actual live/staged directory names before any
  recovery/rename. Missing metadata cannot turn a case alias into a cleanup retry;
  exact-case retries still work. Errors roll back owned provenance and leave
  reader/assets/index/audio data intact. Batch/folder routes reuse the same gate.
- Audiobook reference matching ignores ASCII case for upload IDs on both sides,
  conservatively protecting potentially shared storage; bundled exact matching
  remains unchanged. No source URL, ID, directory, audio or timing migration occurs.

Four permanent synthetic IPC regressions in `translation/case_identity_tests.rs`
cover upper/mixed-case HTML/PDF requests with/without audio, exact mixed-case
legacy variants through translation/single/batch deletion, referenced staged
retries without metadata, live/staged filename aliases and conflicting metadata.
All four failed before the production fix and pass afterward. These assertions
exercise rejection on case-sensitive Linux too, without pretending to emulate
Windows filesystem aliasing. A real case-insensitive/device run remains deferred.

Validation: 517 base Rust library tests passed (4 existing ignores), 238
native-feature translation tests passed (2 existing ignores), 239 worker-only
translation tests passed, and 535 frontend/script tests passed (1 existing skip).
TypeScript, eight locales, Rustfmt, links/evidence and diff checks passed. No new
schema, cache identity, provider admission, model, dependency or app version.
Benchmarks, new inference, smoke and cross-platform CI remain deferred; structural
storage validation does not establish translation quality or release readiness.

### Exact-case deletion review — 2026-10-21

Reviewed `3cca1b6b` and its connected translation/Library single, batch and folder
deletion paths, shared save/delete/reference lock, SQLite ownership/rollback,
live/staged filesystem identity, transfer ID parsing and audiobook references.
Assumed one running app with one foreground translation job and concurrent
local deletion commands. No new actionable correctness, safety or test findings;
the scoped deletion fix is ready. Exact mixed-case legacy identities stay usable,
case aliases and ambiguous metadata fail closed, and rejected operations preserve
reader/assets/index/provenance/audio data. The documented post-commit cleanup
limit remains unchanged.

Fresh validation: 517 base Rust library tests passed (4 existing ignores), 238
native-provider translation tests passed (2 existing ignores), and 239 worker-only
translation tests passed. These include all four case-identity regressions and
both coordinated save/delete order tests. No production changes were needed.
Actual case-insensitive filesystems, packaged devices, inference and fresh
cross-platform CI were not checked. This review does not qualify language quality.

## Next bounded work

Deletion safety has no remaining finding from this review. The next gate is
representative FR/ES meaning, formatted-original review usability and rebuilt-app
integration acceptance using the existing acceptance matrix above. Prepare that
handoff before further feature work; run it only when smoke is explicitly resumed.
Keep the existing source/DOM, cache, writer and reference-lock owners. Provider
admission is unchanged; strict stays retired. Benchmarks, smoke and version bump
stay deferred.

No further UI split is justified by this extraction review alone. Model/language
selection, provider admission and shared busy conditions remain coordinated in
the parent; model actions already reuse one button helper. Retain those owners
unless a concrete independent responsibility needs extraction. A shorter file
does not establish better translations or release readiness.

Release acceptance remains a separate gate: representative FR/ES meaning and
review-original usability, HTML/EPUB search/TTS/read-along and rebuilt packaged
behavior. No architectural cleanup certifies it. Bergamot completeness, arbitrary
semantic formatting projection, Arabic/new providers, mobile translation quality,
glossary/chapter repair, export/backup policy and variant grouping remain future
work; do not reopen an aligner integration or library fork without new evidence.

## History and evidence

The [complete pre-consolidation guide](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md)
retains research inputs/methods, decisions, stage-specific tests and review
receipts. Recover it offline with
`git show bdf65cd6:docs/offline-translation.md`. Its historical commands and planned
stages are **not current build instructions**. Useful entry points:

- [Bergamot versus OPUS evaluation and limitations](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#bergamot-versus-opus-mt-evaluation--2026-10-18)
  and [protected mapping integration](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#protected-rich-markup-integration-2026-10-19).
- [Alignment feasibility, stages and rejection evidence](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#engine-independent-offline-alignment-plan--2026-10-19),
  including auxiliary/fused ownership, OmniAlign FP32/four-bit and official CT2
  attention checks. [Last strict-code snapshot](https://github.com/muhannadnouri/papercut.io/tree/b0ecf08e8d7d4d8a0d029b26cf7e2743ebcf0987/src-tauri/src/translation).
- [Sentence/review decision and regressions](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#whole-sentence-translation-with-review-originals--2026-10-20),
  [provider admission](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#provider-admission--2026-10-21)
  and [strict retirement](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#strict-alignment-pipeline-retirement--2026-10-21).
- [Worker/renderer separation and review](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#worker-and-rendering-boundary-evaluation--2026-10-21)
  and [UI/documentation ownership evaluation](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#ui-and-documentation-consolidation-evaluation--2026-10-21).

[offline-translation-evaluation.json](offline-translation-evaluation.json) remains
byte-identical. It records the 2026-10-18 isolated Linux evaluation and exact
Mozilla runtime/glue/model pins; Rust asset tests and the JS worker test load it.
Its process-level RAM, non-equivalent runtime/kernel/beam configurations, small
authored cases and non-blinded inspection cannot establish app memory, a model
ranking, phone performance or release readiness. Further benchmarks stay deferred.

For upstream API context, consult [CTranslate2](https://opennmt.net/CTranslate2/),
[ct2rs](https://docs.rs/ct2rs),
[Firefox's Bergamot documentation](https://firefox-source-docs.mozilla.org/toolkit/components/translations/resources/03_bergamot.html)
and [W3C ITS within-text flow](https://www.w3.org/TR/its20/#elements-within-text).
These references explain engine/markup boundaries, not new local verification.
