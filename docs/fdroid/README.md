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

The draft provisions the Debian compiler and utilities, SDK platform 36 and
Build Tools 35.0.0 in `sudo`. It stages a SHA-256-checked Node 22.22.1
distribution outside the source checkout before `npm ci`. It reuses the
shared toolchain installer for CMake 4.4.5, Emscripten 4.0.15 and the locked
wasm-bindgen CLI, and selects the pinned Temurin 17 with the Android JDK
helper. The exact r30 NDK remains managed by F-Droid. See the
[production guide](../android-production-packaging.md#repeatable-source-recipes).

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

The [first committed Pagefind CI run](https://github.com/muhannadnouri/papercut.io/actions/runs/42142469659)
passed. Comparing that APK against a local rebuild of the same commit exposed
host Cargo paths inside two bundled Pagefind WASM modules. The browser recipe
now remaps and checks those paths. See the
[cross-checkout evidence](../android-production-packaging.md#pagefind-wasm-path-normalization-2026-11-06).

## Unsigned buildserver preflight

The [next PR CI run](https://github.com/muhannadnouri/papercut.io/actions/runs/42277652623)
passed all nine jobs after the Pagefind fix. Its unsigned arm64 APK and a local
rebuild of its exact merge checkout `a2e72e143e44daaa871e7503a71476c1b5c15c2a`
matched at SHA-256
`66e1a3bd56a3aea0abd7f76ece6004eb185920a4d0ee02e49d8854c200f07a68`.

The independent preflight ran in a fresh Debian Trixie root filesystem from
the fdroiddata CI image, pinned to digest
`sha256:1ea8bef1820bff06c2974e31b4fc4c79c364cd24231b86e71fa4b926797e0dac`,
with fdroidserver commit `cdfdce835a8231996b02392bc79c1879e6bef501`. The
source build used the official F-Droid Gradle launcher and newly downloaded
SDK, compiler and app inputs. The source scan passed with refreshed SUSS
signatures and the existing rules. The separate Node and Java provisioning
commands were also exercised in that image.

The F-Droid test build produced a **byte-identical unsigned arm64 APK** to
the GitHub artifact above, including all 1,723 ZIP entries. The APK passed
the F-Droid binary scan and Papercut's unsigned verifier. All 88 frontend
files matched, including both shipped Pagefind WASM modules. The temporary
Gradle launcher was removed after the build. The independent build took
about 27 minutes on the test machine. Logs and digests are under
`.cache/android-production/fdroid-buildserver-preflight/`.

This was a local run in the official CI image. The test used a disposable
metadata copy with `Binaries` omitted, the exact CI revision, and the
toolchain prepared in the image. It used `--no-tarball` and had no signing
credentials. The dedicated production VM, release-tag build and signed
verification are later gates.

For an unsigned test, copy the draft into a disposable fdroiddata workspace,
remove `Binaries` from that copy, and use the commit being compared. Keep the
release tag and certificate placeholders in the tracked draft. A GitHub PR
merge commit needs an explicit fetch of `refs/pull/91/merge` before F-Droid can
check it out from a regular clone. Release tags are fetched normally.

Run this in a provisioned F-Droid buildserver container with the packages and
SDK from the draft's `sudo` block already installed. A dedicated F-Droid VM
runs that block with `fdroid build --server`.

```sh
fdroid rewritemeta io.papercut.desktop
fdroid lint -f io.papercut.desktop
fdroid build --verbose --test --scan-binary --refresh-scanner io.papercut.desktop:1010000
```

[fdroiddata CI build job](https://gitlab.com/fdroid/fdroiddata/-/blob/master/.gitlab-ci.yml) ·
[F-Droid buildserver setup](https://f-droid.org/en/docs/Build_Server_Setup/)

The source scanner runs before the source recipes fetch their inputs. Review
these inputs separately with F-Droid maintainers.

| Input | Provenance and review |
| --- | --- |
| Host toolchain | Debian packages and F-Droid's Android SDK catalog supply the base tools. Node, Temurin and CMake use exact upstream archives checked by SHA-256. The pinned emsdk manifest selects Emscripten and its bundled Node. Rustup installs the pinned compiler and targets, and Cargo builds wasm-bindgen with its lockfile. Confirm the upstream binary toolchains against the [inclusion policy](https://f-droid.org/en/docs/Inclusion_Policy/). |
| Tauri CLI | npm's lock pins the 2.11.4 host bindings with integrity hashes. The Linux `.node` files are ELF libraries even though the source scan left them in place. The host CLI orchestrates Gradle and Rust. Confirm its prebuilt treatment with reviewers. |
| Sherpa and Android dependencies | Sherpa uses a fixed Git commit and its CMake dependency archives specify SHA-256 hashes. The pinned Maven ORT AAR is verified by the shared recipe and supplies the packaged ORT runtime. Other Android dependencies use the existing Google Maven and Maven Central configuration. |
| PDF/OCR and Pagefind | Recipe Git commits and Tesseract submodules pin source trees. npm/Cargo locks identify package inputs. The extended Pagefind CLI also compiles Japanese UniDic from an archive fetched by Lindera 0.43.3. Lindera checks the archive's MD5. The audited copy's SHA-256 is `24ac75fe4e44d973b55e65847c35fa4aec2db7a41a07b49ca125146db9fc1189` and its COPYING offers BSD, GPL or LGPL terms. The dictionary is used by the host indexer. |

## Next execution gates

1. Owner sets up or reuses the production signing key. For automated signing,
   configure the protected GitHub `android-release` environment as well.
   See [guide section 2](../android-production-packaging.md#2-owner-create-and-configure-the-signing-key).
   Sign/install a candidate by file transfer (ADB optional), then complete
   runtime and same-key/higher-code update tests before merging.
2. Confirm the build-time inputs and host toolchain treatment with F-Droid
   reviewers before submission. The unsigned CI-image comparison is complete.
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

## Developer release paths

| Task | Entry point | Output and key access |
| --- | --- | --- |
| Build locally or in GitHub Actions | Install the locked npm dependencies, run `scripts/fdroid/prepare-android-wasm-tools.js`, then run `scripts/fdroid/build-android-release.sh` | The shared shell script builds Sherpa, OCR, PDF and Pagefind, then invokes `npm run android:apk:release` for the Tauri/Gradle build. The APK is unsigned. |
| Rebuild with F-Droid | Run `fdroid build` with the draft in a provisioned fdroiddata workspace | F-Droid installs dependencies, scans the checkout and removes the declared prebuilt files. The recipe builds esbuild, uses the F-Droid Gradle launcher and calls the same shared shell script. This produces an unsigned comparison APK. |
| Sign for a phone test | Verify the unsigned APK, then run local SDK `apksigner` as shown in the production guide | The developer's key signs the verified APK. Copy the resulting signed APK to the phone and open it. |
| Publish the upstream release | Push the reviewed release tag, or dispatch the Release workflow for that tag on a trusted workflow ref | The build job uploads unsigned bytes. Only the separate `sign-android` job uses the protected `android-release` environment. The publisher consumes the verified signed artifact. |
| Verify for F-Droid publication | Set `Binaries` to the signed release asset and pin its public certificate in `AllowedAPKSigningKeys` | F-Droid rebuilds the tagged source, copies the upstream signature onto its rebuild and verifies it. The release keystore stays with the developer. |

Start with [build and manual signing](../android-production-packaging.md#3-build-verify-and-update)
for commands. Use [owner signing setup](../android-production-packaging.md#2-owner-create-and-configure-the-signing-key)
for the keystore and GitHub environment, then
[production signature comparison](../android-production-packaging.md#production-signature-comparison)
for the final F-Droid proof.

Unsigned equality checks what two builds produced from the same revision.
The release signature lets Android verify the APK's publisher and continue
updates under the same signing identity. Keeping compilation and npm
execution outside the signing job keeps build-time code away from the key.
Review the workflow ref and resolved source tag before granting signing
access. [GitHub environment rules](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments) ·
[F-Droid signature copying](https://f-droid.org/en/docs/Reproducible_Builds/)
