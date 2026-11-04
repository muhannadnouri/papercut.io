import assert from "node:assert/strict"
import test from "node:test"
import {
  checkCertificate,
  checkElf,
  checkEntries,
  checkManifest,
} from "../verify-android-apk.js"

const manifest = "package: name='io.papercut.desktop' versionCode='1010000' versionName='1.10.0'\nsdkVersion:'26'\ntargetSdkVersion:'36'\n"

// Minimal ELF64 header plus one PT_LOAD segment exercises the verifier without compiling/executing native code.
function elf(alignment = 16384n) {
  const data = Buffer.alloc(120)
  data.set([0x7f, 69, 76, 70, 2, 1])
  data.writeUInt16LE(183, 18)
  data.writeBigUInt64LE(64n, 32)
  data.writeUInt16LE(56, 54)
  data.writeUInt16LE(1, 56)
  data.writeUInt32LE(1, 64)
  data.writeBigUInt64LE(alignment, 112)
  return data
}

test("rejects debuggable APKs, identity changes and incorrect versions/API levels", () => {
  checkManifest(manifest, "1.10.0", 1010000)
  for (const bad of [
    manifest + "application-debuggable\n",
    manifest.replace("io.papercut.desktop", "io.papercut.other"),
    manifest.replace("1010000", "1010001"),
    manifest.replace("1.10.0", "1.9.0"),
    manifest.replace("sdkVersion:'26'", "sdkVersion:'27'"),
    manifest.replace("targetSdkVersion:'36'", "targetSdkVersion:'35'"),
    manifest.replace("sdkVersion:'26'", "application-label:'sdkVersion:'26''"),
  ]) {
    assert.throws(() => checkManifest(bad, "1.10.0", 1010000))
  }
})

test("pins the certificate and rejects debug/multiple signers", () => {
  const digest = "ab".repeat(32)
  const output = `Signer #1 certificate SHA-256 digest: ${digest}\n`
  checkCertificate(output, digest.toUpperCase())
  for (const [bad, expected] of [
    [output, "cd".repeat(32)],
    [output, ""],
    [output + "Signer #1 certificate DN: CN=Android Debug", digest],
    [output + output.replace("#1", "#2"), digest],
  ]) {
    assert.throws(() => checkCertificate(bad, expected))
  }
})

test("checks native architecture, header bounds and 16 KB segment alignment", () => {
  checkElf(elf(), "valid.so")
  const wrongArch = elf()
  wrongArch.writeUInt16LE(62, 18)
  const hugeOffset = elf()
  hugeOffset.writeBigUInt64LE(2n ** 63n, 32)
  for (const data of [elf(4096n), wrongArch, hugeOffset, Buffer.from("not ELF"), elf().subarray(0, 80)]) {
    assert.throws(() => checkElf(data, "invalid.so"))
  }
})

test("rejects missing runtime, extra ABIs, duplicates and wildcard/path names", () => {
  const names = ["libapp_lib.so", "libsherpa-onnx-c-api.so", "libonnxruntime.so"].map(name => `lib/arm64-v8a/${name}`)
  names.push("assets/licenses/libtashkeel-1.5.0.txt")
  assert.equal(checkEntries(names).length, 3)
  for (const bad of [
    names.slice(1),
    [...names, "lib/x86_64/libonnxruntime.so"],
    [...names, names[0]],
    [...names, "lib/arm64-v8a/lib*.so"],
    [...names, "lib/arm64-v8a/../evil.so"],
  ]) {
    assert.throws(() => checkEntries(bad))
  }
})
