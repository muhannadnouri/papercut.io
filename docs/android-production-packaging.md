# Android production packaging and F-Droid

Updated 2026-11-03 on task/android-production-packaging. Production packaging is
implemented and locally checked; the F-Droid publication gates remain below.
No production release or F-Droid submission has been made.

## Agreed scope

- Shared Papercut signature across GitHub and F-Droid; prove reproducibility first.
- Retain io.papercut.desktop, arm64/API 26+, TTS, Arabic preprocessing, conceptual
  search, OCR, scanning and the existing reading/library features.
- The owner creates and backs up the production key. Existing debug installations
  are test-only and may be reset/reinstalled.
- Device acceptance uses Google Pixels, including Pixel 6.
- Draft English/Arabic listings with GitHub Issues support. Texts and the existing
  icon are under fastlane/metadata/android/; screenshots await device testing.
- GPL notices/source inventory are prepared for review. Papercut's root MIT license
  declaration has not been changed or approved for replacement.
- Repository-owned packaging scripts and tests use JavaScript.

## 1. What changed, and what remains

| Area | This pass | Remaining gate |
| --- | --- | --- |
| Production command | npm run android:apk:release keeps the shipping TTS/search features and omits debug mode; real unsigned APK checked | Pixel acceptance |
| Signing | Build/check unsigned APK first; sign with SDK apksigner on a fresh runner in the protected android-release environment | Owner creates key and GitHub secrets; validate the actual production certificate |
| Versioning | Committed Android code 1010000 matches the inspected 1.10.0 debug test APK | Choose the first new production version/tag with the owner; test increasing-code updates |
| Native staging | Source-provided SHERPA_ONNX_LIB_DIR is honored without a prebuilt fallback; unused C++/JNI wrappers removed; source-built ORT/Sherpa pair also packaged and checked | Device inference and complete source-build integration |
| APK checks | JS verifier checks identity/version, non-debuggable manifest, certificate, arm64 contents, required libraries, license presence and 16 KB ELF/ZIP alignment; disposable-key signing passes | Production certificate and clean-build reproduction |
| F-Droid | Listing drafts, policy investigation and source/license evidence | Complete dependency closure, tested fdroiddata recipe, review and publication |

Ordinary CI retains debug APKs and additionally checks an unsigned production APK.
The release workflow requires the signed production APK and expected certificate;
its download filename remains Papercut_vX.Y.Z_android-arm64-v8a.apk.

## 2. Owner: create and configure the signing key

Run once with a JDK installed. Keep the keystore outside the repository, make an
encrypted recovery backup, and retain passwords in your password manager. This is
the application signing key for direct APK/F-Droid distribution.

~~~bash
mkdir -p ~/.local/share/papercut-signing
chmod 700 ~/.local/share/papercut-signing
keytool -genkeypair -v -storetype JKS \
  -keystore ~/.local/share/papercut-signing/papercut-release.jks \
  -alias papercut -keyalg RSA -keysize 4096 -validity 10000
keytool -list -v \
  -keystore ~/.local/share/papercut-signing/papercut-release.jks -alias papercut
umask 077
base64 -w 0 ~/.local/share/papercut-signing/papercut-release.jks \
  > ~/.local/share/papercut-signing/papercut-release.jks.base64
~~~

Choose the certificate details when prompted. Record the store and key passwords;
both GitHub entries below are required even if the passwords are identical.
Record the public certificate's SHA-256 fingerprint without colons.

In GitHub repository **Settings → Environments**, create **android-release**.
Restrict it to trusted workflow refs/release tags and enable required reviewers
where available. Manual dispatch runs the selected workflow ref while checking
out the requested release tag: approve both the workflow and tag contents.

| Environment secret | Value |
| --- | --- |
| ANDROID_KEYSTORE_BASE64 | Contents of papercut-release.jks.base64 |
| ANDROID_KEY_ALIAS | papercut |
| ANDROID_KEYSTORE_PASSWORD | Keystore password |
| ANDROID_KEY_PASSWORD | Key password |

Add the environment **variable** ANDROID_SIGNING_CERT_SHA256 with the public
certificate's 64 hexadecimal SHA-256 digits. Do not put the private key/passwords
in chat, source control or F-Droid metadata. Keep using this key for later updates.

