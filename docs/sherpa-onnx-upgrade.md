# Sherpa ONNX upgrade and model selection plan

Status: Stages 1–3a accepted; the owner reports Stage 3b smoke tests pass.
Stage 3c.1 proof and Stage 3c.2 local smokes are accepted. The registry
integration is checkpointed at `a5f3d86`; the separate three-crate audit repair
passes local checks and owner smokes. Stage 4a paired listening is accepted with
the explicit English quality tradeoff below; Stage 4b native frontends and owner smoke tests are accepted.
 Stage 4c catalog/install implementation and local qualification are ready;
immutable artifact publication/public downloads and human UI/install acceptance
remain open. Stage 5 remains pending.
Native platform gates remain mandatory at PR preparation. Owner direction updated
November 15, 2026. Research baseline: local `main` at `6baf1858`, Sherpa 1.13.4.
Current branch runtime: 2.0.1, subject to the platform/human gates below.

This is the implementation and human-review contract for this work. The
[native TTS guide](kokoro-tts.md) describes the shipping architecture;
[model maintenance](model-download-maintenance.md) describes downloads and
monitoring; [Android production packaging](android-production-packaging.md)
describes source builds, notices and distribution evidence. Proposed behavior
here must not be represented as shipped or validated until its gate passes.

Current scheduling decision (owner, November 15): paired English/Arabic 7M work
may proceed after local acceptance. Other native-platform tests are deferred to
PR preparation and remain mandatory before merge. This supersedes the earlier
requirement to finish every Gate 3 platform check before starting Stage 4; it does
not waive native testing or vendor-retirement conditions. The later explicit
English 7M quality tradeoff is scoped below; existing-model quality gates stand.

## 1. Agreed outcome and boundaries

- Add the existing translated **Recommended** label to English Kokoro v1.0 and
  Arabic Piper Kareem Medium. Use those models as the starting selection when
  entering their speech language, without overriding a valid existing choice.
- Upgrade Sherpa surgically while preserving all currently offered languages,
  voices, saved audio, highlighting, and ordinary model installation/playback.
- Qualify **English Kokoro 7M and Arabic Nabra 7M together** as additive,
  single-voice choices. Neither replaces an existing model. Do not quietly ship
  English alone if Arabic fails qualification; return that decision to the owner.
- Prioritize generated speech quality, particularly pronunciation. Existing models
  require parity or improvement. On November 15 the owner specifically accepted
  English 7M as a more robotic/less crisp smaller alternative and Arabic Nabra as
  competitive with Kareem; keep Arabic automatic diacritization enabled. This
  explicit exception is not a general waiver for mispronunciation, omissions or
  existing-model regressions; an Experimental label alone provides no waiver.
- No numeric speed or memory targets, performance-tuning project, or benchmark
  gate is required. Users choose the model appropriate to their hardware.
  Crashes, corrupt audio and ordinary usability failures still fail acceptance.
- Owner device checks: **iPhone 16, Pixel 6 Pro, Pixel 7, Pixel 10 Pro**.
  Record actual OS versions and app artifacts when testing; do not invent them.
- Preserve the current upload rule: automatic document-language suggestions
  skip uploaded documents. The language selector remains available there.
- Handle incompatible unfinished saves with explicit Restart/Cancel. Cancel keeps
  progress; confirmed restart discards that job’s partial audio and starts over.
  An engine update alone never regenerates completed audiobooks.

Do not add GPU/provider selection, multiple runtime versions, a plugin/model
framework, Python/PyTorch/Misaki services in the shipped app, unrelated model
families, narrator cloning, new automatic language detection, or broad refactors.
Keep SILMA's existing platform availability and sidecar behavior intact. Adding
new models does not authorize bundling weights into installers or APKs.

Documentation preparation does not authorize implementation. When the owner
starts Stage 1, create the requested `task/sherpa-onnx` branch from then-current
`main`, recording the exact base SHA. Preserve unrelated local changes and do
not silently carry other feature branches into it. Keep these documentation
changes with the planned work. Commit/push/PR/merge/release actions require the
applicable session authorization; the plan itself is not that authorization.

## 2. Verified starting facts and risks

These findings describe the pre-implementation source inspection of November 11,
not device acceptance evidence. Stage 1 resolves the first row's order dependence;
recheck the remaining findings against the actual implementation base.

| Finding | Consequence |
| --- | --- |
| `AudioSetupPanel` and `suggestTtsModel` select the first language match. The TS fallback puts Kareem before Supertonic Arabic; native Rust puts Supertonic Arabic before Kareem. | Recommendation must be explicit and independent of array order. |
| Audiobook IDs contain stable model/voice IDs, cache version, speed, dtype, preprocessor and document identity; display names are absent. | Labels can change without invalidation. Never rename existing IDs for presentation. |
| Legacy manifests without a model ID default to Kokoro. | Preserve global `DEFAULT_MODEL_ID` / `DEFAULT_TTS_MODEL_ID`; Arabic needs a language-specific recommendation, not a global replacement. |
| Sherpa 2.0 removes embedded eSpeak/Piper phonemization and ignores `data_dir`. | A dependency-only upgrade cannot preserve current synthesis. |
| The public generation API accepts phoneme codepoints or token strings. | Evaluate external native phonemization through this API, without a local Sherpa C++ fork. |
| New Kokoro lexicon code can skip unknown words. | A non-silent WAV is insufficient; names, unknown words and mixed-language passages need listening and input-coverage checks. |
| Upstream moved to ORT 1.28.2; Android currently pins Maven ORT 1.27.0. | Resolve one compatible packaged runtime for TTS, Libtashkeel and embeddings. Do not blindly bump the Rust `ort` crate. |
| The app vendors `sherpa-onnx-sys`, including packaging changes and atomic Unix runtime replacement. | Rebase complete bindings and review each local modification; do not overwrite or discard safeguards wholesale. |
| Android/iOS helpers reuse fixed prepared directories based on file presence. | Version changes must reject stale binaries; changing download constants alone is insufficient. |
| iOS 2.0.1 assets are XCFramework ZIP variants under `xcframework`; the existing helper expects an `ios.tar.bz2` layout. | Inspect the archives and establish linking behavior before selecting an artifact. |
| Interrupted jobs reuse existing chunk WAVs; their manifests lack a synthesis-revision identity. | Prevent old/new frontend audio from being mixed during resume. |
| 7M distills have distinct vocabularies, voice packs and frontend assumptions. | Reuse the Kokoro family loader, but validate each complete model contract separately. |
| An upstream INT8 export fix avoids STFT NaNs on ARM. | A runtime upgrade cannot repair an old exported graph; qualify exact model bytes. |

The existing roster is seven Kokoro language entries (English, Mandarin,
Spanish, French, Hindi, Italian, Brazilian Portuguese), Supertonic English and
Arabic, Piper Kareem Arabic, plus platform-scoped SILMA. English includes both
US and British voices. Recheck the catalog rather than treating this list as
permission to remove additions present on the new base.

## 3. Architecture and identity contracts

### Recommendation policy

Use one small policy in the existing TS model helpers, keyed by base speech
language: `en` maps to the existing Kokoro ID, `ar` to the existing Kareem ID.
The model option label, language-switch default and existing document suggestion
must consume that same policy. Resolve against the available catalog; do not
select a model the native build cannot support. Languages without an explicit
recommendation retain their current behavior without a new endorsement.

Keep native capabilities authoritative for availability and model metadata;
the browser/startup fallback must describe matching IDs and capabilities.
Recommendation is presentation/selection policy, not audio identity. No new
Rust DTO field, persisted preference format or generic ranking system is needed.
Preserve manual selections within the current language, saved/imported audiobook
settings, voice recommendations and existing auto-suggestion exclusions.

### Synthesis boundary

Retain the existing path:

`canonical chunk -> optional preprocessing -> synthesis sanitization/normalization -> model-specific native frontend -> Sherpa -> WAV/timing index`

Preprocessing remains optional, user-selected and versioned (for example,
Libtashkeel versus Original text). Required phonemization is internal to native
synthesis; do not expose it as a user toggle that can leave a model unusable.
Never rewrite canonical text, source hashes/spans, chunk IDs, DOM/PDF locators or
highlighting input to accommodate a phonemizer.

Evaluate the official external `piper-phonemize` route first for current eSpeak
paths. Preserve Mandarin's lexicon/FST behavior unless separately proven
equivalent. Recheck text normalization when passing phonemes: bypassing Sherpa's
text frontend must not inadvertently bypass necessary language normalization.
Validate US/British locale selection and switching languages in the same process.

English 7M requires a training-compatible frontend; generic eSpeak is not assumed
equivalent to its Misaki reference. Arabic 7M requires its reference phoneme
cleanup plus the existing diacritization choice. Do not introduce a second
automatic diacritizer. Python reference tools may be used for isolated evaluation,
never as an implicit application runtime. If native quality parity cannot be
achieved within this boundary, stop and return the tradeoff to the owner.

### Models and durable audio

- Preserve existing model and voice IDs, speaker mappings, pinned model archives,
  shared-install relationships and historical missing-field defaults.
- Give each new artifact/precision contract a stable distinct model ID and
  install directory. Proposed names are not final IDs until Gate 4 review.
- Qualify one precision per new model initially; use evidence to choose it.
  Do not add a precision selector or repurpose `dtype: native` into one.
- Weight-file sizes are not download/installed sizes. Publish exact archive
  bytes, checksum, required files, token table and voice pack for each candidate.
- Keep the original model installed and its saved audiobooks accessible when a
  user tries a new model. Do not migrate or regenerate them automatically.
- Preserve `native-save-v4-segmented`, current bundle compatibility and manifest
  readability. An additive generation-revision field may use a backward-safe
  default; do not bump a global schema/cache version to evade compatibility work.
- Older apps need not synthesize newly introduced models. Do not claim reverse
  compatibility for new-model bundles without evidence. Current-version bundles
  and existing saved audio must remain readable by the upgraded app.

### Accepted simple restart contract (updated November 12, 2026)

The owner explicitly chose simple restart instead of retaining partial generations
through rebuilds. This supersedes the earlier backup/journal proposal. Mixed-engine
WAVs are not inherently corrupt: the guard enforces a chosen synthesis-consistency
policy. The accepted listening comparisons did not establish mixed-version resume
as broken. Historical partial jobs lack provenance and are conservatively unknown.

1. Record optional `synthesisRevision` in schema-2 manifests, reflecting the pinned
   engine/frontend/data/normalization contract, not labels or thread counts.
2. Before pending writes, deletion, pruning or chunk reuse, check compatibility
   under the per-book writer lease. Completed audio is exempt from engine restart,
   including complete legacy WAVs whose index needs repair. SILMA is unaffected.
3. For an incompatible/unknown unfinished Sherpa job, explain that Restart deletes
   its saved progress and regenerates from the beginning. Cancel changes nothing.
   Restored queue jobs require the same decision; consent is never persisted.
4. Recheck the exact confirmed generation token under the writer lease, then clear
   only that job’s chunk directory. Keep old provenance until removal succeeds;
   a failed/interrupted reset must never adopt leftover old chunks as new output.
5. Write current pending provenance after clearing. If later generation fails or
   is interrupted, its new compatible progress resumes normally. No backup,
   staging generation, directory-promotion journal or rollback is retained.
6. Completed audio is not automatically touched by an engine change. Explicit
   source-edit replacement keeps the pre-existing save/PDF UX and behavior; this
   pass does not extend it with transactional replacement or a new HTML restart
   flow. No global cache/schema/bundle bump or whole-library migration is needed.


## 4. Stage gates

Stage 1 was accepted by the owner on November 11, 2026. An agent can prepare the next-stage handoff but must
stop after presenting its gate evidence. A named human reviewer/owner records
acceptance before the next implementation stage starts. Green tests alone do
not grant that acceptance. A failed gate means fix within scope or request an
explicit spec decision; never weaken quality requirements or silently drop a
language/platform/model to pass.

### Stage 1 — Recommendation labels and deterministic selection

Scope: TS model helpers, Audio Setup, existing suggestion callers and focused
tests. Reuse existing localized Recommended copy. Update current-behavior docs
only for the implemented recommendation behavior. Leave runtime, dependencies,
model bytes, cache/schema versions, source chunks and manifests unchanged.

Steps:

1. Read all helper/selection callers, capabilities/fallback and preference restore.
2. Implement the explicit English/Arabic policy and label in the existing UI.
3. Add behavior regressions using reordered/native-like and fallback catalogs.
4. Run focused tests, typecheck, locale check and applicable lint; inspect scope.
5. Present the UI behavior and evidence for Gate 1.

