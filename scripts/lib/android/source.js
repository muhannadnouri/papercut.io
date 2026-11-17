import {
  copyFile,
  mkdir,
  readFile,
  rm,
} from "node:fs/promises"
import { join } from "node:path"

import { checkElf } from "../../android/verify-android-apk.js"
import {
  source,
  output,
  receipt,
  SOURCE_CACHE,
} from "../../fdroid/source-build.js"
import {
  PIPER_SOURCE_REVISION,
  PIPER_VERSION,
  SHERPA_VERSION,
} from "../constants.js"
import { downloadFile } from "../download.js"
import {
  run,
  shQuote,
} from "../process.js"
import {
  androidBuildPathMaps,
  androidNdkDir,
} from "./build-env.js"
import {
  ANDROID_NDK_VERSION,
  ANDROID_ORT_AAR as ortAar,
  SHERPA_ANDROID_SOURCE_REVISION as sherpaRevision,
} from "./constants.js"

// Sherpa and Piper are compiled from source. ORT ships from the trusted Maven AAR the app
// already links at runtime; its source-commit history lives in earlier recipe
// revisions if an independent ORT build is ever needed again.
// Reused by developer, emulator and production preparation; each ABI shares the same pinned sources/AAR.
export async function buildAndroidSourceLibs({ abi = "arm64-v8a", destination } = {}) {
  if (!["arm64-v8a", "x86_64"].includes(abi)) {
    throw new Error("Unsupported source-build ABI: " + abi)
  }
  const ndkVersion = ANDROID_NDK_VERSION
  const ndk = androidNdkDir()

  const sherpa = await source("sherpa-onnx-" + SHERPA_VERSION, "https://github.com/k2-fsa/sherpa-onnx.git", sherpaRevision)

  // The AAR is a ZIP; only the C/C++ API headers and the selected ABI runtime are link inputs.
  const ortDir = join(SOURCE_CACHE, "ort-aar-" + ortAar.version)
  const ortArchive = join(ortDir, "onnxruntime-android-" + ortAar.version + ".aar")
  await mkdir(ortDir, { recursive: true })
  await downloadFile({
    url: ortAar.url,
    dest: ortArchive,
    sha256: ortAar.sha256,
    label: "ort-aar",
  })
  await rm(join(ortDir, "headers"), {
    recursive: true,
    force: true,
  })
  await run("unzip", ["-o", ortArchive, "headers/*", "jni/" + abi + "/libonnxruntime.so", "-d", ortDir])

  // External headers/library prevent Sherpa's default Android path from downloading prebuilt ORT.
  const env = {
    ...process.env,
    SHERPA_ONNXRUNTIME_INCLUDE_DIR: join(ortDir, "headers"),
    SHERPA_ONNXRUNTIME_LIB_DIR: join(ortDir, "jni/" + abi),
  }
  // CMake caches absolute compiler paths; another NDK must start with a fresh build tree.
  const build = join(SOURCE_CACHE, "sherpa-build-" + SHERPA_VERSION + "-" + abi + "-" + ndkVersion)
  const prefixMaps = androidBuildPathMaps()
    .map(([from, to]) => "-ffile-prefix-map=" + shQuote(`${from}=${to}`)).join(" ")
  await run("cmake", ["-S", sherpa, "-B", build,
    "-DCMAKE_TOOLCHAIN_FILE=" + join(ndk, "build/cmake/android.toolchain.cmake"),
    "-DANDROID_ABI=" + abi, "-DANDROID_PLATFORM=android-26", "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_C_FLAGS=" + prefixMaps, "-DCMAKE_CXX_FLAGS=" + prefixMaps,
    // Strip shipping symbols before the linker hashes them into the retained build ID.
    "-DCMAKE_SHARED_LINKER_FLAGS=-Wl,--strip-all",
    "-DBUILD_SHARED_LIBS=ON", "-DSHERPA_ONNX_ENABLE_TTS=ON", "-DSHERPA_ONNX_ENABLE_C_API=ON",
    "-DCMAKE_POLICY_VERSION_MINIMUM=3.5", "-DCMAKE_POLICY_DEFAULT_CMP0169=OLD",
    ...["PYTHON", "TESTS", "CHECK", "PORTAUDIO", "JNI", "WEBSOCKET", "BINARY", "SPEAKER_DIARIZATION"].map(flag => "-DSHERPA_ONNX_ENABLE_" + flag + "=OFF"),
  ], { env })
  await run("cmake", ["--build", build, "--target", "sherpa-onnx-c-api", "--parallel", "2"], { env })

  destination ??= join(SOURCE_CACHE, abi === "arm64-v8a" ? "source-arm64" : "source-" + abi)
  await mkdir(destination, { recursive: true })
  // Cargo links Sherpa, Piper and ORT from this directory. Staging omits ORT; Gradle packages it.
  const piper = await source("piper-phonemize-" + PIPER_VERSION, "https://github.com/csukuangfj/piper-phonemize.git", PIPER_SOURCE_REVISION)
  const piperBuild = join(SOURCE_CACHE, "piper-build-" + PIPER_VERSION + "-" + abi + "-" + ndkVersion)
  await run("cmake", ["-S", piper, "-B", piperBuild,
    "-DCMAKE_TOOLCHAIN_FILE=" + join(ndk, "build/cmake/android.toolchain.cmake"),
    "-DANDROID_ABI=" + abi, "-DANDROID_PLATFORM=android-26", "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_C_FLAGS=" + prefixMaps, "-DCMAKE_CXX_FLAGS=" + prefixMaps,
    "-DCMAKE_SHARED_LINKER_FLAGS=-Wl,--strip-all", "-DBUILD_SHARED_LIBS=ON",
    "-DBUILD_PIPER_PHONEMIZE_PYTHON=OFF", "-DBUILD_PIPER_PHONEMIZE_JNI=OFF",
    "-DBUILD_PIPER_PHONEMIZE_CORE_TESTS=OFF", "-DBUILD_PIPER_PHONEMIZE_CORE_EXE=OFF",
    "-DCMAKE_POLICY_VERSION_MINIMUM=3.5", "-DCMAKE_POLICY_DEFAULT_CMP0169=OLD",
  ])
  await run("cmake", ["--build", piperBuild, "--target", "piper_phonemize_core", "--parallel", "2"])
  const piperTarget = join(destination, "libpiper_phonemize_core.so")
  await copyFile(join(piperBuild, "lib/libpiper_phonemize_core.so"), piperTarget)
  checkElf(await readFile(piperTarget), "libpiper_phonemize_core.so", abi)
  const target = join(destination, "libsherpa-onnx-c-api.so")
  await copyFile(join(build, "lib/libsherpa-onnx-c-api.so"), target)
  await run(join(ndk, "toolchains/llvm/prebuilt/" + (process.platform === "darwin" ? "darwin-x86_64" : process.platform === "win32" ? "windows-x86_64" : "linux-x86_64") + "/bin/llvm-strip" + (process.platform === "win32" ? ".exe" : "")), ["--strip-unneeded", target])
  checkElf(await readFile(target), "libsherpa-onnx-c-api.so", abi)
  // Kept byte-identical to the pinned artifact; its packaged copy comes from the same AAR.
  const ortRuntime = join(destination, "libonnxruntime.so")
  await copyFile(join(ortDir, "jni/" + abi + "/libonnxruntime.so"), ortRuntime)
  checkElf(await readFile(ortRuntime), "libonnxruntime.so", abi)
  await receipt(destination, {
    sherpa: sherpaRevision,
    piper: PIPER_SOURCE_REVISION,
    ortAar,
  }, ["libsherpa-onnx-c-api.so", "libonnxruntime.so", "libpiper_phonemize_core.so"], {
    ndk: ndkVersion,
    api: 26,
    abi,
    cmake: output("cmake", ["--version"]).split("\n")[0],
  })
  console.log("Source-built " + abi + " runtime (not automatically staged into the app): " + destination)

  return destination
}
