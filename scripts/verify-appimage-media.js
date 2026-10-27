import {
  existsSync,
  readdirSync,
} from "node:fs"
import { join } from "node:path"
import { ROOT } from "./lib/paths.js"

const appDir = join(
  ROOT,
  "src-tauri",
  "target",
  "release",
  "bundle",
  "appimage",
  "Papercut.AppDir",
)

if (!existsSync(appDir)) {
  fail("AppImage AppDir was not found. Run `npm run desktop` on Linux first.")
}

const bundledFiles = collectFileNames(appDir)
const requiredFiles = [
  "gst-plugin-scanner",
  "libgstcoreelements.so",
  "libgstwavparse.so",
  "libgstautodetect.so",
]
const missingFiles = requiredFiles.filter((file) => !bundledFiles.has(file))
const audioSinks = [
  "libgstalsa.so",
  "libgstpipewire.so",
  "libgstpulseaudio.so",
]

if (!audioSinks.some((file) => bundledFiles.has(file))) {
  missingFiles.push("a GStreamer audio sink (ALSA, PipeWire, or PulseAudio)")
}

if (missingFiles.length > 0) {
  fail(
    "AppImage media bundle is incomplete. Missing: " +
      missingFiles.join(", ") +
      ". Install the documented GStreamer build dependencies and rebuild.",
  )
}

console.log("[appimage-media] Verified bundled GStreamer WAV playback support.")

// Index basenames across the generated AppDir because linuxdeploy may place the
// same required component under architecture-specific directories on different
// Ubuntu and GStreamer versions.
function collectFileNames(root) {
  const names = new Set()
  const pending = [root]

  while (pending.length > 0) {
    const current = pending.pop()
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name)
      if (entry.isDirectory()) {
        pending.push(path)
      } else {
        names.add(entry.name)
      }
    }
  }

  return names
}

// Keep artifact verification fail-fast so CI cannot publish an AppImage that
// launches normally but freezes when WebKitGTK first initializes audio playback.
function fail(message) {
  console.error("[appimage-media] " + message)
  process.exit(1)
}