Human acceptance:

- [ ] English shows Kokoro v1.0 · Recommended; Arabic shows Piper Kareem Medium · Recommended, alongside existing voice recommendations.
- [ ] Entering English/Arabic chooses the recommended available model regardless of catalog order; no unrelated language receives an invented recommendation.
- [ ] Existing selected Arabic/English alternatives and restored saved/imported settings survive rerender, capability arrival and restart.
- [ ] Existing document suggestions use the policy without expanding detection or changing upload behavior.
- [ ] Known audiobook ID expectations remain byte-identical; labels do not enter cache identity. Fallback and native-like order are both covered.
- [ ] Labels remain understandable in Arabic/RTL and narrow layouts; existing keyboard/screen-reader select behavior remains usable.
- [ ] Diff contains no runtime/model/cache/persistence change. Reviewer records approval to start Stage 2.

#### Stage 1 implementation handoff — November 11, 2026

Owner authorized committing the plan, creating the branch and implementing this
stage. Branch `task/sherpa-onnx` was created from local `main` at `6baf1858`;
the two planning documents were committed as `ce054ead`. The Stage 1 implementation
is committed separately from that documentation commit.

Changes:

- `src/tts/models.ts` owns one explicit `en`/`ar` recommendation lookup, returning
  only models present in the supplied catalog. Arabic document suggestions use
  that lookup before their previous first-match fallback. Detection thresholds,
  Mandarin routing and same-language preservation are unchanged.
- `AudioSetupPanel.tsx` uses that lookup for the model-option label and explicit
  language changes. Missing recommendations keep the previous available-model
  fallback without falsely endorsing it. No effect rewrites an existing selection.
- Focused model/UI tests exercise fallback and reversed catalogs (including
  Supertonic before Kareem), actual select-change events, unavailable recommendations,
  preserved selected alternatives during catalog refresh and existing voice labels.
- Independent historical ID assertions cover Heart, British Emma and diacritized
  Kareem. Audio preferences and saved/imported metadata restoration code are
  unchanged; their installed-app restart/import behavior remains a human check.

Validation on this Stage 1 working diff:

- `npm test -- src/tts`: **31 files, 385 tests passed**, including the Chromium
  fixtures and existing manager/export/import regression coverage. The first
  sandboxed broader runs could not execute browser fixtures (`ECONNRESET`);
  the final run with local browser-process permissions passed. Final log:
  `/tmp/sherpa-stage1-tts-tests.log` (session-local; retain with review evidence
  if needed, or reproduce with the same command).
- `npm run build:typecheck`: passed.
- `npm run check:i18n`: all eight locales passed.
- `npx eslint --max-warnings=0 src/tts/models.ts src/tts/models.test.ts src/tts/components/AudioSetupPanel.tsx src/tts/components/AudioSetupPanel.test.tsx src/tts/storage/AudiobookLibrary.test.ts`: passed.
- Scoped whitespace/local-document-link checks passed. Unrelated pre-existing
  `.gitignore`, `.vscode/settings.json` and owner-only untracked files are excluded
  from the implementation and documentation commit.

Owner accepted Gate 1 on November 11, reporting that smoke tests pass as intended
and the Recommended labels are visible in the UI, and authorized Stage 2. This
records owner acceptance, not an inferred device/accessibility test inventory.
No runtime upgrade,
model download, native inference, app version/cache/schema/ID change, new locale
key or production dependency is part of this stage. Existing uploaded-document
exclusions remain in `useAudiobookManager`; that caller is unchanged.

Manual Gate 1: review the two badges, switch languages, explicitly select
Supertonic in each language, refresh/reopen and confirm it stays selected. Restore
an existing saved/imported audiobook and its settings. Verify Arabic RTL/narrow
layout and keyboard/screen-reader behavior. The owner reported smoke acceptance;
specific device/accessibility receipts were not supplied and are not claimed by
the jsdom tests. Detailed physical-device acceptance remains in Stage 5.

Next, authorized by owner Gate 1 acceptance: Stage 2 only, the bounded native
compatibility investigation. Do not start production migration or expose small
models. No clarification blocks Stage 1; Stage 2 must return evidence for its
frontend, platform-linking and generation-revision/restart proposals.

### Stage 2 — Bounded native compatibility investigation

Scope: reproducible isolated probes and an exact integration proposal. This is
not acceptance to ship new runtime binaries or expose new models. Keep production
defaults/catalog intact while investigating. Reuse existing smoke tooling; keep
temporary outputs in ignored/disposable directories, with reproducible inputs
and retained source-bound evidence rather than an abandoned parallel framework.

Steps:

1. Record the actual base, runtime/wrapper/sys revisions, external phonemizer
   revision, platform archives and hashes. Reverify 2.0.1; a newer tag does not
   automatically expand scope. Do not use GitHub's generic latest-release result
   as a runtime selector because model-only tags also exist.
2. Compare complete upstream Rust/sys APIs and each local vendor modification.
   Classify retained, replaced-by-proven-upstream-equivalent and obsolete changes.
   Preserve atomic Unix library replacement and its regression unless an
   equivalent is verified. Keep Windows behavior explicit.
3. Prove existing Kokoro US/British and Kareem through the public phoneme API,
   plus Mandarin lexicon/FST and smoke of every other existing language route.
   First capture reproducible current-runtime baseline outputs in an isolated
   environment. Compare lexical/phoneme coverage and side-by-side speech,
   including names and out-of-vocabulary words. Keep the original runtime/assets
   available; do not overwrite them in place to run the candidate.
4. Inspect iOS ZIP contents, slices, deployment requirements and linked symbols.
   Choose a layout that supports all app ORT consumers without duplicate/private
   runtime conflicts. Check device and simulator separately.
5. Establish Android source-built Sherpa + external phonemizer + chosen Maven ORT
   inputs. Specify matching source pins, data files, ABI packaging, 16 KB alignment,
   receipts, corresponding-source and notice generation. Automatic prebuilt
   downloads in a new dependency must not bypass the source-build contract.
6. Probe TTS, Libtashkeel and semantic embeddings in one process in both startup
   orders, with repeated model changes/session teardown. Demonstrate the runtime
   actually loaded, not merely the library path requested.
7. Replay prior Nabra failure inputs in a child process, including
   `("لا هذه في ب ب من هل درس " + "الطائرات " * 37).strip()` and the
   reference cleanup of syllable dots/U+02E4/U+032A. Test actual model token/style
   limits, not only source-character limits. The historical issue is not closed
   merely because the new API exists; no local Sherpa C++ patch is authorized.
8. Specify the smallest generation-revision and restart/failure-resume design, exact
   stale-runtime cache invalidation, and the Stage 3 file/test scope.

Human acceptance:

- [ ] Reviewer receives pinned inputs, commands, actual outputs and limits for each probe; compiling or generating a non-silent WAV is not claimed as pronunciation acceptance.
- [ ] Current language routes have a credible supported implementation and reviewed sample evidence; names/unknown words are not silently dropped.
- [ ] Exact iOS linking and Android source/packaging proposals are concrete; unavailable platform execution is explicitly pending, not marked passed.
- [ ] Wrapper, sys bindings and native ABI versions align; one compatible ORT route covers all consumers. Any dependency addition has a specific necessity.
- [ ] Existing native-runtime copy protections survive the proposed vendor rebase; cold, restored and alternating-version caches cannot select stale binaries.
- [ ] Nabra blockers are explicitly resolved by evidence or remain named blockers for the paired model stage.
- [ ] The explicit interrupted-save restart proposal, progress-loss disclosure and failed-reset behavior are reviewable and accepted for implementation.
- [ ] Reviewer approves the exact Stage 3 design and records any remaining execution gates. A platform design that cannot preserve current support blocks progression.

#### Stage 2 evidence and proposed integration — November 11, 2026

Application base: `a8248205925c56e71ee6ae9ec5930dff0184671a` on
`task/sherpa-onnx`, the owner-accepted Stage 1 commit. Production source, Cargo
pins, runtime caches and model catalog were not modified for this investigation.
Only disposable builds read the existing verified model assets. Upstream source
checkouts remained clean; there is no local Sherpa C++ patch.

The local evidence packet is `.cache/sherpa-stage2-2026-11-11/`, ignored by Git.
Its README, scripts, headers, Cargo lock, result JSON, logs, public synthetic text,
audio and `SHA256SUMS.json` describe the actual probes. `listening.html` presents
17 baseline/candidate pairs. Temporary native archives/source/builds remain under
`/tmp/papercut-sherpa-stage2`; the small packet retains their inputs and reproduction
instructions, not a production framework. The packet is not committed or assumed
present on another machine; the pins, results and integration contract below
stand on their own. Reproduce missing artifacts before claiming their checks.

**Pinned investigation inputs** (not a production dependency change):

| Input | Exact identity |
| --- | --- |
| Sherpa wrapper/sys/source | `2.0.1`, upstream commit `af2d8d81dbfa61c4cea4c568da78f3982edca005` |
| External phonemizer source | `1.5.9`, commit `dae4d2d831f9f0817355f7c0e8abd98c2fb0a49d` |
| Linux Sherpa shared archive | `sherpa-onnx-v2.0.1-linux-x64-shared-lib.tar.bz2`, 9,710,131 bytes, SHA-256 `532c45bbd41036d83cd70606797529aa8086c2053395a58967736c727625b120` |
| Linux phonemizer shared archive | `piper-phonemize-v1.5.9-linux-x64-shared-lib.tar.bz2`, 305,777 bytes, SHA-256 `609569420d356f561e13ee6d52ed0edf7dcae225ea498f4cc68587c11d6d295c` |
| iOS Sherpa static XCFramework | `sherpa-onnx-v2.0.1-ios-static.xcframework.zip`, 16,349,037 bytes, SHA-256 `88aa569f71b7903e0bbdfbe6e2e114b3b95862b1ef0759eea3024f82926d7374` |
| Separate iOS ORT static XCFramework | `onnxruntime-ios-static-xcframework-1.28.2.xcframework.zip`, 33,821,118 bytes, SHA-256 `2c2299acbb461d26d4bac4bc85985d40e7c7177ed6072703ae0846d88b0b4599` |
| Android source-build inputs | Pinned NDK `30.0.16248370`, arm64/API 26, the two source commits above and existing Maven ORT `1.27.0` AAR, SHA-256 `077dec5e2d821234c7dc0aba584bec8f999854b546c754cab93a90741c56fbeb` |
| Existing voice models | All three archives match size/hash in `src-tauri/tts/model-manifest.json`; installed Kokoro graph, voices, tokens, lexicons and FSTs were matched against its pinned archive |

All six downloaded Linux/Android/iOS inspection archives were size/SHA-256
verified before extraction; the separate Nabra experiment is described below.
Downloaded Android prebuilt libraries were inspected, not selected for shipping.
The candidate Linux library reports ORT `1.28.2`; current Linux reports `1.27.0`,
using their actual `OrtGetApiBase` version APIs. These versions describe the
inspected binaries, not every untested desktop artifact.

**Executed evidence and limits:**

| Probe | Observed result | What remains |
| --- | --- | --- |
| Baseline 1.13.4 versus 2.0.1 C API | 15 Kokoro/Kareem cases plus Supertonic English/Arabic: 34 successful, finite, non-silent WAV outputs. Covers all current Sherpa language entries, US/British, names, mixed scripts and short input. | Listening parity; the controls bypass app orchestration and do not certify complete saves/highlighting. |
| US/Arabic/Spanish/British/US phonemizer changes | Same process, first/last US phoneme output identical. Current Kokoro/Kareem eSpeak data trees are identical: 355 files, 17,991,651 bytes. | Production initialization/error/lifetime tests and all-platform execution. |
| Official 2.0.1 Rust wrapper with app dependencies | App's actual `tashkeel.rs` adapter, Libtashkeel 1.5.0, ORT rc.13 and FastEmbed 7.1.0 run together. Both speech-first and embedding-first runs pass, including repeated sessions, stable before/after 384-dimensional embeddings and one ORT image in `/proc/self/maps`. | Full app, all embedding variants, concurrency/teardown and other platforms. This is not cross-version ranking parity. |
| Android source builds | Both pinned source projects compile/link for arm64/API 26 against the existing verified ORT 1.27.0 AAR. Both ELF LOAD alignments are at least 16 KB. Phonemizer core has no ORT dependency; Sherpa needs the single shared ORT. | Installed/emulator inference, packaging, source notices and release reproducibility. Probe outputs are unstripped, not release artifacts. Upstream compiler warnings were present. |
| iOS archive/symbol inspection | Device arm64 and arm64/x64 simulator slices exist. Sherpa's aggregate static archive references but does not define `OrtGetApiBase`; separate static ORT defines it. Declared framework minimum OS is 13.0, below the app's current floor. | Actual macOS/Xcode link and inference, simulator thinning, signing/distribution and phonemizer static source build. |
| Upstream iOS Rust mode | A stdlib-only harness running the exact upstream function returns Shared with the static feature enabled. | Keep an explicit-static override and aggregate-static adapter in our sys patch; test actual Apple builds in Stage 3. |
| Warnings/content coverage | French emits two unknown-hyphen warnings on both old and candidate; reviewed English/Arabic controls emit none. Candidate Mandarin numeric FST and lexicon routes generate audio. | No claim that all vocabulary is covered or existing French pronunciation is fixed. Competent listening is still required. |

