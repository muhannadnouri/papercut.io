import { existsSync, mkdirSync, rmSync } from "node:fs"
import { join } from "node:path"
import { ROOT } from "./lib/paths.js"
import { runSync } from "./lib/process.js"

const SILMA_DIR = join(ROOT, "sidecars", "silma")
const WORKER = join(SILMA_DIR, "silma_worker.py")
const CACHE_DIR = join(ROOT, ".cache", "silma-pyinstaller")

const options = parseArgs(process.argv.slice(2))
const target = options.target ?? currentTargetTriple()
const python = options.python ?? defaultPython()
const mode = options.mode ?? "onedir"
const exeBase = "silma-worker-" + target
const exeName = exeBase + (process.platform === "win32" ? ".exe" : "")
const outputDir = options.outputDir ?? join(SILMA_DIR, "runtime", target, mode)

if (options.clean) {
  rmSync(outputDir, { recursive: true, force: true })
  rmSync(CACHE_DIR, { recursive: true, force: true })
}

if (!existsSync(WORKER)) {
  fail("Missing SILMA worker: " + WORKER)
}
if (!python) {
  fail("Could not find .venv-silma Python. Pass --python /path/to/python.")
}
if (!hasPyInstaller(python)) {
  fail(
    "PyInstaller is not installed for " +
      python +
      "\nRun: " +
      python +
      " -m pip install pyinstaller",
  )
}

mkdirSync(outputDir, { recursive: true })
mkdirSync(CACHE_DIR, { recursive: true })

const result = runSync(
  python,
  [
    "-m",
    "PyInstaller",
    "--noconfirm",
    "--clean",
    mode === "onefile" ? "--onefile" : "--onedir",
    "--name",
    exeBase,
    "--distpath",
    outputDir,
    "--workpath",
    join(CACHE_DIR, "build"),
    "--specpath",
    join(CACHE_DIR, "spec"),
    "--collect-all",
    "silma_tts",
    WORKER,
  ],
  { cwd: ROOT },
)

if (result.error) {
  fail("Failed to start PyInstaller: " + result.error.message)
}
if (result.status !== 0) {
  process.exit(result.status ?? 1)
}

console.log("[silma-sidecar] built " + builtWorkerPath(outputDir, mode, exeBase, exeName))

function parseArgs(args) {
  const parsed = { clean: false }
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--clean") {
      parsed.clean = true
    } else if (arg === "--python") {
      parsed.python = requireValue(args, ++index, arg)
    } else if (arg === "--target") {
      parsed.target = requireValue(args, ++index, arg)
    } else if (arg === "--mode") {
      const mode = requireValue(args, ++index, arg)
      if (mode !== "onefile" && mode !== "onedir") {
        fail("--mode must be onefile or onedir")
      }
      parsed.mode = mode
    } else if (arg === "--output-dir") {
      parsed.outputDir = requireValue(args, ++index, arg)
    } else {
      fail("Unknown option: " + arg)
    }
  }
  return parsed
}

function builtWorkerPath(outputDir, mode, exeBase, exeName) {
  return mode === "onefile" ? join(outputDir, exeName) : join(outputDir, exeBase, exeName)
}

function requireValue(args, index, flag) {
  const value = args[index]
  if (!value || value.startsWith("--")) {
    fail("Missing value for " + flag)
  }
  return value
}

function defaultPython() {
  const envPython = process.env.PAPERCUT_SILMA_PYTHON
  if (envPython) return envPython

  const venvPython =
    process.platform === "win32"
      ? join(ROOT, ".venv-silma", "Scripts", "python.exe")
      : join(ROOT, ".venv-silma", "bin", "python")
  if (existsSync(venvPython)) return venvPython

  return process.platform === "win32" ? "python" : "python3"
}

function hasPyInstaller(python) {
  const result = runSync(python, ["-m", "PyInstaller", "--version"], {
    cwd: ROOT,
    stdio: "ignore",
  })
  return !result.error && result.status === 0
}

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

function fail(message) {
  console.error("[silma-sidecar] " + message)
  process.exit(1)
}
