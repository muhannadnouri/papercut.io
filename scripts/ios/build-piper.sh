#!/usr/bin/env bash
set -eu

# Apply the same definition to every translation unit and every XCFramework slice.
piper_config="$(cd "$(dirname "$0")" && pwd)/piper-path.cmake"
cmake() {
  if [[ "$1" == --build ]]; then
    command cmake "$@"
  else
    command cmake "-DCMAKE_PROJECT_piper_phonemize_INCLUDE=$piper_config" "$@"
  fi
}

source "$1/build-ios.sh"
