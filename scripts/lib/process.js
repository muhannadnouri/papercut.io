import {
  spawn,
  spawnSync,
} from "node:child_process"
import { resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { ROOT } from "./paths.js"

export function pathSeparator() {
  return process.platform === "win32" ? ";" : ":"
}

export function shQuote(value) {
  return "'" + value.replaceAll("'", "'\"'\"'") + "'"
}

// Let modules be safe to import while still supporting direct CLI execution.
export function isMain(metaUrl) {
  return process.argv[1] && fileURLToPath(metaUrl) === resolve(process.argv[1])
}

// Promise wrapper keeps async scripts linear while preserving child stdio.
export function run(command, args, options = {}) {
  const {
    cwd = ROOT,
    env = process.env,
    stdio = "inherit",
    label = command + " " + args.join(" "),
  } = options

  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      stdio,
    })
    child.on("error", reject)
    child.on("exit", (code) => {
      if (code === 0) {
        resolveRun()
      } else {
        reject(new Error(label + " exited with code " + code))
      }
    })
  })
}

export function runSync(command, args, options = {}) {
  const { cwd = ROOT, env = process.env, stdio = "inherit" } = options
  return spawnSync(command, args, {
    cwd,
    env,
    stdio,
  })
}

// Sync wrappers use process exit codes so npm/Tauri callers fail fast.
export function exitFromResult(result, errorPrefix) {
  if (result.error) {
    console.error(errorPrefix + result.error.message)
    process.exit(1)
  }
  process.exit(result.status ?? 1)
}
