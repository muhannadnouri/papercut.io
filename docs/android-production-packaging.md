# Android production packaging and F-Droid

Updated 2026-11-04 on task/android-production-packaging. Production packaging is
implemented and locally checked; the F-Droid publication gates remain below.
No production release or F-Droid submission has been made.

For command discovery, start with the [script command map](../scripts/README.md).
Android entry scripts are in scripts/android/; optional source-build investigation
is in scripts/fdroid/. Tests sit in adjacent __tests__/ folders. Existing npm
command names, build flags, CI selection and isolated signing are unchanged.
Source-build caches are excluded from ordinary Vitest test discovery. The
owner-approved Maven ORT route, source inventory and reproducibility work are
documented below.

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
| Native staging | Source-provided SHERPA_ONNX_LIB_DIR is honored without a prebuilt fallback and SHERPA_REQUIRE_SOURCE=1 refuses the archive; unused C++/JNI wrappers removed; source-built Sherpa linked against the pinned Maven ORT AAR packaged and checked | Device inference and complete source-build integration |
| APK checks | JS verifier checks identity/version, non-debuggable manifest, certificate, arm64 contents, required libraries, license presence and 16 KB ELF/ZIP alignment; disposable-key signing passes | Production certificate and clean-build reproduction |
| Source recipes | Pinned JS recipes build the Sherpa library against the pinned Maven ORT AAR, three OCR cores and four PDF codecs with a matching PDF.js worker; local compatibility checks pass | Integrate approved source outputs, freeze the complete environment and prove clean APK reproduction |
| F-Droid | Licensing decisions recorded; notices shipped in APK; corresponding-source archive tooling and fdroiddata metadata draft in repo | Owner attaches source archive per release; fdroiddata lint/scanner/clean build; submit and address review |

Ordinary Android CI builds and verifies one unsigned production APK, runs the
transfer plugin's JVM tests, then uploads that APK. It does not assemble a debug APK.
The release workflow requires the signed production APK and expected certificate;
its download filename remains Papercut_vX.Y.Z_android-arm64-v8a.apk.

### CI scope and timing

