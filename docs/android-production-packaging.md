# Android production packaging and F-Droid plan

Investigation: 2026-11-03. Branch: `task/android-production-packaging`, based on
`main` at `a4f9a1ef`. Status: report and plan only; implementation has not started.

## 1. Report before implementation

Papercut looks suitable for an F-Droid submission, but **signing alone will not
make the current build ready**. Production APK packaging is the smaller task;
native-library and WebAssembly provenance are the larger feasibility gates.
Acceptance remains F-Droid's decision.

| Area | Verified current state | Work needed |
| --- | --- | --- |
| Android releases | `release.yml` calls `android:apk:native-tts`; its default arguments include `--debug`. The workflow copies the debug output into GitHub Releases. | Build and verify a release APK; persist a production signing identity for GitHub downloads. |
| Identity/version | Application ID is `io.papercut.desktop`; app version is `1.10.0`. Gradle obtains Android versions from generated `tauri.properties`. | Keep the ID unless there is a specific reason to change it; commit an authoritative Android version code that F-Droid can read. |
| Native inference | SHA-256-pinned Sherpa 1.13.4 Android archive supplies `.so` files. Four ABI directories are staged, while the default Rust app target is arm64. | Establish an acceptable source-build path and make APK contents match the supported ABI set. |
| PDF/OCR | `prepare-pdf-assets.js` copies PDF.js and Tesseract WASM from npm packages, plus English/Arabic recognition data. | Inventory these compiled assets and their source recipes; obtain reviewer agreement on their treatment or build them from source. |
| Models | Voice and embedding downloads are optional runtime operations. A separate 4,788,213-byte Libtashkeel model is embedded in preprocessing builds. | Audit each model/data license and provenance separately from engine code. |
| Scanner | CameraX and platform graphics; documentation explicitly rejects ML Kit's Play Services requirement. | Retain this design and verify the resolved release dependencies. |

Evidence: [release workflow](../.github/workflows/release.yml),
[Android builder](../scripts/build-android.js),
[native staging](../scripts/lib/android/sherpa.js),
[Android constants](../scripts/lib/android/constants.js),
[Gradle app](../src-tauri/gen/android/app/build.gradle.kts),
[PDF assets](../scripts/prepare-pdf-assets.js), and
[bundled model provenance](../src-tauri/tts/libtashkeel-1.5.0/README.md).

### F-Droid compliance requirements

