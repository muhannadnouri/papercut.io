import { createHash } from "node:crypto"
import {
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { basename, join } from "node:path"
import { ROOT } from "./lib/paths.js"
import { runSync } from "./lib/process.js"

const DEFAULT_PART_BYTES = 1900 * 1000 * 1000
const options = parseArgs(process.argv.slice(2))
const target = options.target ?? currentTargetTriple()
if (target !== "x86_64-unknown-linux-gnu") {
  fail("Unsupported SILMA runtime target: " + target + ". SILMA runtime packs are currently Linux x64 only.")
}
const runtimeId = options.runtimeId ?? runtimeIdForTarget(target)
const sourceDir =
  options.sourceDir ?? join(ROOT, "sidecars", "silma", "runtime", target, "onedir")
const outputDir =
  options.outputDir ?? join(ROOT, "sidecars", "silma", "runtime", target, "archive")
const appRuntimeManifestPath = join(ROOT, "src-tauri", "tts", "silma-runtime-packs.json")
const exeBase = "silma-worker-" + target
const archiveName = options.archiveName ?? "papercut-silma-runtime-" + runtimeId + ".tar.bz2"
if (!archiveName.endsWith(".tar.bz2")) fail("--archive-name must end with .tar.bz2")
const archivePath = join(outputDir, archiveName)
const manifestPath = join(outputDir, archiveName.replace(/\.tar\.bz2$/, ".manifest.json"))
const workerRelativePath = exeBase + "/" + exeBase

if (!existsSync(join(sourceDir, workerRelativePath))) {
  fail("Missing prepared SILMA runtime worker at " + join(sourceDir, workerRelativePath))
}

mkdirSync(outputDir, { recursive: true })
rmSync(archivePath, { force: true })
rmSync(manifestPath, { force: true })
for (const fileName of readdirSync(outputDir)) {
  if (fileName.startsWith(archiveName + ".part")) rmSync(join(outputDir, fileName), { force: true })
}

const tar = runSync("tar", ["-cjf", archivePath, "-C", sourceDir, exeBase], { cwd: ROOT })
if (tar.error) fail("Failed to start tar: " + tar.error.message)
if (tar.status !== 0) process.exit(tar.status ?? 1)

const archiveBytes = statSync(archivePath).size
const archiveSha256 = await sha256File(archivePath)
const parts = archiveBytes > options.partBytes
  ? await splitArchive(archivePath, archiveName, options.partBytes, options.url)
  : []
const manifest = {
  id: "silma-runtime-" + runtimeId,
  runtimeId,
  platform: platformForTarget(target),
  arch: archForTarget(target),
  target,
  url: parts.length > 0 ? "" : options.url ?? "",
  archive: basename(archivePath),
  archiveBytes,
  sha256: archiveSha256,
  ...(parts.length > 0 ? { parts } : {}),
  workerPath: workerRelativePath,
}

writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n")
if (options.updateAppManifest) {
  if (!manifest.url && !manifest.parts?.every((part) => part.url)) fail("--update-app-manifest requires --url")
  updateAppRuntimeManifest(manifest)
}
if (manifest.parts) console.log("[silma-runtime-pack] split " + archivePath)
else console.log("[silma-runtime-pack] wrote " + archivePath)
console.log("[silma-runtime-pack] wrote " + manifestPath)
console.log("[silma-runtime-pack] sha256 " + manifest.sha256)
console.log("[silma-runtime-pack] bytes " + manifest.archiveBytes)
if (manifest.parts) console.log("[silma-runtime-pack] parts " + manifest.parts.length)

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
    } else if (arg === "--update-app-manifest") {
      parsed.updateAppManifest = true
    } else if (arg === "--part-bytes") {
      const value = Number(requireValue(args, ++index, arg))
      if (!Number.isSafeInteger(value) || value <= 0) fail("--part-bytes must be a positive integer")
      parsed.partBytes = value
    } else {
      fail("Unknown option: " + arg)
    }
  }
  parsed.partBytes ??= DEFAULT_PART_BYTES
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
  fail("Unsupported SILMA runtime target: " + platform + "/" + arch + ". SILMA runtime packs are currently Linux x64 only.")
}

