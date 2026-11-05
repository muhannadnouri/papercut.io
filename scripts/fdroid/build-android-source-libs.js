import assert from "node:assert/strict"
import {
  copyFile,
  mkdir,
  readFile,
} from "node:fs/promises"
import { join } from "node:path"
import {
  run,
  shQuote,
} from "../lib/process.js"
import { androidBuildPathMaps } from "../lib/android/build-env.js"
import { ANDROID_NDK_VERSION } from "../lib/android/constants.js"
import { downloadFile } from "../lib/download.js"
import {
  source,
  output,
  receipt,
  SOURCE_CACHE,
} from "./source-build.js"
import { checkElf } from "../android/verify-android-apk.js"

// Only Sherpa is compiled from source. ORT ships from the trusted Maven AAR the app
// already links at runtime; its source-commit history lives in earlier recipe
// revisions if an independent ORT build is ever needed again.
const sherpaRevision = "142807252687d81b40d6315f23470a1512a00de3"
const ortAar = {
  version: "1.27.0",
  url: "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/1.27.0/onnxruntime-android-1.27.0.aar",
  sha256: "077dec5e2d821234c7dc0aba584bec8f999854b546c754cab93a90741c56fbeb",
}
const ndkVersion = ANDROID_NDK_VERSION
const sdk = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT
assert(sdk, "Set ANDROID_HOME or ANDROID_SDK_ROOT")
const ndk = process.env.NDK_HOME || join(sdk, "ndk", ndkVersion)
assert((await readFile(join(ndk, "source.properties"), "utf8")).includes("Pkg.Revision = " + ndkVersion), "Expected NDK " + ndkVersion)

const sherpa = await source("sherpa-onnx", "https://github.com/k2-fsa/sherpa-onnx.git", sherpaRevision)

// The AAR is a ZIP; only the C/C++ API headers and the arm64 runtime are link inputs.
const ortDir = join(SOURCE_CACHE, "ort-aar-" + ortAar.version)
const ortArchive = join(ortDir, "onnxruntime-android-" + ortAar.version + ".aar")
await mkdir(ortDir, { recursive: true })
await downloadFile({
  url: ortAar.url,
  dest: ortArchive,
  sha256: ortAar.sha256,
  label: "ort-aar",
})
await run("unzip", ["-o", ortArchive, "headers/*", "jni/arm64-v8a/libonnxruntime.so", "-d", ortDir])

// External headers/library prevent Sherpa's default Android path from downloading prebuilt ORT.
const env = {
  ...process.env,
  SHERPA_ONNXRUNTIME_INCLUDE_DIR: join(ortDir, "headers"),
  SHERPA_ONNXRUNTIME_LIB_DIR: join(ortDir, "jni/arm64-v8a"),
}
const build = join(SOURCE_CACHE, "sherpa-build")
const prefixMaps = androidBuildPathMaps()
  .map(([from, to]) => "-ffile-prefix-map=" + shQuote(`${from}=${to}`)).join(" ")
await run("cmake", ["-S", sherpa, "-B", build,
  "-DCMAKE_TOOLCHAIN_FILE=" + join(ndk, "build/cmake/android.toolchain.cmake"),
  "-DANDROID_ABI=arm64-v8a", "-DANDROID_PLATFORM=android-26", "-DCMAKE_BUILD_TYPE=Release",
  "-DCMAKE_C_FLAGS=" + prefixMaps, "-DCMAKE_CXX_FLAGS=" + prefixMaps,
  "-DBUILD_SHARED_LIBS=ON", "-DSHERPA_ONNX_ENABLE_TTS=ON", "-DSHERPA_ONNX_ENABLE_C_API=ON",
  "-DCMAKE_POLICY_VERSION_MINIMUM=3.5", "-DCMAKE_POLICY_DEFAULT_CMP0169=OLD",
  ...["PYTHON", "TESTS", "CHECK", "PORTAUDIO", "JNI", "WEBSOCKET", "BINARY", "SPEAKER_DIARIZATION"].map(flag => "-DSHERPA_ONNX_ENABLE_" + flag + "=OFF"),
], { env })
await run("cmake", ["--build", build, "--target", "sherpa-onnx-c-api", "--parallel", "2"], { env })

const destination = join(SOURCE_CACHE, "source-arm64")
await mkdir(destination, { recursive: true })
// sherpa-onnx-sys links both libraries from SHERPA_ONNX_LIB_DIR, so this directory doubles
// as the Rust link-input set. Staging never copies the AAR runtime; Gradle packages it.
const target = join(destination, "libsherpa-onnx-c-api.so")
await copyFile(join(build, "lib/libsherpa-onnx-c-api.so"), target)
await run(join(ndk, "toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"), ["--strip-unneeded", target])
checkElf(await readFile(target), "libsherpa-onnx-c-api.so")
// Kept byte-identical to the pinned artifact; its packaged copy comes from the same AAR.
const ortRuntime = join(destination, "libonnxruntime.so")
await copyFile(join(ortDir, "jni/arm64-v8a/libonnxruntime.so"), ortRuntime)
checkElf(await readFile(ortRuntime), "libonnxruntime.so")
await receipt(destination, {
  sherpa: sherpaRevision,
  ortAar,
}, ["libsherpa-onnx-c-api.so", "libonnxruntime.so"], {
  ndk: ndkVersion,
  api: 26,
  cmake: output("cmake", ["--version"]).split("\n")[0],
})
console.log("Source-built arm64 runtime (not automatically staged into the app): " + destination)
