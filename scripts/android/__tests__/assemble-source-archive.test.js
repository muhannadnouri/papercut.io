import {
  describe,
  expect,
  it,
} from "vitest"
import { extractArchivePins } from "../assemble-source-archive.js"

const cmake = `
  set(espeak_ng_URL  "https://github.com/csukuangfj/espeak-ng/archive/ed530aa113046142eb5115cf2fc9157854d0ffe1.zip")
  set(espeak_ng_HASH "SHA256=e4e262cbe34f7fe21f91f1ba3397f2728e1f30eafbae7853f2b753a9ed13f0dd")
  set(BUILD_ESPEAK_NG_TESTS OFF CACHE BOOL "" FORCE)
`

describe("extractArchivePins", () => {
  it("reads the pinned URL and sha256 for a component", () => {
    expect(extractArchivePins(cmake, "espeak_ng")).toEqual({
      url: "https://github.com/csukuangfj/espeak-ng/archive/ed530aa113046142eb5115cf2fc9157854d0ffe1.zip",
      sha256: "e4e262cbe34f7fe21f91f1ba3397f2728e1f30eafbae7853f2b753a9ed13f0dd",
    })
  })

  it("rejects an incomplete pin instead of building an unverifiable archive", () => {
    const withoutHash = cmake.replace(/set\(espeak_ng_HASH[^\n]*\n/, "")
    expect(() => extractArchivePins(withoutHash, "espeak_ng")).toThrow("archive pin not found")
  })
})