The official repository requires FLOSS application code and compatible dependency
licenses, public source, a free build toolchain, a unique application ID, a working
maintained app, and disclosed anti-features. Dependencies must be source-built or
come from permitted binary sources; a checksum does not make an arbitrary GitHub
native archive acceptable. Proprietary SDKs cannot be excused by an anti-feature
label. Executable downloads require explicit informed opt-in. Asset redistribution
rights also matter. [Inclusion policy](https://f-droid.org/en/docs/Inclusion_Policy/)

Review model downloads and any network/update behavior with packagers; do not
predeclare all ONNX graphs to be either harmless data or prohibited executables.
Optional functionality does not by itself settle licensing or labeling. Non-free
assets/add-ons/services and tracking have distinct labels; there is no confirmed
Papercut labeling verdict yet. [Anti-features](https://f-droid.org/en/docs/Anti-Features/)

MIT licensing of Papercut's own code is a good starting point, not a complete APK
license audit. Trace native transitive components, including phonemization,
models, fonts, PDF codecs and OCR data; preserve notices and check any copyleft
source obligations before choosing the listing's overall license expression.
[Developer licensing guidance](https://f-droid.org/en/docs/FAQ_-_App_Developers/)

### What is needed from the owner

1. **Signing decision:** prioritize interchangeable GitHub/F-Droid updates, or
   accept separate signing identities to simplify initial F-Droid publication.
   Choose who holds the production keystore and its recovery backup. Put secrets
   in GitHub's protected release environment, never in chat or source control.
2. **Existing-user decision:** identify deployed APK versions/certificates and
   accept a tested migration plan from debug installs. A new production key will
   normally require reinstalling those installs; uninstalling can delete local
   documents, bookmarks and audiobooks. Verify backup/restore coverage first.
3. **Initial support:** confirm arm64/API 26+ as the first production target, or
   request additional architectures and their device coverage. Full native TTS,
   Arabic preprocessing, conceptual search and OCR remain the proposed scope.
4. **Listing/maintenance:** provide or approve descriptions, screenshots, support
   contact, privacy/download explanation and any donation links. A GitLab account
   is useful for maintaining the submission; no Play Console account or Apple
   signing certificate is needed for F-Droid.
5. **Acceptance testing:** an Android device, preferably also a de-Googled device,
   and time for install/update, OCR, TTS, search and background-playback checks.

## 2. Why the packaging was done this way

| Evidence | Historical reason and implication |
| --- | --- |
| [PR #2: Create Android Build](https://github.com/muhannadnouri/papercut.io/pull/2) | Explicitly introduced debug-signed APKs for sideloading. The present release path inherited a development distribution mechanism. |
| [PR #5: Papercut 1.0](https://github.com/muhannadnouri/papercut.io/pull/5), commit [`9201947f`](https://github.com/muhannadnouri/papercut.io/commit/9201947f55ee3c259e657aa2e73a672c256d6d47) | Added checked upstream Sherpa downloads and `jniLibs` staging alongside consolidated build helpers. Rust needs an ABI-specific link directory; Gradle separately needs the runtime libraries to include in the APK. |
| Commit [`e532670a`](https://github.com/muhannadnouri/papercut.io/commit/e532670a58590a04828a4d5c02849be381ab7bd1) | Aligned Rust, desktop, Android and iOS on Sherpa 1.13.4 and refreshed mobile checksums. The pin is a cross-platform compatibility choice. |
| Commit [`8085cf5c`](https://github.com/muhannadnouri/papercut.io/commit/8085cf5c4e2d5951f127fcad9ba11a18d47b20ea), [PR #79](https://github.com/muhannadnouri/papercut.io/pull/79) | Shared ONNX Runtime through the official Libtashkeel interface and later added embeddings through the same runtime. This avoided maintaining a Libtashkeel fork or loading competing ORT copies. |
| [TTS architecture](kokoro-tts.md), [scanner decision log](pdf-ocr-scanning.md) | Native TTS replaced a browser inference path that was too slow on Android. Offline model downloads keep large voice assets out of installers. CameraX was chosen to support offline/de-Googled Android. |

MemSearch searches and expanded records from `2026-10-07.md` corroborate the
no-fork adapter, unchanged embedded model and one-runtime tests. Records from
`2026-11-02.md` explain later cache fixes, not the original architecture.
Memory is supporting evidence; the commits and current code establish what shipped.

**Interpretation:** this was pragmatic native packaging for an offline reader,
not a recorded decision against F-Droid. No evidence was found that the existing
archive path was designed or approved for official F-Droid inclusion. Preserve the
shared runtime and change how its libraries are produced.

An important upstream trap: Sherpa's own
[1.13.4 Android build script](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/build-android-arm64-v8a.sh)
downloads prebuilt ONNX Runtime by default. Merely compiling Sherpa is insufficient.
It accepts external ORT headers/libraries, so a source-built ORT can be supplied
without redesigning Papercut's inference layer.
[ORT Android source-build instructions](https://onnxruntime.ai/docs/build/android.html)

## 3. Recommended path and signing choice

**First make GitHub Android releases production-ready; then prove F-Droid
buildability while retaining the current features.** Keep one Android packaging
path where possible. Do not start by deleting TTS/OCR, forking dependencies,
upgrading Sherpa, adding a second runtime, or operating a custom repository.

| Signing route | Benefit | Cost |
| --- | --- | --- |
| F-Droid builds and signs | No upstream private key needed by F-Droid; byte-identical upstream reproduction is not required for this route. | Its APK and a Papercut-signed GitHub APK cannot normally update one another. Choose this split deliberately before launch. |
| Papercut signs; F-Droid independently reproduces | Same application ID and signing certificate can support updates across channels. | F-Droid must reproduce the upstream APK; native code, WASM, toolchains and archive ordering make this additional work. A mismatch blocks publication. |

Aim for the second route, but run a bounded reproducibility check before
committing to its release cadence. If it fails, present the measured remaining
work and choose the first route or delay launch. Do not promise a seamless later
key switch. F-Droid receives the signed APK/public certificate, **not the private
key**, for upstream-signed verification.
[Reproducible builds and signing modes](https://f-droid.org/en/docs/Reproducible_Builds/)

A separately hosted F-Droid-compatible repository is an optional distribution
alternative, not an official listing. It adds hosting/index-signing maintenance
and is unnecessary unless earlier F-Droid-client distribution is a priority.
[Developer FAQ](https://f-droid.org/en/docs/FAQ_-_App_Developers/)

## 4. Implementation sequence on this branch

Each unchecked step is future work. No key generation, build changes or submission
is included in this documentation commit.

1. [ ] **Record identity and migration.** Inspect actual published APK certificates
   and version codes; confirm ID, architecture, key custody and backup/restore
   coverage. Use a distinct debug ID for future development installs if useful.
   **Pass:** no existing-user upgrade/data-loss assumptions remain untested.

2. [ ] **Produce a real release APK.** Update `scripts/build-android.js`,
   `package.json`, tracked Android Gradle configuration and the Android portion of
   `release.yml`. Preserve explicit `native-tts-shared,experimental-search-embeddings`
   features: custom builder arguments currently replace defaults. Add conditional
   signing for trusted release jobs, with unsigned release output available to
   F-Droid. Keep signing secrets outside ordinary PR CI. Update artifact paths and
   README's debug-install instructions; ignore keystores and local signing
   properties in `.gitignore`. Keep CI's debug artifacts separate from production
   release artifacts. Follow
   [Tauri signing](https://v2.tauri.app/distribute/sign/android/).
   **Pass:** APK signature/certificate verified with `apksigner`, expected ID and
   versions, `debuggable=false`, and release install/smoke succeeds.

3. [ ] **Make versioning inspectable.** Commit `bundle.android.versionCode` in
   `tauri.android.conf.json` and include it in release version checks. Keep
   `versionName`, npm/Cargo/Tauri versions and tags consistent. Tauri's current
   default formula gives `1.10.0` code `1010000`; inspect deployed APKs before
   selecting the first explicit code. Avoid per-build counters. Configure
   F-Droid to read committed config rather than ignored `tauri.properties`.
   **Pass:** two clean builds of one tag report the same code; the next release's
   code is greater. [Tauri Android config](https://v2.tauri.app/reference/config/#androidconfig)

4. [ ] **Prove dependency provenance before building a full matrix.** Inventory
   Android `.so`, WASM and model/data inputs plus npm/Cargo/Gradle transitive
   dependencies. First source-build compatible ORT and Sherpa for arm64 using
   pinned commits/free toolchains; pass external ORT headers/libs upstream, then
   reuse `SHERPA_ONNX_LIB_DIR`, `ORT_LIB_LOCATION` and existing JNI staging.
   Resolve OCR/PDF WASM source recipes or an explicitly accepted packaging route.
   Respect the provided JDK; the existing helper already accepts `JAVA_HOME`.
   Trace transitive downloads and licenses, including phonemization dependencies.
   **Pass:** clean source-build logs explain every shipped compiled asset; no
   hidden prebuilt fallback. Full feature scope works with one ORT per ABI.

5. [ ] **Validate APK contents and reproducibility.** Stage only the intended
   ABIs; inspect native `DT_NEEDED` dependencies before removing copied C++/JNI
   libraries. Check ELF and ZIP alignment, including 16 KB page compatibility,
   as production compatibility work. Build twice in matching clean Linux
   environments; compare APKs/signature-copy verification if using the upstream
   signature route. Add only focused checks for feature preservation, artifact
   verification and source-only behavior. **Pass:** packaged English/Arabic TTS,
   preprocessing, semantic search, PDF/OCR and background playback succeed; an
   update preserves documents, bookmarks and saved audio. Include de-Googled
   Android and release minification in acceptance.

6. [ ] **Prepare and submit F-Droid metadata.** Add manually maintained
   `fastlane/metadata/android/en-US/` descriptions, images and version-code-named
   changelogs; Fastlane itself need not be installed.
   [Listing format](https://f-droid.org/en/docs/All_About_Descriptions_Graphics_and_Screenshots/)
   Create `metadata/io.papercut.desktop.yml` in an `fdroiddata` fork with pinned
   toolchains/source revisions, build/output commands, update checks and agreed
   anti-feature disclosures. The recipe belongs there; only shared build helpers
   and listing assets belong here. Run lint, scanner and a clean F-Droid build;
   submit a merge request and address review. Do not broadly ignore binaries to
   bypass provenance checks. **Pass:** recipe builds in F-Droid's environment and
   reviewers accept it. [Submission guide](https://f-droid.org/en/docs/Submitting_to_F-Droid_Quick_Start_Guide/),
   [build metadata reference](https://f-droid.org/en/docs/Build_Metadata_Reference/)

7. [ ] **Rehearse an update before announcing availability.** Publish the next
   higher version through the chosen signing route and test updating an installed
   previous production release. **Pass:** certificate continuity and user-data
   preservation are verified, not inferred from a successful compile.

## 5. Handling every later release

1. Bump app versions and the committed Android code; add
   `fastlane/metadata/android/en-US/changelogs/<versionCode>.txt`. Commit everything
   before creating the stable `vX.Y.Z` tag; do not move published tags.
2. Run release validation and publish the verified, consistently named APK to
   GitHub. Keep using the same production key. For upstream-signed F-Droid builds,
   make that exact APK available at the recipe's versioned binary URL.
3. Configure F-Droid tag checking and automatic build metadata updates, with
   custom `UpdateCheckData` for the committed Tauri version/code if needed.
   Recipe/toolchain changes still need maintenance. This is F-Droid pulling and
   rebuilding releases, not GitHub Actions uploading APKs to the official store.
4. Monitor F-Droid update/build/publication status. Investigate source-build,
   scanner or reproducibility failures and submit recipe changes when necessary.
   Publication is asynchronous; don't promise simultaneous GitHub/F-Droid updates.
5. Users receive update availability through their F-Droid client, subject to its
   settings and Android installation permissions. No new in-app updater is needed.

Sources: [F-Droid version/update FAQ](https://f-droid.org/en/docs/FAQ_-_App_Developers/),
[update metadata](https://f-droid.org/en/docs/Build_Metadata_Reference/), and
[Android signing/upgrade rules](https://developer.android.com/studio/publish/app-signing).

## Investigation limits

Reviewed current code, docs, relevant commits/PR descriptions, MemSearch records
and official requirements. No exhaustive resolved-dependency/license audit,
source rebuild, APK certificate inspection, reproducibility comparison or device
test was performed. Signing is bounded work; native/WASM source builds are the
main engineering uncertainty, and external review has no promised completion date.
