import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { SHERPA_IOS_DEVICE_SLICE, SHERPA_IOS_SIMULATOR_ARM64_SLICE } from "./lib/ios/constants.js"
import { ensureIosSherpaLibs, iosSherpaLibDir } from "./lib/ios/sherpa.js"
import { exitFromResult, runSync } from "./lib/process.js"
import { SRC_TAURI_DIR } from "./lib/paths.js"
import { tauriCommand } from "./lib/tauri.js"

const initProject = process.argv.includes("--init")
const nativeTts = process.argv.includes("--native-tts")
const ciCheck = process.argv.includes("--ci-check")
const rawArgs = process.argv.slice(2)
const extraArgs = rawArgs.filter((arg) => arg !== "--init" && arg !== "--native-tts" && arg !== "--ci-check")
const requestedTarget = optionValue(extraArgs, "--target")
const effectiveTarget = ciCheck ? "aarch64-sim" : requestedTarget
const forwardedExtraArgs = ciCheck ? withoutOption(extraArgs, "--target") : extraArgs
const appleProjectDir = join(SRC_TAURI_DIR, "gen", "apple")
const iosConfigPath = join(SRC_TAURI_DIR, "tauri.ios.conf.json")
const expectedIosBundleId = "io.papercut.app"

verifyIosBundleId()

if (process.platform !== "darwin") {
  fail("iOS builds require macOS with full Xcode. Use a GitHub macos-26 runner or MacInCloud; Linux cannot run tauri ios build.")
}

if (ciCheck && initProject) {
  fail("Use either --ci-check or --init, not both.")
}

if (ciCheck && requestedTarget && requestedTarget !== "aarch64-sim") {
  fail("--ci-check only supports --target aarch64-sim, got " + requestedTarget)
}

if (initProject) {
  runTauriIos(["ios", "init", ...forwardedExtraArgs], "[ios-build] Failed to initialize Tauri iOS project: ")
} else {
  if (!existsSync(appleProjectDir)) {
    fail("Missing " + appleProjectDir + ". Run npm run ios:init on macOS, commit src-tauri/gen/apple, then rerun npm run ios:ipa.")
  }

  const env = { ...process.env }
  const featureArgs = []
  if (nativeTts) {
    const slice = sherpaSliceForTarget(effectiveTarget)
    await ensureIosSherpaLibs({ includeSimulator: slice === SHERPA_IOS_SIMULATOR_ARM64_SLICE })
    const libDir = iosSherpaLibDir(slice)
    env.SHERPA_ONNX_LIB_DIR = libDir
    env.ORT_LIB_LOCATION = libDir
    featureArgs.push("--features", "native-tts-ios")
    console.log("[ios-build] native TTS enabled with SHERPA_ONNX_LIB_DIR=" + env.SHERPA_ONNX_LIB_DIR)
  }

  const args = ciCheck
    ? ["ios", "build", "--target", "aarch64-sim", ...featureArgs, ...forwardedExtraArgs]
    : extraArgs.length > 0
      ? ["ios", "build", ...featureArgs, ...forwardedExtraArgs]
      : ["ios", "build", "--export-method", "app-store-connect", ...featureArgs]

  runTauriIos(args, "[ios-build] Failed to build iOS IPA: ", env)
}

// Match Rust's sherpa library dir to Tauri's effective iOS target.
function sherpaSliceForTarget(target) {
  if (!target || target === "aarch64") {
    return SHERPA_IOS_DEVICE_SLICE
  }
  if (target === "aarch64-sim") {
    return SHERPA_IOS_SIMULATOR_ARM64_SLICE
  }
  fail("Native TTS does not have prepared sherpa iOS libraries for --target " + target)
}

function optionValue(args, name) {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === name) {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) {
        fail(name + " requires a value")
      }
      return value
    }
    if (arg.startsWith(name + "=")) {
      return arg.slice(name.length + 1)
    }
  }
  return null
}

function withoutOption(args, name) {
  const filtered = []
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === name) {
      index += 1
      continue
    }
    if (arg.startsWith(name + "=")) {
      continue
    }
    filtered.push(arg)
  }
  return filtered
}

// Keep CI/release pointed at the App Store Connect bundle id, not the desktop id.
function verifyIosBundleId() {
  if (!existsSync(iosConfigPath)) {
    fail("Missing iOS Tauri config: " + iosConfigPath)
  }

  const config = JSON.parse(readFileSync(iosConfigPath, "utf8"))
  if (config.identifier !== expectedIosBundleId) {
    fail("Expected iOS Bundle ID " + expectedIosBundleId + " in " + iosConfigPath + ", got " + config.identifier)
  }
}

// Pass SHERPA_ONNX_LIB_DIR into Tauri so the Xcode Rust phase inherits the same slice.
function runTauriIos(args, errorPrefix, env = process.env) {
  const { command, args: tauriArgs } = tauriCommand(args)
  const result = runSync(command, tauriArgs, { env })
  exitFromResult(result, errorPrefix)
}

function fail(message) {
  console.error("[ios-build] " + message)
  process.exit(1)
}
