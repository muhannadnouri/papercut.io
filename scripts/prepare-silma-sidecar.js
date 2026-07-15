import { chmodSync, cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { ROOT } from "./lib/paths.js"
import { runSync } from "./lib/process.js"

const SILMA_DIR = join(ROOT, "sidecars", "silma")
const WORKER = join(SILMA_DIR, "silma_worker.py")
const options = parseArgs(process.argv.slice(2))
const target = options.target ?? currentTargetTriple()
const python = options.python ?? defaultPython()
const exeBase = "silma-worker-" + target
const outputDir = options.outputDir ?? join(SILMA_DIR, "runtime", target, "onedir")
const appDir = join(outputDir, exeBase)
const workerPath = join(appDir, exeBase)

if (options.clean) {
  rmSync(outputDir, { recursive: true, force: true })
}

if (!existsSync(WORKER)) {
  fail("Missing SILMA worker: " + WORKER)
}
if (!python) {
  fail("Could not find Python. Pass --python /path/to/python.")
}

const pythonInfo = getPythonInfo(python)
if (pythonInfo.prefix !== pythonInfo.basePrefix) {
  fail(
    "Selected Python is a virtual environment. Build SILMA release runtimes " +
      "from a full Python prefix so the archive includes the interpreter and stdlib. " +
      "Pass --python /path/to/full/python.",
  )
}
if (!existsSync(join(pythonInfo.prefix, "lib"))) {
  fail("Selected Python prefix is missing lib/: " + pythonInfo.prefix)
}

mkdirSync(appDir, { recursive: true })
copyPythonPrefix(pythonInfo.prefix, join(appDir, "python"))
mkdirSync(join(appDir, "worker"), { recursive: true })
cpSync(WORKER, join(appDir, "worker", "silma_worker.py"))
writeLauncher(workerPath)

console.log("[silma-sidecar] built source runtime " + appDir)

if (options.selfTest) {
  runWorkerCheck(workerPath, "--self-test", "self-test")
}
if (options.importCheck) {
  runWorkerCheck(workerPath, "--import-check", "import check")
}

function parseArgs(args) {
  const parsed = { clean: false }
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--clean") {
      parsed.clean = true
    } else if (arg === "--self-test") {
      parsed.selfTest = true
    } else if (arg === "--import-check") {
      parsed.importCheck = true
    } else if (arg === "--python") {
      parsed.python = requireValue(args, ++index, arg)
    } else if (arg === "--target") {
      parsed.target = requireValue(args, ++index, arg)
    } else if (arg === "--output-dir") {
      parsed.outputDir = requireValue(args, ++index, arg)
    } else {
      fail("Unknown option: " + arg)
    }
  }
  return parsed
}

// Ask Python for its install prefix; copying that keeps source files and native modules intact.
function getPythonInfo(python) {
  const result = runSync(
    python,
    [
      "-c",
      "import json, sys; print(json.dumps({\"executable\": sys.executable, \"prefix\": sys.prefix, \"basePrefix\": sys.base_prefix, \"version\": sys.version_info[:3]}))",
    ],
    { cwd: ROOT, stdio: "pipe" },
  )
  if (result.error) fail("Failed to inspect Python: " + result.error.message)
  if (result.status !== 0) fail("Python inspection failed: " + result.stderr.toString())
  return JSON.parse(result.stdout.toString())
}

function copyPythonPrefix(source, destination) {
  rmSync(destination, { recursive: true, force: true })
  cpSync(source, destination, {
    recursive: true,
    dereference: true,
    filter: (path) => !shouldSkipPythonPath(path),
  })
}

function shouldSkipPythonPath(path) {
  const normalized = path.replaceAll("\\", "/")
  return (
    normalized.includes("/__pycache__") ||
    normalized.endsWith("/.cache") ||
    normalized.includes("/pip/_vendor/cachecontrol/caches")
  )
}

// Build a tiny shell entrypoint instead of freezing Python; relocated Torch and
// CUDA wheels need their bundled native-library directories visible at launch.
function writeLauncher(path) {
  const script = `#!/usr/bin/env sh
set -eu
SELF="$0"
DIR=$(CDPATH= cd -- "$(dirname -- "$SELF")" && pwd)
PY="$DIR/python/bin/python3"
if [ ! -x "$PY" ]; then
  PY="$DIR/python/bin/python"
fi
export PYTHONHOME="$DIR/python"
export PYTHONNOUSERSITE=1
LIB_PATH="$DIR/python/lib"
for EXTRA_LIB_DIR in \\
  "$DIR"/python/lib/python*/site-packages/torch/lib \\
  "$DIR"/python/lib/python*/site-packages/torchaudio/lib \\
  "$DIR"/python/lib/python*/site-packages/nvidia/*/lib; do
  if [ -d "$EXTRA_LIB_DIR" ]; then
    LIB_PATH="$LIB_PATH:$EXTRA_LIB_DIR"
  fi
done
export LD_LIBRARY_PATH="$LIB_PATH\${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$PY" "$DIR/worker/silma_worker.py" "$@"
`
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, script)
  chmodSync(path, 0o755)
}

function runWorkerCheck(workerPath, flag, label) {
  const result = runSync(workerPath, [flag], { cwd: ROOT })
  if (result.error) {
    fail("Failed to start packaged worker " + label + ": " + result.error.message)
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

function requireValue(args, index, flag) {
  const value = args[index]
  if (!value || value.startsWith("--")) {
    fail("Missing value for " + flag)
  }
  return value
}

// Prefer CI/system Python for release runtimes; venvs do not contain a full relocatable stdlib.
function defaultPython() {
  const envPython = process.env.PAPERCUT_SILMA_PYTHON
  if (envPython) return envPython
  return process.platform === "win32" ? "python" : "python3"
}

// Use Tauri/Rust-style triples so runtime folders line up with desktop targets.
function currentTargetTriple() {
  const platform = process.platform
  const arch = process.arch
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-gnu"
  if (platform === "linux" && arch === "arm64") return "aarch64-unknown-linux-gnu"
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin"
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin"
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc"
  if (platform === "win32" && arch === "arm64") return "aarch64-pc-windows-msvc"
  fail("Unsupported sidecar target: " + platform + "/" + arch + ". Pass --target explicitly.")
}

// Print one consistent prefix so script failures are easy to spot in npm logs.
function fail(message) {
  console.error("[silma-sidecar] " + message)
  process.exit(1)
}
