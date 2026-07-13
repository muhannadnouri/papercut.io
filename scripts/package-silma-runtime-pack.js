import { createHash } from "node:crypto"
import { createReadStream, existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { basename, join } from "node:path"
import { ROOT } from "./lib/paths.js"
import { runSync } from "./lib/process.js"

const options = parseArgs(process.argv.slice(2))
const target = options.target ?? currentTargetTriple()
const runtimeId = options.runtimeId ?? runtimeIdForTarget(target)
const sourceDir =
  options.sourceDir ?? join(ROOT, "sidecars", "silma", "runtime", target, "onedir")
const outputDir =
  options.outputDir ?? join(ROOT, "sidecars", "silma", "runtime", target, "archive")
const exeBase = "silma-worker-" + target
const archiveName = options.archiveName ?? "papercut-silma-runtime-" + runtimeId + ".tar.bz2"
const archivePath = join(outputDir, archiveName)
const manifestPath = join(outputDir, archiveName.replace(/\.tar\.bz2$/, ".manifest.json"))
const workerRelativePath = exeBase + "/" + exeBase + (target.includes("windows") ? ".exe" : "")

if (!existsSync(join(sourceDir, workerRelativePath))) {
  fail("Missing prepared SILMA runtime worker at " + join(sourceDir, workerRelativePath))
}

mkdirSync(outputDir, { recursive: true })
rmSync(archivePath, { force: true })
rmSync(manifestPath, { force: true })

const tar = runSync("tar", ["-cjf", archivePath, "-C", sourceDir, exeBase], { cwd: ROOT })
if (tar.error) fail("Failed to start tar: " + tar.error.message)
if (tar.status !== 0) process.exit(tar.status ?? 1)

const manifest = {
  id: "silma-runtime-" + runtimeId,
  runtimeId,
  platform: platformForTarget(target),
  arch: archForTarget(target),
  target,
  url: options.url ?? "",
  archive: basename(archivePath),
  archiveBytes: statSync(archivePath).size,
  sha256: await sha256File(archivePath),
  workerPath: workerRelativePath,
}

writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n")
console.log("[silma-runtime-pack] wrote " + archivePath)
console.log("[silma-runtime-pack] wrote " + manifestPath)
console.log("[silma-runtime-pack] sha256 " + manifest.sha256)
console.log("[silma-runtime-pack] bytes " + manifest.archiveBytes)

function parseArgs(args) {
  const parsed = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--target") {
      parsed.target = requireValue(args, ++index, arg)
    } else if (arg === "--runtime-id") {
      parsed.runtimeId = requireValue(args, ++index, arg)
    } else if (arg === "--source-dir") {
      parsed.sourceDir = requireValue(args, ++index, arg)
    } else if (arg === "--output-dir") {
      parsed.outputDir = requireValue(args, ++index, arg)
    } else if (arg === "--archive-name") {
      parsed.archiveName = requireValue(args, ++index, arg)
    } else if (arg === "--url") {
      parsed.url = requireValue(args, ++index, arg)
    } else {
      fail("Unknown option: " + arg)
    }
  }
  return parsed
}

function requireValue(args, index, flag) {
  const value = args[index]
  if (!value || value.startsWith("--")) {
    fail("Missing value for " + flag)
  }
  return value
}

function currentTargetTriple() {
  const platform = process.platform
  const arch = process.arch
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-gnu"
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin"
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin"
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc"
  fail("Unsupported SILMA runtime target: " + platform + "/" + arch + ". Pass --target.")
}

function runtimeIdForTarget(target) {
  if (target === "x86_64-unknown-linux-gnu") return "linux-x64-cpu"
  if (target === "x86_64-apple-darwin") return "macos-x64-cpu"
  if (target === "aarch64-apple-darwin") return "macos-aarch64-cpu"
  if (target === "x86_64-pc-windows-msvc") return "windows-x64-cpu"
  fail("No default runtime id for " + target + ". Pass --runtime-id.")
}

function platformForTarget(target) {
  if (target.includes("linux")) return "linux"
  if (target.includes("apple-darwin")) return "macos"
  if (target.includes("windows")) return "windows"
  return "unknown"
}

function archForTarget(target) {
  if (target.startsWith("x86_64")) return "x64"
  if (target.startsWith("aarch64")) return "arm64"
  return "unknown"
}

function sha256File(file) {
  return new Promise((resolveHash, reject) => {
    const hash = createHash("sha256")
    const stream = createReadStream(file)
    stream.on("error", reject)
    stream.on("data", (chunk) => hash.update(chunk))
    stream.on("end", () => resolveHash(hash.digest("hex")))
  })
}

function fail(message) {
  console.error("[silma-runtime-pack] " + message)
  process.exit(1)
}