function runtimeIdForTarget(target) {
  if (target === "x86_64-unknown-linux-gnu") return "linux-x64-cpu"
  fail("No default runtime id for " + target + ". SILMA runtime packs are currently Linux x64 only.")
}

function platformForTarget(target) {
  if (target.includes("linux")) return "linux"
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

// Split only when GitHub Release's per-asset limit would reject the full archive.
async function splitArchive(archivePath, archiveName, partBytes, archiveUrl) {
  const parts = []
  let partIndex = 1
  let remainingInPart = 0
  let writer = null
  let partPath = ""
  let partSize = 0

  const closePart = () => new Promise((resolvePart, reject) => {
    if (!writer) return resolvePart()
    writer.once("error", reject)
    writer.once("finish", resolvePart)
    writer.end()
  })

  const startPart = () => {
    const partName = archiveName + ".part" + String(partIndex).padStart(3, "0")
    partPath = join(outputDir, partName)
    writer = createWriteStream(partPath)
    remainingInPart = partBytes
    partSize = 0
    partIndex += 1
    return partName
  }

  let partName = startPart()
  for await (const chunk of createReadStream(archivePath)) {
    let offset = 0
    while (offset < chunk.length) {
      const size = Math.min(remainingInPart, chunk.length - offset)
      await writeChunk(writer, chunk.subarray(offset, offset + size))
      offset += size
      remainingInPart -= size
      partSize += size
      if (remainingInPart === 0) {
        await closePart()
        parts.push(partManifest(partName, partPath, archiveUrl))
        partName = startPart()
      }
    }
  }
  if (partSize > 0) {
    await closePart()
    parts.push(partManifest(partName, partPath, archiveUrl))
  } else {
    await closePart()
    rmSync(partPath, { force: true })
  }
  rmSync(archivePath, { force: true })
  return parts
}

// Respect stream backpressure so splitting a multi-GB archive stays memory-flat.
function writeChunk(writer, chunk) {
  return new Promise((resolveWrite, reject) => {
    const cleanup = () => {
      writer.off("error", onError)
      writer.off("drain", onDrain)
    }
    const onError = (err) => {
      cleanup()
      reject(err)
    }
    const onDrain = () => {
      cleanup()
      resolveWrite()
    }
    writer.once("error", onError)
    if (writer.write(chunk)) {
      cleanup()
      resolveWrite()
    } else {
      writer.once("drain", onDrain)
    }
  })
}

// Derive part URLs from the full-archive URL because release assets share a prefix.
function partManifest(partName, partPath, archiveUrl) {
  return {
    url: archiveUrl ? archiveUrl + "." + partName.split(".").at(-1) : "",
    archive: partName,
    bytes: statSync(partPath).size,
  }
}

// Release helper: copy generated artifact metadata into the app's checked manifest.
function updateAppRuntimeManifest(runtime) {
  const appManifest = JSON.parse(readFileSync(appRuntimeManifestPath, "utf8"))
  if (!Array.isArray(appManifest.runtimes)) {
    fail("Invalid app runtime manifest: missing runtimes array")
  }
  const entry = {
    runtimeId: runtime.runtimeId,
    platform: runtime.platform,
    arch: runtime.arch,
    target: runtime.target,
    url: runtime.url,
    archiveBytes: runtime.archiveBytes,
    sha256: runtime.sha256,
    ...(runtime.parts ? { parts: runtime.parts } : {}),
    workerPath: runtime.workerPath,
  }
  const index = appManifest.runtimes.findIndex((item) => item.runtimeId === runtime.runtimeId)
  if (index >= 0) appManifest.runtimes[index] = entry
  else appManifest.runtimes.push(entry)
  writeFileSync(appRuntimeManifestPath, JSON.stringify(appManifest, null, 2) + "\n")
  console.log("[silma-runtime-pack] updated " + appRuntimeManifestPath)
}

function fail(message) {
  console.error("[silma-runtime-pack] " + message)
  process.exit(1)
}
