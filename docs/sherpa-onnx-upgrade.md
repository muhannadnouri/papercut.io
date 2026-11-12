# Sherpa ONNX upgrade and model selection plan

Status: Stages 1–2 accepted by the owner; Stage 3 authorized; Stages 4–5 not started. Owner direction
confirmed November 11, 2026. Research baseline: local `main` at `6baf1858`, Sherpa
1.13.4. Candidate runtime: 2.0.1, subject to the compatibility gate below.

This is the implementation and human-review contract for this work. The
[native TTS guide](kokoro-tts.md) describes the shipping architecture;
[model maintenance](model-download-maintenance.md) describes downloads and
monitoring; [Android production packaging](android-production-packaging.md)
describes source builds, notices and distribution evidence. Proposed behavior
here must not be represented as shipped or validated until its gate passes.

## 1. Agreed outcome and boundaries

- Add the existing translated **Recommended** label to English Kokoro v1.0 and
  Arabic Piper Kareem Medium. Use those models as the starting selection when
  entering their speech language, without overriding a valid existing choice.
- Upgrade Sherpa surgically while preserving all currently offered languages,
  voices, saved audio, highlighting, and ordinary model installation/playback.
- Qualify **English Kokoro 7M and Arabic Nabra 7M together** as additive,
  single-voice choices. Neither replaces an existing model. Do not quietly ship
  English alone if Arabic fails qualification; return that decision to the owner.
- Prioritize generated speech quality, particularly pronunciation. Acceptance
  requires parity or improvement; speed or smaller weights cannot excuse
  mispronunciation, omissions, or other speech-quality regressions. An
  Experimental label does not waive this requirement.
- No numeric speed or memory targets, performance-tuning project, or benchmark
  gate is required. Users choose the model appropriate to their hardware.
  Crashes, corrupt audio and ordinary usability failures still fail acceptance.
- Owner device checks: **iPhone 16, Pixel 6 Pro, Pixel 7, Pixel 10 Pro**.
  Record actual OS versions and app artifacts when testing; do not invent them.
- Preserve the current upload rule: automatic document-language suggestions
  skip uploaded documents. The language selector remains available there.
- Handle incompatible interrupted saves explicitly, preserving existing data
  until the user authorizes regeneration and a replacement is safely committed.

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

### Interrupted-save proposal for Gate 3 review

The agreed requirement is explicit, non-destructive handling. The concrete
interaction below is proposed and must receive human review before activation.

1. Record a small synthesis revision in the existing manifest, representing
   changes relevant to generated output. Define exact inputs during Stage 2;
   recommendations, display labels and thread counts are not such changes.
2. Check it before `write_pending_manifest`, pruning, reuse or any replacement.
   A missing revision on an older incomplete job is unknown, not proven current.
3. Continue compatible incomplete jobs normally. Preserve all completed audio
   for playback/export, even when its original synthesizer revision is old.
4. For an incompatible incomplete job, explain that regeneration is required
   and offer an explicit restart/cancel decision. Queue restoration must obey
   the same rule and must not silently accept on the user's behalf.
5. On cancel, leave the old manifest/chunks unchanged. On restart, isolate the
   replacement from old chunks and preserve the old generation until the new
   one is validated and committed. A failed, cancelled or interrupted rebuild
   must not destroy the retained generation. Use existing atomic file-commit
   mechanisms and define any directory promotion/recovery before implementation.
6. Insufficient disk space or failed replacement reports an actionable error;
   it must not silently delete retained data to make room. Reuse existing space
   checks where available. No new whole-library migration framework.

Generation provenance is a synthesis/resume concern, not a reason to make old
saved audio unavailable. Check native status, queue restoration, direct save,
import/export and manifest-index rebuild paths, not only the visible button.

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
8. Specify the smallest generation-revision and restart/recovery design, exact
   stale-runtime cache invalidation, and the Stage 3 file/test scope.

Human acceptance:

- [ ] Reviewer receives pinned inputs, commands, actual outputs and limits for each probe; compiling or generating a non-silent WAV is not claimed as pronunciation acceptance.
- [ ] Current language routes have a credible supported implementation and reviewed sample evidence; names/unknown words are not silently dropped.
- [ ] Exact iOS linking and Android source/packaging proposals are concrete; unavailable platform execution is explicitly pending, not marked passed.
- [ ] Wrapper, sys bindings and native ABI versions align; one compatible ORT route covers all consumers. Any dependency addition has a specific necessity.
- [ ] Existing native-runtime copy protections survive the proposed vendor rebase; cold, restored and alternating-version caches cannot select stale binaries.
- [ ] Nabra blockers are explicitly resolved by evidence or remain named blockers for the paired model stage.
- [ ] The non-destructive interrupted-save proposal, copy requirements and failure recovery are reviewable and accepted for implementation.
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
- Keep current audiobook IDs and `chunks/` layout. Regenerate into a sibling
  directory, using the existing save/index validation code. Retain the original
  directory and source references through cancellation, failure and disk shortage.
  Check the expected old generation again before publication to reject stale jobs.
- Publication needs more than `commit_staged_file`: that helper atomically
  replaces a file, while the existing import promoter refuses to overwrite an
  incomplete directory. Under the audiobook reference/write lock, persist a
  small same-parent replacement journal, rename old to retained backup, then
  rename validated staging to the canonical path. Never pre-delete the old data.
- Recovery before reading/writing that book: a validated published new manifest
  wins; if canonical is absent and a valid backup exists, restore the backup;
  if old canonical still exists, preserve it. Unknown/conflicting states fail
  closed and retain all copies. Journal paths must be derived/validated app-owned
  names, not arbitrary filesystem paths from JSON. Remove the backup only after
  new publication is validated. Serialise save/import/delete/status recovery so
  another operation cannot mistake an active transaction for abandoned work.
