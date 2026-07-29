import { existsSync, readdirSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { spawnSync } from "node:child_process"

const requireSignatures = process.argv.includes("--require-signatures")
const appPath = resolve(process.argv.find((arg) => arg.endsWith(".app")) ?? "src-tauri/target/release/bundle/macos/Papercut.app")
const contentsDir = join(appPath, "Contents")
const macosDir = join(contentsDir, "MacOS")
const resourcesDir = join(contentsDir, "Resources")
const appBinary = join(macosDir, "app")

const requiredDylibs = ["libsherpa-onnx-c-api.dylib", "libonnxruntime.dylib"]
const versionedOnnxRuntime = /^libonnxruntime\.\d.*\.dylib$/

const failures = []

// Records a verifier failure while keeping the run going so CI shows all missing pieces.
function fail(message) {
  failures.push(message)
  console.error("[verify-macos-bundle-libs] " + message)
}

// Runs a macOS command and returns its result for explicit status handling.
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options })
  if (result.error) throw result.error
  return result
}

// Lists dylibs staged into Contents/Resources, where the app resolves sherpa runtime deps.
function listResourceDylibs() {
  if (!existsSync(resourcesDir)) {
    fail("missing Resources directory: " + resourcesDir)
    return []
  }

  return readdirSync(resourcesDir)
    .filter((name) => name.endsWith(".dylib"))
    .sort()
}

// Parses otool output into install names referenced by a Mach-O binary or dylib.
function parseOtoolLibraries(binary) {
  const result = run("otool", ["-L", binary])
  if (result.status !== 0) {
    fail("otool -L failed for " + binary + "\n" + result.stderr)
    return []
  }

  return result.stdout
    .split("\n")
    .slice(1)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.split(" (")[0])
}

// Identifies Apple system libraries that should not be bundled inside the app.
function isSystemDependency(dep) {
  return dep.startsWith("/System/Library/") || dep.startsWith("/usr/lib/")
}

// Chooses the first candidate path that exists on disk.
function existingCandidate(paths) {
  return paths.find((path) => existsSync(path))
}

// Resolves a dependency install name to the path that should exist inside the app bundle.
function resolveBundledDependency(dep, loaderPath) {
  if (isSystemDependency(dep)) return null

  if (dep.startsWith("@rpath/")) {
    const basename = dep.slice("@rpath/".length)
    return existingCandidate([
      join(resourcesDir, basename),
      join(macosDir, basename),
    ]) ?? join(resourcesDir, basename)
  }

  if (dep.startsWith("@loader_path/")) {
    return resolve(dirname(loaderPath), dep.slice("@loader_path/".length))
  }

  if (dep.startsWith("@executable_path/")) {
    return resolve(macosDir, dep.slice("@executable_path/".length))
  }

  if (dep.startsWith("/")) {
    return dep
  }

  return existingCandidate([
    join(resourcesDir, dep),
    join(dirname(loaderPath), dep),
    join(macosDir, dep),
  ]) ?? join(resourcesDir, dep)
}

// Verifies signatures on the app binary and bundled dylibs during protected release builds.
function verifySignatures(paths) {
  if (!requireSignatures) return

  for (const path of paths) {
    const result = run("codesign", ["--verify", "--strict", "--verbose=2", path], { stdio: "pipe" })
    if (result.status !== 0) {
      fail("codesign verification failed for " + path + "\n" + result.stderr)
    }
  }
}

// Coordinates dylib presence, dependency closure, and optional signature verification.
function main() {
  if (process.platform !== "darwin") {
    console.log("[verify-macos-bundle-libs] skipped: not macOS")
    return
  }

  if (!existsSync(appBinary)) fail("missing app binary: " + appBinary)

  const dylibNames = listResourceDylibs()
  const dylibPaths = dylibNames.map((name) => join(resourcesDir, name))
  const dylibNameSet = new Set(dylibNames)

  for (const required of requiredDylibs) {
    if (!dylibNameSet.has(required)) fail("missing required dylib: " + join(resourcesDir, required))
  }

  if (!dylibNames.some((name) => versionedOnnxRuntime.test(name))) {
    fail("missing versioned ONNX Runtime dylib matching " + versionedOnnxRuntime)
  }

  console.log("[verify-macos-bundle-libs] bundled dylibs:")
  for (const name of dylibNames) console.log("  " + name)

  const binaries = [appBinary, ...dylibPaths].filter((path) => existsSync(path))
  const expectedBundled = new Set(dylibPaths.map((path) => resolve(path)))

  for (const binary of binaries) {
    const deps = parseOtoolLibraries(binary)
    for (const dep of deps) {
      const resolved = resolveBundledDependency(dep, binary)
      if (!resolved) continue

      const normalized = resolve(resolved)
      const depBase = dep.split("/").pop()
      const binaryBase = binary.split("/").pop()

      // A dylib's own install name is allowed when it resolves to itself or to
      // another bundled compatibility name that exists in Resources.
      if (depBase === binaryBase && existsSync(normalized)) continue

      if (!existsSync(normalized)) {
        fail(binary + " references missing dependency " + dep + " (expected " + normalized + ")")
      } else if (normalized.startsWith(resourcesDir) && !expectedBundled.has(normalized)) {
        fail(binary + " resolved dependency outside known bundled dylib set: " + dep + " -> " + normalized)
      }
    }
  }

  verifySignatures(binaries)

  if (failures.length > 0) {
    console.error("[verify-macos-bundle-libs] failed with " + failures.length + " issue(s)")
    process.exit(1)
  }

  console.log("[verify-macos-bundle-libs] ok")
}

main()
