<p align="center">
  <img src="src-tauri/icons/icon.png" alt="Papercut App Icon" width="96" height="96">
</p>

# Papercut

[![Latest release](https://img.shields.io/github/v/release/muhannadnouri/papercut.io?logo=github&color=6366f1)](https://github.com/muhannadnouri/papercut.io/releases/latest) [![CI](https://github.com/muhannadnouri/papercut.io/actions/workflows/ci.yml/badge.svg)](https://github.com/muhannadnouri/papercut.io/actions/workflows/ci.yml) [![React](https://img.shields.io/badge/React-19-20232A?logo=react&logoColor=61DAFB)](https://react.dev/) [![Tauri + Rust](https://img.shields.io/badge/Tauri_+_Rust-2.x_|_1.77+-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE.md)

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io.svg?type=shield&issueType=license)](https://app.fossa.com/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io?ref=badge_shield&issueType=license) [![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io.svg?type=shield&issueType=security)](https://app.fossa.com/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io?ref=badge_shield&issueType=security)

**Homepage:** 👉 [https://trypapercut.app](https://trypapercut.app) 👈
- (Backup) [Netlify Homepage URL](https://trypapercut.netlify.app) 

[![Download for Android](https://img.shields.io/badge/Download-Android-3DDC84?logo=android&logoColor=white)](https://trypapercut.netlify.app/#downloads-title) [![Download for Linux](https://img.shields.io/badge/Download-Linux-FCC624?logo=linux&logoColor=black)](https://trypapercut.netlify.app/#downloads-title) [![Download for Windows](https://img.shields.io/badge/Download-Windows-0078D4?logo=windows11&logoColor=white)](https://trypapercut.netlify.app/#downloads-title) [![Download for macOS](https://img.shields.io/badge/Download-macOS-000000?logo=apple&logoColor=white)](https://trypapercut.netlify.app/#downloads-title)


Papercut is an offline reader for searching, reading, and listening to document collections. Built with Tauri, React, Vite, Pagefind, SQLite FTS, and native sherpa-onnx TTS.

Bundled documents are indexed at build time using Pagefind, which creates a compressed search index. User-imported HTML and EPUB documents are indexed at runtime into a local SQLite FTS database in Tauri app data, so users can add their own documents without rebuilding the app. EPUB uploads are parsed as a sibling runtime format that emits the same normalized document sections before indexing. At runtime, only the relevant search provider is queried and results are merged into one UI. The entire application runs offline with no server or internet connection required.

## Prerequisites

| Tool  | Minimum Version | Recommended Version |
|-------|-----------------|---------------------|
| Node  | >= 22.12.0      | 22.22.1             |
| npm   | >= 10.9.0       | 10.9.4              |
| Rust  | >= 1.77.2       | 1.94.0              |
| Cargo | >= 1.77.2       | 1.94.0              |

<details>
<summary><strong>Platform setup details</strong></summary>

### Install Node.js

**Linux / macOS** — using [nvm](https://github.com/nvm-sh/nvm) (recommended):

```bash
nvm install 22
nvm use 22
```

**Windows** — using [nvm-windows](https://github.com/coreybutler/nvm-windows) (recommended):

```powershell
nvm install 22.22.1
nvm use 22.22.1
```

Or install directly from the [Node.js download page](https://nodejs.org/en/download).

### Install Rust

**Ubuntu/Debian/Mint:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Arch-based (CachyOS, Manjaro, etc.):**

```bash
sudo pacman -S rustup
rustup default stable
```

> If using fish shell, the Cargo bin directory is already on your PATH via the system rustup package. Verify with `rustc --version`.

**Windows:** download and run [rustup-init.exe](https://www.rust-lang.org/tools/install). Choose the default `stable-x86_64-pc-windows-msvc` toolchain. Open a new terminal after install so `cargo` and `rustc` are on `PATH`.

### System Dependencies (Linux)

Tauri requires the following system libraries. Refer to the Tauri [documentation](https://v2.tauri.app/start/prerequisites/#linux) for full details.

**Debian-based (Ubuntu,Mint etc.):**

```bash
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev build-essential curl wget file libssl-dev libxdo-dev patchelf gstreamer1.0-plugins-base gstreamer1.0-plugins-good
```

**Arch-based (CachyOS, Manjaro, etc.):**

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool patchelf gst-plugins-base gst-plugins-good
```

### System Dependencies (macOS)

Tauri on macOS uses the system WebKit (WKWebView) bundled with the OS, so there are no WebKitGTK/GTK-style system packages to install. Only the Xcode Command Line Tools are required:

```bash
xcode-select --install
```

The Rust toolchain (above) and Node.js (above) cover the rest. Native sherpa-onnx TTS dylibs are downloaded automatically during the build and bundled into the `.app` via Tauri resources, so no manual library setup is needed.

### Android Prerequisites

Required to build the Android APK:

| Tool        | Minimum Version |
|-------------|-----------------|
| Java (JDK)  | 17              |
| Android SDK | API 26+         |
| Android NDK | 29.0.13846066   |

**Install Java 17:**

The Android build scripts can prepare a repo-local Eclipse Temurin JDK 17 automatically. This is useful in sandboxed editor environments where system package managers are not available:

```bash
npm run prepare:jdk
```

The local JDK is extracted to `src-tauri/tts/runtime/jdk/temurin-17`, which is ignored by git. The fallback archive is pinned to Eclipse Temurin 17.0.19+10 and verified with SHA-256 before extraction. The helper downloads through a temporary file and only promotes the archive after a completed fetch, so interrupted downloads do not leave a partial archive in place. `npm run android:apk` and `npm run android:apk:native-tts` set `JAVA_HOME` to this local JDK automatically when no external `JAVA_HOME` is available.

System JDK installs also work:

```bash
# Ubuntu/Debian
sudo apt install openjdk-17-jdk

# Arch-based
sudo pacman -S jdk17-openjdk
```

**Install Android SDK and NDK:**

Install [Android Studio](https://developer.android.com/studio) (recommended) or the command-line tools only. Then install the NDK via SDK Manager:

```bash
sdkmanager "ndk;29.0.13846066"
```

**Install Rust Android targets** (one-time):

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

**Set required environment variables:**

```bash
export ANDROID_HOME=$HOME/Android/Sdk
export NDK_HOME=$ANDROID_HOME/ndk/29.0.13846066
```

### System Dependencies (Windows)

Tauri on Windows needs two things beyond Node and Rust:

1. **Microsoft Visual Studio C++ Build Tools** — required for the MSVC linker used by Rust. Install from the [Visual Studio Build Tools page](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and select the "Desktop development with C++" workload.
2. **Microsoft Edge WebView2 Runtime** — the renderer Tauri uses. Preinstalled on Windows 11 and up-to-date Windows 10. If missing, install the [Evergreen Bootstrapper](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

Refer to the Tauri [Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows) for full details.

</details>

## Getting Started

### Install dependencies

```bash
npm install
```

### Development

```bash
npm run tauri:dev
```

This starts the Vite dev server and launches the Tauri desktop window with hot reload. Bundled-document search requires a built Pagefind index, so bundled search is only available after `npm run build`. Runtime uploaded-document search works inside the Tauri app after documents are imported.

<details>
<summary><strong>Production, release, Android, TTS, and browser builds</strong></summary>

### Production build

```bash
npm run desktop
```

When this command is run from a Flatpak-hosted editor terminal, the script automatically delegates the desktop build to the host OS with `flatpak-spawn --host`. That keeps the command the same while letting Tauri and `linuxdeploy` see the real host WebKitGTK/GTK libraries needed for `.deb`, `.rpm`, and AppImage bundling. You do not need to source `tauri-env.sh` before `npm run desktop`; if it has already been sourced, the desktop wrapper removes the Flatpak pkg-config variables before running the host build.

This runs the full pipeline:

1. TypeScript compilation
2. Vite frontend build
3. Pagefind indexes all HTML documents in `public/documents/`
4. Tauri compiles the Rust backend and bundles the desktop application

The built binary is output to `src-tauri/target/release/app` (`app.exe` on Windows). Installers are generated in `src-tauri/target/release/bundle/`:

- **Linux:** `.deb`, `.rpm`, and `.AppImage`
- **Windows:** `.msi` (WiX) under `bundle/msi/` and `.exe` (NSIS) under `bundle/nsis/` when building on Windows
- **macOS:** `.dmg` (and `.app`) under `bundle/dmg/` and `bundle/macos/` when building on macOS

`npm run desktop` uses the shared native TTS build to keep release compilation/linking memory lower. On Linux, the build copies the sherpa-onnx shared libraries into the Tauri resource directory before bundling, and the app binary includes an rpath to `/usr/lib/Papercut` so installed `.deb`, `.rpm`, and AppImage builds can find those libraries at launch. The AppImage also bundles the GStreamer media framework used by WebKitGTK for audiobook playback; local Linux builders therefore need the GStreamer base and good plugin packages listed above. If you specifically need a fully static native TTS build, use `npm run desktop:static`; that path can require substantially more RAM and may be killed by the OS on memory-constrained machines.

Install the generated Debian package with a dependency-aware command so WebKitGTK and GTK are installed if needed:

```bash
sudo apt install ./src-tauri/target/release/bundle/deb/Papercut_1.0.0_amd64.deb
```

If you previously used `sudo dpkg -i ...` and the app did not launch, run `sudo apt -f install` once to finish installing missing dependencies, then reinstall the newly generated `.deb`.

**macOS Gatekeeper:** Official release `.dmg` artifacts are built per-architecture and the release workflow signs, notarizes, and verifies them through the protected `apple-release` GitHub Environment. CI or local builds without Apple signing secrets are development artifacts and may still require right-click (or Control-click) > **Open** on first launch. Release artifact names are `Papercut_<version>_aarch64.dmg` for Apple Silicon and `Papercut_<version>_x64.dmg` for Intel. Pick the one matching your Mac. Native sherpa-onnx TTS dylibs are bundled inside the `.app` resources and resolved via an `@loader_path/../Resources` rpath, so no separate runtime library install is needed.

**AppImage troubleshooting:** `npm run desktop` sets `NO_STRIP=1` because the `linuxdeploy` tool used to bundle the AppImage can fail when its bundled `strip` cannot handle the host ELF format. If AppImage packaging reports `Could not find dependency: libwebkit2gtk-4.1.so.0`, the build is running in an environment that cannot see host WebKitGTK libraries. If the build succeeds but `npm run verify:appimage-media` reports missing files, install the GStreamer base and good plugin packages above and rebuild. The desktop build wrapper handles Flatpak editor terminals by re-running the build on the host; outside Flatpak, install the Linux system dependencies above and rerun `npm run desktop`. Tauri's AppImage media bundling is fully supported on Ubuntu build systems, and Papercut builds and verifies its Linux release artifacts on Ubuntu 24.04 CI.

### Version bump checklist

For an app release, keep the frontend package version, Tauri bundle version, and Rust crate version in sync.

Update these files:

- `package.json` — React/frontend package version.
- `package-lock.json` — npm lockfile version metadata.
- `src-tauri/tauri.conf.json` — Tauri app/bundle version used by installers.
- `src-tauri/Cargo.toml` — Rust crate version.
- `src-tauri/Cargo.lock` — refreshed if Cargo records the local crate version change.

Suggested flow:

```bash
VERSION=1.0.1
npm version "$VERSION" --no-git-tag-version
```

Then set `version` to the same value in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`, and run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --features native-tts-shared
npm run build
```

Commit the changed version files together with the release changes.
Create or update `RELEASE_NOTES/vX.Y.Z.md`, and prefer a new patch tag instead of replacing a published tag if release validation finds an installer/runtime packaging issue.

### Running the AppImage (Arch-based systems)

On Arch-based systems, the AppImage may show a blank screen due to a WebKit GBM buffer allocation failure with modern Mesa drivers. Set `WEBKIT_DISABLE_COMPOSITING_MODE=1` to disable GPU compositing:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./Papercut_1.0.0_amd64.AppImage
```

To avoid setting this every time, export it permanently in your shell:

```fish
set -Ux WEBKIT_DISABLE_COMPOSITING_MODE 1
```

### Android APK build

Before building for Android the first time, initialize the Android project (run once, commit the generated files):

```bash
npm run tauri -- android init
```

Then build the APK. The wrapper prepares/uses the local JDK and sets `JAVA_HOME` automatically:

```bash
npm run android:apk
```

Use `npm run android:apk:native-tts` when building the Android APK with native audiobook generation and background playback. Native playback uses Android Media3/ExoPlayer through the official, exactly pinned `tauri-plugin-native-audio` 1.0.5 packages; API 26 is the minimum supported Android version.

Audiobooks are not pre-rendered into the APK. Users save full audiobooks on demand from the document UI, and generated audio is stored as local app user data.

The APK is output to:

```
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

The `--debug` flag signs the APK automatically with a debug keystore, which is required for sideloading. Unsigned release APKs are silently rejected by Android at install time.

To sideload on an Android device, enable **Install unknown apps** in Settings and transfer the `.apk` file directly (via USB, ADB, or file share).

### iOS IPA build

iOS builds use the App Store display name `Papercut Offline` and Bundle ID `io.papercut.app` from `src-tauri/tauri.ios.conf.json`. They require macOS with full Xcode, but they do not require owning a MacBook. Use a GitHub-hosted `macos-26` runner or MacInCloud for the Apple project initialization and release build. App Store uploads require Xcode with the iOS 26 SDK or newer.

`src-tauri/gen/apple` contains the generated Tauri Apple project and should stay committed. Without a MacBook, regenerate it from MacInCloud or by temporarily restoring a macOS GitHub Actions bootstrap workflow, then replace `src-tauri/gen/apple`. The equivalent macOS command is:

```bash
npm ci
npm run ios:init
```

PR CI runs Tauri's unsigned iOS simulator build on `macos-26` to catch project and compile regressions without Apple secrets. After `src-tauri/gen/apple` is committed and Apple signing/provisioning secrets exist in the protected `apple-release` GitHub Environment, the release workflow builds, verifies, uploads the iOS IPA artifact for CI inspection, and submits it to App Store Connect/TestFlight. To build the IPA manually on macOS:

```bash
npm run ios:ipa
npm run ios:ipa:native-tts
```

The iOS native TTS build uses the official sherpa-onnx iOS static XCFramework archive, verifies its SHA-256, prepares thin Cargo link archives under `src-tauri/tts/runtime/sherpa-onnx-ios/cargo-libs/`, and builds the App Store Connect IPA with `native-tts-ios`. Device builds link the upstream `ios-arm64` aggregate archives directly; simulator CI thins the upstream universal simulator archives to arm64 before Rust sees them. The iOS feature also enables Libtashkeel through static ONNX Runtime linking by pointing `ORT_LIB_LOCATION` at the prepared iOS archive directory. Native background playback uses `tauri-plugin-native-audio` on iOS, and the generated Apple target declares Background Modes > Audio.

### Android build troubleshooting

If Cargo prints `Blocking waiting for file lock on artifact directory`, another Rust/Tauri build is holding the target directory lock. Wait for the other build to finish, or stop the older terminal process and rerun the command. If no Cargo/Rust process is running, rerunning with a fresh terminal normally clears it.

### Offline native multilingual text-to-speech

Papercut uses one native sherpa-onnx TTS architecture on desktop, arm64 Android, and iOS. React selects a catalog model and voice; Rust downloads, verifies, loads, and caches that model through a generic engine interface. Browser preview can display the UI but cannot synthesize audio. iOS uses the official sherpa-onnx static XCFramework archive rather than desktop dylib bundling.

Supported catalog models:

- **Kokoro English v1.0**: existing default, 27 voices, 349,418,188-byte archive.
- **Piper Kareem Medium (`ar-JO`)**: Arabic option using sherpa VITS, one voice, 67,177,830-byte archive. SHA-256: `9ebbcea30e0fbd588f7b2cb45ee897d6aeb1bf5791cbc037a7b5a3f641e3dbce`.

Models are not packaged in installers, APKs, or IPAs. The selected model is downloaded on demand from the pinned official sherpa-onnx TTS-model release, verified before extraction, and stored in Tauri app data. Desktop, Android, and iOS share model archives; only native sherpa libraries differ by platform.

Arabic-dominant documents automatically suggest Piper Kareem. Users can override the model selector. Arabic sentence and clause punctuation is recognized during chunking, and every synthesis request has a hard character bound to reduce native crashes on long unpunctuated text. Piper is practical and much smaller, but it should not be described as Kokoro-equivalent quality; voice naturalness must be evaluated on target Arabic material and devices. The upstream model repository is MIT-licensed, while its dataset provenance/license is not clearly stated, so redistribution should receive a license review. On-demand download reduces app distribution risk but does not replace that review.

Arabic pronunciation remains a separate concern from HTML extraction. Piper uses eSpeak-ng phonemization, so undiacritized Arabic can still produce ambiguous or poor vowels. Native shared builds and iOS native builds include an optional Libtashkeel 1.5.0 preprocessing pipeline. Piper defaults to `libtashkeel-1.5.0`; users can select `none` to synthesize the original text. The 4,788,213-byte bundled diacritization model runs through the same ONNX Runtime used by sherpa-onnx: shared library loading on desktop/Android, and static archive linking on iOS. Source chunks and DOM spans are never rewritten: only the synthesis copy is diacritized, so highlighting remains aligned to the original document. Libtashkeel improves contextual vowel restoration but is not an Arabic language oracle; names, case endings, dialect, and ambiguous prose still require listening tests.

The HTML narration adapter now preserves prose placed directly inside readable wrappers such as legacy table cells, even when those wrappers also contain nested headings or paragraphs. A generic Arabic HTML fixture covers this pattern: its first paragraph is a direct `td` text node, while the next paragraph is inside `p`. The former extractor omitted the direct text before Piper received any chunk. Bracketed inline footnote reference links such as `[1]` and `[2*]` are skipped during narration extraction and DOM highlight indexing, while the actual footnote paragraphs remain readable later in the document. Native synthesis also expands standalone four-digit historical years on the synthesis copy only, so `1984` is spoken as "nineteen eighty four" without rewriting source chunks or highlight spans.

Compatibility is preserved: `native-save-v4-segmented` is unchanged, Kokoro keeps its exact model ID and cache key, and old preferences, manifests, records, and bundles without preprocessing metadata default to `none`. Imported audiobook bundles use their stored chunk metadata for playback/status instead of re-chunking restored HTML, so older completed WAV audiobooks remain playable. Current import validation still rejects bundles whose stored preprocessor is not supported by the current runtime, even though completed bundles already contain their WAV audio; that is a known UI/runtime shortcoming to fix separately from packaging Libtashkeel for iOS generation. New bundle exports retain optional source spans for each chunk, letting re-imported HTML/EPUB generated-reader audiobooks restore highlighting without rediscovering positions from text. Older bundles still rebuild highlight spans lazily from restored HTML and graft them only when the rebuilt chunk ids/text exactly match the bundle chunks; playback is not blocked while that work runs. When legacy HTML repairs or text extraction changes make those rebuilt spans unavailable or wrong, playback can still use a cached live-reader text-match fallback for the current rendered DOM. The fallback tries exact text first, then tolerates Arabic Unicode differences such as tashkeel, tatweel, bidi controls, and common Arabic/Persian letter variants. This is intentionally a short-term compatibility path for imported bundles, not the full long-term locator model; future format adapters should still provide chapter/page-aware locators for very large EPUB/PDF-style readers. A diacritized Piper generation receives a separate audiobook ID, so it cannot silently reuse older undiacritized WAV chunks. Most books produce identical chunks. A book affected by the wrapper-text omission must be regenerated to include the newly retained prose; its corrected source signature and chunk sequence intentionally do not match the incomplete generation.

See [docs/kokoro-tts.md](docs/kokoro-tts.md) for architecture, model metadata, mobile constraints, and maintenance rules.

Narration chunks and generated WAV files remain native app user data. Desktop uses bounded chunk playback; Android prepares a reusable local `playback.wav` for background and lock-screen playback. Build helpers continue to orchestrate npm, Cargo, Tauri, Android SDK tooling, checked downloads, and platform library staging; they do not replace those package managers.

The audio UI supports model, voice, and optional text-processing selection, saved-only playback, resumable generation, background controls, chunk navigation/highlighting, thread tuning, opt-in diagnostics, import/export/delete, and saved-audio filtering.

### Browser build and preview

Build the frontend, generate the Pagefind index, then run the browser preview server with one command:

```bash
npm run browser
```

For CI or packaging steps that only need the built frontend artifacts, run:

```bash
npm run build
```

The `build` command is split into named stages for troubleshooting and CI reuse:

```bash
npm run build:typecheck
npm run build:vite
npm run build:search-index
```

</details>

## Adding Documents

Papercut now has two document paths:

- **Bundled documents** live in `public/documents/` and are indexed by Pagefind during the production build. This is still the best path for documents you ship to every user.
- **User uploads** are imported from the app UI and indexed incrementally into a local SQLite FTS database. This is the scalable path for documents users add themselves, because it does not require a rebuild or a packaged Pagefind index update.

The upload/indexing architecture is documented in [docs/user-document-search.md](docs/user-document-search.md). EPUB implementation notes and remaining follow-up work are tracked in [docs/epub-implementation-plan.md](docs/epub-implementation-plan.md).

<details>
<summary><strong>Document formats and search behavior</strong></summary>

### Bundled HTML Documents

Place your HTML files in `public/documents/`. Each document should have a standard HTML structure:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Your Document Title</title>
</head>
<body>
  <h1>Your Document Title</h1>
  <p>Your content here.</p>
</body>
</html>
```

Pagefind will automatically extract and index the text content on the next build. The `<title>` tag is used as the document title in search results.

### User-Uploaded HTML And EPUB Documents

From the document list, open **Import** and choose **HTML** to select a local `.html` or `.htm` file, or **EPUB** to select a local `.epub` book. The native import path stores sanitized HTML for HTML uploads and generated reading HTML for EPUB uploads under Tauri app data, extracts readable sections, and indexes those sections into SQLite FTS5. Uploaded documents appear under **User Uploads**, open in the app-owned reader surface, participate in the same search UI, and can use the same TTS playback/save flow when native TTS is available. The app supports persisted Light, System, and Dark theme choices, and the HTML/EPUB reader uses the same theme tokens as the rest of the UI. The reader also includes persisted font, font-size, line-height, and width controls that restyle the live reader with CSS variables without changing stored documents or audiobook metadata. Reader font choices use bundled offline fonts for consistent desktop and Android rendering, with Arabic-focused options available explicitly rather than forced as fallbacks for every document. Uploaded documents can also be deleted from the document list; delete removes the SQLite rows and the stored source file directory to free local storage.

EPUB import validates the archive container, follows the OPF spine, stores a sanitized generated reading HTML copy, and outputs the same normalized sections before indexing. The generated reading copy is rendered directly into the reader DOM so ToC links, footnotes, Find, and TTS highlighting share one scroll model. PDF should follow the same shared import/search contract later, while keeping a PDF-specific viewer and page-aware locators; PDF document pages are not expected to be recolored by the app dark theme.

### Search Behavior

Search is **explicit**: the app only searches when the user clicks the **Search** button next to the input or presses **Enter**. Typing does not trigger search. This keeps CPU and memory flat at scale (thousands of documents, large per-result fragment fetches). Bundled-document queries go through Pagefind, uploaded-document queries go through SQLite FTS5, and the React UI merges both result sets.

- Queries are lowercased before being passed to the search providers, making search case-insensitive regardless of how the user types it (`The quick brown fox jumped over the lazy dog` and `the quick brown fox jumped over the lazy dog` return the same results).
- Selected **Filter By Document** entries are passed into the search scope, so provider limits are spent on the chosen documents instead of searching everything and hiding unrelated results afterward.
- Wrapping a phrase in double quotes (`"the quick brown fox jumped over the lazy dog"`) runs an **exact phrase** match. Pagefind and SQLite FTS are still used first to find likely candidate documents, then the app reads the actual bundled or uploaded document source, normalizes whitespace and curly quotes, lowercases, and checks for the quoted phrase as a substring. Candidates that do not contain every quoted phrase are dropped.
- The Pagefind `content` field on `result.data()` and SQLite FTS section counts are not used as exact phrase counts because they come from broad candidate matching. Quoted-search results are reported as document-level phrase matches with source-verified occurrence counts; unquoted searches can show matching section counts when the search provider exposes them. Source reads are cached per URL in memory for the session to avoid re-fetching across queries.
- Clearing the input clears the results panel immediately, without triggering a search.
- In-flight stale results are dropped: if the user fires a new search before the previous one resolves, the earlier result set is discarded and never rendered.
- Uploaded-document snippets are produced by SQLite FTS and sanitized again before rendering in React. Uploaded matches are collapsed to one result card per uploaded document, using the first/best matching snippet. Opening a result jumps to the likely match and highlights it when the rendered reader text can be matched; users can still use in-document Find to move through additional matches.
- The "No documents found" message only appears after a search has actually been submitted (via Search button or Enter), not while the user is still typing.

</details>

## Flatpak Environment

<details>
<summary><strong>Flatpak development setup</strong></summary>

If running VS Code or Codium from a Flatpak, source the environment helper before running development Tauri commands:

```bash
source ./tauri-env.sh
npm run tauri:dev
```

This sets `PKG_CONFIG_PATH` and `PKG_CONFIG_SYSROOT_DIR` to point at the host system libraries mounted at `/run/host/`. Production desktop builds use `npm run desktop`, which delegates to the host automatically when Flatpak is detected so AppImage bundling can resolve host WebKitGTK dependencies.

</details>

## Project Structure

<details>
<summary><strong>Repository layout</strong></summary>

```
papercut.io/
├── public/documents/              # Bundled HTML documents indexed by Pagefind
├── src/                           # React frontend
│   ├── assets/                    # Bundled UI assets, including the header icon
│   ├── components/                # Reusable UI and reader/search/library panels
│   ├── hooks/                     # Shared React state hooks
│   ├── tts/                       # Audiobook API, components, hooks, storage, diagnostics
│   ├── uploads/                   # User-upload client API and types
│   ├── utils/                     # Search, formatting, document, and debug helpers
│   ├── viewers/                   # Document viewer registry and viewer implementations
│   ├── App.tsx                    # App shell and tab orchestration
│   ├── App.css                    # Main app styles
│   ├── index.css                  # Base styles
│   └── main.tsx                   # Entry point
├── src-tauri/                     # Tauri / Rust backend
│   ├── src/document_uploads/      # Runtime HTML upload + SQLite FTS indexing
│   ├── src/native_tts/            # Native sherpa-onnx TTS and audiobook bundles
│   ├── tts/model-manifest.json    # Pinned native TTS model catalog
│   ├── tauri.conf.json            # Base Tauri config
│   ├── tauri.ios.conf.json        # iOS Bundle ID / App Store config
│   └── tauri.linux.conf.json      # Linux shared-library bundle config
├── scripts/                       # Desktop/Android build orchestration
│   └── lib/                       # Shared and platform-specific script helpers
├── docs/                          # Feature and architecture notes
├── index.html                     # HTML shell
├── vite.config.ts                 # Vite configuration
├── package.json                   # Scripts and dependencies
└── tauri-env.sh                   # Flatpak development environment helper
```

</details>

## License

Papercut is available under the [MIT License](LICENSE.md).

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io.svg?type=small)](https://app.fossa.com/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io?ref=badge_small)

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io.svg?type=large&issueType=license)](https://app.fossa.com/projects/git%2Bgithub.com%2Fmuhannadnouri%2Fpapercut.io?ref=badge_large&issueType=license)
