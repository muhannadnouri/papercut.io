import {
  dirname,
  join,
} from "node:path"
import { fileURLToPath } from "node:url"

const SCRIPTS_DIR = join(dirname(fileURLToPath(import.meta.url)), "..")
export const ROOT = join(SCRIPTS_DIR, "..")
export const SRC_TAURI_DIR = join(ROOT, "src-tauri")
export const TTS_RUNTIME_DIR = join(SRC_TAURI_DIR, "tts", "runtime")

export function fromRuntime(...parts) {
  return join(TTS_RUNTIME_DIR, ...parts)
}
