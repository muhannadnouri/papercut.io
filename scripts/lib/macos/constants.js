import { join } from "node:path"
import { SHERPA_VERSION } from "../constants.js"
import { fromRuntime, SRC_TAURI_DIR } from "../paths.js"

export { SHERPA_VERSION }

// sherpa-onnx-sys downloads per-arch macOS shared archives and extracts them
// under src-tauri/target/sherpa-onnx-prebuilt/. Node's process.arch matches the
// native build target on GitHub macOS runners (x64 on macos-13, arm64 on macos-14).
export function macosArch() {
  if (process.arch !== "x64" && process.arch !== "arm64") {
    throw new Error("Unsupported macOS build architecture: " + process.arch)
  }
  return process.arch
}

export const SHERPA_MACOS_SHARED_SOURCE_DIR = join(
  SRC_TAURI_DIR,
  "target",
  "sherpa-onnx-prebuilt",
  "sherpa-onnx-v" + SHERPA_VERSION + "-osx-" + macosArch() + "-shared-lib",
  "lib",
)

// Fixed staging name so tauri.macos.conf.json can reference it statically.
// Each build produces one arch, so a single dir is reused without collisions.
export const SHERPA_MACOS_SHARED_OUT_DIR = fromRuntime("macos-shared-libs")

export const SHERPA_MACOS_REQUIRED_LIBS = [
  "libsherpa-onnx-c-api.dylib",
  "libonnxruntime.dylib",
]
export const SHERPA_MACOS_OPTIONAL_LIBS = ["libsherpa-onnx-cxx-api.dylib"]
