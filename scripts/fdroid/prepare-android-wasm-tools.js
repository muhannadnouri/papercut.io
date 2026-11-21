import assert from "node:assert/strict"
import { rm } from "node:fs/promises"
import { join } from "node:path"
import { ANDROID_RUST_TOOLCHAIN } from "../lib/android/constants.js"
import { extractTar } from "../lib/archive.js"
import { downloadFile } from "../lib/download.js"
import { run } from "../lib/process.js"
import {
  source,
  SOURCE_CACHE,
} from "./source-build.js"

assert(process.platform === "linux" && process.arch === "x64", "The Android production toolchain requires Linux x86-64")
const emsdk = await source("emsdk", "https://github.com/emscripten-core/emsdk.git", "c0bb220cb6e6f4e0fabb6f6db9efd53390ef5e56")
const emscriptenBuild = "b412b6307e541b93dd93f01b61181e15c17302ec"
const emsdkNode = "node-v22.16.0-linux-x64.tar.xz"
// Use the filenames the pinned installer reads with EMSDK_KEEP_DOWNLOADS.
await downloadFile({
  url: "https://storage.googleapis.com/webassembly/emscripten-releases-builds/linux/" + emscriptenBuild + "/wasm-binaries.tar.xz",
  dest: join(emsdk, "downloads", emscriptenBuild + "-wasm-binaries.tar.xz"),
  // Published by the pinned emsdk revision in bazel/revisions.bzl.
  sha256: "c0ed25c30e1d747072de0c9053cde27ca83a8d2424e8bc6b7d39dccf42fcba35",
  label: "emscripten",
})
await downloadFile({
  url: "https://nodejs.org/dist/v22.16.0/" + emsdkNode,
  dest: join(emsdk, "downloads", emsdkNode),
  // Node.js v22.16.0 signed SHASUMS256.txt.
  sha256: "f4cb75bb036f0d0eddf6b79d9596df1aaab9ddccd6a20bf489be5abe9467e84e",
  label: "emsdk-node",
})
// Reinstall from the verified archives instead of trusting an old extracted SDK.
await rm(join(emsdk, "upstream"), {
  recursive: true,
  force: true,
})
await rm(join(emsdk, "node"), {
  recursive: true,
  force: true,
})
await run(join(emsdk, "emsdk"), ["install", "4.0.15"], {
  cwd: emsdk,
  env: {
    ...process.env,
    EMSDK_KEEP_DOWNLOADS: "1",
  },
})
await run(join(emsdk, "emsdk"), ["activate", "4.0.15"], { cwd: emsdk })

const archive = await downloadFile({
  url: "https://github.com/Kitware/CMake/releases/download/v4.4.5/cmake-4.4.5-linux-x86_64.tar.gz",
  dest: join(SOURCE_CACHE, "cmake-4.4.5-linux-x86_64.tar.gz"),
  sha256: "ed85135c5a5135b45e50423cb406dea04ca3338da0b8ef3c9ede963559680be0",
  label: "release-cmake",
})
await extractTar({
  archive,
  destination: SOURCE_CACHE,
  compression: "gzip",
})
await run("rustup", ["target", "add", "--toolchain", ANDROID_RUST_TOOLCHAIN, "wasm32-unknown-unknown"])
await run("cargo", ["+" + ANDROID_RUST_TOOLCHAIN, "install", "wasm-bindgen-cli", "--version", "0.2.100", "--locked",
  "--root", join(SOURCE_CACHE, "wasm-tools"), "--target-dir", join(SOURCE_CACHE, "wasm-tools-target")])
