import { createRequire } from "node:module"
import {
  dirname,
  resolve,
} from "node:path"

const require = createRequire(import.meta.url)

// Invoke the installed Tauri CLI with Node so Windows does not depend on .cmd shims.
export function tauriCommand(args) {
  return {
    command: process.execPath,
    args: [
      resolveTauriCliEntry(),
      ...args,
    ],
  }
}

function resolveTauriCliEntry() {
  const packageJsonPath = require.resolve("@tauri-apps/cli/package.json")
  const manifest = require("@tauri-apps/cli/package.json")
  const bin = typeof manifest.bin === "string" ? manifest.bin : manifest.bin?.tauri

  if (!bin) {
    throw new Error("@tauri-apps/cli does not expose a tauri bin entry. Run npm install and check the installed package metadata.")
  }

  return resolve(dirname(packageJsonPath), bin)
}
