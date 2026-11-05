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

| Files | Count | Treatment still needed |
| --- | ---: | --- |
| `@esbuild/linux-x64/bin/esbuild`, `esbuild/bin/esbuild`, `@pagefind/linux-x64/bin/pagefind_extended` | 3 | Host build tools: establish an acceptable tool/source provenance route |
| `@tesseract.js-data/{ara,eng}/{4.0.0,4.0.0_best_int}/*.traineddata.gz` | 4 | OCR data: document origins/licenses and agree the archive treatment |
| `pdfjs-dist/wasm/{jbig2,openjpeg,qcms_bg,quickjs-eval}.wasm` | 4 | Shipped PDF codecs/engine: existing source probes are available but not staged |
| `tesseract.js-core/tesseract-core*.wasm` | 6 | Installed OCR engines, including unused legacy variants; shipping uses the three LSTM variants |

Do not move `npm ci` after the scanner or add `scanignore: node_modules` to make
this appear clean. Reuse the existing PDF/OCR source probes for the runtime code,
keep matched JS loaders/workers and notices, and resolve each remaining finding
with a narrow, explained treatment. The native recipe also fetches Sherpa/CMake
dependencies during `build`; this source scan did not cover those later trees.

The scanner's Gradle-wrapper removal also needs integration work: the Tauri
pipeline must use the trusted buildserver's matching Gradle distribution without
depending on the removed wrapper JAR. Merely passing the source scan is not enough.

Local reports, tool versions, signature data and the disposable export are under
`.cache/android-production/fdroid-preflight/` (ignored; not release artifacts).
The root sample APK and owner files were left untouched.

## NDK decision

The tested pin is **r29 beta3**, not r29 final. F-Droid's signed catalog resolves:

| Metadata request | Installed revision |
| --- | --- |
| `29.0.13846066` | `29.0.13846066-beta3` |
| `r29` | `29.0.14206865` (final) |
| `30.0.16248370` | r30 LTS |

The beta3 Linux ZIP SHA-256 is
`ca0eaac8f992635a4a74c2927e2e76d788f18b8ba52c668ddc7f675fe6265d48`.

The draft now requests the exact numeric pin, which F-Droid
[supports](https://f-droid.org/en/docs/Build_Metadata_Reference/).
This preserves the existing CI/local reproducibility baseline. The pin is also
the default in the installed [Tauri CLI 2.11.4 source](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/mobile/android/mod.rs).

Google currently lists [r30 LTS](https://developer.android.com/ndk/downloads)
(`30.0.16248370`) as its current LTS release. Prefer a separately reviewed and
tested migration before production publication, rather than silently treating the
old beta as a long-term choice. New compiler/linker/libc++ inputs can change APK
bytes: update source/CI/release/metadata pins together, rebuild Sherpa and Rust
outputs, recheck arm64/API 26/16 KB compatibility, repeat fresh/relocated APK
comparison and exact-commit CI reproduction, then device acceptance. A different
APK hash across NDK versions is expected; two builds of the new pinned environment
must match each other. No production NDK upgrade was performed in this preflight.

## Next execution gates

1. Review the r30 migration, then resolve the scanner findings and Gradle bridge.
   Provision the actual buildserver's Node/npm, Rust 1.98.1 Android target,
   Java 17, CMake and SDK/build-tools inputs explicitly; the host preflight did
   not prove their availability in that image. Source PDF/OCR wiring will also
   need the documented Emscripten/Rust/WASM toolchains.
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
