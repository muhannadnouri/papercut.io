import {
  describe,
  expect,
  it,
} from "vitest"
import {
  compareEntries,
  entryMap,
} from "../check-build-reproducibility.js"

const listing = `
Archive:  test.apk
  Length      Method    Crc    Cmpr    Date    Time    CRC-32    Name
--------  ------  --------  ----  ---------- -----  --------  ----
     1939  Stored 1939   0% 1981-01-01 01:01 deadbeef   assets/dexopt/baseline.prof
 27985944  Stored 27985944   0% 1981-01-01 01:01 f336ca24   lib/arm64-v8a/libonnxruntime.so
--------          -------  ----                            -----
  27987883          27987883   0%                            2 files
`

describe("entryMap", () => {
  it("parses per-entry crc and size from unzip -v output", () => {
    const map = entryMap(listing)
    expect(map.get("assets/dexopt/baseline.prof")).toEqual({
      crc: "deadbeef",
      size: "1939",
    })
    expect(map.get("lib/arm64-v8a/libonnxruntime.so")).toEqual({
      crc: "f336ca24",
      size: "27985944",
    })
    expect(map.size).toBe(2)
  })
})

describe("compareEntries", () => {
  const base = entryMap(listing)

  it("passes identical maps and reports changed, missing and extra entries", () => {
    expect(compareEntries(base, new Map(base))).toEqual([])

    const changed = new Map(base)
    changed.set("assets/dexopt/baseline.prof", {
      crc: "0000ffff",
      size: "1939",
    })
    expect(compareEntries(base, changed)).toEqual([
      "content differs: assets/dexopt/baseline.prof (crc deadbeef vs 0000ffff)",
    ])

    const withoutLib = new Map(base)
    withoutLib.delete("lib/arm64-v8a/libonnxruntime.so")
    expect(compareEntries(base, withoutLib)).toEqual(["only in first build: lib/arm64-v8a/libonnxruntime.so"])
    expect(compareEntries(withoutLib, base)).toEqual(["only in second build: lib/arm64-v8a/libonnxruntime.so"])
  })
})
