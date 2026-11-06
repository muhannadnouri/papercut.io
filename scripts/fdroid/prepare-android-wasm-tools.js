import { join } from "node:path"
import { ANDROID_RUST_TOOLCHAIN } from "../lib/android/constants.js"
import { extractTar } from "../lib/archive.js"
import { downloadFile } from "../lib/download.js"
import { run } from "../lib/process.js"
import {
  source,
  SOURCE_CACHE,
} from "./source-build.js"

const emsdk = await source("emsdk", "https://github.com/emscripten-core/emsdk.git", "c0bb220cb6e6f4e0fabb6f6db9efd53390ef5e56")
await run(join(emsdk, "emsdk"), ["install", "4.0.15"], { cwd: emsdk })
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
