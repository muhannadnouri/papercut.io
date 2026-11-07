# Script command map

Run commands from the repository root. Prefer the existing npm command names;
the files below implement them. No build, signing or dependency policy changes
are implied by this directory layout.

## Where things live

| Folder | Purpose | Used by |
| --- | --- | --- |
| [android](android/) | APK builds, JDK/native library preparation, APK verification, license notice and corresponding-source assembly | Local Android development, CI and releases |
| [desktop](desktop/) | Desktop development/builds, native library staging, bundle verification | Local desktop development, CI and releases |
| [ios](ios/) | iOS builds, native library preparation, unsigned device build check | Local Apple development, CI and releases |
| [pdf](pdf/) | Stage installed PDF/OCR assets; generate manual PDF boundary fixtures | Frontend development/builds; optional diagnostics |
| [checks](checks/) | Localization, model-download monitoring, mobile provider runner | Shared checks and explicit CI inference runs |
| [fdroid](fdroid/) | Native source build and optional OCR/PDF probes, with cached inputs/receipts | Android CI/releases and manual F-Droid preparation |
| [search](search/) | Deterministic bundled search-index build and fixed-corpus search evaluation | Frontend builds and explicit evaluation runs |
| [silma](silma/) | SILMA sidecar/runtime packaging and optional CUDA installation | SILMA workflows and explicit local setup |
| [lib](lib/) | Shared process, path, download and platform helpers | Imported by entry scripts |
| [fixtures](fixtures/) | Shared fixed inputs used by script, frontend and Rust checks | Tests and diagnostics |

Tests live in the owning folder's `__tests__/`. Cross-platform build tests live
in [__tests__](__tests__/); Android library helper tests live in
[lib/android/__tests__](lib/android/__tests__/). Production verification commands
stay beside the builds they verify. A `check-*` filename alone does not identify
a unit test: model monitoring and installed-device inference are executable checks.

## Common commands

| Command | When to use it | Requirements |
| --- | --- | --- |
| `npm run dev` / `npm run build` | Frontend development / production assets | Installed npm dependencies; asset preparation runs automatically |
| `npm run build:search-index` | Index bundled HTML documents in fixed relative-path order | Existing `dist/documents/**/*.html`; uses locked Pagefind's Node API and writes `dist/pagefind` |
| `npm run tauri:dev` / `npm run desktop` | Desktop development / packaging | Matching OS desktop toolchain; [build instructions](../README.md) |
| `npm run android:apk:native-tts` | Development APK with shipping native features | Android SDK/NDK, Rust target and JDK; [Android guide](../docs/android-production-packaging.md) |
| `npm run android:apk:release` | Unsigned arm64 production APK | Public CI/releases first run the native source recipe and export its source-required environment; see the Android guide; signing is a separate release job |
| `npm run prepare:license-notices` | Regenerate the shipped Android third-party notices from pinned sources | Ignored `.cache/android-production` source checkouts and local npm/cargo state; `--check` verifies currency |
| `npm run prepare:source-archive` | Build the per-release corresponding-source archive (GPL archives, MPL crate sources, notices, manifest) | Same checkouts plus cargo registry state; run at the final release commit and attach the output to that release |
| `npm run check:android-reproducibility` | Prove two fresh source-only builds produce byte-identical unsigned APKs | Full Android toolchain; wipes Rust/Gradle outputs and rebuilds everything, 30-60 minutes |
| `npm run ios:ci` / `npm run ios:ci:device` | Simulator / unsigned device build checks | macOS, Xcode and iOS Rust targets; [Apple guide](../docs/apple-distribution.md) |
| `npm run check:i18n` | Validate localization keys and plural forms | Installed npm dependencies |
| `npm run test:pdf-extraction` | Check the fixed PDF extraction fixture | Installed npm dependencies |
| `npm run test:search-eval -- <report.json>` | Run the explicit fixed-corpus search evaluation | Native Rust build/toolchain; [evaluation protocol](../docs/search-validation.md) |
| `npm run prepare:silma-sidecar` / `npm run package:silma-runtime` | Explicit SILMA runtime packaging | Platform/Python setup in the [SILMA guide](../docs/silma-tts-sidecar.md) |
| `npm run install:silma-cuda-runtime` | Install an optional local CUDA runtime into app data | Linux x64/NVIDIA; explicit setup, not routine packaging |

Production packaging commands resolve the checked-out commit with
[lib/build-revision.js](lib/build-revision.js) and pass it as
`PAPERCUT_BUILD_REVISION` for transfer diagnostics. Source exports without `.git`
must supply that full revision from their release metadata; mismatched checkout
overrides fail. See the [revision contract](../docs/android-production-packaging.md#production-source-revision-contract).

Use `npm run` to list all existing commands, including platform preparation and
force-refresh variants. CI-only commands remain explicit in their workflows:

- [CI](../.github/workflows/ci.yml): packaging, regression checks and optional
  mobile inference. Job ordering and selection are unchanged by the relocation.
- [Release](../.github/workflows/release.yml): builds, artifact verification and
  isolated Android signing.
- [Model downloads](../.github/workflows/model-downloads.yml): release metadata
  checks and optional archive downloads; see the [monitoring guide](../docs/model-download-maintenance.md).
- [SILMA runtime](../.github/workflows/silma-runtime.yml): dedicated runtime builds.

## Tests and optional source probes

Vitest runs `*.test.js` with the normal `npm test` command. Node's existing
`*-selftest.js` / `*-selftest.mjs` files keep their explicit `node --test` calls;
moving them does not migrate test runners. For example:

```bash
npm test -- scripts/__tests__/shipping-builds.test.js scripts/lib/android/__tests__/sherpa.test.js
node --test scripts/android/__tests__/check-android-packaging-selftest.js
npm run test:lint-policy
```

Plain `npm test` excludes `.cache/`, retaining Vitest's default exclusions too.
Source-build checkouts therefore do not add upstream tests to the project suite.
Use `npm test -- src scripts` when you want an explicitly scoped project run.

The [fdroid](fdroid/) recipes write only to the ignored
`.cache/android-production/` tree. Android CI/releases use the native Sherpa
recipe; the OCR/PDF recipes and checks remain optional investigations requiring
WASM compilers. Their checks use already-built probe outputs and are under
`fdroid/__tests__/`. Those probes do not prove clean APK reproducibility. Follow the
[single Android guide](../docs/android-production-packaging.md) for those commands,
their prerequisites and the unresolved source/licensing decisions.
