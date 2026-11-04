import assert from "node:assert/strict"
import { existsSync } from "node:fs"
import {
  copyFile,
  mkdir,
  readFile,
} from "node:fs/promises"
import { join } from "node:path"
import { run } from "../lib/process.js"
import {
  source,
  output,
  receipt,
  SOURCE_CACHE,
} from "./source-build.js"
import { checkElf } from "../android/verify-android-apk.js"

const revisions = {
  onnxruntime: "8f0278c77bf44b0cc83c098c6c722b92a36ac4b5",
  sherpa: "142807252687d81b40d6315f23470a1512a00de3",
}
const ndkVersion = "29.0.13846066"
const sdk = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT
assert(sdk, "Set ANDROID_HOME or ANDROID_SDK_ROOT")
const ndk = process.env.NDK_HOME || join(sdk, "ndk", ndkVersion)
assert((await readFile(join(ndk, "source.properties"), "utf8")).includes("Pkg.Revision = " + ndkVersion), "Expected NDK " + ndkVersion)

const ort = await source("onnxruntime", "https://github.com/microsoft/onnxruntime.git", revisions.onnxruntime, true)
const sherpa = await source("sherpa-onnx", "https://github.com/k2-fsa/sherpa-onnx.git", revisions.sherpa)
const venv = join(SOURCE_CACHE, "ort-venv")
const python = join(venv, "bin/python")
if (!existsSync(python)) {
  await run("python3", ["-m", "venv", venv])
}
// Pin the small build-only Python set; do not let ORT's helper install floating requirements.
const pythonPackages = ["flatbuffers==25.12.19", "numpy==2.5.4", "packaging==26.3", "protobuf==7.36.2"]
await run(python, ["-m", "pip", "install", "--disable-pip-version-check", ...pythonPackages])
const ortBuild = join(SOURCE_CACHE, "ort-build")
await run(python, [join(ort, "tools/ci_build/build.py"),
  "--config", "Release", "--android", "--android_abi", "arm64-v8a", "--android_api", "26",
  "--android_sdk_path", sdk, "--android_ndk_path", ndk,
  "--build_shared_lib", "--skip_tests", "--skip_pip_install", "--skip_submodule_sync", "--parallel", "2",
  "--cmake_generator", "Unix Makefiles", "--build_dir", ortBuild,
  "--cmake_extra_defines", "onnxruntime_BUILD_UNIT_TESTS=OFF",
])

// External headers/library prevent Sherpa's default Android path from downloading prebuilt ORT.
const env = {
  ...process.env,
  SHERPA_ONNXRUNTIME_INCLUDE_DIR: join(ort, "include/onnxruntime/core/session"),
  SHERPA_ONNXRUNTIME_LIB_DIR: join(ortBuild, "Release"),
}
const build = join(SOURCE_CACHE, "sherpa-build")
await run("cmake", ["-S", sherpa, "-B", build,
  "-DCMAKE_TOOLCHAIN_FILE=" + join(ndk, "build/cmake/android.toolchain.cmake"),
  "-DANDROID_ABI=arm64-v8a", "-DANDROID_PLATFORM=android-26", "-DCMAKE_BUILD_TYPE=Release",
  "-DBUILD_SHARED_LIBS=ON", "-DSHERPA_ONNX_ENABLE_TTS=ON", "-DSHERPA_ONNX_ENABLE_C_API=ON",
  "-DCMAKE_POLICY_VERSION_MINIMUM=3.5", "-DCMAKE_POLICY_DEFAULT_CMP0169=OLD",
  ...["PYTHON", "TESTS", "CHECK", "PORTAUDIO", "JNI", "WEBSOCKET", "BINARY", "SPEAKER_DIARIZATION"].map(flag => "-DSHERPA_ONNX_ENABLE_" + flag + "=OFF"),
], { env })
await run("cmake", ["--build", build, "--target", "sherpa-onnx-c-api", "--parallel", "2"], { env })

const destination = join(SOURCE_CACHE, "source-arm64")
await mkdir(destination, { recursive: true })
for (const [name, origin] of [
  ["libonnxruntime.so", join(ortBuild, "Release/libonnxruntime.so")],
  ["libsherpa-onnx-c-api.so", join(build, "lib/libsherpa-onnx-c-api.so")],
]) {
  const target = join(destination, name)
  await copyFile(origin, target)
  await run(join(ndk, "toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"), ["--strip-unneeded", target])
  checkElf(await readFile(target), name)
}
await receipt(destination, revisions, ["libonnxruntime.so", "libsherpa-onnx-c-api.so"], {
  ndk: ndkVersion,
  api: 26,
  cmake: output("cmake", ["--version"]).split("\n")[0],
  pythonPackages,
})
console.log("Source-built arm64 runtime (not automatically staged into the app): " + destination)