- An isolated filesystem design self-check passed interruptions before retiring
  old data, after retiring it, and after publication, retaining old data in every
  case. This is not production Rust recovery, Windows rename testing or a
  power-loss guarantee. Stage 3 must test actual implementation, queue/native
  race guards, cancellation, full disk and interruption at each transition.

**Gate 2 disposition:** owner accepted November 11, 2026 after reviewing
`listening.html`, reporting no regression and quality at parity, and authorized
Stage 3. This records that listening decision without extending it to untested
platforms or future production integration. Review the proposed
C API/source-build/static-link and restart/recovery contracts above. Actual
Windows/macOS/iOS execution and Android inference/packaging remain explicit
Stage 3 gates, followed by the four owner devices in Stage 5. If listening finds
a regression, resolve it before accepting migration; do not waive it for speed.

On approval, implement **Stage 3 only**, with existing models: native phonemizer,
complete sys rebase with retained safeguards, matching platform preparation,
and non-destructive interrupted-save handling. Stage 4 model additions remain
separate. Changes to this report are not authorization to start that work.

### Stage 3 — Upgrade existing models and protect interrupted saves

Scope: matching Cargo/lock/vendor/runtime changes, required native frontend,
platform preparation/bundling, backward-readable provenance and restart handling.
Do not add new catalog models in this stage.

Steps:

1. Implement the reviewed runtime/frontend integration and narrowly rebase vendor
   changes. Update pins, hashes, source/notice tooling and CI paths together.
2. Make prepared native caches version-aware; test existing caches and restoration
   as well as clean builds. Fail closed on incompatible source receipts/ABI.
3. Implement provenance checks and explicit restart across UI, queue and native
   entry points, with checks before any destructive operation.
4. Run the existing suites appropriate to the changed paths plus native inference
   and packaging checks. Use the validation matrix below and real old-format
   fixtures made from disposable copies; do not mutate the owner's library.
5. Update shipping architecture/maintenance/packaging docs to describe only the
   implemented behavior. Present Gate 3 evidence with exact source revision.

Human acceptance:

- [ ] Every existing model/language/voice remains available on its existing platforms; original IDs, speaker mappings and download identities are preserved.
- [ ] Current-runtime versus upgraded-runtime samples have pronunciation and overall speech-quality parity or improvement, including US/British and multilingual cases. No missing-word regression is accepted.
- [ ] Completed historical HTML/EPUB/PDF audio plays, seeks, highlights and exports offline without redownload or regeneration; imported bundles restore original settings.
- [ ] Compatible incomplete saves resume; incompatible/unknown legacy saves require explicit restart. Cancel and queue restoration cannot bypass that decision.
- [ ] Failed/cancelled rebuild, disk-full and interruption before/after commit preserve the retained generation; restart never reuses incompatible chunks. Recovery is deterministic after relaunch.
- [ ] Valid current manifests remain readable, including missing-field defaults and timing-index recovery; a global cache/schema bump was not used as a shortcut.
- [ ] Clean and restored-cache builds/inference pass on the required platform matrix; native library loading, initialization order and packaging/source/notice checks are evidenced.
- [ ] Reviewer accepts migration of the existing roster and explicitly authorizes Stage 4. Remaining owner device checks are recorded for Stage 5, not asserted passed.

### Stage 4 — Add both small models and their selection UX

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
   authorize a speech-quality downgrade.

Human acceptance:

- [ ] Both candidates have reproducible pinned artifacts, verified vocab/style mappings and native frontend behavior; no Python runtime or unreviewed diacritizer is introduced.
- [ ] Both pass side-by-side pronunciation/quality review. A candidate failing this gate is not exposed merely for smaller size; obtain an owner decision if the pair cannot qualify.
- [ ] Install, preview, save, resume, offline playback, delete and reinstall work independently of existing models. Failed/checksum-mismatched downloads cannot become installed.
- [ ] UI communicates language and single-voice capability, distinguishes recommendations from experiments, preserves old selections and works in RTL/narrow layouts.
- [ ] New IDs create distinct audiobook generations; existing records and bundles remain available and unchanged. Native/fallback catalog and download monitoring cover both additions.
- [ ] No numeric speed/memory acceptance threshold or unmeasured Faster claim has been introduced. Reviewer approves the pair for final device acceptance.

### Stage 5 — Owner device quality and release readiness

Use the exact reviewed candidate artifact on iPhone 16, Pixel 6 Pro, Pixel 7 and
Pixel 10 Pro. Record artifact/source revision and OS version for each device.
The owner or a named competent language listener performs listening; an agent
must not substitute non-silence checks or its own unsupported listening claim.

Human acceptance:

- [ ] On all four devices, compare existing-model baseline recordings with upgraded existing models, then audition both small candidates using the same text/settings.
- [ ] English and Arabic pronunciation, word coverage and overall quality are parity or better. Obtain competent listening coverage for the other offered languages; preserve US/British distinctions and Mandarin numeric behavior.
- [ ] Verify clean-profile install/download and an upgrade profile with completed and interrupted audiobooks, including explicit restart/cancel/recovery.
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
| Failure/recovery | Download/checksum failure, source receipt/version mismatch, cancelled jobs, insufficient disk, failed replacement and process interruption; retained-data hashes before/after |

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
  provenance mismatch/recovery, stale-runtime rejection and phoneme/token
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
| ABI and platform preparation | `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/vendor/sherpa-onnx-sys/` including `runtime_copy.rs`, `scripts/lib/constants.js`, platform helpers under `scripts/lib/{android,ios,linux,macos,desktop}/` |
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
| 3. Existing-roster upgrade/resume | Not started | — | — |
| 4. Paired small-model addition | Not started | — | — |
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
