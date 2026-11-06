#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."

cache="$PWD/.cache/android-production"
export EMSDK="${EMSDK:-$cache/emsdk}"
node --input-type=module -e '
  import assert from "node:assert/strict"
  import { NODE_VERSION } from "./scripts/lib/constants.js"
  assert.equal(process.versions.node, NODE_VERSION, "Use the pinned Node.js version for Android releases")
'
if [[ -x "$cache/cmake-4.4.5-linux-x86_64/bin/cmake" ]]; then
  export PATH="$cache/cmake-4.4.5-linux-x86_64/bin:$PATH"
fi
if [[ "$(cmake --version | sed -n '1p')" != 'cmake version 4.4.5' ]]; then
  echo 'The Android source build requires CMake 4.4.5.' >&2
  exit 1
fi

if [[ -n "${PAPERCUT_GRADLE:-}" ]]; then
  project="$PWD/src-tauri/gen/android"
  wrapper="$project/gradlew"
  if [[ "$PAPERCUT_GRADLE" != /* || ! -x "$PAPERCUT_GRADLE" || -e "$wrapper" || -L "$wrapper" ]]; then
    echo 'Set PAPERCUT_GRADLE to an absolute executable path in a scanner-cleaned checkout.' >&2
    exit 1
  fi
  version=$(sed -n 's|^distributionUrl=.*/gradle-\(.*\)-bin\.zip$|\1|p' "$project/gradle/wrapper/gradle-wrapper.properties")
  installed=$(cd "$project" && "$PAPERCUT_GRADLE" --version)
  if [[ -z "$version" ]] || ! grep -Fxq "Gradle $version" <<<"$installed"; then
    echo "The trusted Gradle launcher must use Gradle $version." >&2
    exit 1
  fi
  # Tauri executes the project launcher by absolute path after the scanner removes it.
  ln -s "$PAPERCUT_GRADLE" "$wrapper"
  trap 'rm -f -- "$wrapper"' EXIT
fi

node scripts/fdroid/build-android-source-libs.js
node scripts/fdroid/build-source-ocr.js
node scripts/fdroid/build-source-pdf-codecs.js
npm run android:apk:release