The build job uploads an unsigned input artifact. A separate protected signing
job uses a fresh runner with no npm install, compilation or restored build cache.
It verifies the input before creating the temporary keystore, supplies passwords
to apksigner through environment variables, removes the key even on failure,
then verifies the public certificate. Only the verified signed artifact matches
the publisher's artifact selection; the publisher waits for signing to succeed.
Prepare the key now, but do not publish until the license, reproducibility and
device acceptance gates below are complete.
[Android signing](https://developer.android.com/studio/publish/app-signing),
[Tauri signing](https://v2.tauri.app/distribute/sign/android/)

## 3. Build, verify and update

The existing npm run android:apk:native-tts remains a development/debug build.

~~~bash
npm run android:apk:release
node scripts/verify-android-apk.js --unsigned \
  src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
~~~

The verifier uses Node, Cargo, unzip, Android SDK Build Tools 35.0.0 and
ANDROID_HOME or ANDROID_SDK_ROOT. It adds no npm dependency. Unsigned APKs are
validation inputs, not installable release downloads.

For local signing, set ANDROID_KEYSTORE_PATH to the absolute keystore path,
ANDROID_KEY_ALIAS, ANDROID_KEYSTORE_PASSWORD and ANDROID_KEY_PASSWORD. Read
passwords interactively rather than entering them in shell history. Set
ANDROID_SIGNING_CERT_SHA256 to verify the result:

~~~bash
"$ANDROID_SDK_ROOT/build-tools/35.0.0/apksigner" sign \
  --ks "$ANDROID_KEYSTORE_PATH" --ks-key-alias "$ANDROID_KEY_ALIAS" \
  --ks-pass env:ANDROID_KEYSTORE_PASSWORD --key-pass env:ANDROID_KEY_PASSWORD \
  --out src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk \
  src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
node scripts/verify-android-apk.js \
  src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
~~~

Do not modify/realign an APK after signing. The unsigned build must pass alignment
checks first. [apksigner](https://developer.android.com/tools/apksigner)

For every public update:

1. Synchronize npm/package lock, Cargo and Tauri versions; increase the committed
   bundle.android.versionCode in tauri.android.conf.json. Avoid per-build counters.
2. Add fastlane/metadata/android/en-US/changelogs/<versionCode>.txt and the Arabic
   counterpart. Commit before creating the stable vX.Y.Z tag; never move it.
3. Publish the verified APK using the same key and stable asset filename.
4. F-Droid's recipe detects stable tags, rebuilds source and compares against that
   exact signed APK. A mismatch blocks publication for the shared-signature route.
   Configure UpdateCheckData for committed Tauri version/code rather than ignored
   generated tauri.properties; maintain the recipe when toolchains change.
5. Monitor F-Droid build/publication results and test the installed update.
   Publication is asynchronous; no new in-app updater is needed.

F-Droid's own signing is an alternative, but has not been selected. Independently
signed APKs cannot normally update one another. F-Droid needs the upstream signed
APK/public certificate for reproduction, not the private key.
[Reproducible builds](https://f-droid.org/en/docs/Reproducible_Builds/),
[update metadata](https://f-droid.org/en/docs/Build_Metadata_Reference/)

## 4. Why the existing packaging looked this way

[PR #2](https://github.com/muhannadnouri/papercut.io/pull/2) deliberately introduced
debug-signed sideloading.
[PR #5](https://github.com/muhannadnouri/papercut.io/pull/5) and commit
[9201947f](https://github.com/muhannadnouri/papercut.io/commit/9201947f55ee3c259e657aa2e73a672c256d6d47)
added checked Sherpa downloads/JNI staging: Rust needs an ABI-specific link
directory, while Gradle separately needs runtime libraries to package.

Commit [e532670a](https://github.com/muhannadnouri/papercut.io/commit/e532670a58590a04828a4d5c02849be381ab7bd1)
aligned platforms on Sherpa 1.13.4. Commit
[8085cf5c](https://github.com/muhannadnouri/papercut.io/commit/8085cf5c4e2d5951f127fcad9ba11a18d47b20ea)
and [PR #79](https://github.com/muhannadnouri/papercut.io/pull/79) established the
shared ORT path without a Libtashkeel fork. MemSearch records from 2026-10-07
corroborate those decisions. Native TTS replaced slow browser inference; large
voice models remained optional downloads. CameraX was chosen to support
offline/de-Googled Android, avoiding Play Services.

There is no evidence that the archive path was designed or approved for F-Droid.
The proposed change preserves the shared runtime and changes build provenance.
[TTS details](kokoro-tts.md), [scanner decisions](pdf-ocr-scanning.md)

## 5. Source and license evidence

F-Droid requires FLOSS code, compatible dependency licenses, public source, a free
toolchain, a unique ID, a maintained working app and appropriate disclosures.
Binary dependencies need acceptable source-build or permitted repository
provenance; checksums alone do not approve arbitrary GitHub binaries. Proprietary
SDKs cannot be excused by anti-feature labels. Executable downloads require
informed opt-in. Assets also need redistribution rights.
[Inclusion policy](https://f-droid.org/en/docs/Inclusion_Policy/)

| Source | Pinned revision/evidence |
| --- | --- |
| Sherpa 1.13.4 | 142807252687d81b40d6315f23470a1512a00de3 |
| ORT 1.27.0 | 8f0278c77bf44b0cc83c098c6c722b92a36ac4b5 |
| Tesseract.js core 7.0.0 | acffef2b66eb44a31df297e11d905f4b39001068 and pinned submodules |
| PDF.js 6.2.108 | 0365cbde028bd92e58f2dab1bb70cd30ac7acfd7 |
| Piper phonemization | f3ff95afc03640bc1399e113e83361192a2fafb4, MIT |
| eSpeak NG | ed530aa113046142eb5115cf2fc9157854d0ffe1, GPLv3 with later-version grants in source headers |

**Native probe:** ORT and Sherpa's C API compiled for arm64/API 26 with NDK 29.0.13846066,
TTS enabled, JNI/executables/tests/PortAudio/WebSockets/diarization disabled.
Both pass 16 KB ELF alignment. Sherpa was relinked against the source-built ORT;
its only non-system dependency is libonnxruntime.so, which needs only Android
system libraries. This is a source-built native pair, not yet an inference result
or proof that every executable dependency in the APK is built from source.

Sherpa's own Android script downloads prebuilt ORT by default. Supply external
ORT headers/libraries instead. Current ORT probe: Release/shared, full CPU operator
set, API 26, two workers, no tests/proprietary provider. Local commands, fetched
sources and logs are in ignored .cache/android-production/.
The stripped pair is in source-arm64/ beneath that cache. Its receipt.json records
the local output hashes; these are probe results, not approved reproducibility
reference hashes. Staging with SHERPA_ONNX_LIB_DIR pointing there and repackaging
produces an APK that passes verification; both packaged native libraries exactly
match the source-built inputs. That probe reused the compiled app library and
current WASM assets. The ordinary release path still uses the existing
checksum-pinned archive pending device inference and complete source integration.
[Sherpa Android script](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/build-android-arm64-v8a.sh),
[ORT source builds](https://onnxruntime.ai/docs/build/android.html)

**WASM:** Tesseract's upstream build uses Emscripten 4.0.15. The probe uses an
isolated local installation because Docker's daemon is unavailable. CMake 4
requires CMAKE_POLICY_VERSION_MINIMUM=3.5; Sherpa also uses
CMAKE_POLICY_DEFAULT_CMP0169=OLD. No upstream source fork was needed. Rebuild and
replace paired LSTM/SIMD/relaxed-SIMD WASM loaders together, then test English/Arabic
OCR. Source outputs have not silently replaced the app's npm assets.
All three shipping LSTM cores (plain, SIMD, relaxed SIMD) initialize the installed
English/Arabic data and correctly recognize upstream's English small-image OCR
fixture and a synthetic Arabic line rendered with the repository's Noto Naskh
font. That smoke test does not establish general recognition quality or Android
browser compatibility. The extra legacy variant was stopped after the three
shipping outputs completed.

PDF.js has separate OpenJPEG/QCMS/JBIG2/QuickJS builds. QCMS history identifies
wrapper revision 2ae4ee72334782928210ba4050e3e77d9b2d35be, but its Dockerfile uses
rust:latest, unversioned helpers and cargo update. Pin those inputs and trace the
other wrappers; ordinary PDF.js bundling only copies their existing binaries.
No PDF codec rebuild/rendering acceptance is established.

**GPL review:** Sherpa's TTS core statically incorporates Piper phonemization and
eSpeak NG. Shared linking of Papercut to Sherpa does not remove that incorporated
GPL code. Upstream pins the eSpeak/Piper archives and their hashes in
[espeak-ng-for-piper.cmake](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/cmake/espeak-ng-for-piper.cmake)
and [piper-phonemize.cmake](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/cmake/piper-phonemize.cmake).
This also merits review for existing distributions using the same engine.

Proposed notice, **for owner review before shipping**:

> This Android application incorporates eSpeak NG and Unicode tools through
> Sherpa ONNX/Piper phonemization. These components carry GPLv3 terms, with
> later-version permission where granted. The combined distribution must satisfy
> applicable GPL terms. Papercut's original source retains MIT notices; other
> dependencies retain their notices. Matching corresponding source and build
> instructions will accompany each release.

The source package/notices must actually exist before publication. Preserve exact
Papercut/native/transitive revisions, build scripts, locks, patches and relevant
copyright/license texts; review Eigen/MPL, OpenFst/Kaldi, KissFFT and other linked
code. Do not infer “MIT only” from the root license or approve an overall F-Droid
license expression before that review.

| Model/data | Evidence and remaining issue |
| --- | --- |
| Supertonic 3 | The exact pinned archive's README distinguishes MIT sample code from OpenRAIL-M weights, despite an MIT LICENSE file. The [model license](https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE) has use restrictions; agree F-Droid disclosure/anti-feature treatment. Feature retained |
| Kokoro | [Model card](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/README.md) declares Apache-2.0; converted artifacts/voice/data notices still need closure |
| Piper Kareem | [Model card](https://huggingface.co/rhasspy/piper-voices/blob/main/ar/ar_JO/kareem/medium/MODEL_CARD) points dataset licensing to AliMokhammad/arabicttstrain, which could not be retrieved |
| Libtashkeel | Embedded 4,788,213-byte unchanged model with checksum/MIT provenance in [its record](../src-tauri/tts/libtashkeel-1.5.0/README.md) |
| E5 embeddings | Pinned Small model card declares MIT; audit every offered variant/tokenizer/converted asset |
| OCR data/fonts | Check underlying trained-data terms separately from npm wrapper licenses; retain existing OFL/Apache font notices |

Model downloads being optional does not settle licensing or F-Droid labeling.
[Anti-features](https://f-droid.org/en/docs/Anti-Features/)

## 6. Security review for this pass

- Production signing uses a separate protected job on a fresh runner. Ordinary
  CI and build processes never share its runner or signing environment. No npm
  install, compilation or build cache runs in the signing job. The key is created
  after input verification, stored with restrictive permissions and removed
  before signed-output verification, including on signing failure. This closes
  the background-process exposure possible with build/sign on the same runner.
- Android build checkout explicitly names refs/tags/; signing checks out the
  build's resolved commit to avoid a moved tag changing its verification code.
  GitHub Environment
  reviewers/ref restrictions remain owner configuration; repository code cannot
  make an untrusted workflow safe merely by adding signature checks.
- Verifier subprocesses use argument arrays without a shell. Duplicate APK
  entries, wrong ABI, malformed ELF headers, unexpected certificate and debug
  builds fail. Native ZIP entry names exclude paths/wildcards before unzip.
- Android archive SHA-256 checking is retained; an invalid external library
  directory fails without downloading an unrequested fallback. Complete source
  provenance and reproducibility still need independent validation.
- Actions in the Android signing job are pinned to their resolved immutable
  commits, with matching pins for shared actions in CI. Other references retain
  their existing versions.
- No application network permissions, telemetry, user-data formats or npm/Rust
  dependencies are added.

Artifact checks verify expected packaging/identity, not the absence of malicious
code. The fresh npm audit reports no high/critical findings and 14 moderate package
reports tracing to a minimatch denial-of-service advisory in the lint dependency
chain. That is an existing development-tool issue; this pass adds no dependency
or lockfile change. No local Cargo advisory rerun is claimed.
[GitHub Actions secure use](https://docs.github.com/en/actions/reference/security/secure-use)

## 7. Remaining acceptance sequence

1. [ ] Owner configures the real key and validates its certificate; unsigned APK
   checks and disposable-key signing verification already pass. Choose the first
   production version/tag before publication.
2. [ ] Complete native/WASM source builds, transitive license/source inventory and
   owner review of proposed GPL compliance. Retain the current feature scope.
3. [ ] Build matching clean Linux environments and verify upstream APK
   reproduction/signature copying. A repeated packaging run with reused native
   objects alone does not prove a complete reproducible build.
4. [ ] On Pixel 6 and another Pixel: install after resetting the debug test app;
   test import/open-with, bookmarks, English/Arabic OCR, scanner, voice
   download/preview, Arabic preprocessing, conceptual search, audiobook generation
   and background/lock-screen playback.
5. [ ] Install a higher-code production candidate with the same key; verify
   documents/bookmarks/audio survive. Include de-Googled Android and a 16 KB
   environment, plus an API 26 emulator when available; Pixel 6 alone does not
   establish those results.
6. [ ] Add screenshots/changelogs; prepare metadata/io.papercut.desktop.yml in an
   fdroiddata fork, with pinned build inputs, output path, binary URL, expected
   signing key and update checks. Run lint/scanner/clean build, then submit and
   address review. Do not hide runtime binaries with broad scanner ignores.
7. [ ] Rehearse the next update through F-Droid before announcing availability.

Validation: 14 shipping/staging regression checks and four APK verifier self-tests
pass; targeted ESLint, whitespace, workflow YAML, document links and metadata
length checks pass. A full production APK build succeeds. Its cleaned APK passes
unsigned verification, post-build signing and certificate verification with a
disposable key; wrong-certificate and signed-as-unsigned checks reject as expected.
The test key/password file was deleted. The source-built native pair packages
correctly, and all three source-built OCR cores pass English/Arabic smoke tests.
Two packaging runs with the normal cleaned libraries produce byte-identical APKs;
they reuse compiled objects and do not prove complete clean-build reproduction.
Production-key validation, complete reproducibility and device results remain
explicit gates.

[Submission guide](https://f-droid.org/en/docs/Submitting_to_F-Droid_Quick_Start_Guide/),
[listing format](https://f-droid.org/en/docs/All_About_Descriptions_Graphics_and_Screenshots/)
