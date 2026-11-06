# F-Droid recipe preflight

The [metadata](metadata/io.papercut.desktop.yml) is a **draft**, not a submission.
`commit: v1.10.0`, `CurrentVersion` and the build version/code remain historical
placeholders: that tag predates this branch's packaging changes. Set them to the
real release after testing, merging and preparing the planned **v1.10.1**. Add
`AllowedAPKSigningKeys` with the validated production certificate SHA-256 before
using `Binaries`. Never put a private key or password in metadata.

English/Arabic listings come from `fastlane/metadata/android/` in the app source.
The production wrapper derives `PAPERCUT_BUILD_REVISION` from the checkout; a
no-Git source export needs the explicit matching release commit, as described in
the [production guide](../android-production-packaging.md#reproducibility-protocol).

## Checked 2026-11-05

Preflight used an isolated Python 3.12 environment with **fdroidserver 2.4.5** and
**sdkmanager 0.7.2**, and a clean `git archive` of app commit `685e3ff3`.
Category/anti-feature definitions and their required icons came from fdroiddata
commit [`8069ed5f6a0009921b7972e8ba1a2250195481c2`](https://gitlab.com/fdroid/fdroiddata/-/tree/8069ed5f6a0009921b7972e8ba1a2250195481c2/config).
The export was scanned before and after `npm ci` (Node 24.21.0/npm 11.19.0).
These were direct `fdroidserver.scanner.scan_source` calls using the draft's
build entry, with refreshed SUSS signatures and no ignore/delete rules. The
Rust-install prebuild command and full `prepare_source`/buildserver were not run.

| Check | Result |
| --- | --- |
| Exact NDK installation | F-Droid's sdkmanager installed `29.0.13846066`; fdroidserver resolved the revised metadata to that installation. Independently checked ZIP SHA-256 against the signed catalog and compared all 9,655 regular archive files with the installed files; all match |
| Compiler identity | Installed `clang` SHA-256 `8a380f6bb33db6b84d4248684feef33133d1cddf4ee4a3402a9c25beb2e4b0e6` matches the local APK-proof toolchain |
| `fdroid lint -f io.papercut.desktop` | Formatting/category checks pass; command still fails because `Binaries` lacks `AllowedAPKSigningKeys`. No fake key supplied |
| Clean exported source | Zero fatal findings/warnings; scanner removes `gradlew`, `gradlew.bat` and `gradle-wrapper.jar` |
| After `npm ci` | **17 fatal binary/archive findings**, zero warnings; details below |
| Verified CI APK | `fdroid scanner --verbose --exit-code` passes on CI41987142956 APK, SHA-256 `2cf33d0acdd9c4c05a069eb03ed96c13c630e9195bc04d4c699effcce470a752` |

The APK check looks for known non-free DEX classes and extra signing blocks. It
does not establish licensing clearance, source provenance of WASM/native code,
reproducibility in F-Droid, production signing or device behavior. A source-scan
binary finding does not by itself mean that the file is proprietary or malicious.

All 17 findings are under `node_modules/`:

| Files | Count | Initial finding |
| --- | ---: | --- |
| `@esbuild/linux-x64/bin/esbuild`, `esbuild/bin/esbuild`, `@pagefind/linux-x64/bin/pagefind_extended` | 3 | Host build tools: establish an acceptable tool/source provenance route |
| `@tesseract.js-data/{ara,eng}/{4.0.0,4.0.0_best_int}/*.traineddata.gz` | 4 | OCR data: document origins/licenses and agree the archive treatment |
| `pdfjs-dist/wasm/{jbig2,openjpeg,qcms_bg,quickjs-eval}.wasm` | 4 | Source mode stages rebuilt codecs and matching PDF.js bundles. Handle the npm copies still present at scan time |
| `tesseract.js-core/tesseract-core*.wasm` | 6 | Source mode stages three rebuilt LSTM pairs. Handle all six installed npm variants at scan time |

Do not move `npm ci` after the scanner or add `scanignore: node_modules` to make
this appear clean. The [source staging pass](../android-production-packaging.md#source-built-pdf-and-ocr-runtime-2026-11-05)
changes which engine files reach the APK. It leaves the installed npm tree and
its scanner findings in place. Resolve each finding with a narrow, explained
treatment. Review the Sherpa/CMake and WASM recipe dependency trees as part of
the buildserver audit because the historical scan did not include those trees.

This scan identified the Gradle-wrapper removal. The
[shared release path](#shared-release-path-preflight-2026-11-05) now has a
bridge for Tauri.

Local reports, tool versions, signature data and the disposable export are under
`.cache/android-production/fdroid-preflight/` (ignored; not release artifacts).
The root sample APK and owner files were left untouched.

## NDK selection

The initial preflight tested **r29 beta3**, not r29 final. F-Droid's signed catalog resolves:

| Metadata request | Installed revision |
| --- | --- |
| `29.0.13846066` | `29.0.13846066-beta3` |
| `r29` | `29.0.14206865` (final) |
| `30.0.16248370` | r30 LTS |

The beta3 Linux ZIP SHA-256 is
`ca0eaac8f992635a4a74c2927e2e76d788f18b8ba52c668ddc7f675fe6265d48`.

The subsequent migration pins [r30 LTS](https://developer.android.com/ndk/downloads)
(`30.0.16248370`) across local Android builds, CI, release and this draft. F-Droid
[supports exact numeric pins](https://f-droid.org/en/docs/Build_Metadata_Reference/).
Its installer catalog and fdroidserver resolution were checked against the exact
r30 installation. Local native TTS debug and release require the same prepared
arm64 runtime. See the [r30 evidence](../android-production-packaging.md#shared-r30-toolchain-2026-11-05).

The table above describes the historical installer comparison. The earlier CI
artifact and source-scan reports still describe the beta3 preflight; they are
not a buildserver or signature comparison of the r30 candidate.

## Shared release path preflight (2026-11-05)

The Android CI job, GitHub release and draft recipe now invoke the shared
`scripts/fdroid/build-android-release.sh` command. It builds Sherpa, PDF.js and
Tesseract from the pinned sources. The production APK wrapper requires the
resulting PDF/OCR receipts.

A fresh Git export of `08986769` with this pass overlaid was scanned using
fdroidserver 2.4.5, the earlier cached SUSS signatures and Node 22.22.1. It
reported **one fatal finding and zero warnings**.

| Input | Treatment in the draft |
| --- | --- |
| esbuild | Delete both installed executables. Build the locked 0.28.2 from commit `609683d892977362a0f99026cb74b96263d728a9` with Go and select it through `ESBUILD_BINARY_PATH`. This is consistent with the source-build treatment in fdroiddata. [fdroiddata example](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/au.how2vote.app.yml) |
| OCR data | Delete the unused `4.0.0` copies. The two exact exceptions are English and Arabic `4.0.0_best_int` data archives. Their bytes match the pinned Apache-2.0 tessdata revision, and the release asset step enforces both hashes. These exceptions are for reviewer consideration. [Source inventory](../android-source-inventory.json) |
| PDF/OCR engines | Delete the four npm PDF WASM files, six npm OCR WASM files, and the npm OCR loaders that embed those binaries. The production asset step takes the rebuilt engine files from the source receipts. |
| Pagefind | The locked 1.4.0 extended binary is still flagged. Its output includes browser WASM and JS embedded in the npm binary. A source build needs the coupled frontend and WASM build as well as the Rust CLI. [Upstream 1.4.0 release build](https://github.com/Pagefind/pagefind/blob/v1.4.0/.github/workflows/release.yml) |

The published Pagefind 1.4.0 Cargo package includes 31 prebuilt browser WASM
files. After decompression, its English and fallback WASM match the two files
in Papercut's starter index. A plain `cargo install pagefind` would still
carry those prebuilt browser inputs. [Pagefind crate](https://crates.io/crates/pagefind/1.4.0)

The scanner deletes Tauri's project launchers and wrapper JAR. After scanning,
the shared command checks the buildserver's absolute `gradle` executable
against the version in `gradle-wrapper.properties` (8.14.3). It temporarily
links Tauri's expected project launcher to that executable and removes the
link on exit. F-Droid's Gradle launcher selects the distribution from the
project and verifies its download. [F-Droid Gradle launcher](https://gitlab.com/fdroid/gradlew-fdroid)

The local integration check used a scanner-treated export, source-built esbuild
and Gradle 8.14.3 without the original wrapper JAR. It retained the flagged
Pagefind input for the manual build. The resulting unsigned APK matched the
normal wrapper build byte-for-byte, and the temporary launcher was removed.
See the [APK evidence](../android-production-packaging.md#shared-android-release-and-scanner-preflight-2026-11-05).

For the clean buildserver, provision Node 22.22.1/npm, Rust 1.98.1 with the
Android and `wasm32-unknown-unknown` targets, JDK 17, SDK platform 36, Build
Tools 35.0.0, r30 NDK, CMake 4.4.5, Go, and the documented Emscripten and
wasm-bindgen tools. Confirm the sources and toolchain with F-Droid reviewers.
The GitHub toolchain setup installs the official CMake release and Emscripten
SDK. The draft expects the buildserver to provide those tools and the locked
wasm-bindgen CLI. See the [production guide](../android-production-packaging.md#repeatable-source-recipes).
The local scan covers the application checkout and its npm install. Review
the Tauri host CLI and the inputs fetched by the source recipes when validating
the full buildserver.

## Source-built Pagefind preflight (2026-11-05)

The draft deletes the npm Pagefind executable by exact path. The shared
Android build rebuilds the extended CLI, coupled browser JS and all 31 WASM
variants from the pinned Pagefind 1.4.0 tag. Its upstream JS build scripts
use their own npm locks and Papercut's pinned esbuild, which is rebuilt from
source for F-Droid. The production wrapper points Pagefind's existing Node
API at the rebuilt executable. See the [recipe and evidence](../android-production-packaging.md#source-built-pagefind-2026-11-05).

A fresh Git export at `022cbb4` with this stage overlaid and npm
dependencies reported **zero fatal findings and zero warnings** in
fdroidserver 2.4.5 with the earlier cached SUSS signatures. The scanner
removed `node_modules/@pagefind/linux-x64/bin/pagefind_extended`. The two
existing exact exceptions for pinned OCR data remain in the draft. This
local scan covers the export before the build stage.

The same-host integration check built Pagefind separately in the two
checkouts. The scanner-treated export used trusted Gradle and source-built
esbuild, and its verified unsigned APK matched the normal wrapper's output
byte-for-byte. See the [APK evidence](../android-production-packaging.md#source-built-pagefind-2026-11-05).

## Next execution gates

1. Verify Android CI at the committed Pagefind source path. Run the clean
   F-Droid buildserver preflight with its own toolchain and review the inputs
   fetched during the build.
2. Owner: set up/reuse the production signing key and protected GitHub
   `android-release` environment using [guide section 2](../android-production-packaging.md#2-owner-create-and-configure-the-signing-key).
   Sign/install a candidate by file transfer (ADB optional), then complete
   runtime and same-key/higher-code update tests before merging.
3. Prepare v1.10.1 after merge/testing. Refresh the final release tag, version/code,
   screenshots/changelogs, public certificate fingerprint and corresponding-source
   archive. Resolve the existing model-rights/listing follow-ups before submission.
4. In an fdroiddata fork containing its `config/` directory, copy this metadata to
   `metadata/io.papercut.desktop.yml`. Run with the selected buildserver tooling:

   ```sh
   fdroid lint -f io.papercut.desktop
   fdroid scanner --json io.papercut.desktop > papercut-scanner.json
   fdroid build --verbose --test io.papercut.desktop
   ```

   Review the JSON findings as well as command status: fdroidserver 2.4.5's source
   scanner does not reliably make source findings a nonzero CLI exit; the build
   command refuses fatal scan findings. These commands are future gates, not a
   claim that the current placeholders/build recipe can pass. Complete a clean
   buildserver build and compare with the matching upstream signed APK using
   production signature copying, as described in the production guide.
