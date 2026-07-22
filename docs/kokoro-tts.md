# Native multilingual TTS with sherpa-onnx

Papercut's TTS path is native sherpa-onnx with a catalog of model families. Kokoro remains the English default; Piper Kareem Medium adds Arabic (`ar-JO`) through sherpa VITS. The previous browser Web Worker / kokoro-js fallback has been removed from active playback and audiobook saving because it was too slow on Android and was limited by browser/WebView inference constraints.

The design goal is still offline-first: model files live on the user's device, audiobook chunks are generated only when a user asks to save a document, and generated audio is stored as user data instead of being pre-rendered into the app bundle.

## Runtime Architecture

The frontend owns document parsing, HTML narration chunking, the platform-neutral playback state consumed by controls and highlighting, saved-audiobook state, downloads, and diagnostics. Native code owns synthesis, the fast audiobook-save path, persisted playback indexes, and mobile background playback.

The boundary is intentionally small:

- `src/tts/api/nativeTts.ts` calls Tauri commands and subscribes to native model-install/save progress events.
- `src-tauri/src/native_tts/` is the Rust backend module. Its `commands` layer exposes the Tauri commands and dispatches, via one `#[cfg]` switch, to either the `engine` submodules (real sherpa-onnx synthesis, compiled with `native-tts-core`) or a `stub` fallback when native TTS is not compiled. Inside `engine`: `models` defines catalog metadata and family-specific loading data, `model` downloads/verifies the selected model, `synth` loads sherpa-onnx and synthesizes individual chunks, `preprocess` owns optional language-aware synthesis-text transforms, `save` runs native batch generation and writes a durable timing index, `cache` scans saved chunks and parses WAV headers, `playback` prepares or reuses one cached mobile playback track, and `bundle` (`export`/`import`/`manage`) handles audiobook export/import and deletion. `paths`/`config` hold shared path/id/constant helpers, and OS-specific tuning (thread counts) lives in `platform`.
- `src/tts/hooks/useTtsPlayer.ts` exposes one playback state contract to the UI. Desktop reads a bounded window of saved chunk WAVs; mobile maps one native track timeline back to chunk-local state. It never synthesizes missing chunks live.
- `src/tts/playback/browserAudioCache.ts` owns the desktop window of WAV Blob URLs, concurrent chunk reads, and deterministic URL revocation. Browser playback itself stays on the standard `HTMLAudioElement` API.
- `src/tts/playback/nativeMobileAudio.ts` is the narrow adapter around the official, exactly pinned `tauri-plugin-native-audio` 1.0.5 API. Papercut serializes bridge commands and owns its foreground polling cadence instead of forking or modifying the plugin.
- `src/tts/hooks/useTtsModelRuntime.ts` owns model selection, native capability discovery, thread limits, install status/progress, and SILMA sidecar probing.
- `src/tts/hooks/useAudiobookDocument.ts` owns document save-chunk preparation, imported bundle metadata restoration, and legacy source-span reconstruction for highlighting.
- `src/tts/hooks/useAudiobookCache.ts` checks native audiobook files and starts long-running native save jobs.
- `src/tts/hooks/useAudiobookDownloadQueue.ts` owns the active native save, resumable queue records, throttled progress persistence, cancellation, and completed-job cleanup.
- `src/tts/hooks/useSavedAudiobookActions.ts` owns native saved-audiobook import, export, and delete execution plus their busy states and transient notices.
- `src/tts/hooks/useAudiobookManager.ts` coordinates playback actions, model suggestions, confirmations, reader updates after saved-audiobook actions, and the prop bundles consumed by the audio UI components. Saved audiobooks can export either a re-importable `.papercut-audiobook` bundle or a plain stitched `.wav` file for use outside Papercut.
- `src/components/DocumentViewer/DocumentViewer.tsx` hosts the reader shell and exposes slots for TTS controls/diagnostics while owning Find, scroll-to-top, same-document link scrolling, and current-chunk highlighting.
- `src/tts/components/AudioControls.tsx`, `src/tts/components/AudiobooksPanel.tsx`, `src/tts/hooks/useTtsHighlight.ts`, `src/tts/alignment/highlightRanges.ts`, and `src/tts/utils/format.ts` keep document playback controls, saved-download UI, React highlight lifecycle, DOM range mapping, and audiobook display formatting out of `App.tsx`.

This keeps expensive inference and large WAV writes out of the WebView while preserving the existing React UI, highlighting contract, portable bundle format, and offline cache metadata.

## Model Catalog And Download

The app does not package voice models into desktop installers or Android APKs. The model selector uses Rust capabilities as the authoritative catalog; a matching TypeScript fallback keeps startup and browser UI deterministic. Adding a model requires catalog metadata, required-file validation, and a sherpa family loader, not a parallel save/playback implementation.

Pinned models:

