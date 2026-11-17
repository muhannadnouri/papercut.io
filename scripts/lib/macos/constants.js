import { SHERPA_VERSION } from "../constants.js"
import { fromRuntime } from "../paths.js"
import { nativeCacheDir } from "../native-paths.js"

export { SHERPA_VERSION }

// The app prepares per-arch archives; Node matches the native CI runner architecture.
export function macosArch() {
  if (process.arch !== "x64" && process.arch !== "arm64") {
    throw new Error("Unsupported macOS build architecture: " + process.arch)
  }
  return process.arch
}

export const SHERPA_MACOS_SHARED_SOURCE_DIR = process.env.SHERPA_ONNX_LIB_DIR || nativeCacheDir(
  "sherpa-onnx-prebuilt",
  "sherpa-onnx-v" + SHERPA_VERSION + "-osx-" + macosArch() + "-shared-lib",
)

// Keep the version in sync with tauri.macos.conf.json; old staged dylibs must not overwrite new Cargo runtimes.
// Each build produces one arch, so a single dir is reused without collisions.
export const SHERPA_MACOS_SHARED_OUT_DIR = fromRuntime("macos-shared-libs-" + SHERPA_VERSION)

export const SHERPA_MACOS_REQUIRED_LIBS = [
  "libsherpa-onnx-c-api.dylib",
  "libonnxruntime.dylib",
]
export const SHERPA_MACOS_OPTIONAL_LIBS = ["libsherpa-onnx-cxx-api.dylib"]