The [mobile-scope baseline run](https://github.com/muhannadnouri/papercut.io/actions/runs/41739896570/job/126282579237)
at 5126597f passed shared checks, Android and iOS. Android took 20m17s: debug APK
7m26s, unsigned release APK 10m06s, plugin tests 39s and release verification 2s.
Rust's cache missed; optional emulator inference was skipped. These results do
not establish installed-app acceptance or a release-only timing improvement.

Routine CI keeps the production feature set and APK checks. The existing
testDebugUnitTest task compiles/tests the library-transfer plugin on the JVM;
its name does not imply that the app's debug APK must be built. Run it after
the release build, which generates the ignored tauri.settings.gradle and plugin
paths that Gradle needs on a fresh checkout. Local development keeps
npm run android:apk:native-tts for a debug APK. Provider inference remains an
explicit workflow_dispatch option with its existing model-artifact dependency.

Local validation: the standalone unit-test task graph contains no app/Rust build
or debug APK assembly tasks; all nine tests executed and passed with no skips.
The generated project metadata was present for that local test. Workflow checks
confirm one APK build/upload and preserve the remaining jobs, filters and optional
inference steps.

Release-only CI is now verified in two runs at 3d7692a8. Both Android jobs passed
release verification and the executed plugin unit-test task, and each run uploaded
exactly one artifact, papercut-android-release-unsigned-apk. Emulator inference
remained skipped. Durations below are approximate, measured from job log timestamps.

| Android job | Total | APK build step(s) | Plugin tests | Rust cache |
| --- | --- | --- | --- | --- |
| Baseline above, debug + release | 20m17s | 7m26s + 10m06s | 39s | Miss |
| [Release-only run 1](https://github.com/muhannadnouri/papercut.io/actions/runs/41750102273/job/126315238787) | 12m24s | 9m45s | 42s | Exact hit |
| [Release-only run 2](https://github.com/muhannadnouri/papercut.io/actions/runs/41752729236/job/126324396071) | 7m45s | 5m30s | 29s | Exact hit |

Both new runs restored the same approximately 1.25 GiB Rust cache with key
v0-rust-android-Linux-x64-ade52a28-81f06068 and reported it up to date afterward.
They also hit the same npm cache. This is not a cold/warm comparison: the second
run's improvement cannot be attributed to a cache newly saved by the first.
Other steps varied too (npm ci: 37s versus 18s); the logs do not establish the
cause. The observed job reductions of about 39% and 62% combine the workflow
change, cache state and run variability, rather than isolating one cause or
establishing a guaranteed duration. No release-only cold-cache run was forced.

Retain the single release build and current cache policy. Shared checks took
about 6m07s and 7m18s respectively before Android could start. Assess parallelizing
packaging with shared checks as a separate proposal: it trades faster feedback
for extra work on failing changes and must preserve required checks and the
optional inference-model handoff. Signing remains isolated in the release workflow.

### Deferred: parallel mobile CI

Shelved at the owner's request while F-Droid preparation resumes. The design
below is retained for later review; implementation is not approved. The owner's
agreement to require CI result applies only if that future check is implemented,
and is not a prerequisite for continuing F-Droid preparation.

Start Android/iOS packaging alongside checks after change detection. Keep the
desktop matrix and SILMA packaging behind checks for this first stage. The
measured mobile checks barrier was 6-7 minutes; overlapping it can reduce waiting,
but runner queues and resource variation prevent a guaranteed time saving.
Packaging will consume runner time even when checks subsequently fail.

| Job | Proposed prerequisites | Purpose |
| --- | --- | --- |
| checks | changes | Existing tests/audits and optional verified model upload |
| build-android, build-ios | changes | Existing selected mobile packaging checks |
| New Android/iOS inference jobs | changes, checks, matching platform build | Only explicit mobile/full provider_inference runs |
| New CI result job | changes, checks, all packaging/inference jobs | Validate expected outcomes, including skipped jobs |

Implementation sequence after approval:

1. Move the existing mobile inference steps into two optional platform jobs.
   Retain the same-run verified model artifact and existing native test harness.
   Prepare native libraries through the existing Android/iOS preparation commands
   on each fresh runner; compile the provider tests without rebuilding an APK or
   iOS app there. These are native-provider tests, not installed-app smoke tests.
2. Remove the checks dependency from the two mobile packaging jobs. Keep their
   job IDs, path/scope filters, production features and APK verification. Desktop
   and SILMA keep their current sequencing.
3. Share dependency caches between each platform's packaging/inference jobs.
   Android retains shared-key: android. Use shared-key: ios in both iOS jobs;
   replacing the current job-derived key creates a one-time cold iOS namespace.
   Keep failure caching off and leave protected release signing untouched.
4. Add an always-evaluated CI result job and a small JS checker/self-test. Change
   detection must succeed; every requested job must succeed. Reject failures,
   cancellation and unexpected skips. Accept skips only for work that was not
   selected, including documentation-only PRs and disabled inference. Existing
   checks remain visible; the result job also covers the desktop matrix/SILMA
   whenever their scopes select them.
5. Review main's required-check configuration separately. The branch endpoint
   currently reports protected: false with checks enforcement off, and the
   ruleset collection is empty. Detailed protection reads returned 403, so these
   findings need owner confirmation. The owner approved requiring CI result before merging;
   a workflow status alone does not configure merge enforcement. No repository
   setting change is authorized by this proposal.
6. Validate documentation-only, cheap, mobile and full scopes; both inference
   settings; and checker cases for failure, cancellation and unexpected skips.
   Then have the owner push and run mobile CI with inference off and on. Confirm
   parallel starts, same-run model availability, complete result reporting and
   one unsigned Android APK. Once CI result has appeared, the owner can select
   it as a required check before merging, as agreed. Compare
   timing and runner use before expanding to
   desktop/SILMA parallelism.

Proposed scope: ci.yml, one JS result checker with self-tests, and this guide.
No implementation or settings change has been made for this proposal.
[Job prerequisites](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs),
[required status checks](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)

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
node scripts/android/verify-android-apk.js --unsigned \
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
node scripts/android/verify-android-apk.js \
  src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
~~~

Do not modify/realign an APK after signing. The unsigned build must pass alignment
checks first. [apksigner](https://developer.android.com/tools/apksigner)

For every public update:

The planned next public release is **v1.10.1**, after this branch is tested and
merged. Current APK candidates still identify as 1.10.0; the final version bump,
changelogs and release-specific evidence belong to the later release stage.

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
| ORT 1.27.0 | Maven Central AAR SHA-256 077dec5e2d821234c7dc0aba584bec8f999854b546c754cab93a90741c56fbeb; source-probe commit 8f0278c77bf44b0cc83c098c6c722b92a36ac4b5 retained in history |
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
The stripped source-built Sherpa library is in source-arm64/ beneath that cache;
the recipe now links it against the Maven AAR's ORT inputs (see the provisioning
paragraph below). Its source-receipt.json records
the local output hashes; these are probe results, not approved reproducibility
reference hashes. Staging with SHERPA_ONNX_LIB_DIR pointing there and repackaging
produces an APK that passes verification; in the earlier probe both packaged
native libraries exactly matched the source-built inputs. The current output
provides the same link inputs to the Rust build, with the packaged ORT always
coming from the AAR.
That probe reused the compiled app library and
current WASM assets. The ordinary release path still uses the checksum-pinned
Sherpa archive for libsherpa-onnx-c-api.so and the Maven artifact for ORT,
pending device inference and an F-Droid-style build through this source path.
[Sherpa Android script](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/build-android-arm64-v8a.sh),
[ORT source builds](https://onnxruntime.ai/docs/build/android.html)

**Production ORT provisioning (approved 2026-11-03):** Gradle supplies
libonnxruntime.so from the pinned Apache-2.0 Maven artifact
`com.microsoft.onnxruntime:onnxruntime-android:1.27.0`; F-Droid permits
appropriate FLOSS artifacts from trusted Maven repositories, and this app
already ships 100+ resolved Maven dependencies. Verified against that exact
artifact: arm64 16 KB ELF alignment, embedded version string 1.27.0 identical
to the archive-bundled ORT, and libsherpa-onnx-c-api.so's only non-system
dependency being libonnxruntime.so. Sherpa staging now copies only
libsherpa-onnx-c-api.so and deletes stale libonnxruntime.so copies that would
collide with the AAR at packaging time; the AAR's unused libonnxruntime4j_jni.so
Java-binding glue is excluded. Release builds keep the arm64-only ABI filter;
debug/emulator builds merge all four AAR ABIs. The source recipe now links this
same AAR for Sherpa's build-time ORT, and staging honors
SHERPA_REQUIRE_SOURCE=1 to refuse the prebuilt archive in source-only builds.
The earlier full ORT source-build recipe remains in git history as the
investigation/fallback path.

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
browser compatibility. The committed recipe builds only these three shipping
LSTM variants, omitting upstream's unused legacy variants.

### Repeatable source recipes (approved local investigation)

Run from the repository root on Linux x86-64. These are investigation recipes;
the ordinary app/release asset paths are unchanged. All generated files stay in
ignored .cache/android-production/. The scripts reject a wrong source revision,
tracked edits and untracked files outside upstream ignore rules. Existing source
checkouts are never reset. Named *-work directories are disposable and replaced
on each OCR/PDF run. Do not store personal work there.

Prerequisites: npm ci for the application; Node 24, Git,
OpenJDK 17, CMake (tested 4.4.5), make, patch, autoconf, automake, libtool,
pkg-config, a host C/C++ compiler and Android SDK/NDK 29.0.13846066.
Use two build workers as encoded in the recipes. Install Emscripten 4.0.15 in
an isolated directory and set EMSDK to its absolute path. The local probe used
emsdk installer commit c0bb220cb6e6f4e0fabb6f6db9efd53390ef5e56:

~~~bash
# A new directory; skip this bootstrap if an isolated, activated 4.0.15 SDK exists.
git init .cache/android-production/emsdk
git -C .cache/android-production/emsdk remote add origin https://github.com/emscripten-core/emsdk.git
git -C .cache/android-production/emsdk fetch --depth=1 origin c0bb220cb6e6f4e0fabb6f6db9efd53390ef5e56
git -C .cache/android-production/emsdk checkout --detach FETCH_HEAD
.cache/android-production/emsdk/emsdk install 4.0.15
.cache/android-production/emsdk/emsdk activate 4.0.15
export EMSDK="$PWD/.cache/android-production/emsdk"

# Named toolchain only; do not change the machine's default Rust toolchain.
rustup toolchain install 1.98.1 --profile minimal --target wasm32-unknown-unknown
cargo +1.98.1 install wasm-bindgen-cli --version 0.2.100 --locked \
  --root "$PWD/.cache/android-production/wasm-tools" \
  --target-dir "$PWD/.cache/android-production/wasm-tools-target"

node scripts/fdroid/build-android-source-libs.js
node scripts/fdroid/build-source-ocr.js
node scripts/fdroid/build-source-pdf-codecs.js

# Recheck already-built outputs without recompiling.
node --test scripts/fdroid/__tests__/check-source-build-selftest.js
node scripts/fdroid/__tests__/check-source-ocr.js
node scripts/fdroid/__tests__/check-source-pdf.js
~~~

The native recipe downloads the pinned Maven ORT AAR, re-verifying its
SHA-256 even on cached copies, extracts the C/C++ headers and arm64 runtime,
builds Sherpa's C API against them, strips the Sherpa library and checks ELF
alignment, and copies the AAR runtime beside it as the Rust link input. It no
longer compiles ORT. Source archive
hashes remain governed by the pinned upstream CMake/dependency files. The OCR
recipe uses upstream's pinned image libraries and compiler flags, with only a
two-worker limit and selection of the three LSTM targets. It does not run the
upstream dependency installer against the host OS. Generated loaders and WASM
must move together.

PDF.js's normal build copies already-built codecs. Its separate wrapper
Dockerfiles use floating Emscripten/Rust tags; QCMS additionally runs cargo update.
Our recipe pins Emscripten, Rust, QCMS's existing Cargo.lock and wasm-bindgen CLI,
applies the pinned upstream patches, and keeps compiler caches inside the project.
It builds the JS fallbacks too. It then rebuilds PDF.js's API, worker and sandbox
with the matching glue using its own npm lock and npm ci --ignore-scripts.
The PDF checkout includes enough history for Gulp to generate version 6.2.108;
a one-commit checkout silently reports 6.2.0 and is rejected. No application npm
or Cargo dependencies were upgraded.

| PDF component | Wrapper pin | Underlying input |
| --- | --- | --- |
| OpenJPEG | 8bb19dc1aeecb104ce2ed3493d41fe131fc681e0 | OpenJPEG 6c4a29b00211eb0430fa0e5e890f1ce5c80f409f plus upstream buffer-stream patch |
| QCMS | 2ae4ee72334782928210ba4050e3e77d9b2d35be | qcms 0.3.0 and wasm-bindgen 0.2.100 from wrapper Cargo.lock |
| JBIG2/CCITT | 0b32db6d0eaf94844d46e38828ad4e0b7e36cb55 | PDFium 0455e822ded1a5537d826703988e986a33d2d4a1 |
| QuickJS | b62f7cd527363ca2c1fe7467f274bc9acbf78c24 | QuickJS 3d5e064e9dd67c70f7962836505a7fa067bf0a4e plus upstream sandbox patch |

The JBIG2 wrapper includes build-only fixes made after PDF.js's binary update;
decoder sources are unchanged. This is a tested replacement candidate, not a
claim that the original binary was produced from this exact wrapper/toolchain.
Other pins were traced through the PDF.js binary-update history and wrapper
Dockerfiles. Full pins, source URLs and license evidence are in the inventory.

Outputs: source-arm64/ holds the stripped source-built Sherpa library plus the
AAR's arm64 runtime; sherpa-onnx-sys resolves both Rust link inputs from this
one SHERPA_ONNX_LIB_DIR directory. source-ocr/ holds six
LSTM loader/WASM files; source-pdf/ holds four codec pairs and fallbacks;
pdf-worker-work/build/generic/ holds the matching PDF.js bundles and resources.
Each output directory has source-receipt.json with source pins and output hashes.
Those hashes describe a local build, not an independently reproduced reference.

Validation in this stage:

- Native recipe rebuilt Sherpa against the pinned AAR's headers and runtime —
  the changed headers recompiled the core, so this is not a bare relink, but it
  reuses the existing checkout and toolchain and is not a second clean native
  build. The stripped library passes arm64/16 KB ELF checks. An F-Droid-style
  build with SHERPA_REQUIRE_SOURCE=1 and SHERPA_ONNX_LIB_DIR produced an
  unsigned APK that passes verification; its packaged ORT bytes match the
  pinned Maven artifact and its Sherpa bytes match the source-built output.
  The host environment remains non-hermetic; clean-room reproduction stays a
  later gate.
- OCR recipe rebuilds its disposable work tree from source. All three variants
  recognize the English and Arabic fixtures correctly. The Arabic fixture is
  our synthetic line, “مرحبا بكم في عالم القراءة”, rendered with the repository's
  OFL Noto Naskh font; no user document is involved. Upstream LSTM link steps emit
  warnings for exported legacy/orientation/SSE symbols. The exercised LSTM APIs
  pass; this does not validate those unused APIs or Android worker loading.
- All four PDF codecs and matching worker build. JPEG2000, JBIG2 and CMYK fixtures
  from the pinned PDF.js checkout render pixel-identically to the current npm
  package using the same Node rasterizer. Tests assert the new JPEG2000/JBIG2
  WASM decoders actually load, compare QCMS ICC color conversions, and initialize
  QuickJS with an arithmetic assertion. These are focused compatibility checks,
  not a full PDF corpus or Android sandbox test. Node-only browser API polyfills
  come from PDF.js's existing locked core-js dependency.

Remaining technical boundary: stage the rebuilt PDF API/worker/glue as a set;
replacing only WASM under the npm worker can break its import ABI. Integration
requires a separate reviewed change to asset preparation and native build inputs.
The local emsdk bootstrap uses upstream prebuilt FLOSS tools; this is not an
accepted F-Droid toolchain declaration. Select F-Droid-supported/source-built
tools and freeze host packages/environment in the clean-build stage. Ignored
cache products, upstream downloads and host versions are not hermetic yet.

### Dependency and notice inventory for review

[android-source-inventory.json](android-source-inventory.json) keeps the
hand-gathered review evidence standard tools cannot regenerate: source/license
hashes for the native/WASM projects, Sherpa's fetched subprojects, OCR submodule
pins, model archives and font/data evidence, plus the reviewed lockfile hashes.
Its former bulk Rust/npm/Maven package listings were replaced on 2026-11-03 by
the standardRefresh commands stored in the file: cargo metadata for the resolved
Android graph, npm sbom for CycloneDX npm output, and gradle dependencies for
Maven candidates from universalReleaseRuntimeClasspath. Regenerated listings are
declarations and review candidates, not a claim that every package/file is
shipped or licensed correctly. The two local Rust plugins inherit the
repository's original-code MIT scope but do not declare a license in their own
Cargo manifests.

Notice bundle contents, as shipped in every APK since this stage
(owner-approved 2026-11-03). `assets/licenses/android-third-party-notices.md`
carries the combined-work statement, verbatim license texts from pinned
sources, the Kareem model card and per-package attribution; regenerate with
`npm run prepare:license-notices` (the committed output is what builds package;
`--check` verifies it is current):

| Group | Retain with each release |
| --- | --- |
| Papercut/eSpeak/Piper | Original MIT notices, eSpeak/ucd GPLv3 texts and source-header grants, Unicode data notices, Piper MIT/uni-algo notices; exact corresponding source, patches and build instructions for the combined application |
| Sherpa/ORT | Apache/MIT texts and ORT ThirdPartyNotices plus cmake/deps.txt; Eigen MPL2 and per-file BSD/Apache/MINPACK notices, OpenFst/Kaldi Apache notices, KissFFT BSD, JSON MIT, sentencepiece notices and exact fetched archives |
| Rust/JS/Maven | Per-package copyright/license texts from pinned packages, including MPL-covered source files, Unicode/CDLA data terms and Apache notices; use the resolved inventory to audit final artifact contents |
| OCR | Tesseract Apache, Leptonica BSD-style, IJG JPEG, giflib MIT, libpng/libtiff/libwebp, openlibm's MIT/BSD/ISC/FDLIBM/public-domain and LGPL test notices, and zlib texts; English/Arabic trained-data Apache source license separately from MIT npm wrapper metadata |
| PDF | PDF.js/JBIG2 wrapper Apache, OpenJPEG BSD, PDFium BSD-3-Clause, QuickJS MIT and QCMS MIT source notices; retain existing published notices pending reconciliation, not a blanket Apache label |
| Fonts/data/models | Existing OFL/Apache font notices, PDF standard-font Foxit/Liberation notices and Libtashkeel MIT; model/voice/dictionary notices separately from inference-engine code |

PDFium's actual source LICENSE is BSD-3-Clause even though the JBIG2 wrapper
README links to it as Apache. QCMS's pinned source is MIT while the published
PDF.js notice is BSD-style; retain both evidence sets for review when staging
rebuilt assets. CameraX 1.5.3 camera-core's official Google Maven AAR supplies
libimage_processing_util_jni.so and libsurface_util_jni.so, with Apache and libyuv
BSD notices. F-Droid permits appropriate FLOSS artifacts from trusted Maven
repositories; that does not waive license/notice review. Four Maven candidates
inherit license declarations through parent POMs rather than direct license
elements; the inventory records that distinction instead of guessing.

The inventory is not the corresponding-source archive or an APK-visible license
screen. Assemble and verify both source and notice delivery before shipping;
choosing a release-wide GPL expression remains an owner review decision.

**GPL review:** Sherpa's TTS core statically incorporates Piper phonemization and
eSpeak NG. Shared linking of Papercut to Sherpa does not remove that incorporated
GPL code. Upstream pins the eSpeak/Piper archives and their hashes in
[espeak-ng-for-piper.cmake](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/cmake/espeak-ng-for-piper.cmake)
and [piper-phonemize.cmake](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.4/cmake/piper-phonemize.cmake).
This also merits review for existing distributions using the same engine.

Decided 2026-11-03: the F-Droid metadata declares **GPL-3.0-only** for the
combined Android build; the repository root LICENSE.md stays MIT. The shipped
combined notice is [android-third-party-notices.md](../src-tauri/licensing/android-third-party-notices.md),
generated from pinned sources by `npm run prepare:license-notices`; its
combined-work statement reads:

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
| Supertonic 3 | The exact pinned archive's README distinguishes MIT sample code from OpenRAIL-M weights, despite an MIT LICENSE file. The [model license](https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE) has use restrictions. Kept 2026-11-03 as an optional download with the F-Droid NonFreeAssets anti-feature declared; the MIT-code/OpenRAIL-M-weights distinction stays in notices |
| Kokoro | Exact manifest-pinned archive contains an Apache-2.0 LICENSE. Kept 2026-11-03; its bundled eSpeak data/dictionaries and CppJieba dictionaries are in scope for the notice/source bundle mapping |
| Piper Kareem | Exact manifest-pinned archive's MODEL_CARD points dataset licensing to AliMokhammad/arabicttstrain, which could not be retrieved, and records fine-tuning from lessac. MIT repository metadata does not settle these rights. Kept 2026-11-03 as an optional download with the status disclosed in the listing, pending an upstream rights statement (owner action: ask AliMokhammad for a license declaration) |
| Libtashkeel | Embedded 4,788,213-byte unchanged model with checksum/MIT provenance in [its record](../src-tauri/tts/libtashkeel-1.5.0/README.md) |
| E5 embeddings | All three pinned [Small](https://huggingface.co/intfloat/multilingual-e5-small/raw/614241f622f53c4eeff9890bdc4f31cfecc418b3/README.md), [Base](https://huggingface.co/intfloat/multilingual-e5-base/raw/d128750597153bb5987e10b1c3493a34e5a4502a/README.md) and [Large](https://huggingface.co/intfloat/multilingual-e5-large/raw/3d7cfbdacd47fdda877c5cd8a79fbcc4f2a574f3/README.md) cards declare MIT. Retain notices for the hash-pinned ONNX/tokenizer assets; no model retraining/quantization reproduction is claimed |
| OCR data/fonts | Both installed English/Arabic data files byte-match [naptha/tessdata at 806cd9a](https://github.com/naptha/tessdata/tree/806cd9adc8c6e8abc11c782db1818c990576bebc), whose source LICENSE is Apache-2.0. Preserve that independently of npm wrapper MIT metadata, plus existing OFL/Apache font and Foxit/Liberation notices |

Model downloads being optional does not settle licensing or F-Droid labeling.
[Anti-features](https://f-droid.org/en/docs/Anti-Features/)

### F-Droid metadata draft

[fdroid/metadata/io.papercut.desktop.yml](fdroid/metadata/io.papercut.desktop.yml)
is a draft for an fdroiddata fork: License GPL-3.0-only, the NonFreeAssets
anti-feature for restricted/unclear model weights, the release binary URL, and
a build recipe that runs the real production pipeline — the Sherpa-only source
recipe, `SHERPA_REQUIRE_SOURCE=1`, then `npm run android:apk:release` — with
the unsigned APK as fdroidserver's `output`. Localized name, summaries,
descriptions and future changelogs come from the committed
`fastlane/metadata/android/` directory (English and Arabic), so the metadata
carries no duplicated listing text.

Owner steps before submission, in an fdroiddata fork:

1. Set `commit` to the first F-Droid release tag and, after signing-key
   validation, `AllowedAPKSigningKeys` to the certificate SHA-256.
2. Verify against the actual buildserver image: NDK `r29` availability (our
   recipe asserts exactly 29.0.13846066), Node/npm and Rust
   aarch64-linux-android target availability, build-tools 35.0.0 for the
   verifier, and that gradle/maven/network fetches work under fdroidserver.
3. Run `fdroid lint`, the scanner and a clean build. Do not hide runtime
   binaries with broad scanner ignores; resolve findings instead.
4. Add screenshots and per-version changelogs under `fastlane/metadata/`.

### Reproducibility protocol

`npm run check:android-reproducibility` is the developer-side gate-5 proof:
two builds of the source-only release path, each after wiping the Rust target
and Gradle build outputs, must produce byte-identical unsigned APKs. It also
checks the packaged Sherpa library against the source recipe's recorded
hash, prints a per-entry CRC report when anything differs, and keeps both APK
copies under `.cache/android-production/reproducibility/`. Expect 30-60
minutes; it is not part of CI.

Reference environment for the comparison (record what you actually use when
it changes): Linux x86-64, Node 24, rustc/cargo 1.98.1 with the committed
lockfiles, NDK 29.0.13846066, OpenJDK 17, CMake 4.4.5, Gradle wrapper as
committed. Baseline profiles are disabled as described below.

The first runs of this check caught real non-determinism, all fixed:

- Tauri serializes the config's CSP *object* form through a `HashMap`, randomizing
  directive order into the embedded config and the Rust library. The CSP is a
  single string in `tauri.conf.json` — same directives, one deterministic order.
  Keep it a string.
- The `bundle.resources` *map* form has the same problem with more than one
  entry. Resources are staged through one directory mapping: `licensing/` is
  packaged as `licenses/`, holding the notices file and a copy of the vendored
  libtashkeel license (canonical file: `src-tauri/tts/libtashkeel-1.5.0/LICENSE`).
- Merged dependency art profiles produced a non-deterministic
  `assets/dexopt/baseline.prof` (upstream sherpa-onnx documents the same). The
  art/baseline profile tasks are disabled in the Gradle config; first-launch
  optimization is the trade-off. Revisit if AGP makes the merge deterministic.
- `tauri-codegen` embedded CSP hashes in filesystem traversal order. The
  owner-approved upgrade to 2.7.4 uses upstream's sorted script/style hash
  lists; the local 2.6.3 vendor patch and its Cargo override are removed.
  Commit `99aa9a23` retains that patch as a fallback if a future rollback is
  needed; do not combine it with the upgrade.

The 2.7.4 upgrade also changes runtime dependencies through `tauri-utils`
2.9.3 → 2.10.2: Brotli 8.0.2 → 9.1.1 (decompressor 5.0.0 → 6.2.0),
`cfb` 0.7.3 → 0.14.0 / `infer` 0.19.0 → 0.22.0, `ctor` 0.8.0 →
1.0.13 and `urlpattern` 0.3.0 → 0.6.0. The graph adds `dom_query` 0.28.0,
`html5ever` 0.39.0, `markup5ever` 0.39.0, `selectors` 0.38.0,
`cssparser` 0.37.0 and `cssparser-macros` 0.7.1 alongside existing versions.
Other changes include `alloc-no-stdlib` 3.0.0, `alloc-stdlib` 0.3.0,
`ico` 0.6.0, `base64` 0.23.1, `json-patch` 4.2.0, `jsonptr` 0.7.1 and
`swift-rs` 1.0.9; the old ctor/dtor proc-macro packages and `unic-*` packages
are removed. `Cargo.lock` records the complete resolution. Regenerated
notices retain the same license groups with updated package attribution.

PR #91 exposed a compatibility gap in that initial update: `tauri` 2.11.5's
test helper omits the new `AppConfig.app_directories_override` field from
`tauri-utils` 2.10.2. Production APK builds and `npm test` do not compile that
helper. The fix aligns both runtime and test dependencies to `tauri` 2.12.5,
`tauri-build`/`tauri-macros` 2.7.4, `tauri-runtime` 2.12.2 and
`tauri-runtime-wry` 2.12.3. The Android CLI also requires the matching minor
of `@tauri-apps/api`, now exactly 2.12.4. The existing CLI remains unchanged.
Rust/Cargo 1.90 is now the declared minimum; CI already uses 1.98.1.

The runtime update also moves Wry 0.55.1 → 0.57.1, Tao 0.35.3 → 0.37.2,
Muda 0.19.3 → 0.20.0, dirs 6 → 7, keyboard-types 0.7 → 0.8.3,
window-vibrancy 0.6.0 → 0.8.1 and the Windows WebView bindings 0.38.2 →
0.39.1. `Cargo.lock` records the build/TOML/Windows support changes too.
Removing the older duplicate `dom_query` 0.27 chain also removes
`cssparser` 0.36.0 and `selectors` 0.36.1; the current Android graph contains
489 package candidates and nine MPL crate trees. Notices and corresponding
source must be refreshed with this resolution. Full platform CI and device
acceptance remain necessary after these runtime changes.

Dependency validation must include the actual Rust suites, independently of
the frontend suite. On Linux, use CI's native-library preparation and loader
path for the shipping-feature suite:

~~~bash
mkdir -p src-tauri/tts/runtime/linux-x64-shared-libs
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
cargo build --manifest-path src-tauri/Cargo.toml --locked --lib \
  --features native-tts-shared,experimental-search-embeddings
node scripts/desktop/copy-sherpa-linux-libs.js
task_shared_dir="$PWD/src-tauri/tts/runtime/linux-x64-shared-libs"
LD_LIBRARY_PATH="$task_shared_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --lib \
  --features native-tts-shared,experimental-search-embeddings
~~~

Post-alignment proof on 2026-11-04 passed in the reference environment above:
both fresh unsigned APKs have SHA-256
`f5730bc3b5765b920719760a40c12e79f321e7bff9728a79ff11412ceee761f9`.
The packaged Sherpa library matches the source receipt; the resulting APK
passes the unsigned production verifier and contains the current notices
byte-for-byte. This remains a one-environment proof.

PR [#91](https://github.com/muhannadnouri/papercut.io/pull/91)'s
[CI run 41923862017](https://github.com/muhannadnouri/papercut.io/actions/runs/41923862017)
passes all nine jobs at `83be8497`, covering the full build/test surface with
optional provider inference skipped. Reset the debug install before Pixel testing:
HTML/EPUB import and styled rendering, PDF, English/Arabic OCR, TTS voice
download/preview, conceptual search and library transfer. These runtime checks
remain required even when local compilation and reproducibility pass.

The release workflow still uses the default prebuilt Sherpa path, while this
proof uses source-built Sherpa. Comparing the owner's downloaded CI APK from
that run with the retained source-only APK found the same 1,723 entry names
and order, with exactly two differing entry contents:

| Entry | CI bytes | Source-only bytes | Finding |
| --- | ---: | ---: | --- |
| `lib/arm64-v8a/libapp_lib.so` | 44,996,208 | 45,000,208 | Embedded Cargo/workspace/standard-library source paths differ; matching compiler version strings alone do not establish identical build inputs |
| `lib/arm64-v8a/libsherpa-onnx-c-api.so` | 4,406,888 | 4,373,144 | Prebuilt archive uses API 21 / NDK r29 build 14206865; source recipe uses API 26 / r29-beta3 build 13846066; embedded C++ source paths also differ |

All other 1,721 entries, including ONNX Runtime, DEX, manifest, notices and
assets, match in both decompressed and compressed bytes. ZIP offsets and native
alignment padding change with the differing library sizes. The CI APK passes
the unsigned production verifier, but the APKs are **not byte-identical**:

- CI SHA-256: `c353a6c16cfac79a10c2843765f7b2f59bd2571d7913375e6eeaafae0edbaa84`
- Source-only SHA-256: `f5730bc3b5765b920719760a40c12e79f321e7bff9728a79ff11412ceee761f9`

The proposed next stage is to use the existing source Sherpa recipe for upstream
Android production builds too, normalize Rust/C/C++ source paths, align actual
toolchain/source inputs and repeat the cross-environment comparison. Cold build
time will increase. Review the workflow change before implementation and preserve
the isolated signing job. Path differences are confirmed; additional native
differences have not yet been excluded. Signing secrets are not needed for this
unsigned comparison; production signing and signature copying remain later gates.

Completing gate 5 needs a production key and three more steps, run by the
owner when that key exists (F-Droid rebuilds and compares the same way):

~~~bash
pip install apksigcopier reproducible-apk-tools
apksigcopier copy released.apk repro-1of2.apk rebuilt-signed.apk
"$ANDROID_SDK_ROOT/build-tools/35.0.0/apksigner" verify --print-certs rebuilt-signed.apk
node scripts/android/verify-android-apk.js rebuilt-signed.apk
cmp released.apk rebuilt-signed.apk
~~~

`copy` takes the signed APK directly. If signatures were extracted into a
directory instead, use `apksigcopier patch sig/ repro-1of2.apk rebuilt-signed.apk`.
See [apksigcopier's command examples](https://github.com/obfusk/apksigcopier#copy-extract--patch).

If comparison tools report differences only in ZIP padding, align with
reproducible-apk-tools' zipalign `--pad-like-apksigner` and 16 KiB page size
before copying. A mismatch blocks publication for the shared-signature route.

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
- Packaging changes add no application network permissions, telemetry or
  user-data formats. The approved Tauri alignment changes npm/Rust dependencies
  as recorded above; it does not enable `appDirectoriesOverride`.
- Source recipes use full Git pins, tracked/untracked source checks and existing
  lockfiles. Subprocess arguments do not invoke a shell; upstream build scripts
  still execute trusted source/build dependencies. PDF npm lifecycle hooks are
  disabled. Disposable work trees and the changed Emscripten compiler-cache
  location stay in the ignored project cache; they never stage files into the
  app automatically or use signing secrets. Build output receipts are evidence
  of what ran, not a security attestation. Final clean environments and dependency
  vulnerability/scanner checks remain release gates.

Artifact checks verify expected packaging/identity, not the absence of malicious
code. The earlier packaging pass's npm audit reported no high/critical findings and 14 moderate package
reports tracing to a minimatch denial-of-service advisory in the lint dependency
chain. That was an existing development-tool issue. The Tauri alignment was
audited separately with CI's cargo-audit 0.22.2 policy: zero blocking findings,
with the same ten nonblocking warnings as the previous lockfile against the
same RustSec snapshot. The existing RUSTSEC-2026-0235 waiver is unchanged;
the all-features/all-targets guard confirms its rkyv 0.7 edge remains inactive.
This is not a warning-free dependency graph or a fresh npm audit claim.
Build-tool environments have separate dependencies.
[GitHub Actions secure use](https://docs.github.com/en/actions/reference/security/secure-use)

## 7. Remaining acceptance sequence

1. [ ] Owner configures the real key and validates its certificate; unsigned APK
   checks and disposable-key signing verification already pass. Choose the first
   production version/tag before publication.
2. [x] Codify native/WASM source recipes and focused local checks; prepare a
   transitive declaration/source inventory and proposed notices for review.
3. [ ] Licensing decisions recorded 2026-11-03: GPL-3.0-only declaration for the
   combined Android build (root LICENSE stays MIT), Kareem kept with disclosure
   pending an upstream rights statement, Supertonic kept with NonFreeAssets.
   The notice bundle is assembled and ships in every APK
   (`assets/licenses/android-third-party-notices.md`, verifier-enforced;
   regenerate with `npm run prepare:license-notices`), and the
   corresponding-source archive tooling is in place: `npm run
   prepare:source-archive` bundles the pinned eSpeak NG/Piper archives,
   the nine MPL-2.0 Rust crate trees, the shipped notices and a manifest
   naming the release commit (deterministic output). Regenerate after the final
   release commit so its manifest identifies the source actually being released.
   Remaining at release time: owner attaches the archive, tracks the upstream
   Kareem reply, and reconciles the QCMS notice if rebuilt PDF assets ever
   ship. Retain current features until any remaining licensing trade-off is
   decided.
4. [ ] Source-only native wiring is proven locally (2026-11-03): the recipe
   links the pinned Maven AAR, SHERPA_REQUIRE_SOURCE=1 fails closed, and an
   F-Droid-style build produced a verifier-passing unsigned APK whose ORT bytes
   match the artifact and whose Sherpa bytes match the source build. Remaining:
   repeat the proof in a frozen, hermetic environment, and add OCR/PDF source
   inputs only if review objects to npm WASM. Preserve matched API/worker/loader
   pairs and current versions.
5. [ ] Reproducibility protocol in place (see the section above):
   `npm run check:android-reproducibility` proves fresh-compile reproducibility
   on one environment, with both APK copies retained. Remaining: repeat the
   proof in a frozen second environment, then complete signature copying with
   the production key and confirm fdroidserver's rebuild matches the published
   APK.
6. [ ] On Pixel 6 and another Pixel: install after resetting the debug test app;
   test import/open-with, bookmarks, English/Arabic OCR, scanner, voice
   download/preview, Arabic preprocessing, conceptual search, audiobook generation
   and background/lock-screen playback.
7. [ ] Install a higher-code production candidate with the same key; verify
   documents/bookmarks/audio survive. Include de-Googled Android and a 16 KB
   environment, plus an API 26 emulator when available; Pixel 6 alone does not
   establish those results.
8. [ ] Metadata draft ready at
   [fdroid/metadata/io.papercut.desktop.yml](fdroid/metadata/io.papercut.desktop.yml).
   Remaining: add screenshots/changelogs, set the release tag and signing-key
   fingerprint, copy into an fdroiddata fork, verify buildserver assumptions,
   then run lint/scanner/clean build, submit and address review. Do not hide
   runtime binaries with broad scanner ignores.
9. [ ] Rehearse the next update through F-Droid before announcing availability.

Earlier packaging/source-probe validation: 14 shipping/staging regression checks
and four APK verifier self-tests pass; targeted ESLint, whitespace, workflow YAML,
document links and metadata
length checks pass. A full production APK build succeeds. Its cleaned APK passes
unsigned verification, post-build signing and certificate verification with a
disposable key; wrong-certificate and signed-as-unsigned checks reject as expected.
The test key/password file was deleted. The source-built native pair packages
correctly, and all three source-built OCR cores pass English/Arabic smoke tests.
The current source-recipe stage additionally passes its revision/edit rejection
self-test, targeted zero-warning ESLint, a fresh disposable-tree OCR rebuild and
all four PDF codec builds with a matching worker. Three PDF rendering fixtures,
QCMS conversions and QuickJS initialization pass against the pinned reference.

The 2026-11-04 Tauri alignment passes the default Rust suite (274 passed,
four ignored) and shipping-feature suite (332 passed, nine ignored), plus full
`npm test` (96 files, 765 passed, one skipped), `npm run build`, script ESLint
with zero warnings, four APK verifier self-tests and ten targeted
Sherpa/reproducibility/notices/archive tests. All nine Android library-transfer
Robolectric tests pass against the upgraded Tauri Android library. Notices
`--check` passes.
Two review source-archive runs are byte-identical, with all nine MPL crate
trees and matching lockfiles/notices verified. The review archive is
`papercut-android-1.10.0-tauri-align-review-sources.tar.bz2`; regenerate the
release archive after the final source commit so its manifest points to the
committed inputs. Two fresh source-only APK builds pass the byte comparison
described above; the unsigned production verifier and packaged notice-byte
checks pass. The earlier OCR/PDF source probes were not rerun for this
upgrade. Production-key validation, second-environment/signature-copy proof,
full platform CI and Pixel runtime results remain explicit gates.

[Submission guide](https://f-droid.org/en/docs/Submitting_to_F-Droid_Quick_Start_Guide/),
[listing format](https://f-droid.org/en/docs/All_About_Descriptions_Graphics_and_Screenshots/)