SILMA is outside Sherpa inference. Its catalog/backend remains unchanged; its
sidecar was not rerun in this probe. No Windows/macOS runtime or iOS app was
executed on this Linux host, and no owner phone ran the candidate runtime.

**Selected architecture for Gate 2 review:**

1. Keep the app's existing native pipeline and optional Libtashkeel boundary.
   Add a small Rust owner for the phonemizer's public C API in the native TTS
   engine: checked CString/count/pointer handling, copied phoneme vectors and
   deterministic result destruction. Link `piper_phonemize_core`, not its
   C++/JNI wrappers. No Sherpa implementation patch, Python service or generic
   frontend framework is needed. Pass phonemes through the official Rust
   `GenerationConfig` API; preserve Mandarin lexicon/FST and Supertonic text paths.
2. Do not adopt `piper-phonemize` 1.5.9's Rust crate unchanged. Inspection finds
   unconditional unversioned data downloading, a global temporary extraction
   directory and no iOS archive-selection branch. Its public C API is available
   on all relevant source-build targets and avoids those build/lifecycle policies.
   Bind only that published boundary, with safety comments and focused tests.
3. Reuse verified eSpeak data from the installed model packs instead of introducing
   an unversioned build-time download. Current Kokoro/Kareem trees match exactly;
   deterministic tree digest (UTF-8 sorted records: relative path, TAB, lowercase
   file SHA-256, LF)
   is `43ebef9ebca9edcb42dd5d49a7935038731af4c6450a677dc0cae732dae0ce6a`.
   Validate the expected data before first initialization and retain a stable
   directory for its process lifetime. New small-model packages must carry the
   same pinned data so they can install/run independently of the old models.
   No current in-app voice-model removal command was found; do not add removal
   or allow future removal of the initialized data without managing its lifetime.
4. Cache the first initialization Result in the app. Upstream initialization uses
   `call_once`: a failed first attempt cannot be repaired by assuming a later zero
   return means success. Report repair/restart, and serialize calls through the
   existing native synthesis ownership. Keep US/British and per-language locales.
5. Preserve Rust sanitization/English normalization and source-span identity.
   Check emitted phoneme/token limits and split only at safe boundaries before
   the model's style limit; do not truncate or split a word merely because a C++
   helper has a count threshold. The passing boundary probe is not a universal
   crash/quality proof. Add production regressions in Stage 3.
6. Android: retain the supported Maven ORT 1.27.0 route, source-building Sherpa
   2.0.1 and the phonemizer against it. The Maven 1.28.2 POM returns HTTP 404;
   do not change Gradle to a nonexistent version or pair its current ORT with
   Sherpa prebuilts compiled for a newer runtime. Extend the existing source
   preparation/receipt path to cover normal developer builds and the x64 emulator
   fixture as well as arm64 release builds. Reuse it from existing entrypoints,
   with matching source/data pins, required libraries and 16 KB checks.
7. iOS: stage separate aggregate Sherpa, ORT and phonemizer static archives,
   preserving one ORT definition. Use the verified Sherpa/ORT XCFrameworks above;
   build pinned phonemizer source through its existing iOS recipe (aggregate
   core/eSpeak/ucd, device/simulator), since no suitable versioned library archive
   was found in the inspected 1.5.9 release. Do not extract a Flutter demo app as
   a dependency. Extend Mac CI preparation and verify the real link before merge.
8. Desktop: stage the matching official phonemizer core alongside Sherpa/ORT,
   using existing platform preparation, verified download and bundling patterns.
   Refresh exact Windows/macOS asset pins and verify actual runtime dependencies
   on those hosts during implementation. Update signing/resource staging together.

**Vendor/build delta disposition:**

| Current/new build behavior | Required Stage 3 action |
| --- | --- |
| Old local sys bindings and static component list | Rebase complete 2.0.1 bindings/metadata, including new generation input and config layout. Pin wrapper/sys exactly and verify Cargo resolves the local sys patch rather than silently bypassing it. Adopt upstream removal of embedded eSpeak/Piper and addition of utf8proc; do not selectively append one FFI field. |
| Existing Unix atomic runtime replacement | Retain `runtime_copy.rs` and its regression; upstream still uses direct `fs::copy` for loaded Unix runtime destinations. |
| Old empty-directory cache repair | Retain and strengthen to required-library/version/receipt validation. Upstream directory-presence checks do not solve stale/restored caches. |
| Existing Windows copies into `deps` | Retain; upstream still copies only to profile/examples. Keep tests resolving the intended DLLs. |
| iOS static adapter | Retain aggregate Sherpa/ORT linkage and make explicit static features win over upstream mobile shared defaults. Update archive paths/thinning for inspected layouts. |
| New automatic Tauri Android JNI copying | Keep app-owned source receipt/JNI staging authoritative. Prevent the new sys helper from copying a second ORT or bypassing source-built inputs. |
| Upstream custom-profile/device/simulator fixes | Adopt upstream fixes, then test them alongside our retained patch. Do not assume they replace the app's static ownership. |
| Source/notice/archive tooling assumes eSpeak under Sherpa `_deps` | Point it at the separately pinned phonemizer/eSpeak source, update receipts and source inventory, keep existing GPL distribution obligations. Moving eSpeak outside Sherpa does not remove it from the app. |

**Nabra findings and constraints for the later paired-model stage:**

The current upstream `nabra-82m-arabic-int8.tar.bz2` is 82,810,819 bytes,
SHA-256 `a8608a504b1db4dd538b821409fe50b2d5cf37650c0a8753fa215bd6445e9964`.
It differs from the October 82,775,122-byte archive with SHA-256
`e7c644fabb1529190de9a2f7d6044f88c26bf6d9b717ad41ef5d2b748e01459b`.
Do not silently change an old pin or claim identical-weight replay. This model
is not in current main's catalog and was not added by this investigation.

With the current archive and the plan's boundary text, the old 1.13.4 child
process still exits 255; 2.0.1 explicit phoneme input succeeds. Raw input still
contains model-inappropriate dots/markers. A native cleanup experiment matches
the public Arabic `clean_phonemes` output on the two tested cases and removes
their unknown-marker warnings. This establishes a supported integration route,
not pronunciation acceptance or exhaustive closure of the historical issue.
The inspected reference was matched byte-for-byte at Hugging Face revision
`03b95013d3dcd8be6f0b0412a157d25518c7b756`, file SHA-256
`7754aab4d0d8e33822ee4f736b641291b6c445608e1df568c12b9762f7c0f55c`.

The Arabic reference's *text* normalization deletes Latin runs. Do not copy
that deletion into Papercut; mixed-script input may not silently lose words.
Keep the approved Libtashkeel choice instead of importing its Camel-tools path.
The English 7M reference still requires Misaki-compatible phonemes; current 82M
eSpeak success does not qualify it. Both 7M candidates remain Stage 4 work and
must meet the same quality gate together; smaller weights are not a waiver.

**Interrupted-save design proposed for Stage 3:**

- Add optional `synthesisRevision` to the existing manifest, with missing =
  unknown. Derive it from the pinned Sherpa/native frontend/data/normalization
  contract, not labels, recommendation order or thread count. Existing model and
  preprocessor IDs already identify their artifacts/options. SILMA's unaffected
  legacy jobs should not be invalidated by a Sherpa revision.
- Read/validate completion and revision before pending-manifest writes or pruning.
  A complete legacy generation stays usable. Compatible partial saves resume.
  Unknown/incompatible Sherpa partial saves return a specific regeneration reason;
  UI/queue restoration must request explicit restart or leave the old job intact.
  Distinguish this from the existing PDF-source regeneration reason.
- Keep current audiobook IDs and `chunks/` layout. The initial Stage 2 design
  proposed a retained sibling generation and recovery journal. On November 12,
  the owner replaced that proposal with the simple restart contract above.
- Under the book writer lease, validate the confirmed generation token, clear
  incompatible partial chunks, and only then stamp current pending provenance.
  Failed clearing retains old provenance, so leftover chunks cannot be resumed
  under a falsely current revision. Once the new pending manifest exists, later
  interruptions resume its compatible progress without another restart prompt.
- No directory replacement/recovery framework or preservation of discarded partial
  progress is required. Existing completed-book playback and source-edit behavior
  remain separate; do not reintroduce staging/backup journals for this upgrade.

**Gate 2 disposition:** owner accepted November 11, 2026 after reviewing
`listening.html`, reporting no regression and quality at parity, and authorized
Stage 3. This records that listening decision without extending it to untested
platforms or future production integration. Review the proposed
C API/source-build/static-link contracts above and the subsequently simplified
restart contract accepted November 12. Actual
Windows/macOS/iOS execution and Android inference/packaging remain explicit
Stage 3 gates, followed by the four owner devices in Stage 5. If listening finds
a regression, resolve it before accepting migration; do not waive it for speed.

On approval, implement **Stage 3 only**, with existing models: native phonemizer,
complete sys rebase with retained safeguards, matching platform preparation,
and the accepted explicit partial-save restart handling. Stage 4 model additions remain
separate. Changes to this report are not authorization to start that work.

### Stage 3 — Upgrade existing models and protect interrupted saves

Scope: matching Cargo/lock/vendor/runtime changes, required native frontend,
platform preparation/bundling, backward-readable provenance and restart handling.
Do not add new catalog models in this stage.

Implement Stage 3 in two reviewable passes. **3a is the simple restart prerequisite**
on the existing 1.13.4 runtime; **3b is the runtime/frontend/platform upgrade**.
The inspected save path wrote a pending manifest and pruned old chunks before
loading the engine. Review the small restart guard before changing native binaries.
The owner’s November 12 decision removes partial-generation backup/rollback from
acceptance scope; completed-book compatibility remains required. Stage 3 is not complete merely because 3a passes.

Steps:

1. Implement and review the 3a provenance/restart safeguards described below,
   including UI, restored queue jobs and native entry points. Stop for review.
2. In 3b, implement the reviewed runtime/frontend integration and narrowly rebase vendor
   changes. Update pins, hashes, source/notice tooling and CI paths together.
3. Make prepared native caches version-aware; test existing caches and restoration
   as well as clean builds. Fail closed on incompatible source receipts/ABI.
4. Advance the synthesis revision for the actual integrated 2.0.1 frontend/data
   contract; test a 3a/current-revision interrupted save crossing that boundary.
5. Run the existing suites appropriate to the changed paths plus native inference
   and packaging checks. Use the validation matrix below and real old-format
   fixtures made from disposable copies; do not mutate the owner's library.
6. Update shipping architecture/maintenance/packaging docs to describe only the
   implemented behavior. Present Gate 3 evidence with exact source revision.

Human acceptance:

- [ ] Every existing model/language/voice remains available on its existing platforms; original IDs, speaker mappings and download identities are preserved.
- [ ] Current-runtime versus upgraded-runtime samples have pronunciation and overall speech-quality parity or improvement, including US/British and multilingual cases. No missing-word regression is accepted.
- [ ] Completed historical HTML/EPUB/PDF audio plays, seeks, highlights and exports offline without redownload or regeneration; imported bundles restore original settings.
- [ ] Compatible incomplete saves resume; incompatible/unknown legacy saves require explicit restart. Cancel and queue restoration cannot bypass that decision.
- [ ] Cancel/stale consent leaves old progress untouched. Confirmed restart discards only the unfinished job’s old chunks. Failed/interrupted clearing cannot adopt old chunks as current; new compatible progress resumes after relaunch. No partial-generation rollback is promised.
- [ ] Valid current manifests remain readable, including missing-field defaults and timing-index recovery; a global cache/schema bump was not used as a shortcut.
- [ ] Clean and restored-cache builds/inference pass on the required platform matrix; native library loading, initialization order and packaging/source/notice checks are evidenced.
- [ ] Reviewer accepts migration of the existing roster and explicitly authorizes Stage 4. Remaining owner device checks are recorded for Stage 5, not asserted passed.

