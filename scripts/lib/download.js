import { createHash } from "node:crypto"
import {
  createReadStream,
  existsSync,
} from "node:fs"
import {
  mkdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises"
import { dirname } from "node:path"

// Download through a temp path so interrupted fetches never poison cache.
export async function downloadFile({ url, dest, force = false, sha256, label = "download" }) {
  if (!force && existsSync(dest)) {
    if (sha256) {
      await assertSha256(dest, sha256, label)
    }
    console.log("[" + label + "] cached archive")
    return dest
  }

  const temp = dest + ".tmp-" + process.pid
  await rm(temp, { force: true })

  try {
    console.log("[" + label + "] downloading " + url)
    const response = await fetch(url)
    if (!response.ok) {
      throw new Error("Failed to download " + url + ": " + response.status + " " + response.statusText)
    }

    await mkdir(dirname(dest), { recursive: true })
    await writeFile(temp, Buffer.from(await response.arrayBuffer()))
    if (sha256) {
      await assertSha256(temp, sha256, label)
    }
    await rename(temp, dest)
    return dest
  } catch (err) {
    await rm(temp, { force: true })
    throw err
  }
}

// Validate optional pinned assets before promoting or reusing them.
async function assertSha256(file, expected, label = "download") {
  const actual = await sha256File(file)
  if (actual !== expected) {
    throw new Error("[" + label + "] SHA-256 mismatch for " + file + ": expected " + expected + ", got " + actual)
  }
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
