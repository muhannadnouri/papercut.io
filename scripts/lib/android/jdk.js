import { existsSync } from "node:fs"
import {
  mkdir,
  readFile,
  rm,
  stat,
} from "node:fs/promises"
import { join } from "node:path"
import {
  JDK_ARCHIVE,
  JDK_DOWNLOAD_URL,
  JDK_HOME,
  JDK_RELEASE,
  JDK_SHA256,
  JDK_ROOT,
} from "./constants.js"
import { downloadFile } from "../download.js"
import { extractTar } from "../archive.js"

// Prefer existing Java, otherwise install repo-local Temurin for repeatable APK builds.
export async function ensureLocalJdk({ force = false } = {}) {
  const existing = findExistingJavaHome()
  if (!force && existing) {
    return existing
  }

  if (force) {
    await rm(JDK_HOME, {
      recursive: true,
      force: true,
    })
  }

  if (!await hasLocalJdk()) {
    await mkdir(JDK_ROOT, { recursive: true })
    await downloadFile({
      url: JDK_DOWNLOAD_URL,
      dest: JDK_ARCHIVE,
      force,
      sha256: JDK_SHA256,
      label: "local-jdk",
    })
    await extractJdkArchive()
    await rm(JDK_ARCHIVE, { force: true })
  }

  if (!await hasLocalJdk()) {
    throw new Error("Local JDK install failed: " + JDK_HOME)
  }

  return JDK_HOME
}

// Environment-provided JDKs win so local installs stay optional.
function findExistingJavaHome() {
  const candidates = [process.env.PAPERCUT_JAVA_HOME, process.env.JAVA_HOME]
  for (const candidate of candidates) {
    if (
      candidate &&
      existsSync(join(candidate, "bin", javaExecutable())) &&
      existsSync(join(candidate, "bin", javacExecutable()))
    ) {
      return candidate
    }
  }
  return null
}

// Require javac too; Gradle needs a full JDK, not just a JRE.
async function hasLocalJdk() {
  try {
    const java = await stat(join(JDK_HOME, "bin", javaExecutable()))
    const javac = await stat(join(JDK_HOME, "bin", javacExecutable()))
    return java.isFile() && javac.isFile() && await hasPinnedLocalJdk()
  } catch {
    return false
  }
}

async function hasPinnedLocalJdk() {
  try {
    const release = await readFile(join(JDK_HOME, "release"), "utf8")
    return release.includes("FULL_VERSION=\"" + JDK_RELEASE + "\"") ||
      release.includes("SEMANTIC_VERSION=\"" + JDK_RELEASE + "\"")
  } catch {
    return false
  }
}

// Extract into final JDK_HOME shape regardless of archive root folder name.
async function extractJdkArchive() {
  console.log("[local-jdk] extracting " + JDK_ARCHIVE)
  await rm(JDK_HOME, {
    recursive: true,
    force: true,
  })
  await mkdir(JDK_HOME, { recursive: true })
  await extractTar({
    archive: JDK_ARCHIVE,
    destination: JDK_HOME,
    compression: "gzip",
    stripComponents: 1,
  })
}

function javaExecutable() {
  return process.platform === "win32" ? "java.exe" : "java"
}

function javacExecutable() {
  return process.platform === "win32" ? "javac.exe" : "javac"
}
