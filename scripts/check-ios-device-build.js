import { readFileSync } from "node:fs"
import { cp, mkdir, stat } from "node:fs/promises"
import { dirname, join } from "node:path"
import { SHERPA_IOS_DEVICE_SLICE } from "./lib/ios/constants.js"
import { ensureIosSherpaLibs, iosSherpaLibDir } from "./lib/ios/sherpa.js"
import { runSync } from "./lib/process.js"
import { ROOT, SRC_TAURI_DIR } from "./lib/paths.js"

const iosConfigPath = join(SRC_TAURI_DIR, "tauri.ios.conf.json")
const iosDeploymentTarget = readIosDeploymentTarget()
const appleProjectDir = join(SRC_TAURI_DIR, "gen", "apple")
const assetsDir = join(appleProjectDir, "assets")
const distDir = join(ROOT, "dist")
const cargoLibPath = join(SRC_TAURI_DIR, "target", "aarch64-apple-ios", "release", "libapp_lib.a")
const xcodeLibPath = join(appleProjectDir, "Externals", "arm64", "release", "libapp.a")

if (process.platform !== "darwin") {
  fail("iOS device checks require macOS with full Xcode. Use a GitHub macos-26 runner or MacInCloud.")
}

await ensureIosSherpaLibs({ includeSimulator: false })
await ensureAssets()
await buildRustDeviceLib()
await stageRustDeviceLib()

const args = [
  "-project",
  "app.xcodeproj",
  "-scheme",
  "app_iOS",
  "-configuration",
  "release",
  "-sdk",
  "iphoneos",
  "-destination",
  "generic/platform=iOS",
  "CODE_SIGNING_ALLOWED=NO",
  "CODE_SIGNING_REQUIRED=NO",
  "CODE_SIGN_IDENTITY=",
  "DEVELOPMENT_TEAM=",
  "build",
]

runOrFail("xcodebuild", args, {
  cwd: appleProjectDir,
  env: {
    ...process.env,
    PAPERCUT_SKIP_TAURI_IOS_XCODE_SCRIPT: "1",
  },
}, "[ios-device-check] Failed unsigned iOS device build: ")

// Tauri-generated iOS projects read frontend files from gen/apple/assets.
async function ensureAssets() {
  if (await isDir(assetsDir)) {
    return
  }
  if (!await isDir(distDir)) {
    fail("Missing frontend dist. Run npm run build before ios:ci:device.")
  }
  await mkdir(appleProjectDir, { recursive: true })
  await cp(distDir, assetsDir, { recursive: true })
}

// Build the Rust static library directly so xcodebuild can link without the Tauri CLI helper server.
// The deployment target must match Xcode, otherwise Swift package builds can default too low.
async function buildRustDeviceLib() {
  const env = {
    ...process.env,
    IPHONEOS_DEPLOYMENT_TARGET: iosDeploymentTarget,
    SHERPA_ONNX_LIB_DIR: iosSherpaLibDir(SHERPA_IOS_DEVICE_SLICE),
    ORT_LIB_LOCATION: iosSherpaLibDir(SHERPA_IOS_DEVICE_SLICE),
  }
  runOrFail("cargo", [
    "build",
    "--package",
    "app",
    "--manifest-path",
    join(SRC_TAURI_DIR, "Cargo.toml"),
    "--target",
    "aarch64-apple-ios",
    "--features",
    "native-tts-ios tauri/custom-protocol",
    "--lib",
    "--release",
  ], { env }, "[ios-device-check] Failed Rust iOS device build: ")
}

// Cargo names the staticlib from [lib] (`app_lib`), while Xcode expects Tauri's staged libapp.a.
async function stageRustDeviceLib() {
  if (!await isFile(cargoLibPath)) {
    fail("Missing Rust iOS static library after cargo build: " + cargoLibPath)
  }
  await mkdir(dirname(xcodeLibPath), { recursive: true })
  await cp(cargoLibPath, xcodeLibPath, { force: true })
}

function readIosDeploymentTarget() {
  const config = JSON.parse(readFileSync(iosConfigPath, "utf8"))
  const target = config.bundle?.iOS?.minimumSystemVersion
  if (!target) {
    fail("Missing bundle.iOS.minimumSystemVersion in " + iosConfigPath)
  }
  return target
}

async function isDir(path) {
  try {
    return (await stat(path)).isDirectory()
  } catch {
    return false
  }
}

async function isFile(path) {
  try {
    return (await stat(path)).isFile()
  } catch {
    return false
  }
}

function runOrFail(command, args, options, errorPrefix) {
  const result = runSync(command, args, options)
  if (result.error) {
    console.error(errorPrefix + result.error.message)
    process.exit(1)
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

function fail(message) {
  console.error("[ios-device-check] " + message)
  process.exit(1)
}
