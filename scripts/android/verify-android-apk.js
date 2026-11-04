import assert from "node:assert/strict"
import {
  execFileSync,
  spawnSync,
} from "node:child_process"
import { readFileSync } from "node:fs"
import {
  join,
  resolve,
} from "node:path"
import { parseArgs } from "node:util"
import { ROOT } from "../lib/paths.js"
import { isMain } from "../lib/process.js"

// Filenames are not authoritative: compare installed identity/version and release flags to committed config.
export function checkManifest(badging, version, code) {
  const metadata = badging.match(/^package: name='([^']+)' versionCode='(\d+)' versionName='([^']+)'/)
  assert(metadata, "APK package/version metadata is missing")
  assert.deepEqual(metadata.slice(1), ["io.papercut.desktop", String(code), version], "APK identity/version differs from committed configuration")
  assert(/^sdkVersion:'26'$/m.test(badging), "APK must support minimum API 26")
  assert(/^targetSdkVersion:'36'$/m.test(badging), "APK must target API 36")
  assert(!badging.includes("application-debuggable"), "Refusing to publish a debuggable APK")
}

// Pin the certificate, whose digest stays stable across updates; an APK digest changes with every release.
export function checkCertificate(output, expected) {
  assert(/^[0-9a-f]{64}$/i.test(expected ?? ""), "ANDROID_SIGNING_CERT_SHA256 must contain the expected 64-digit certificate digest")
  const digests = [...output.matchAll(/Signer #\d+ certificate SHA-256 digest: ([0-9a-f]+)/g)].map(match => match[1])
  assert.deepEqual(digests, [expected.toLowerCase()], "APK signing certificate does not match ANDROID_SIGNING_CERT_SHA256")
  assert(!output.includes("CN=Android Debug"), "Refusing to publish an Android Debug certificate")
}

// This arm64 gate reads ELF64 PT_LOAD alignment; ZIP alignment alone cannot establish 16 KB compatibility.
export function checkElf(data, name) {
  assert(data.length >= 64 && data.subarray(0, 6).equals(Buffer.from([0x7f, 69, 76, 70, 2, 1])), `${name}: expected little-endian ELF64`)
  assert.equal(data.readUInt16LE(18), 183, `${name}: expected AArch64 code`)
  const offset = data.readBigUInt64LE(32)
  const size = data.readUInt16LE(54)
  const count = data.readUInt16LE(56)
  assert(size >= 56 && count > 0 && offset + BigInt(size * count) <= BigInt(data.length), `${name}: invalid ELF program headers`)
  const loads = []
  for (let index = 0; index < count; index++) {
    const header = Number(offset) + size * index
    if (data.readUInt32LE(header) === 1) {
      loads.push(data.readBigUInt64LE(header + 48))
    }
  }
  assert(loads.length && loads.every(alignment => alignment >= 16384n), `${name}: native LOAD segments do not support 16 KB pages`)
}

// unzip interprets wildcard entry names even without a shell; admit only ordinary flat arm64 library paths.
export function checkEntries(names) {
  assert.equal(names.length, new Set(names).size, "APK contains duplicate ZIP entries")
  const libraries = names.filter(name => name.startsWith("lib/") && name.endsWith(".so"))
  assert(libraries.length && libraries.every(name => /^lib\/arm64-v8a\/[a-z0-9_.+-]+\.so$/i.test(name)), "APK must contain only arm64 native libraries with ordinary filenames")
  for (const required of ["libapp_lib.so", "libsherpa-onnx-c-api.so", "libonnxruntime.so"]) {
    assert(libraries.includes(`lib/arm64-v8a/${required}`), `APK is missing ${required}`)
  }
  assert(names.includes("assets/licenses/libtashkeel-1.5.0.txt"), "APK is missing the Arabic preprocessing license")
  return libraries
}

// Avoid shell interpolation and bound captured output, including each decompressed native library.
function command(file, args, options = {}) {
  return execFileSync(file, args, {
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    ...options,
  })
}

// Unsigned mode is for CI/F-Droid build inputs; publication must run the signed gate with the expected certificate.
export function verifyApk(apk, { unsigned = false } = {}) {
  apk = resolve(apk)
  const sdk = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT
  assert(sdk, "Set ANDROID_HOME or ANDROID_SDK_ROOT")
  const tools = join(sdk, "build-tools", "35.0.0")
  const tauri = JSON.parse(readFileSync(join(ROOT, "src-tauri/tauri.conf.json"), "utf8"))
  const android = JSON.parse(readFileSync(join(ROOT, "src-tauri/tauri.android.conf.json"), "utf8")).bundle.android
  const version = tauri.version
  for (const filename of ["package.json", "package-lock.json"]) {
    assert.equal(JSON.parse(readFileSync(join(ROOT, filename), "utf8")).version, version, `${filename} version differs from Tauri`)
  }
  const cargoManifest = join(ROOT, "src-tauri/Cargo.toml")
  const cargo = JSON.parse(command("cargo", ["metadata", "--manifest-path", cargoManifest, "--no-deps", "--format-version", "1", "--offline", "--locked"]))
  assert.equal(cargo.packages.find(pkg => resolve(pkg.manifest_path) === cargoManifest)?.version, version, "Cargo version differs from Tauri")
  assert(!process.env.RELEASE_TAG || process.env.RELEASE_TAG === `v${version}`, "Release tag differs from committed app version")
  checkManifest(command(join(tools, "aapt"), ["dump", "badging", apk]), version, android.versionCode)
  const signature = spawnSync(join(tools, "apksigner"), ["verify", "--verbose", "--print-certs", apk], { encoding: "utf8" })
  if (signature.error) {
    throw signature.error
  }
  assert.notEqual(signature.status, null, "apksigner terminated before completing verification")
  if (unsigned) {
    assert(signature.status !== 0, "Expected an unsigned APK, but signature verification succeeded")
  } else {
    assert.equal(signature.status, 0, "APK signature verification failed: " + signature.stderr)
    checkCertificate(signature.stdout, process.env.ANDROID_SIGNING_CERT_SHA256)
  }
  const libraries = checkEntries(command(join(tools, "aapt"), ["list", apk]).trim().split(/\r?\n/))
  for (const name of libraries) {
    checkElf(command("unzip", ["-p", apk, name], { encoding: "buffer" }), name)
  }
  command(join(tools, "zipalign"), ["-c", "-P", "16", "4", apk])
  console.log(`Verified ${unsigned ? "unsigned" : "signed"} arm64 release APK: ${version} (${android.versionCode})`)
}

if (isMain(import.meta.url)) {
  const { values, positionals } = parseArgs({
    options: { unsigned: { type: "boolean" } },
    allowPositionals: true,
  })
  assert.equal(positionals.length, 1, "Usage: node scripts/android/verify-android-apk.js [--unsigned] <apk>")
  verifyApk(positionals[0], values)
}