#### Stage 3a handoff — November 12, 2026

The owner approved **simple explicit restart** on November 12, replacing the
initial uncommitted retained-generation design. Implementation is committed for
owner smoke review on `task/sherpa-onnx`, following `280cff78`. In 3a, no Cargo/lock/vendor/native-library or
model pin changed; its runtime was 1.13.4. Stage 3b and model additions had not
started at that handoff. The owner subsequently reported all smoke tests for this pass passed and
authorized Stage 3b. Platform/runtime upgrade checks remain separate.

Implemented contract:

- Optional schema-2 `synthesisRevision` in 3a recorded
  `sherpa/1.13.4;frontend/embedded;normalization/1`. Missing/different provenance
  triggers Restart/Cancel only for unfinished Sherpa audio. Completed legacy
  audio/index repair and imported bundles retain original provenance. SILMA
  resumes as before, including historical NFE defaults.
- Primary Save checks its actual 1x destination immediately before confirmation;
  Resume checks all document types using their stored speed/settings. The dialog
  explains deletion of old saved progress and offers **Restart from beginning**.
  Cancel or a failed status check starts nothing. Consent/token is transient and
  not stored in queue records/preferences.
- Native preflight rechecks settings and the exact confirmed manifest token under
  the per-book writer lease. It removes the old unfinished chunk directory before
  current provenance is written. A deletion failure/interruption leaves old
  provenance and requires confirmation again; surviving old chunks cannot be
  stamped current or mixed into new generation. A job that has completed since
  confirmation is exempt and must not be deleted by a stale restart request.
- After current pending provenance is written, new generation uses the original
  directory/chunk layout. Later interrupted progress resumes without a repeated
  engine-restart prompt. The old partial progress has intentionally been discarded
  and cannot be rolled back. Existing atomic WAV/manifest file commits remain.
- Removed sibling staging, retained backups, replacement journals/checkpoints,
  directory-promotion recovery, read/list recovery hooks and their tests. There
  is no second-generation storage requirement. Old prototype artifacts were never
  shipped and do not require production migration/support.
- Retained the small `fs4` writer lease shared by save/import/delete/transfer;
  a reset must not race another writer. The existing engine busy lock covers
  preflight/reset and saving; the global source-reference lock is used only for
  short pending-source publication. Function comments explain the ordering and
  compatibility obligations.
- Existing completed-book playback/seek/highlighting/export and IDs are unchanged
  by engine upgrades. Source-edited completed books are outside engine restart:
  their explicit re-save still uses pre-existing behavior, including the PDF
  replacement warning. The broader source-edit protection from the earlier
  prototype was removed. Upload language-suggestion behavior is unchanged.

The simplification removed over 400 Rust lines including tests/comments compared
with the initial Stage 3a prototype, mainly journal/backup machinery. The subsequent
pre-commit review grouped module/test imports, shortened repeated qualified helper
paths and corrected a misplaced module comment. Function comments retain the
non-obvious consent, locking and reset-order contracts.

Automated evidence on this Linux host:

| Check | Result |
| --- | --- |
| Full `npm test -- src/tts` (approved local Chromium fixtures) | 31 files, 394 passed |
| Full native suite, `--offline --features native-tts-core,sherpa-onnx/shared`, existing 1.13.4 libraries | 349 passed, 5 existing ignored |
| Default/providerless `cargo check --offline` | Passed |
| TypeScript build, eight-locale validation, strict touched-file ESLint, scoped rustfmt and whitespace | Passed |
| Native restart helpers using disposable manifests/WAVs | Missing/stale consent, Cancel/byte preservation, confirmed reset/no old-chunk reuse, failed/interrupted clearing, current-progress resume, completed legacy/index repair, completion after confirmation, damaged caches and SILMA defaults |
| UI/queue regressions | Restart warning/button, Save and restored HTML/PDF/EPUB jobs, Cancel/no native save, no persisted token, failed status checks and legacy generated speed |

Fresh source-bound patch/check logs and reproduction commands are retained locally
in `.cache/sherpa-simple-restart-2026-11-12/`. Earlier prototype evidence remains
under `.cache/sherpa-stage3a-2026-11-11/` for history and does not validate the
revised behavior. No owner library data or model/runtime binaries are included.
No physical full-disk, forced shutdown, installed-app or non-Linux result is
claimed from Linux helper tests.

Gate 3a — owner reported all smoke tests passed, November 12, 2026:

- [x] Review that native consent precedes deletion, old provenance survives an
  incomplete reset, and current provenance is written only after old chunks clear.
- [x] In a disposable profile, interrupt a multi-chunk save; close the app and
  remove only that copy’s `synthesisRevision`. Relaunch and Cancel: hashes and
  progress must stay unchanged, and no generation should start.
- [x] Confirm **Restart from beginning**: old progress is discarded and generation
  starts from zero. Interrupt again and relaunch; Resume must keep the newly
  generated compatible chunks without another engine-restart confirmation.
- [x] Verify completed legacy/imported audio still plays/seeks/highlights/exports
  offline without synthesis, redownload or changed identity. Check that an engine
  change alone never offers to discard a completed book.
- [x] Accept the revised 3a implementation and authorize 3b. Actual installed-app
  and Windows/macOS/mobile checks remain in the combined Stage 3/5 matrix.

Use a freshly rebuilt/restarted native app and disposable copies only; a
browser-only or frontend-HMR session cannot validate the Rust restart guard. On Linux, isolated `XDG_DATA_HOME`/`XDG_CACHE_HOME`
work with the normal native dev launcher; download/copy model inputs into that
profile as needed. Older binaries do not honor these leases/provenance and must
not concurrently share the writable profile or resume candidate partial jobs.

#### Library maintenance boundary for Stage 3b

No upstream library has been modified in 3a. For 3b, keep Sherpa, Piper phonemizer,
eSpeak and ONNX Runtime C/C++ source unchanged. Call the published phonemizer C API
through a small checked Rust adapter in the app, with result ownership and cached
initialization failure. Reuse verified model-pack eSpeak data. Avoid adopting the
phonemizer Rust crate's unconditional unversioned data downloader or maintaining
a fork of it; the inspected details/pins are in the Stage 2 report above.

There is an existing local `sherpa-onnx-sys` Rust build adapter, so upgrades are not
maintenance-free. Narrowly rebase that packaging delta: iOS explicit static
precedence/aggregate linking, atomic Unix runtime replacement, required-cache
validation and Windows test DLL placement. Keep the full matching upstream 2.0.1
bindings. Future upgrades must recheck the public C API, pinned data/archives,
static/shared link inputs, notices/source receipts and single-ORT behavior. This
keeps the maintenance burden in app integration/build plumbing rather than a
modified speech-engine implementation. Record any necessary additional patch
explicitly and return a C++-patch requirement to the owner before taking it.

#### Stage 3b handoff — November 12, 2026

**Review candidate, uncommitted:** `task/sherpa-onnx` after
`31fa781f5ecb4f1f4989087b38fbf7b8772f953f`. The owner accepted 3a smoke tests and
authorized this pass. No new model, display ID, voice ID, speaker mapping, model
archive pin, upload suggestion behavior or global cache/schema version changed.
The restored `engine/mod.rs` architecture comment keeps the useful shared
`{model,synth,save,cache,bundle}` overview alongside the current flow diagram.

Implemented:

1. Exact Sherpa wrapper/sys **2.0.1**, full matching upstream bindings; no C/C++
   patches. The existing locally modified Rust sys **build adapter remains**.
   The [build-integration report](sherpa-no-fork-proof.md) records the
   maintenance history, replacement proof and removal gates. This is a real
   build/packaging fork, not a maintenance-free registry dependency.
2. App-owned `build/phonemizer.rs` prepares pinned official desktop Piper 1.5.9
   artifacts. `engine/phonemizer.rs` calls its published C API, owns/destroys
   native results, checks version and data identity, caches first initialization
   success **or failure**, and supplies word-safe phoneme sentences to Sherpa's
   official generation API. No Piper Rust crate/fork, Python runtime, C++ patch
   or application-specific speech algorithm is embedded in the vendor crate.
3. Non-Mandarin Kokoro retains per-voice US/British/Spanish/French/Hindi/Italian/
   Brazilian Portuguese phonemization; Kareem uses Arabic. Existing English
   normalization/optional Arabic preprocessing run first. Mandarin retains
   lexicons and numeric FSTs; Supertonic retains its text tokenizer. Existing
   model-pack eSpeak data is verified against tree SHA
   `43ebef9ebca9edcb42dd5d49a7935038731af4c6450a677dc0cae732dae0ce6a`.
   First-init repair/removal requires app restart. Oversized individual words
   error explicitly rather than being truncated or silently omitted.
4. Desktop archive size/SHA pins and extracted-file receipts reject missing,
   empty or changed caches. Linux/macOS Tauri resource staging is now versioned:
   old staged 1.13.4 resources had actually overwritten the new Cargo runtime
   during this pass; the native version check caught it before inference. Keep
   Node constants, Tauri resource JSON and CI/model-download workflow paths in sync.
5. Android production/developer/emulator builds share the unchanged-source
   Sherpa/Piper recipe with source/tool/ABI/output receipts. NDK 30.0.16248370,
   API 26 and Maven ORT 1.27.0 stay pinned. Only Sherpa/Piper C libraries are
   staged; Gradle packages ORT once. arm64-v8a and x86_64 use the same recipe.
   Explicit invalid inputs fail closed; no 2.0 prebuilt Android fallback is used.
6. iOS prepares separate pinned Sherpa/ORT XCFramework ZIPs (ORT 1.28.2), plus
   unchanged pinned Piper source through its upstream iOS build script. App
   preparation thins the three aggregate archives; explicit static linking is
   preserved. Extracted upstream caches require matching hash receipts.
7. New synthesis revision is
   `sherpa/2.0.1;piper/1.5.9;espeak/43ebef9e;normalization/1`. Disposable tests
   cross the exact 3a revision boundary: unfinished old progress needs explicit
   restart; completed old audio stays byte-identical. Confirmed restart still
   intentionally discards partial progress, with no journal/backup rollback.
8. License/source tools now track external Piper/eSpeak and utf8proc; notices
   regenerated. Android inventory preserves historical evidence as historical.
   The local corresponding-source archive is review evidence, not a release;
   its HEAD reference must be regenerated from the matching committed tree before distribution.

Local evidence is retained under `.cache/sherpa-stage3b-2026-11-12/` (ignored):

| Check | Result/scope |
| --- | --- |
| Complete native Rust suite, TTS/preprocessing/embeddings features enabled | 364 passed, 10 opt-in tests ignored; original restart/save/bundle checks plus exact old-revision completed/partial cases |
| TTS and adjacent packaging/script suites | 38 files, 433 passed; APK verifier self-checks: 4 passed |
| Production `cached_frontend_smoke` corpus | 18 finite/non-silent WAVs: all existing Sherpa languages, US/British, names, mixed scripts, short input, Supertonic English/Arabic; English phonemes identical after language switching. Not human quality acceptance. |
| Real Arabic preprocessing + Kareem synthesis | Passed with cached model/Libtashkeel through actual app code |
| Cached E5 Small FP32 lifecycle/index/query regression on new desktop ORT | Passed; INT8/Base/Large and other platforms not rerun here |
| Android source builds | Both arm64 and x86_64 compiled; source/ABI/hash receipts and 16 KB ELF LOAD checks pass |
| Local arm64 debug APK | Built. Native library inventory, 16 KB ELF alignment, notices and exact Sherpa/Piper/AAR ORT byte matches verified. Debug artifact only; no release signature/reproducibility or physical inference acceptance. |
| Linux shared/static variants | Shared resource staging includes Sherpa, the single ORT runtime and Piper core. Static compile/link and the same 18-case corpus pass using the separate static Piper/eSpeak/ucd libraries. |
| Upstream boundary | All 15 sys binding files byte-identical to crates.io 2.0.1; Sherpa/Piper source checkouts clean |
| iOS cache boundary tests | Missing receipt/empty/changed extracted archive triggers repair; actual Apple compilation remains pending |
| Notices/corresponding-source tooling | Generated notice check passes; local pinned-source archive assembled |
| TypeScript/eight-locale check, touched-file ESLint, scoped rustfmt, whitespace | Passed; unrelated files preserved |

The fresh `listening.html` pairs the 15 reproducible Stage 2 Kokoro/Kareem
baseline passages with **current production-path** audio; it also includes
standalone current Supertonic and return-to-English samples. Stage 2 owner
listening approval is preserved, but must not be represented as approval of
these new integrated outputs. Different waveform bytes/durations do not by
themselves establish a pronunciation regression or parity.

