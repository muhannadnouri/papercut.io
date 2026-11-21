import assert from "node:assert/strict"
import test from "node:test"
import { assertSourceState } from "../source-build.js"

test("source recipes accept only the pinned revision without tracked edits", () => {
  const pin = "ab".repeat(20)
  assertSourceState(pin + "\n", pin, "")
  assert.throws(() => assertSourceState("cd".repeat(20), pin, ""), /pinned revision/)
  assert.throws(() => assertSourceState(pin, pin, "diff --git changed.c"), /tracked edits/)
  assert.throws(() => assertSourceState(pin, pin, "", "injected.h"), /untracked files/)
})
