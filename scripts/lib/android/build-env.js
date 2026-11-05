import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { homedir } from "node:os"
import {
  join,
  resolve,
} from "node:path"
import {
  ANDROID_NDK_VERSION,
  ANDROID_RUST_TOOLCHAIN,
} from "./constants.js"
import { ensureLocalJdk } from "./jdk.js"
import {
  pathSeparator,
  run,
} from "../process.js"
import { ROOT } from "../paths.js"
import { tauriCommand } from "../tauri.js"
import { productionBuildRevision } from "../build-revision.js"

export { ensureLocalJdk }

// Tauri otherwise selects its own default NDK; every Android entry point uses this pin.
export function androidNdkDir(env = process.env) {
  const sdk = env.ANDROID_HOME || env.ANDROID_SDK_ROOT
  assert(sdk, "Set ANDROID_HOME or ANDROID_SDK_ROOT")
  const directory = resolve(env.NDK_HOME || env.ANDROID_NDK_HOME || env.ANDROID_NDK
    || join(sdk, "ndk", ANDROID_NDK_VERSION))
  const revision = readFileSync(join(directory, "source.properties"), "utf8")
    .match(/^Pkg\.Revision\s*=\s*(.+)$/m)?.[1].trim()
  assert.equal(revision, ANDROID_NDK_VERSION, "Android builds require NDK " + ANDROID_NDK_VERSION)
  return directory
}

// More specific paths must win when Cargo caches or the NDK live inside the checkout.
export function androidBuildPathMaps() {
  return [
    [ROOT, "/build/papercut"],
    [process.env.CARGO_HOME || join(homedir(), ".cargo"), "/cargo"],
    [process.env.RUSTUP_HOME || join(homedir(), ".rustup"), "/rustup"],
    [androidNdkDir(), "/android-ndk"],
  ].sort(([a], [b]) => a.length - b.length)
}

export function androidProductionEnv() {
  const maps = androidBuildPathMaps()
  const env = {
    ...process.env,
    RUSTUP_TOOLCHAIN: ANDROID_RUST_TOOLCHAIN,
  }
  const options = {
    env,
    encoding: "utf8",
  }
  const compiler = process.env.RUSTC || "rustc"
  const version = execFileSync(compiler, ["-vV"], options)
  assert.equal(version.match(/^release: (.+)$/m)?.[1], ANDROID_RUST_TOOLCHAIN,
    "Android production builds require the pinned Rust toolchain")
  const commit = version.match(/^commit-hash: ([a-f0-9]{40})$/m)?.[1]
  assert(commit, "Rust compiler commit hash is missing")
  const sysroot = execFileSync(compiler, ["--print", "sysroot"], options).trim()
  // Installed rust-src can turn libstd's virtual paths into machine-local paths.
  maps.push([join(sysroot, "lib/rustlib/src/rust"), "/rustc/" + commit])
  const flags = process.env.CARGO_ENCODED_RUSTFLAGS?.split("\x1f")
    ?? process.env.RUSTFLAGS?.split(/\s+/) ?? []
  return {
    RUSTUP_TOOLCHAIN: ANDROID_RUST_TOOLCHAIN,
    PAPERCUT_BUILD_REVISION: productionBuildRevision(),
    CARGO_ENCODED_RUSTFLAGS: [
      ...flags.filter(Boolean),
      // shortcut: encoded flags replace cargo-mobile's link flags; recheck these libraries on CLI upgrades.
      "-Clink-arg=-landroid", "-Clink-arg=-llog", "-Clink-arg=-lOpenSLES",
      ...maps.map(([from, to]) => `--remap-path-prefix=${from}=${to}`),
    ].join("\x1f"),
  }
}

// Force Android builds onto a known JDK while still allowing extra native env.
export async function androidBuildEnv(extra = {}) {
  const ndk = androidNdkDir({
    ...process.env,
    ...extra,
  })
  const javaHome = await ensureLocalJdk()
  return {
    ...process.env,
    JAVA_HOME: javaHome,
    PATH: join(javaHome, "bin") + pathSeparator() + process.env.PATH,
    ...extra,
    NDK_HOME: ndk,
    ANDROID_NDK_HOME: ndk,
    ANDROID_NDK: ndk,
  }
}

// One Android entry path keeps normal and native-TTS APK builds consistent.
export async function runTauriAndroidBuild(args, extraEnv = {}) {
  const env = await androidBuildEnv(extraEnv)
  const tauri = tauriCommand(args)
  await run(tauri.command, tauri.args, {
    cwd: ROOT,
    env,
    label: "tauri " + args.join(" "),
  })
}