**macOS support policy clarified by the owner — November 12, 2026:**
Target the oldest macOS major release still receiving Apple's regular security
updates, and newer maintained releases. For this dated review, use **macOS
Sequoia 15** as the oldest supported major; recommend its current security patch.
Apple's [security-release list](https://support.apple.com/en-us/100100) lists
Golden Gate 27, Tahoe 26 and Sequoia 15 in the current update round, including
[Sequoia 15.8.3 on November 9, 2026](https://support.apple.com/en-us/149720).
This target is inferred from Apple's ongoing release pattern, not a promised
Apple three-release support guarantee. Recheck the maintained major releases
when preparing an app release; record any minimum change explicitly.

The intended packaging major-version floor is therefore **15.0**, with actual
macOS 15 qualification still required. Stage 3c.2 aligns the Tauri minimum and Cargo deployment default to 15.0.
This metadata correction is not actual minimum-OS qualification or stage acceptance.

For historical context, inspection of the official Intel 1.13.4 archive found
Sherpa minOS 10.14 / ORT minOS 15.5; the 2.0.1 archive has Sherpa 10.14 / ORT
10.15; Piper 1.5.9 has minOS 10.14. The new binary floors fit the intended 15.x
support target. These headers do not establish actual installed-app compatibility.
See `macos-minimums.json` and the verified archive inspection script in the local
packet. Intel/Apple Silicon builds and actual minimum-OS execution remain gates.

The owner reported all requested Stage 3b smoke tests pass on November 15. This
records functional smoke acceptance without inventing per-device/OS or SDK/CI
results. The owner requested a fork-free build evaluation before committing the
vendor approach. The following broad platform/design gates remain applicable.

Gate 3b — remaining design/platform review:

- [x] Owner chose the unchanged-registry Cargo override approach and authorized
  production integration on November 15. Native acceptance remains separate.
- [ ] Listen to integrated outputs: pronunciation, names/unknown words, mixed
  scripts, all existing languages and long passages must retain parity or improve.
- [ ] Rebuild/restart the native app. Use disposable old partials to check
  Restart/Cancel across the real 1.13.4→2.0.1 boundary; verify completed old/imported
  HTML/EPUB/PDF playback, seek/highlight/export offline without regeneration.
- [ ] Run required Windows/macOS/iOS builds and installed-app loader/static/shared
  checks. Run Android inference (not just compilation) and verify release/source
  receipts through the established workflow. No remote CI run is claimed here.
- [ ] Align macOS packaging/deployment metadata with the owner's clarified 15.0
  major-version floor and qualify macOS 15. Record any remaining device
  matrix items explicitly; the iPhone 16 / Pixel 6 Pro / Pixel 7 / Pixel 10 Pro
  quality checks remain Stage 5 unless actually performed earlier.
- [ ] Accept Gate 3 and authorize Stage 4. Next implementation is the **paired**
  English Kokoro 7M + Arabic Nabra 7M artifact/frontend qualification and additive
  selection UX; neither existing recommendation changes.

Suggested commit title: `feat(tts): integrate Sherpa 2.0.1 with native Piper phonemization`

Suggested description:

> Upgrade the existing roster to exact Sherpa 2.0.1 with app-owned native Piper
> 1.5.9 phonemization, preserving Mandarin/Supertonic frontends and stable model
> identities. Advance synthesis provenance so unfinished old saves require the
> accepted explicit restart while completed audio remains usable. Keep upstream
> speech sources/bindings unchanged; document the retained local sys build fork.
> Update pinned native preparation, Android source/ORT receipts, desktop and iOS
> packaging, CI paths and corresponding-source/notices together. Local native,
> TTS/script, multilingual inference and arm64 debug APK checks pass; non-Linux
> execution, minimum-OS qualification and owner listening remain review gates.

#### Stage 3c — Fork-free integration proof and replacement

The owner accepted the bounded proof and authorized production integration on
November 15. **3c.1 is complete within its documented scope.** Its frozen packet
retains full Linux shared/static inference, 364 native regressions, both single-ORT
initialization orders, relocation, cache repair and actual Tauri/Android build
chains. Apple/Windows metadata checks are not native SDK/runtime acceptance.

**3c.2 local integration passed owner smokes on November 15.** Cargo now resolves the unchanged
registry sys crate, with target links overrides in root `.cargo/config.toml`.
App-owned Rust selects existing platform link modes and shares verified archive
preparation with Piper; desktop automatic preparation remains available. Native
pins, archive-bound extraction receipts, input-triggered restaging, Windows test
DLLs and Unix atomic replacement are retained. The dormant vendor directory is
not active and is deliberately retained until native qualification passes.

The [integration report](sherpa-no-fork-proof.md) records exact current-pass
results, smoke instructions and remaining gates. macOS Tauri/Cargo deployment
metadata now agrees on 15.0; actual minimum-OS testing remains required. No model
IDs, synthesis revision, saved audio or generation behavior changes in this pass.

The owner accepts repo-root commands and explicit Cargo `--config` from outside
the checkout. Keep feature-based static/shared selection in app-owned linking,
not mutable per-feature global config. Use `CARGO_TARGET_DIR` for a custom desktop
bundle output root so Node cache lookup and Cargo agree.

The owner will complete the other native-platform tests at PR preparation. Keep
those gates open and retain the dormant vendor reference until they pass. This
deferral does not authorize treating metadata-only checks as native acceptance.

Acceptance gates for the human reviewer:

- [ ] Review the registry identity, absence of an active Cargo patch, and app-owned
  preparation/link/staging code; confirm the maintenance boundary is acceptable.
- [ ] Run focused smokes: native app launch, existing Kokoro and Kareem previews,
  Arabic preprocessing, an existing completed book, and save/cancel/reopen. If
  using embeddings, activate/query/disable while speech remains usable. Existing
  compatible 2.0.1 partials must not request a new restart for this build-only change.
- [x] Repair four baseline audit findings separately: Ammonia 4.2.5, quinn-proto
  0.11.19 and quinn-udp 0.5.16. Audit passes with the existing two waivers; native
  and default tests pass. No other package version/dependency reference changes.
- [ ] Accept real Windows MSVC, Intel/Apple Silicon macOS and iOS SDK builds,
  installed-app loading, minimum macOS 15 execution and Android device/emulator
  inference. Local target metadata is not this evidence. CI has not been dispatched.
- [ ] Remove the dormant tracked vendor crate after the replacement's platform
  gates pass. Exclude `src-tauri/vendor/sherpa-onnx-sys/PAPERCUT.md` from commits;
  clean its obsolete manifest include with the retired crate. Keep rationale here.
- [x] Owner accepts replacement smokes and allows paired Stage 4 work after local
  acceptance. Deferred native qualification and vendor retirement remain PR/merge
  gates; current physical-device quality acceptance is not inferred.

Suggested commit for the pending Stage 3b + 3c set: `feat(tts): upgrade Sherpa with registry-based native builds`

> Upgrade the existing roster to Sherpa 2.0.1 with native Piper 1.5.9 and
> unchanged registry Rust bindings. Replace the sys patch with Cargo links
> overrides and app-owned verified preparation, platform linking and single-ORT
> staging. Preserve model identities/completed audio, use explicit restart for
> incompatible old-engine partials, align macOS metadata to 15.0, and refresh
> packaging/CI/notices. Retain the unused vendor reference pending qualification.
> Four unchanged baseline audit findings and native SDK/device gates remain open.

### Stage 4 — Add both small models and their selection UX

Stage 4a listening is accepted for both candidates (November 15). The owner
explicitly accepts English 7M's more robotic/less crisp output as a smaller
alternative; Arabic Nabra is competitive with Kareem. This is not overall English
quality-parity evidence. Keep full Kokoro recommended and Arabic diacritization
enabled by default. Stage 4b native frontend integration and owner smokes are accepted.
See the [artifact/frontend qualification record and reviewer instructions](#stage-4a--paired-qualification-record)
and [immutable input pins/private assembly proofs](../src-tauri/tts/qualification-artifacts.json).
Both native frontends use distinct IDs and focused guards; Misaki is pinned
without its native eSpeak feature. Stage 4c now adds both candidate catalog/selector
entries and locally qualified complete artifacts. The publication/public-download
and human install/UI gates remain open before merge/ship.

Scheduling: owner permits local paired-model work before deferred native platform
qualification. Carry the remaining Gate 3 platform checks into PR/merge acceptance.

Scope: English Kokoro 7M and Arabic Nabra 7M, using the existing family, installer,
preview, save and playback contracts. Both must qualify. No defaults move from
Kokoro v1.0/Kareem as part of this stage.

Steps:

1. Qualify exact converted artifacts, required token/style assets and native
   frontend/reference behavior for each language. Check INT8 ARM export issues
   when applicable; choose one validated precision per candidate.
2. Record model IDs, archive size/SHA-256, immutable source revision, licensing
   evidence, installation files, supported language and sole voice/speaker ID.
   Use existing download/verification/atomic-install helpers and monitoring.
   Source/provenance review is not waived by on-demand download.
3. Add matching native/fallback entries with distinct identities/directories.
   Preserve shared archives for existing multilingual entries. Never map the
   new single voice to the existing Heart/Emma roster by assumption.
4. Retain Language -> Model -> Voice. Suggested display names are
   `Kokoro 7M · Small · Experimental` and `Nabra 7M · Small · Experimental`;
   finalize readable/localized copy with the reviewer. Show valid voices,
   exact download size, install state and existing Preview Voice behavior.
5. Compare English with current recommended Kokoro v1.0 and Arabic with current
   recommended Kareem, using the common quality material. Record model-specific
   voice/preprocessing differences; use like-for-like text and settings where
   supported, including vocalized and unvocalized Arabic. A different timbre or
   voice identity alone is not a pronunciation failure; assess intelligibility, pronunciation,
   omissions, fluency and artifacts separately. Experimental status does not
   authorize a speech-quality downgrade. The November 15 English 7M tradeoff
   is an explicit owner approval, not an inference from its Experimental label.

Human acceptance:

- [ ] Both candidates have reproducible pinned artifacts, verified vocab/style mappings and native frontend behavior; no Python runtime or unreviewed diacritizer is introduced.
- [x] Owner accepts both after side-by-side review (November 15). English 7M is explicitly approved as a smaller, more robotic/less crisp alternative; full Kokoro remains recommended. Arabic is competitive with Kareem with default automatic diacritization preserved. This specific approval does not waive other pronunciation/content-loss or existing-model regression gates.
- [ ] Install, preview, save, resume, offline playback, delete and reinstall work independently of existing models. Failed/checksum-mismatched downloads cannot become installed.
- [ ] UI communicates language and single-voice capability, distinguishes recommendations from experiments, preserves old selections and works in RTL/narrow layouts.
- [ ] New IDs create distinct audiobook generations; existing records and bundles remain available and unchanged. Native/fallback catalog and download monitoring cover both additions.
- [ ] No numeric speed/memory acceptance threshold or unmeasured Faster claim has been introduced. Reviewer approves the pair for final device acceptance.

#### Stage 4a — Paired qualification record

Status: **paired listening accepted; native frontend integration candidate**.
The owner accepted
security-pass smoke tests and authorized paired qualification. Other native
platform tests remain PR/merge gates. Neither small model is in the app catalog.

##### Review this packet

Local evidence and listening page: `.cache/sherpa-stage4a-2026-11-15/listening.html`.
The packet is ignored, machine-local evidence, not a committed audio fixture.
Its runtime/source baseline is `88483fad7a3c62cdf1dc13ea1f35cc6fef038b07`.
Stage 4a changed no production code. The Stage 4b integration below adds the
frontends and an optional registry dependency, with no catalog/download exposure
or changes to existing defaults, saved-book identities or upstream source.

Review all 22 rows (11 per language), comparing:

1. **Current recommendation**: Kokoro v1.0 Heart for English, Kareem for Arabic.
2. **7M reference frontend FP32**: Python reference phones through the same Sherpa
   renderer and pause settings as the candidate. English includes Misaki's
   official unknown-word fallback; Arabic retains mixed Latin text.
3. **7M native candidate FP32**: prospective native frontend, same model assets
   and renderer settings as column 2.

Record English and Arabic verdicts separately, with case ID and exact words for
any pronunciation error, omission, repetition, fluency or audio artifact. A
voice/timbre difference alone is not a pronunciation failure. Inspect the
synthesis-text details for existing number normalization/optional diacritization.
Pay particular attention to `en-oov-negative` (invented `qzxwv`), names, mixed
Arabic/Latin, pharyngeals, numbers and the three comma-pause cases below.

Collapsed diagnostics contain independent direct ONNX inference with uncompressed
pauses, two raw-eSpeak English controls and two short INT8 controls. These are
not additional shipping choices. The direct reference has longer pauses than
the primary columns; do not mistake that processing difference for omitted words.

Human gate:

- [x] Owner accepts English as an optional smaller alternative, explicitly finding
  it more robotic and less crisp than recommended full Kokoro. This is a specific
  approved quality tradeoff, **not a claim of overall quality parity**.
- [x] Owner accepts Arabic Nabra as competitive with Kareem. Both have weak
  pronunciation without tashkeel; preserve automatic diacritization enabled by
  default. No undiacritized-pronunciation fix or dialect support is claimed.
- [x] Owner approves integration of both on November 15, 2026 after reviewing the
  packet. The English tradeoff is scoped to this additive option; existing-model
  quality protections and platform/device gates remain.

##### Exact candidate contract

[Machine-readable pins](../src-tauri/tts/qualification-artifacts.json) record immutable
source URLs, SHA-256 and sizes, plus **private assembly proofs**, not approved
production download endpoints. Source revisions:

| Candidate | Original model revision | Converted ONNX revision |
| --- | --- | --- |
| English Kokoro 7M | `9357717e6499717b769ab65de61797aa7c501c86` | `c5663e70326392471bb13be92d37127025bdbf89` |
| Arabic Nabra 7M | `03b95013d3dcd8be6f0b0412a157d25518c7b756` | `33b4e69edf70a3dd58af32abf41e3330dc64a9fe` |

| Property | English | Arabic |
| --- | --- | --- |
| Primary precision | FP32 | FP32 |
| ONNX graph bytes | 30,641,467 | 30,641,486 |
| Token count | 114 | 116, including `ʕ`/`ħ` |
| Voice | Sole `af_msa`, speaker 0 | Sole `af_msa`, speaker 0 |
| Language scope | US English | Modern Standard Arabic |
| Private complete archive bytes | 35,615,853 | 35,568,384 |
| Proposed distinct ID | `sherpa-onnx/kokoro-7m-distill-en-fp32` | `sherpa-onnx/nabra-7m-distill-ar-fp32` |

Both use 24 kHz output and a 510 × 1 × 256 style pack. Vocabulary IDs exactly
match their respective original config plus Arabic's reference extra symbols;
original PyTorch style bytes exactly match converted `voices.bin`. The two
supplied packs happen to have the same SHA-256; do not generalize this to other
versions. They cannot inherit the existing multilingual token table or voice list.

The exact converted graphs include baked notch FIR constants; recursive graph
checks pass and the inspected INT8 graphs have no ConvInteger/QLinearConv nodes.
Two short Linux CPU INT8 renders pass, which does **not** qualify INT8 on ARM.
Use FP32 for this pair's primary review; no graph rewriting is proposed.

Sherpa selects style row N while the reference uses N−1. A synthetic echo model
confirms this at N=1,17,350. **Every one of the 510 real rows is byte-identical**,
so the index difference has no effect for these exact packs. No shifted pack,
style adapter, graph modification or upstream source patch is needed. Recheck
this invariant if assets change; do not silently apply the conclusion elsewhere.

The private archives include each unchanged graph, vocabulary, style pack and
verified phonemizer data, rather than requiring an entire 82M installation for
unknown-word fallback. The data tree hash is
`43ebef9ebca9edcb42dd5d49a7935038731af4c6450a677dc0cae732dae0ce6a`.
Assembly repeats byte-for-byte; extraction verifies 358 files per archive.
`QUALIFICATION.json` identifies these as private evaluation bundles. Final public
packaging/notices may change archive hashes; do not promote these local pins as
published catalog artifacts. Both original model cards declare Apache 2.0;
review converted provenance and package all required notices before publication.

##### Frontend result and maintenance boundary

English 7M needs Misaki-compatible phones. Feeding current 82M eSpeak output
directly to it is not a qualified shortcut. The isolated probe evaluates published
`misaki-rs` 0.9.0 with `default-features=false`, embedded lexicons/POS assets and a
small bridge to the app's existing Piper 1.5.9 fallback. This avoids the port's
optional second eSpeak native build. It requires no Python runtime or fork of
Misaki, Piper, Sherpa or ONNX Runtime. Stage 4b now integrates this optional exact-pinned dependency and bridge.
The lock adds eight package identities, updates none of the existing versions
and excludes `espeak-rs`. Existing dependency references retain their resolution;
only the required num2words/fancy-regex disambiguation is added.

The bridge translates Piper IPA into the pinned Python Misaki 0.9.4 fallback
conventions. It is maintenance work even without a fork: qualify unknown-word
coverage, error handling, cache/thread use and initialization before promotion.
The Stage 4a isolated dependency lock audits clean locally (82 resolved dependencies).
The port's MIT license and its embedded/data/transitive notices must be covered
by the existing notice workflow; this is not blanket platform/MSRV qualification.

Measured on actual app-normalized synthesis text:

| Probe | Result | Difference requiring review |
| --- | --- | --- |
| English native + Piper fallback vs Python Misaki + official fallback | 10/11 exact phone strings | `en-oov-negative`: `…ʌbəl…` vs `…ʌbᵊl…` in spelling `qzxwv` |
| English without fallback vs Python without fallback | 11/11 exact | Both can produce unresolved markers; unsafe as a shipping route |
| Arabic native cleanup vs reference cleanup | 8/11 exact | `ar-pharyngeals`, `ar-numbers`, `ar-numbers-tashkeel`: native retains comma pauses |

The converter's no-fallback English recipe can omit unknown words; it is not the
primary reference or proposed shipping policy. Unresolved/unsupported phones must
produce a recoverable error, never silent word removal. The Arabic reference's
Latin-deletion normalization is deliberately excluded: mixed Papercut/Edmonton
text remains in synthesis. Native cleanup preserves `ʕ`/`ħ`; it folds IPA before
removing Arabic syllable markers. Reuse existing optional Libtashkeel 1.5.0 and
text normalization rather than adding a diacritizer or changing source text.

All 92 Stage 4a final listening entries generate finite, non-silent audio: 66 primary
clips, 22 independent-direct diagnostics and four controls. This proves execution,
not quality. Long samples preserve all phones in word-bounded chunks of at most
350; they do not exercise app save/seek/highlighting or full-book chunk timing.
Four equal-input, uncompressed-pause direct/Sherpa calibrations have matching
sample counts. Samples are not equal: the graphs contain random operators and a
repeat through the direct renderer itself differs. No waveform-parity claim is
made. Primary reference/native audio uses the same app silence scaling (0.2).

##### Evidence and next implementation gate

The local packet retains `artifact-pins.json`, original source files, `cases.json`,
`frontend-results.json`, `contracts.json`, `graph-style-checks.json`,
`bundle-results.json`, `audio-results.json`, calibration/repeat results, logs and
probe sources. `evidence-sha256.json` binds the report inputs and tool sources.
Python package versions are frozen in `reference-requirements.txt`; the private
Rust probe has its own Cargo.lock. Python is reference tooling only (CPU Torch,
Misaki 0.9.4). Direct ONNX reference uses the same native ORT 1.28.2 as Sherpa,
not the unrelated Python ORT installation. Native probes import unchanged app
phonemizer, normalization and Tashkeel modules. These tools have checkout-local
paths; a new machine must restore pinned inputs and adjust those paths.

Human listening is accepted under the explicit English tradeoff above. Native
frontend integration is implemented in Stage 4b below; keep catalog exposure behind
the remaining artifact/installer gate. Before adding selectable entries:

1. Resolve/review the known OOV and comma differences; add focused differential,
   unknown-token, empty/oversized-input and long-text coverage. Route by new model
   identity; existing models keep their current frontend and synthesis revision.
2. Confirm a distributable immutable archive location and final notices/pins.
   Reuse current verified/atomic installer and per-model directories. A clean
   profile must install either candidate independently. Respect the existing
   phonemizer's process-global data-directory lifetime; do not add casual cache
   deletion or assume data is disposable after initialization.
3. Review then implement additive Language → Model → Voice entries for both
   languages, with one valid voice each, exact complete download size and
   Experimental/Small copy. Current Kokoro v1.0/Kareem recommendations, saved
   selections, uploads' language-suggestion behavior and existing languages stay.
4. Human-smoke clean install, interrupted/bad downloads, preview, save/resume,
   playback and old-book compatibility; RTL/narrow UI; independent models and
   offline operation. No silent word loss or model substitution is acceptable.

Other native platforms, minimum OS, installed applications and the four owner
phones remain required before merge/release under Stages 3/5. Do not retire the
inactive vendor reference merely because this Linux probe passes. No speed/RAM
threshold, extra precision selector, publication or release action is added here.

##### Primary references

- [English original model](https://huggingface.co/oddadmix/Kokoro-7M-Distill) and [converted files](https://huggingface.co/marwanelamami/kokoro-7m-distill-sherpa-onnx).
- [Arabic original model](https://huggingface.co/oddadmix/Nabra-7M-Distill) and [converted files](https://huggingface.co/marwanelamami/nabra-7m-distill-sherpa-onnx).
- [Sherpa 2.0.1 reference frontend](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/scripts/kokoro/7m-distill/g2p_frontend.py).
- [Misaki Rust port](https://github.com/MicheleYin/misaki-rs); the evaluated registry source records revision `01022f471a6aee532db40d79648b7b6f3d2a57b7`.

#### Stage 4b — Integrated native frontend handoff

The two reserved IDs live beside the other model identities in `models.rs` but
are not entries in `MODELS`. All model metadata, loading, inference, preprocessing,
WAV persistence and future installation/catalog APIs remain shared. The separate
`phoneme_adapter.rs` contains only the training-specific phoneme adapters and
checks; putting these in the metadata catalog would mix pronunciation processing
with model declarations. It is not a second engine, loader or catalog.

`SherpaTtsEngine` owns an optional adapter alongside its existing native handle.
Only the two exact new IDs initialize it; existing models do not load Misaki or
change frontend routing. The existing engine mutex serializes use. English loads
one US Misaki pipeline with the existing Piper fallback; Arabic reuses Piper and
reference-compatible cleanup, keeping accepted comma pauses and Latin text.
`af_msa` cannot select locale by prefix: Nabra explicitly uses Arabic. Existing
optional Libtashkeel stays upstream of this adapter, with no new preprocessor.

Load validates the pinned vocabulary and voice hashes. Generation rejects empty,
unresolved and unsupported phones before Sherpa, and reuses the existing 350-phone
word-safe splitter with no truncation. Native fallback errors propagate to the
caller; a failed frontend cannot commit a successful audio chunk. New models will
need model-specific synthesis provenance when added to the catalog; do not bump
the existing-roster revision just to version these new frontends.

“Preserving the synthesis contract” means retaining the existing text-to-phones
behavior for the current model roster. The distills were trained with different
phoneme conventions: English uses Misaki's compact symbols, and Nabra needs its
Arabic cleanup/vocabulary. Applying these transforms to every Kokoro-family model
would change already-reviewed pronunciation and could disrupt multilingual paths.
This is frontend selection inside one engine, not a second model implementation.
An intentional existing-model frontend change would need renewed listening and
synthesis-provenance/restart review; completed WAVs do not depend on this frontend.

Function comments explain the shared engine/data lifetime, transform ordering,
locale trap, accepted OOV variant and error boundaries. Existing useful comments
are retained. No upstream crate, C/C++ library or model/voice bytes are patched.

The existing `src-tauri/tts/frontend-smoke.json` now retains all 22 reviewed
processed-text/phone pairs beside the existing 18 roster cases. The existing
`cached_frontend_smoke` runs both groups through the same production loader and
`generate_audio` route. Optional expected-phone fields assert exact reviewed
outputs for the distills, plus sole-voice rejection and sanitized-empty behavior.
A temporary test-only metadata fallback allows the reserved IDs before catalog
exposure; remove it when real catalog entries land. There is no separate fixture
store or synthesis harness for the new models. Ordinary tests
cover opt-in isolation across the entire current roster, word-safe preservation,
unknown/oversized/invalid phones, pack mismatches, fallback errors, unresolved
words and Arabic/English transforms. They download no models.

Local reproduction from the repository root (prepared native libraries required):

```sh
cargo test --manifest-path src-tauri/Cargo.toml --offline --locked \
  --features native-tts-shared,experimental-search-embeddings --lib
PAPERCUT_TTS_MODEL_ROOT="$PWD/.cache/sherpa-stage4b-2026-11-15/models" \
PAPERCUT_TTS_SMOKE_OUTPUT_DIR="$PWD/.cache/sherpa-stage4b-2026-11-15/audio" \
  cargo test --manifest-path src-tauri/Cargo.toml --offline --locked \
  --features native-tts-shared,experimental-search-embeddings --lib \
  cached_frontend_smoke -- --ignored --nocapture
```

The assets root uses the normal model directory names, including
`kokoro-7m-distill-en-fp32/` and `nabra-7m-distill-ar-fp32/`, each with unchanged
graph bytes as `model.onnx`, `tokens.txt`, `voices.bin` and verified
`espeak-ng-data/`. Existing roster directories are present for the same test.
The local probe uses links to the frozen Stage 4a inputs; it does not modify them
or the owner's installed models. Full regressions need loopback access for
unrelated existing transfer/download tests; sandbox denials are retained separately.

Dependency notices include the port, original Misaki dictionaries/logic, spaCy
and model source notices, tokenizer and stemmer. All four dictionary JSON trees
match Misaki 0.9.4. The pinned upstream extraction script reproduces the embedded
tagger parameters and labels byte-for-byte from `en_core_web_sm` 3.8.0. Evidence is
in `.cache/sherpa-stage4b-2026-11-15/embedded-data-provenance.json`; regenerated
notices and the source inventory bind this dependency change.

The versioned `licensing/misaki-0.9.0.txt` is a reviewed input to the existing
notice generator and packaged license directory (the same convention as
Libtashkeel). Its location is a packaging choice, not a special Misaki requirement.
It preserves the port and embedded-data/dependency notices reproducibly without
depending on a developer's registry layout or fetching licenses during app builds.
It contains attribution text, not a library fork.

Review gate for this pass:

- [x] Owner accepts the shared-engine/adapter boundary and focused-test pass
  for checkpointing (November 15).
- [x] Owner reports the requested existing recommended English/Arabic preview
  and save/playback smokes pass (November 15); Arabic
  automatic diacritization remains enabled. Existing language and upload behavior
  stay unchanged. The 7M options will not appear in the UI yet.
- [x] Owner accepts frontend readiness and requests this checkpoint. The paired
  immutable artifact, installer and additive catalog/selection stage is next; it
  remains unimplemented. No further listening decision
  is required for the unchanged reviewed frontend/assets, but device testing and
  any later pronunciation-affecting change still require qualification.

Before that next stage exposes either model: finalize distributable archive URL,
notices and final bytes/SHA; bind distinct model/voice/preprocessor/provenance
metadata; test verified atomic installs and independent operation; preserve full
Kokoro/Kareem recommendations and default Arabic diacritization. Public uploading
or publishing an archive remains a separately authorized action. No performance
claim has been measured or added. Other native platforms remain PR/merge gates.

Executed Stage 4b checks: native/provider suite **372 passed, 10 ignored**;
default suite **293 passed, 4 ignored**; explicit cached paired-corpus test
**passes all 40 cases (18 existing + 22 reviewed distill cases)**; notice/source script suites **6 passed**;
changed JavaScript lint, notice regeneration/currentness, existing-waiver RustSec
audit and local corresponding-source assembly are recorded in the Stage 4b
packet. Ordinary ignored tests and other native platforms are not asserted passed.
The initial sandboxed full suite's local-listener denials are retained; the full
suite passes when rerun with its required loopback access.

#### Stage 4c — Paired artifacts and catalog integration (November 15)

Status: **implementation/local qualification ready; publication and human install
acceptance pending**. Baseline is `912451af5d90a8f327bdca793d6c90b1801ea7f6`.
This working branch exposes the two candidate selections for review; it must not
merge/ship until their immutable public assets and download checks pass. The
public monitor currently passes the three existing archives and fails with the
expected HTTP 404 for the unpublished new release. No upload, release publication,
workflow dispatch, push or application release has occurred.

The authoritative `MODELS` and browser fallback now contain both distinct IDs,
with one `af_msa` speaker (native index 0), the Kokoro family loader and independent
model directories. English retains the existing normalization and Misaki/Piper
adapter. Nabra uses the same Arabic preprocessing capabilities/default as Kareem,
including default Libtashkeel when that capability is compiled. No new diacritizer,
engine or model/voice inference is introduced. Default recommendations, existing
selections and upload-language-suggestion exclusion stay unchanged.

The existing Language → Model → Voice controls append localized **Small** and
**Experimental** labels to these two model options, show a localized single-voice
label, and use the existing exact-size/install-state/preview controls. Full Kokoro
and Kareem retain their model-level Recommended labels. There is no precision
selector or unmeasured speed/RAM claim. Native and browser candidate capability
DTOs compare identically in the local packet.

New per-model provenance suffixes are `misaki/0.9.0;piper-fallback/1;distill/1`
and `nabra-cleanup/1;distill/1`. Existing model synthesis revisions and the global
cache version remain unchanged. Same-voice distills still produce separate book
IDs. Completed books and the accepted existing-model partial-save policy stand.
The temporary test-only model metadata fallback has been removed; the shared
40-case smoke corpus now resolves real catalog entries.

Final prepared model artifacts (FP32, original reviewed graph/token/style bytes):

| Archive | Bytes | SHA-256 |
| --- | ---: | --- |
| `kokoro-7m-distill-en-fp32.tar.bz2` | 35,634,041 | `864a694715b41bde0cb711535664ecdb4f451464ac3684fff1c092e4b0f0b364` |
| `nabra-7m-distill-ar-fp32.tar.bz2` | 35,583,076 | `2a98a1f637156d8b927be4311a3d94edbed0f18e888bd4363614ab2e5f73eabf` |
| `piper-espeak-corresponding-source.tar.bz2` | 27,911,709 | `66c0103fb4a685dbdb67b1d4d2744f956626060699ee4030bb031e41ef3223c0` |

Each model archive includes its original model card, `MODEL_SOURCE.json`,
`NOTICE.txt`, Apache/GPL texts, graph, tokens, voice and full verified phonemizer
data. It no longer contains the old private `QUALIFICATION.json`. The published
Piper source's `swift-api-examples/espeak-ng-data` tree is byte-identical to the
reviewed data hash; provenance names that exact commit/path. The companion offers
unmodified, checksum-pinned Piper and eSpeak/ucd source archives with a manifest
binding them to these two model hashes. It is component source, not an application
binary or an app-release qualification claim.

`scripts/checks/prepare-model-archives.js` uses the shared downloader/extractor and
Node/GNU tar, with no Python or library fork. Canonical ordering, owner, time and
permissions make all three outputs reproducible. Fresh source/model downloads and
independent assembly produce identical bytes. Atomic no-clobber promotion rejects
a different archive under an existing output name. Data links, incorrect trees,
changed cached source hashes and partial/aborted downloads fail rather than
entering release output. Developer preparation fetches have a five-minute abort
signal; other downloader callers retain their previous behavior.

```sh
node scripts/checks/prepare-model-archives.js --out /tmp/papercut-distill-assets --check-manifest
node --test scripts/checks/__tests__/prepare-model-archives.test.js scripts/checks/__tests__/check-model-downloads-selftest.js
```

`--source-dir` optionally chooses the verified input cache; `--data-dir` optionally
uses an already prepared verified data tree. Defaults reconstruct it from the
pinned Piper source archive. Sources/recipes are beside other TTS metadata in
`qualification-artifacts.json`; final downloads are in `model-manifest.json`.
The older private assembly proofs remain historical and must not be uploaded.

The production manifest also pins the separate source companion; monitoring checks
its associated release/size/hash without advertising it as a TTS model or making
the app download it. Full developer download checks include it.

The existing **Model Downloads** workflow adds `prepare-distills`. It skips the
not-yet-published metadata gate, assembles/verifies both candidates and their
companion against committed pins, runs ordinary native tests and each candidate's
production archive smoke in a fresh process, then uploads the verified packet and
creates a **draft** release. It does not publish or overwrite a release. Original
`prepare-mirror` remains scoped to the original multilingual Kokoro archive by
exact name; family-wide checks still include the pair.

Local results/evidence: `.cache/sherpa-stage4c-2026-11-15/`. Native/provider
regressions pass **374/10 ignored**; default Rust passes **293/4 ignored**.
TTS frontend tests pass **400 total** (329 non-browser and 71 browser); the browser
fixtures require local browser/server access and their initial sandbox resets
are retained separately. Focused frontend/notice/source checks pass **32**; archive
preparation checks pass **4** and monitor checks **3**. Typecheck, all eight
locales, changed-file lint/formatting, existing-waiver audit and workflow YAML/seven
shell blocks pass. Real production verification/extraction/synthesis passes for all
12 Sherpa entries across five archives. The shared 40-case corpus passes with all
22 reviewed candidate phone strings unchanged. Each new model separately passes
from its own extracted archive in a fresh process without the full Kokoro tree.
Frontend/catalog/UI/cache-ID tests, translations/typecheck, archive/source and
mirror-scope checks pass. Other platform/device and native GUI acceptance are not
inferred from these local results.

Publication and human review gate (before merge/ship):

- [ ] Review `release-final/` in the packet, `SHA256SUMS.txt`, cards, provenance,
  notices and corresponding source. Approve the exact paired artifacts.
- [ ] After commit/push of the reviewed code, run **Model Downloads →
  prepare-distills** on that exact ref, or upload the exact locally verified packet
  to a new draft. Confirm the draft contains both models, the source companion,
  checksums and the prepared manifest; never use `--clobber`.
- [ ] With GitHub immutable releases enabled, publish
  `tts-distill-7m-fp32-2026-11-15` as **non-latest**. Publication requires separate
  owner authorization. A changed artifact requires a new release identity.
- [ ] Run `check-model-downloads.js` metadata and fresh full-download checks, plus
  production `model_archive_smoke`, against the public assets. The local complete
  packet is not proof of hosted availability/immutability. Do not bypass this gate.
- [ ] In a clean app profile, select/download/preview both models, check sole voice,
  exact sizes, Arabic default diacritization/on-off behavior and the unchanged
  recommended choices. Check interrupted/checksum-failed installs, offline
  playback, save/cancel/resume/reopen, source highlighting and separate same-book
  variants; verify existing completed books still play.
- [ ] Check RTL/narrow layouts and valid saved selections. There is no existing
  model-delete UI/API to extend here; use an isolated restarted profile for removal
  and reinstall qualification, respecting Piper's process-global data lifetime.
- [ ] Accept Stage 4 before Stage 5 device/release acceptance. Native-platform
  qualification and dormant vendor deletion remain mandatory PR/merge gates.

No publishing authority, model performance claims, native qualification or vendor
retirement is inferred from implementation completion. Public-asset publication
is the next action before normal end-to-end new-model download smokes.

### Stage 5 — Owner device quality and release readiness

Use the exact reviewed candidate artifact on iPhone 16, Pixel 6 Pro, Pixel 7 and
Pixel 10 Pro. Record artifact/source revision and OS version for each device.
The owner or a named competent language listener performs listening; an agent
must not substitute non-silence checks or its own unsupported listening claim.

Human acceptance:

- [ ] On all four devices, compare existing-model baseline recordings with upgraded existing models, then audition both small candidates using the same text/settings.
- [ ] Existing English/Arabic models retain quality parity or better; both small models reproduce their accepted listening profiles on device, including the explicit English 7M quality tradeoff and default Arabic diacritization. No new omissions/pronunciation regression is accepted. Obtain competent listening coverage for the other offered languages; preserve US/British distinctions and Mandarin numeric behavior.
- [ ] Verify clean-profile install/download and an upgrade profile with completed and interrupted audiobooks, including explicit restart/cancel and normal resume of new compatible progress.
- [ ] Verify preview, saved playback, seek, source highlighting, background audio/lock-screen controls and bundle import/export on installed apps.
- [ ] No crash, NaN/corrupt audio, silent loss of content, unexpected model switch or saved-data loss occurs. Users can choose existing or small models freely.
- [ ] Platform CI and installed desktop checks are recorded; Android source-built package, alignment/notices/corresponding-source and applicable reproducibility checks have current evidence. iOS signing/distribution checks apply before its release.
- [ ] Owner approves release scope and any documented limitations. A quality regression cannot be converted into a waived performance tradeoff without a new explicit owner decision.

This gate does not authorize publication, signing, a version bump or release
dispatch by itself. Record the intended release action and applicable owner
authorization separately.

## 5. Shared validation material and evidence

Prepare one small, redistributable text corpus before recording comparisons.
Retain exact text, canonical chunking/settings and baseline audio so later
agents and human listeners compare the same material. Do not use private books
or uploads as committed fixtures. Hardware/runtime/precision can change, so
waveform equality is not the quality criterion.

| Coverage | Required cases |
| --- | --- |
| English | US and British voices; ordinary prose, names/OOV words, abbreviations, currency/years/times, punctuation, numbers and contractions; short/long and mixed-script text |
| Arabic | Vocalized and unvocalized MSA; Original text and existing Libtashkeel default; names, Arabic punctuation/numbers, mixed Arabic/Latin text and the historical Nabra failure/cleanup cases |
| Other existing Kokoro languages | Mandarin (including numbers/dates/phone normalization), Spanish, French, Hindi, Italian and Brazilian Portuguese; native punctuation and vocabulary not covered by English lexicons |
| Other existing engines | Supertonic English/Arabic and platform-supported SILMA; no changed catalog visibility/default preprocessing or unintended phonemization |
| Model limits | Empty/unspeakable text, short fragments, long unpunctuated input and actual phoneme/token/style boundaries; no unhandled native exit |
| Audio integrity | Finite, valid, non-silent speech where expected; sensible duration/sample rate and timing boundaries; short-text ARM INT8 coverage |
| Durable audio | Pre-upgrade completed/incomplete manifests, missing provenance, compatible/new provenance, source edits, restored bundles and timing-index recovery |
| Failure/resume | Download/checksum failure, source receipt/version mismatch, Cancel/stale-token hash preservation, failed/interrupted reset, new compatible progress after restart and insufficient disk |

Required platform evidence follows existing supported build targets: Linux and
Windows x64, macOS Intel/Apple Silicon, Android arm64 installed apps plus the
existing x64 emulator fixture, and iOS arm64 device/simulator. Tests on this
Linux host cannot stand in for another platform. Distinguish build, inference,
packaged launch and physical-device/listening evidence. Automated CI can cover
supported inference routes; owner checks cover the named physical devices.

Reuse existing commands/tests rather than creating a new validation framework:

- Stage 1: focused `src/tts/models.test.ts`, `AudioSetupPanel.test.tsx`,
  `useAudiobookManager.test.tsx` and audiobook-ID regressions as applicable;
  `npm run build:typecheck`, `npm run check:i18n`, applicable lint.
- Native changes: default and actual production-feature Rust suites; existing
  `model_archive_smoke` and `cached_arabic_preprocessing_smoke`, plus semantic
  embedding inference under the packaged runtime. Ignored tests need explicit
  verified assets and disposable app data; ordinary tests must not download them.
- Packaging changes: existing Android Sherpa helper/source/notices/archive tests,
  APK verifier/reproducibility tools, desktop staging and iOS device/simulator
  checks, and `.github/workflows/ci.yml` / model-download workflow coverage.
- New nontrivial logic needs a focused regression: recommendation reordering,
  provenance mismatch/restart, stale-runtime rejection and phoneme/token
  contracts. Keep test scope proportional; do not mirror implementation details.

Resolve commands from the actual source revision and record exact invocations,
feature sets, logs, input hashes, results and skipped prerequisites. Old paths
containing `v1.13.4` in docs/CI must be updated with the upgrade; copied command
examples are not validation evidence. Full suites apply to the integrated native
change; do not repeatedly rerun them for unrelated prose edits.

## 6. Scope map for the implementing agent

Read callers before edits; this is a navigation map, not a requirement to touch
every listed file. Avoid whole-branch cherry-picks from earlier Nabra experiments.

| Concern | Existing locations |
| --- | --- |
| Recommendation/UI/preferences | `src/tts/models.ts`, `types.ts`, `components/AudioSetupPanel.tsx`, `hooks/useTtsModelRuntime.ts`, `hooks/useAudiobookManager.ts`, `storage/audioPreferences.ts`, corresponding tests and existing locale files |
| Catalog/download contract | `src-tauri/src/native_tts/engine/models.rs`, `model.rs`, `src-tauri/src/native_tts/types.rs`, `src-tauri/tts/model-manifest.json`, generated metadata in `src-tauri/build.rs`, TS fallback |
| Native synthesis/preprocessing | `src-tauri/src/native_tts/engine/synth.rs`, `preprocess.rs`, `src-tauri/src/onnx_runtime.rs`, semantic embedding runtime consumers |
| Resume/identity/bundles | `engine/save.rs`, `manifest.rs`, `cache.rs`, `paths.rs`, `file_commit.rs`, `bundle/`, `src/tts/storage/AudiobookLibrary.ts`, queue/cache/saved-audiobook hooks and native IPC |
| ABI and platform preparation | `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/build/`, root `.cargo/config.toml`, `src-tauri/tts/{native,phonemizer}-artifacts.json` (dormant vendor reference pending retirement), `scripts/lib/constants.js`, platform helpers under `scripts/lib/{android,ios,linux,macos,desktop}/` |
| Android source/distribution | `scripts/fdroid/build-android-source-libs.js`, `scripts/android/prepare-license-notices.js`, `assemble-source-archive.js`, verification tools, Gradle Maven ORT dependency, licensing outputs and F-Droid metadata as applicable |
| Integration evidence/docs | Existing CI/release/model-download workflows, `docs/kokoro-tts.md`, `docs/model-download-maintenance.md`, `docs/android-production-packaging.md`, F-Droid guide and README only where behavior changes |

## 7. Human gate record and handoff

Status meanings: **Not started**, **In progress**, **Awaiting human review**,
**Accepted**, **Blocked**. Only the human gate decision makes a stage Accepted.
Record explicit N/A with a reason, never silently check an unexecuted criterion.
Separate logical/design acceptance from outstanding platform execution.

| Stage | Status | Source/artifact | Human reviewer and decision/date |
| --- | --- | --- | --- |
| 1. Recommendation policy | Accepted | `task/sherpa-onnx`, implementation after `ce054ead`; handoff above | Owner, November 11, 2026: smoke passes, Recommended labels visible; proceed to Stage 2 |
| 2. Compatibility investigation | Accepted | `a8248205`; dated evidence/proposal above, local listening packet | Owner, November 11, 2026: listening parity/no regression; proceed to Stage 3. Platform execution gates remain. |
| 3a. Simple partial-save restart | Accepted | `31fa781f5ecb4f1f4989087b38fbf7b8772f953f`; revised handoff above | Owner, November 12: all smoke tests pass; proceed to 3b. No unreported per-device results implied. |
| 3b. Existing-roster runtime/frontend upgrade | Owner local smokes passed; native gates deferred | Checkpoint `a5f3d86`; handoff/evidence below | Owner, November 15: all requested smoke tests pass; fork-free replacement requested before more feature work. Remaining platform/minimum-OS gates are not asserted passed. |
| 3c.1. Fork-free build proof | Accepted for integration | Isolated registry export after `31fa781f`; [report](sherpa-no-fork-proof.md) | Owner authorized production integration November 15; frozen proof retains its documented platform limits. |
| 3c.2. Production replacement/qualification | Owner local smokes passed; native gates deferred | Checkpoint `a5f3d86`; separate three-crate security repair passes audit and local tests | Owner permits paired 7M work after local acceptance. Native SDK/minimum-OS/device qualification and dormant vendor retirement remain PR/merge gates. |
| 4a. Paired artifact/frontend qualification | Accepted | [Report](#stage-4a--paired-qualification-record), pinned inputs and machine-local listening packet | Owner, November 15: both approved; explicit English crispness/robotic-quality tradeoff for smaller option; Arabic competitive with Kareem, preserve default diacritization. |
| 4b. Native frontend integration | Accepted | Same shared engine; reserved IDs, pinned Misaki/Piper adapter and focused guards | Owner, November 15: requested smoke tests pass; commit checkpoint. No catalog exposure. |
| 4c. Paired artifact/installer/catalog integration | Implemented; local checks passed, publication/human install gates open | Exact paired archives/source companion, native/browser catalog, existing installer/UI and preparation workflow | No public upload/dispatch. Approve/publish immutable artifacts, then run hosted download and native GUI smokes before accepting Stage 4. |
| 5. Device/release acceptance | Not started | — | — |

At every handoff, update this record and provide:

1. Current branch, base/head SHA, dirty files and exact work completed.
2. Accepted decisions versus proposals/open blockers; cite the owner's gate
   decision, without treating an agent's recommendation as approval.
3. Changed paths, dependency/artifact pins and identity/compatibility impact.
4. Executed checks with revision-bound logs/artifacts, plus skips and reasons.
5. Listening reviewer, language, text/settings and baseline/candidate verdicts;
   do not mark listening passed based on file validity.
6. Retained user-data/recovery evidence and reproducible failure cases.
7. The next single stage and the exact actions authorized. Stop at its gate.

Keep detailed binaries/logs in ignored output or appropriate CI artifacts, with
stable references and reproduction instructions in the handoff. The committed
spec must stand on its own; private MemSearch journals are helpful context, not
a required dependency for another agent. Do not commit model weights, runtime
libraries, generated audio or private user data.

For every remaining pass, review function-level comments on added/changed code
and correct outdated explanations in the touched flow. Record non-obvious intent,
caller obligations, examples, compatibility decisions and failure/recovery
gotchas where maintainers need them. Keep shared architecture in module/docs
comments and avoid restating self-evident implementation steps or test names.
Keep touched Rust modules and test scopes organized with grouped standard-library,
external and local imports. Import repeated long helper paths where names remain
clear; retain qualification where it resolves ambiguity or clarifies a namespace.
Follow surrounding conventions and scoped rustfmt without unrelated reformatting.

Suggested first-stage instruction for a new agent:

> Read `docs/sherpa-onnx-upgrade.md` and the current TTS guide. Start Stage 1 only,
> on `task/sherpa-onnx` from current main, preserving unrelated changes. Use
> Ponytail and MemSearch. Implement/test recommendation labels and deterministic
> English/Arabic selection while preserving existing preferences and saved-audio
> identity. Present Gate 1 evidence and stop for human review. Do not begin the
> runtime upgrade, add models, commit or push without session authorization.

## 8. Primary research references

These support the dated investigation; Stage 2 must verify the exact candidate
inputs. A merged conversion PR is not proof of our packaged-app quality.

- [Sherpa 2.0.1 release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v2.0.1)
- [2.0.1 changelog](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/CHANGELOG.md)
- [Removal of embedded phonemization, #4055](https://github.com/k2-fsa/sherpa-onnx/pull/4055)
- [Rust generation API](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/sherpa-onnx/rust/sherpa-onnx/src/tts.rs)
- [Official external-phonemizer Rust example](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/rust-api-examples/examples/kokoro_tts_en_phonemize.rs)
- [External piper-phonemize 1.5.9](https://github.com/csukuangfj/piper-phonemize/tree/v1.5.9)
- [Kokoro text/phoneme/lexicon implementation](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/sherpa-onnx/csrc/offline-tts-kokoro-impl.h)
- [Kokoro/Nabra 7M conversion, #3954](https://github.com/k2-fsa/sherpa-onnx/pull/3954)
- [7M reference frontend](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/scripts/kokoro/7m-distill/g2p_frontend.py)
- [English 7M model card](https://huggingface.co/oddadmix/Kokoro-7M-Distill)
- [Arabic 7M model card](https://huggingface.co/oddadmix/Nabra-7M-Distill)
- [INT8 export correction, #4087](https://github.com/k2-fsa/sherpa-onnx/pull/4087)
- [iOS XCFramework artifacts](https://github.com/k2-fsa/sherpa-onnx/releases/tag/xcframework)
- [Upstream Rust build fixes, #3991](https://github.com/k2-fsa/sherpa-onnx/pull/3991)
- [Inspected phonemizer C API/source](https://github.com/csukuangfj/piper-phonemize/blob/dae4d2d831f9f0817355f7c0e8abd98c2fb0a49d/src/c-api.cpp)
- [Inspected phonemizer Rust build policy](https://github.com/csukuangfj/piper-phonemize/blob/dae4d2d831f9f0817355f7c0e8abd98c2fb0a49d/rust/piper-phonemize-sys/build.rs)
- [Separate ORT 1.28.2 framework assets](https://github.com/csukuangfj/onnxruntime-libs/releases/tag/v1.28.2)
- [Pinned Arabic cleanup reference](https://huggingface.co/oddadmix/Nabra-7M-Distill/raw/03b95013d3dcd8be6f0b0412a157d25518c7b756/arabic_g2p.py)
- [Historical Nabra boundary/cleanup issue, #4057](https://github.com/k2-fsa/sherpa-onnx/issues/4057)