| Model | Family | Language | Archive bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| Kokoro English v1.0 | Kokoro | `en-US` | 349,418,188 | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Kokoro Mandarin v1.0 | Kokoro | `zh-CN` | 349,418,188 (shared) | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Kokoro Spanish v1.0 | Kokoro | `es-ES` | 349,418,188 (shared) | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Kokoro French v1.0 | Kokoro | `fr-FR` | 349,418,188 (shared) | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Kokoro Hindi v1.0 | Kokoro | `hi-IN` | 349,418,188 (shared) | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Kokoro Italian v1.0 | Kokoro | `it-IT` | 349,418,188 (shared) | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Kokoro Brazilian Portuguese v1.0 | Kokoro | `pt-BR` | 349,418,188 (shared) | `c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046` |
| Piper Kareem Medium | VITS/Piper | `ar-JO` | 67,177,830 | `9ebbcea30e0fbd588f7b2cb45ee897d6aeb1bf5791cbc037a7b5a3f641e3dbce` |
| Supertonic 3 English | SupertonicTTS | `en-US` | ~123,000,000 | `82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427` |
| Supertonic 3 Arabic | SupertonicTTS | `ar` | ~123,000,000 | `82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427` |

These archives come from `https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models` and are listed in `src-tauri/tts/model-manifest.json`. Rust downloads into a temporary cache directory, verifies SHA-256, extracts, validates required files, and atomically promotes the selected model into `models/sherpa-onnx/<model-directory>/`. Incomplete installs are not used.

SupertonicTTS 3 is exposed as two experimental catalog entries, English and Arabic, backed by one shared multilingual int8 archive. sherpa selects language through `GenerationConfig.extra["lang"]`, so Papercut keeps separate model IDs for cache identity while installing the same model directory. Treat Supertonic speed and quality as measured device behavior, not a guaranteed win over Piper or Kokoro; use TTS diagnostics to compare `realTimeFactor`, `synthesisMs`, and `preprocessMs` before changing defaults.

Kokoro keeps one English model entry and its existing stable voice IDs, but selects
`en-us` for American `a*` voices and `en-gb` for British `b*` voices per synthesis
request. Voice labels show the upstream locale and overall training-data grade;
the [upstream voice catalog](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md)
describes those grades as estimates rather than guarantees of subjective voice
quality. These display and phonemization changes do not alter audiobook identity
or invalidate existing saved WAV files.

Mandarin is a separate catalog entry with its own stable model ID and the eight
official `zf_*`/`zm_*` speakers, but it reuses the same `kokoro-multi-lang-v1_0`
directory and verified archive as English. sherpa receives the archive's Chinese
lexicon plus `phone-zh.fst`, `date-zh.fst`, and `number-zh.fst`; the per-request
language is `zh`. Existing English preferences and saved-audiobook identities are
unchanged, while Mandarin generations remain distinct because they use the new
model ID.

Spanish, French, Hindi, Italian, and Brazilian Portuguese are exposed through
[sherpa-onnx's multilingual Kokoro frontend](https://github.com/k2-fsa/sherpa-onnx/pull/2303)
and their matching Kokoro speaker embeddings.
They reuse the same archive and install directory but receive separate model IDs
so saved-audiobook identity remains language-specific. These entries still need
native-speaker validation: upstream warns that non-English quality can be
limited by G2P and training-data coverage. Voice labels retain a grade from the
[upstream voice catalog](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md)
when one exists; Spanish and Portuguese omit grades because upstream does not
publish one.

Shared audiobook chunking recognizes Latin and Arabic sentence punctuation,
Chinese `。！？`, Hindi `।॥`, and common CJK clause marks. This keeps long
multilingual prose bounded at natural pauses before native synthesis.

Japanese speaker embeddings exist in `voices.bin`, but native Japanese text is
not exposed because sherpa-onnx does not currently ship the Japanese frontend
needed to convert it into Kokoro token IDs. In Papercut's current runtime those
embeddings are accent choices, not working Japanese-language support.

Piper Kareem is about 64 MB compressed and suitable for offline Arabic, but a medium Piper voice is not expected to match Kokoro's naturalness. Treat quality as an empirical product decision. The Piper voice repository declares MIT for model files; the dataset card does not clearly state training-data licensing, so legal/provenance review is required before bundling or broadly redistributing it. Papercut currently downloads it on demand rather than embedding it.

Existing saved audiobook files are unchanged. The cache version stays `native-save-v4-segmented`; Kokoro retains the same model ID, voice IDs, and audiobook identity. Legacy preferences, download records, saved records, imported uploads, native manifests, and bundles default missing model metadata to Kokoro and missing preprocessing metadata to `none`. Existing completed WAV audiobooks remain playable. Diacritized Piper saves use a distinct ID and never overwrite or reuse undiacritized audio. Documents unaffected by the wrapper-text omission keep the same narration text. An affected document must be regenerated to include the newly retained prose; the corrected source signature and chunk sequence intentionally differ from the incomplete generation.

## Arabic Pronunciation And Diacritization

Piper Kareem uses eSpeak-ng phonemization through sherpa-onnx. Arabic normally omits short vowels, so an undiacritized spelling can represent several pronunciations and meanings. Piper and eSpeak-ng do not provide enough contextual language understanding to resolve every ambiguity by themselves.

Shared native builds and iOS native builds now compile `libtashkeel_base = 1.5.0` as an optional preprocessing backend. The model catalog exposes preprocessing as data instead of Piper-specific UI logic:

- `none`: pass source text to synthesis unchanged. This is the only Kokoro option and preserves all historical audiobook IDs.
- `libtashkeel-1.5.0`: run the bundled 4,788,213-byte Arabic diacritization model before Piper synthesis. This is the Piper default on desktop shared builds, Android, and iOS native builds.

The Rust `TextPreprocessor` boundary is deliberately small. Save retains canonical `chunk.text`, chunk IDs, source hashes, and source spans; it creates a separate synthesis string, applies Libtashkeel once per missing chunk, then passes that result through the normal sherpa text sanitizer and selected model. React only consumes model capabilities and persists the selected preprocessing ID. A future Arabic preprocessor or another language pipeline can therefore be added in the model catalog without changing audiobook orchestration.

Highlighting continues to use original source text and spans. Diacritics are never inserted into the HTML, React chunk text, search index, or DOM range matching. Current highlighting is chunk-based rather than phoneme-timed, so a longer diacritized synthesis string does not shift the highlighted source range.

Audiobook identity includes the versioned preprocessing ID only when it is not `none`. This has two useful properties:

- Existing Kokoro and undiacritized audiobook IDs remain byte-for-byte unchanged and older generated audio stays playable.
- Diacritized Piper audio uses a separate cache directory, saved record, queue entry, manifest, and export-bundle field, so it cannot reuse audio produced from different vocalization.

Legacy local records, native manifests, imported uploads, and version-2 bundles that omit preprocessing metadata deserialize as `none`. Imported audiobook bundles replay from their stored bundle chunk metadata instead of re-chunking the restored source HTML, so older Kokoro bundles remain playable even when newer document extraction or chunk-boundary code would produce a different source signature. New bundle exports retain optional chunk source spans, so a re-imported HTML/EPUB generated-reader audiobook can use those spans immediately. Older bundles rebuild highlighting lazily from the restored HTML after open: if the current chunker produces the same ordered chunk ids/text, Papercut grafts fresh DOM source spans onto the bundle chunks; if not, playback remains available and the imported-bundle fallback can try to recover spans from the live reader text. That fallback matches exact text first, then applies Arabic-aware matching for tashkeel, tatweel, bidi controls, and common Arabic/Persian letter variants. The cache version remains `native-save-v4-segmented`; no global invalidation is needed. Imported bundle validation still requires the current runtime to support the bundle's stored preprocessor, even though completed bundles already contain WAV payloads and do not need Libtashkeel to play. Treat that as a known compatibility bug in the import path, separate from the iOS packaging/linking work needed for new Arabic audiobook generation.

Libtashkeel uses its bundled neural model and preserves user-provided Arabic diacritics as inference hints. Papercut supplies already bounded narration chunks and calls the preprocessed input path, avoiding a second sentence-segmentation layer. The crate limit is 12,000 characters, while Papercut save chunks are capped far below that. Text containing no Arabic characters bypasses inference.

Quality remains probabilistic. Automatic tashkeel should improve many vowels, but names, foreign words, dialect, syntax-dependent case endings, and genuinely ambiguous sentences can still be wrong. Piper Kareem also remains a medium single-speaker voice and should not be presented as Kokoro-equivalent naturalness. Keep the `Original text` option available for user-supplied fully vocalized text and for debugging regressions.

The crate and model code are MIT/Apache-2.0 licensed. `ort-sys` is the low-level Rust FFI to ONNX Runtime, while `ort` is the safe Rust API used by Libtashkeel. Both are pinned to `2.0.0-rc.1` because that is the API version required by Libtashkeel 1.5.0. Desktop and Android use dynamic loading so Libtashkeel and sherpa share one packaged `libonnxruntime` instead of embedding a second runtime. iOS uses static linking against the prepared sherpa iOS `libonnxruntime.a` archive. `ORT_LIB_LOCATION` is build plumbing, not an application preference: desktop, Android, and iOS build helpers point Cargo at the already prepared sherpa library directory so `ort-sys` can find the correct platform/ABI library. At runtime desktop/Android call `ort::init_from`, while iOS calls `ort::init` against the linked archive.

Potential future upgrades remain separate product choices. CATT can be evaluated as a more accurate ONNX preprocessor if mobile cost is acceptable. SILMA TTS is a future full speech-engine candidate, not a drop-in preprocessor; its current Python/PyTorch distribution is too large for the present sherpa mobile architecture.

Primary references:

- Libtashkeel repository: https://github.com/mush42/libtashkeel
- Libtashkeel crate: https://crates.io/crates/libtashkeel_base
- CATT repository and ONNX path: https://github.com/abjadai/catt
- SILMA TTS model card: https://huggingface.co/silma-ai/silma-tts
- Piper eSpeak-ng phonemization: https://github.com/OHF-Voice/piper1-gpl
- sherpa-onnx Piper integration: https://k2-fsa.github.io/sherpa/onnx/tts/piper.html
- sherpa-onnx SupertonicTTS integration: https://k2-fsa.github.io/sherpa/onnx/tts/supertonic.html

## Build And Run

Browser preview still works for document/search UI:

```bash
npm run browser
```

TTS synthesis is native-only, so browser preview will report native TTS as unavailable.

Desktop builds compile the native TTS feature:

```bash
npm run desktop
```

Android debug APK builds without native TTS still use the normal command:

```bash
npm run android:apk
```

Android native TTS uses the official sherpa-onnx Android shared-library archive, copies the shared objects into Tauri's generated `jniLibs`, and builds only the common arm64 target by default:

```bash
npm run prepare:jdk
npm run prepare:sherpa-android-libs
npm run android:apk:native-tts
```

The explicit prepare commands are useful for setup and troubleshooting, but `npm run android:apk:native-tts` also ensures the sherpa Android libraries are present before building. The native Android wrapper sets `JAVA_HOME`, `SHERPA_ONNX_LIB_DIR`, and `ORT_LIB_LOCATION` automatically for the default arm64 APK path. `npm run prepare:jdk` installs a repo-local Eclipse Temurin JDK 17 into `src-tauri/tts/runtime/jdk/temurin-17` when a system JDK is not available. The fallback JDK archive is pinned to Eclipse Temurin 17.0.19+10, and both the JDK archive and sherpa Android archive are verified with SHA-256 before extraction. Downloads are written through temporary files before being promoted to their cache path. The Android build still requires the normal Android SDK/NDK prerequisites. Native background audio raises the app minimum to Android API 26, matching the plugin's Media3 service requirement.

The Node build scripts are orchestration around npm, Cargo, Tauri, Gradle, and SDK Manager rather than a replacement for those tools. Shared helpers in `scripts/lib/` own project paths, version constants, child-process execution, archive extraction, and checked downloads. Platform-specific helpers live in `scripts/lib/android/` for Android JDK/sherpa setup, `scripts/lib/linux/` for Linux shared-library bundling, `scripts/lib/macos/` for macOS dylib bundling, and `scripts/lib/ios/` for iOS static XCFramework prep. Script entrypoints such as `prepare:sherpa-android-libs`, `prepare:sherpa-ios-libs`, `android:apk:native-tts`, `ios:ipa:native-tts`, and `desktop` call those helpers instead of relying on import side effects or duplicated archive/spawn code. Android APK variants are handled by one top-level `scripts/build-android.js`; the native TTS npm command passes `--native-tts` to that script. iOS uses `scripts/build-ios.js` to select the device or arm64 simulator sherpa static-library slice and build with `native-tts-ios`, which includes static Libtashkeel preprocessing. A local `sherpa-onnx-sys` patch teaches the Rust build script to use the upstream iOS aggregate archives (`libsherpa-onnx.a` and `libonnxruntime.a`) instead of desktop component archives. The generated iOS app target also includes a tiny Swift bridge source so Xcode links Swift runtime libraries needed by Tauri/plugin Swift objects inside `libapp.a`, and PR CI performs both simulator and unsigned generic iPhoneOS device native-TTS builds. The device check prebuilds the Rust static library with Cargo using the configured iOS minimum deployment target, stages Cargo's `libapp_lib.a` output as `libapp.a` in `gen/apple/Externals/arm64/release/`, and then uses direct `xcodebuild build` with the generated Tauri script skipped so it stops before archive/export signing.

On macOS, `sherpa-onnx-sys` downloads per-arch (`osx-x64` / `osx-arm64`) shared-library archives during the Cargo build, copies the `.dylib`s next to the dev binary, and emits an `@loader_path` rpath for `npm run tauri:dev`. The desktop build helper runs a fast macOS `cargo check` first so `sherpa-onnx-sys` downloads the dylibs, stages every `.dylib` from the sherpa macOS archive before Tauri scans `bundle.resources` (including versioned ONNX Runtime siblings such as `libonnxruntime.1.24.4.dylib`), signs staged dylibs with the Developer ID identity and secure timestamp when release signing is active, and the `tauri.macos.conf.json` `beforeBundleCommand` (`scripts/copy-sherpa-macos-libs.js`) refreshes that staging directory before bundling. `src-tauri/build.rs` adds an `@loader_path/../Resources` rpath so the installed `.app` (binary in `Contents/MacOS`, resources in `Contents/Resources`) resolves the dylibs at launch. The same shared ONNX Runtime is reused by Libtashkeel via `ort::init_from`. macOS builds are produced per-architecture on `macos-15-intel` (Intel x86_64) and `macos-15` (Apple Silicon aarch64) runners. Release builds use the protected `apple-release` GitHub Environment to import the Developer ID certificate, sign with hardened runtime entitlements, notarize, staple, and verify the `.dmg` artifacts; ordinary PR/CI or local builds without Apple secrets remain unsigned development artifacts.

`npm run android:apk:native-tts` does not copy model files into Android assets. The first TTS use on Android follows the same in-app model download path as desktop, so the APK stays smaller and developers do not need to carry large model assets between machines.

## Cache Strategy

Papercut does not generate corpus-wide audiobook files at build time. That approach does not scale because every new document would increase build time and bundle size.

Instead, audio is generated on demand:

1. The user opens a supported readable document and clicks Save. HTML and EPUB use the sanitized HTML source loaded by the reader; EPUB source is generated by the upload parser from the OPF spine.
2. The app builds deterministic narration chunks for that document.
3. The native audiobook directory in app data is scanned for existing WAV chunks.
4. For each missing chunk, the selected text preprocessor creates synthesis text while preserving canonical source text and spans.
5. A single native save job generates every missing chunk in sequence and writes WAV files directly to app data.
6. Native progress events update the React Audiobooks panel and TTS diagnostics.
7. A localStorage registry marks the audiobook complete only when every chunk exists.
8. Save writes a versioned `manifest.json` containing compact chunk timing/size metadata. On first mobile Play, Rust reuses a restored bundle track when available or streams the saved chunks into an atomic cached `playback.wav`; later plays reuse that track and its `playback.json` boundaries.

Save uses a conservative chunk profile that is separate from playback chunking. Sentence detection includes Arabic `؟`; clause splitting includes `،` and `؛`; a final word-boundary hard split guarantees no request exceeds the profile maximum even when punctuation is missing. Playback keeps larger chunks for comfortable skip/highlight behavior, while Save uses smaller sentence-like chunks so one problematic text range is less likely to kill a long Android job and Resume has less work to retry.

### Narration Text Alignment

Narration chunks are built from reusable readable-text segments instead of raw `body.textContent`. The HTML adapter assigns each text node to its nearest readable block and emits ordered owner runs. This preserves direct text owned by wrapper elements before or after nested headings and paragraphs without duplicating nested content. Legacy HTML often places prose directly inside table cells; treating every wrapper with readable descendants as structure previously dropped that prose before chunking or synthesis. Bracketed inline footnote reference anchors (`[1]`, `[2*]`, `[8a]`, etc.) are excluded from both narration extraction and the DOM segment index, but footnote paragraphs themselves stay readable in their normal location. This keeps footnote markers from becoming tiny chunks or destabilizing chunk boundary highlights.

This extraction correction does not justify a global cache-version bump. Normal block-structured books retain their existing chunks and WAV reuse. Only sources that previously lost wrapper-owned text receive a different source signature and require regeneration. Existing audio files remain readable, but the old generation is incomplete by definition.

Future PDF adapters should produce the same segment shape instead of adding format-specific rules to the TTS hooks. EPUB currently reuses the generated reading HTML from the upload pipeline so existing DOM-span highlighting remains valid; a richer EPUB viewer can later map chunks to EPUB-specific locations. Chunking keeps headings separate from paragraph merges so playback highlights do not disappear or span awkwardly across visual section changes. See `docs/epub-implementation-plan.md` for EPUB implementation notes and remaining reader-quality work.

The viewer highlighter requires the CSS Custom Highlight API provided by the supported desktop, Android, and iOS WebViews. Chunking retains runtime-only source spans that identify each chunk's readable segment indexes and normalized offsets without changing chunk text or ids. The viewer builds a one-pass index of the rendered reader root only when active playback highlighting needs it, using idle time when possible and an immediate fallback if playback starts first; it does not concatenate the full document or allocate per-character node/offset arrays. Only the active chunk's boundary segments are scanned to create a DOM `Range`, and an LRU cache retains up to 128 visited chunk ranges. A second lightweight highlight marks the approximate current word inside the active chunk by segmenting the chunk text and inferring word position from chunk playback progress; saved manifests do not contain true word timestamps. Users can turn the word marker off from the playback menu without disabling the broader chunk highlight. A small playing-only lead compensates for coarse audio progress events so the visual marker does not feel consistently behind the spoken word. Word ranges are built once per active chunk and cached with the same bounded strategy, so long audiobooks do not rescan the whole reader as playback advances. One registered `Highlight` object owns the active chunk range, another owns the word range, and shutdown clears both before removing the registry entries. Search-result jumps use a separate named CSS Highlight entry, so opening a result can mark the likely match without clearing the audiobook playback highlight. Smooth scrolling waits briefly for rapid navigation to settle so repeated skip taps do not start competing scroll animations.

Playback navigation is latest-intent-wins on both backends. Backward, Forward, automatic advance, and chapter-list jumps update one target. Desktop keeps separate pending and committed indexes around its foreground chunk loader. Mobile keeps one serialized native seek worker, one queued target, and one active pending target that is cleared when the seek commits or exits. Repeated taps replace the queued target, so obsolete intermediate seeks do not commit audio or highlighting. Desktop and mobile pending indexes remain separate so a completed mobile jump cannot become the base for a later Forward or Backward action.

Desktop audio loading remains bounded: one foreground chunk load may run alongside one sequential speculative lookahead worker, and only a small Blob URL window is retained. Mobile performs no per-tap file read, base64 decode, Blob creation, or source replacement: Media3/AVPlayer streams one local track and each chunk jump is a global seek. The timing array is `O(n)` small metadata, and active-chunk lookup uses binary search (`O(log n)`). Native command responses update React immediately; ordinary visible progress uses one non-overlapping 250 ms poll, avoiding the plugin's high-frequency event stream. The document reader is memoized, the chapter menu virtualizes its rows, and highlight alignment is bounded to the current chunk plus its approximate current-word marker. This keeps steady-state work bounded for hundreds or thousands of chunks.

The earlier segmented-save change set the native audiobook cache version to `native-save-v4-segmented`; records and exported bundles from cache versions before that are incompatible and must be re-saved/re-exported. That is still the current version — the synthesis-text normalization described below does not change chunk boundaries or bump it.

After optional preprocessing, native synthesis sanitizes the synthesis copy before calling sherpa-onnx: smart punctuation is normalized, zero-width/control characters are removed, whitespace is collapsed, and emoji/non-BMP symbols are dropped. English Kokoro models then receive additional English-only normalization (gated by `english_text_normalization()`): an uppercase roman numeral following a capitalized section keyword (`Chapter IV` -> `Chapter 4`); common abbreviations and units expanded to spoken words so a trailing period is not read as a sentence stop (`Dr.` -> `Doctor`, `e.g.` -> `for example`, `St.` -> `Saint`/`Street` by context, dotted initialisms like `U.S.A.` -> `USA`, and units like `mph` -> `miles per hour`), while a sentence-final `etc.` keeps its full stop; currency amounts voiced via num2words with the unit after the amount (`$5` -> `five dollars`, `$5.50` -> `five dollars and fifty cents`, `$1984` -> `one thousand nine hundred and eighty-four dollars`), keeping the digits and voicing the unit after a following magnitude word (`$5 million` -> `5 million dollars`); clock times expanded to spoken words (`9:00` -> `nine o'clock`, `10:45` -> `ten forty-five`) while ratios with a single-digit right side (`2:1`, `16:9`) are left alone; a decimal fraction the chunker split and rejoined with a stray space (`3. 14` -> `3.14`); standalone four-digit years in the 1000-2099 range (`1984` -> `nineteen eighty-four`); (year, cardinal, and currency words come from the `num2words` crate) and pause punctuation softened to commas so Kokoro produces a medium pause where a marker is otherwise dropped or rushed -- clause-level semicolons/colons (ratios like `2:1` are preserved), space-flanked em/en dashes, brackets around a parenthetical aside, and ellipses. Arabic BMP characters and punctuation are preserved for Piper tokenization, and non-English models receive none of the English rewrites. These synthesis-only changes do not rewrite source chunks, DOM spans, search text, or bundle metadata, so the cache version is unchanged and existing saved audiobooks stay valid (only newly saved chunks reflect the new pronunciation).

Read playback is saved-only: the viewer Play button appears when the current document has a complete saved audiobook for the selected voice and stored generated speed. When the document has saves made with other Audio Setup choices, a saved-audiobooks menu lists every completed version and explains when none matches the current setup; selecting one restores its model, voice, generated speed, preprocessing, and SILMA quality before the existing cache check enables Play. Its compact technical metadata remains English and LTR so mixed model, voice, and runtime labels stay stable in every UI locale. The Save button remains available for creating another version with the current setup. New saves generate audio at `1x`; older or imported audiobooks may still carry their original generated-speed metadata for cache identity. Playback reads native saved-audiobook files and does not synthesize missing chunks live. Desktop uses the bounded React chunk window. Mobile hands one cached local track to the official native plugin and delegates playback, notification controls, lock-screen controls, audio focus, and wake behavior to the platform player. Native global time is converted back to the same `currentChunkIndex`, chunk-local time, and progress fields used by highlighting and the existing controls. The playback-rate selector in the active player changes media playback speed only; it does not regenerate audio, change cache keys, or make saved audiobooks unavailable.

While the WebView is hidden or the screen is locked, the native player and media session remain the playback source of truth and continue without React. Papercut stops foreground polling while hidden. On return it reads the current native state once, updates chunk refs and highlighting, and restarts one generation-fenced, non-overlapping poll. Mobile controls wait for that foreground synchronization before acting.

These playback and highlighting rules do not change narration text, chunk ids, cache keys, or chunk WAV filenames, so they do not require an audiobook cache-version bump. Highlight source spans are runtime DOM alignment data, but new native manifests and bundle exports preserve them as optional chunk metadata. For normal documents they are still rebuilt from source HTML whenever the document opens; for newly imported bundles they can be used directly when present. Existing saved audio and imported bundles do not need regeneration. Imported bundles without stored spans keep the short-term compatibility fallback: if a grafted span is missing, disconnected, or resolves to text that does not match the current chunk, the reader may build one cached text map for the live DOM and recover a span by matching the chunk text against that map. The fallback is exact-first, then Arabic-aware for common Unicode differences such as tashkeel, tatweel, bidi controls, and Arabic/Persian letter variants. Playback remains available while imported highlight spans prepare; the audio controls show a lightweight "Preparing highlights..." notice during the lazy rebuild. Stored spans and fallback matching keep old and new bundles usable, but they are not a substitute for chapter/page-aware locators because repeated text and very large books can still make rediscovery or full-DOM rendering more expensive or less certain. Internal native manifests use one exact current schema; older local manifest schemas are intentionally treated as incompatible and can be replaced by saving the audiobook again. This does not change bundle compatibility because every import writes a fresh current manifest from the bundle's canonical source and chunk WAVs. A desktop-generated `.papercut-audiobook` remains portable because mobile import restores its source HTML, chunk WAVs, and bundled single track when present. Bundles contain no device path or plugin-specific state; if a track is absent, mobile derives it from the restored chunks.

### Native Mobile Playback Constraints

- First mobile play is `O(n)` in saved audio bytes when no valid cached or imported track exists: Rust streams chunk payloads into an atomic `playback.wav`. Later plays reuse it. Imported `.papercut-audiobook` bundles normally restore their included single track and only need lightweight metadata validation.
- Runtime playback does not load the whole audiobook into JavaScript memory. The native player streams the local track while React retains chunk text, timing boundaries, and normal UI state.
- New bundle exports include optional chunk source spans, so re-imported audiobooks can often avoid the live-reader text fallback. When fallback is still needed, it builds a cached text map from the live reader only when playback highlighting needs it and the ordinary span path is unusable. Exact matching stays first; the compatibility matcher also keeps a tolerant Arabic index that removes visual marks and folds common Arabic/Persian variants while mapping matches back to real DOM offsets. That is `O(rendered reader text)` once per stable reader DOM, not once per chunk. Very large fully-rendered books can still cause a noticeable first-highlight pause; the long-term fix is chapter/page-level rendering with format-aware locators.
- The cached track approximately duplicates chunk audio on disk and uses the standard RIFF/WAV 4 GB size field. The stitcher streams data rather than loading the full book into memory; compressed native playback can be evaluated separately if books approach that format limit.
- Official plugin 1.0.5 does not expose a non-destructive Stop or clear-source command. Papercut implements runtime Stop as pause plus seek-to-zero. On Android, the paused system media card may remain available until Media3 retires the inactive service or the app tears down the session.
- Papercut pins the official Rust crate and JavaScript package to 1.0.5. App-owned Rust prepares the track and chunk boundaries; app-owned TypeScript handles command serialization, latest-target seek coalescing, global-time mapping, and the 250 ms visible polling cadence. No plugin source is vendored or patched.
- The frontend path recognizes iOS and the plugin uses AVPlayer/Now Playing controls. The same single-track contract applies on Android and iOS; the generated Apple target enables Background Modes -> Audio. iOS native TTS uses sherpa's static XCFramework archive and should be validated on TestFlight real devices for model download, generation, saved playback, background controls, and highlighting.

## Audio UI

The document header exposes one consolidated audio control surface:

- Play starts reading the current document.
- Pause and Resume control the active desktop audio element or native mobile media session.
- Stop cancels playback, clears temporary desktop object URLs, and pauses/resets the reusable native mobile session. Full native disposal is reserved for app teardown.
- Backward and Forward jump by narration chunk.
- The burger/list button opens a mobile-friendly chunk list for long audiobooks. Rows show an estimated start timestamp when total saved duration is known and jump directly to that chunk.
- Playback speed in the active player changes how fast the saved WAV plays. It is a listener preference and is persisted separately from generated-audio settings.
- Save generates and persists the full audiobook for the current supported document. HTML and EPUB both save from the sanitized HTML source loaded by the reader.
- Saved appears when the full audiobook exists locally for the selected voice and stored generated speed.
- Model selects the engine/language. Download voice model installs the selected pinned model into app data.
- Text processing selects a model-supported synthesis preprocessor. Piper offers automatic Arabic diacritization or original text; Kokoro exposes original text only.
- New saves generate audio at `1x`. Use the player speed control for ordinary listening-speed changes.
- Threads controls the native ONNX Runtime thread count for benchmarking save throughput on the current device. Options extend to the logical CPU parallelism detected by Rust, each app session starts with the conservative native platform default (1 on Android, up to 4 on desktop), and the UI reports the backend-confirmed count applied to the active or most recent save. Selecting more than 4 threads shows a warning because extra parallelism can increase memory use, heat, battery drain, and thermal throttling without guaranteeing faster synthesis.

The home screen Audiobooks panel shows active or resumable saves with progress bars, completed saved audiobooks, export/delete actions, saved duration, saved percent, stored size, and the model, voice, generated speed, and preprocessing metadata for each item. Completed saved audiobooks are shown regardless of the currently selected voice; opening one switches the app to the model, voice, stored generated speed, and preprocessing choice stored on that record before viewing the document. The Library's **Saved Audio** filter shows every document with at least one completed saved audiobook, regardless of the current Audio Setup, and does not filter Search results. Playback rate is not part of saved-audio identity.

## Multilingual Quality Validation

Non-English Kokoro entries remain marked `Experimental` until native speakers
validate complete audiobook saves. For each language, test at least one
multi-page prose sample with names, dates, numbers, quotations, and paragraph
boundaries; listen for skipped words, wrong phonemes, unstable pacing, and
sentence-boundary artifacts. Exercise every available voice and at least one
desktop and one mobile build before removing the experimental label. Passing
model load or generating a WAV is not sufficient evidence of comprehensible,
high-quality narration.

## Diagnostics

The in-app TTS diagnostics panel is the primary way to monitor desktop and mobile builds without devtools. Enable it from Audiobooks -> Audio Setup -> Advanced -> Diagnostics, or with the existing debug flag during development. When enabled, the panel appears below the Audiobooks panel and model source/release details are shown inside Audio Setup; the normal Installed status remains visible even when diagnostics are off.

The panel stores only the latest bounded set of events in localStorage, supports category and severity filters, and can copy the filtered events as JSON for bug reports. Capability events are summarized before logging so the panel shows useful fields such as model count and model ids instead of dumping large nested model objects. Nested diagnostic values are still preserved in bounded form when they are useful for debugging.

The native path emits:

- `tts-model-install-progress`
- `[tts-native] capabilities`
- `[tts-save] native chunk start`
- `[tts-save] native chunk`
- `[tts-save] native performance summary`
- `[tts-save] completed`
- `[tts-save] failed`
- `[tts-playback] native preparation completed`
- `[tts-playback] native source loaded`
- `[tts-highlight] DOM segment index built`
- `[tts-highlight] slow chunk range built`
- `[tts-highlight] chunk range unavailable`

Useful fields are `backend`, `modelDir`, `totalChunks`, `cachedChunks`, `generatedChunks`, `chunkNumber`, `chunkId`, `textPreview`, source and synthesis previews, `generateMs`, `preprocessMs`, `synthesisMs`, `writeMs`, `validateMs`, `indexingMs`, `audioDurationSec`, `realTimeFactor`, `wavBytes`, `totalSourceChars`, `totalSynthesisChars`, `threadCount`, `appliedThreadCount`, `dir`, `segments`, `elapsedMs`, and `reason`.

Interpretation guide:

- High first-chunk time usually means native model load plus first inference warmup.
- High `realTimeFactor` after warmup means synthesis is the bottleneck.
- Save-summary `realTimeFactor` uses generated-run `audioDurationSec`; `totalAudioDurationSec` includes cached chunks and is for whole-audiobook progress, not generation-speed benchmarking.
- Compare `synthesisMs` and `preprocessMs` before blaming model speed; Arabic diacritization can add preprocessing time while Kokoro/Piper inference shows up under synthesis time.
- High `writeMs`, `validateMs`, or `indexingMs` points to disk or manifest-index work rather than model inference, especially on very large saved books.
- If a higher `threadCount` improves `realTimeFactor`, keep it for that device; if it crashes, heats up, or throttles during long saves, return to 1 thread on Android.
- If Resume crashes at the same point, inspect the last `[tts-save] native chunk start` entry. The `chunkNumber`, `chunkId`, and `textPreview` identify the text range being passed to native synthesis when the process died.
- Repeated cache misses usually mean the document, chunk text, voice, generated speed, model id, text preprocessor, or audiobook cache version changed.
- A high `[tts-highlight] DOM segment index built` time means reader DOM traversal is still expensive; normal work scales with DOM nodes, not audiobook characters.
- `[tts-highlight] slow chunk range built` usually identifies one unusually large readable block. `[tts-highlight] chunk range unavailable` means runtime source spans and reader structure disagree and includes a reason field.

## Version-Control Practice

Large runtime assets stay out of git. Commit code, scripts, docs, Cargo files, package metadata, and `src-tauri/tts/model-manifest.json`; do not commit downloaded model files, generated Android libraries, built frontend output, or generated audiobook WAV chunks.

The current native audiobook cache version is `native-save-v4-segmented`. Updating that value intentionally invalidates older saved-audiobook records and incomplete downloads because their chunk boundaries or text normalization may no longer match. The app hides old records but does not automatically delete their files from user data.

Generated files that should stay uncommitted:

```
src-tauri/tts/runtime/
dist/
```

Do not commit generated audiobook WAV chunks. Full Save writes runtime user data under the app data audiobook cache.

## Why The Browser Worker Was Removed

The browser Kokoro worker was useful for proving the feature, but it hit the wrong performance ceiling:

- Android WebView worker execution was slow for full audiobook generation.
- WebGPU availability and performance varied across Chromium, Linux desktop WebViews, and Android WebView.
- Cross-origin isolation improved WASM threading but did not solve the main inference cost.
- Keeping q8/q4/WebGPU/WASM switches made the UI and cache model more complex without producing LocalReader-Pro-class throughput.

Native sherpa-onnx is a better base because inference runs outside the WebView sandbox and can use native ONNX Runtime threading/provider support.

## Further Improvements

The next valuable work is native execution depth, not more browser fallback tuning:

- Add a native Android foreground service for long-running audiobook saves so generation can continue while the app is backgrounded.
- Add deeper cancellation support inside sherpa generation itself so Cancel can interrupt a long current chunk, not only stop before the next chunk.
- Return saved WAV chunks as raw Tauri IPC bytes from an async command instead of base64-encoding them in Rust and decoding/copying them in JavaScript.
- Add resumable/range-aware model downloads if interrupted downloads become common on mobile networks.
- Evaluate compressed export audio if users need a portable single track beyond the standard 4 GB RIFF/WAV limit.
- Extend saved-audiobook support to PDFs with page-aware manifests and text-layer positions.
- Explore native GPU/NNAPI/CoreML/DirectML provider support only after the CPU native path is measured on target devices.

## Sources

- sherpa-onnx Android build docs: https://k2-fsa.github.io/sherpa/onnx/android/build-sherpa-onnx.html
- sherpa-onnx Rust crate docs: https://docs.rs/sherpa-onnx
- Tauri command IPC: https://v2.tauri.app/develop/calling-rust/
- native audio plugin: https://github.com/uvarov-frontend/tauri-plugin-native-audio
