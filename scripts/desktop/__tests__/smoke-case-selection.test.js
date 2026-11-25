import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  expect,
  it,
} from 'vitest'

it.each([
  [[], 'No desktop smoke cases found'],
  [['DS-001-reading.mjs'], 'Unknown desktop smoke case DS-999. Available: DS-001'],
  [['DS-001-reading.mjs', 'DS-001-import.mjs'], 'Invalid or duplicate desktop smoke case'],
])('rejects invalid selection before building with files %j', (files, message) => {
  // An isolated checkout exercises CLI discovery without touching real cases or launching Papercut.
  const root = mkdtempSync(join(tmpdir(), 'papercut-case-selection-'))
  try {
    const desktop = join(root, 'scripts', 'desktop')
    const cases = join(desktop, 'cases')
    mkdirSync(cases, { recursive: true })
    const runner = join(desktop, 'smoke-desktop.mjs')
    copyFileSync(new URL('../smoke-desktop.mjs', import.meta.url), runner)
    copyFileSync(new URL('../smoke-cleanup.mjs', import.meta.url), join(desktop, 'smoke-cleanup.mjs'))
    for (const file of files) {
      writeFileSync(join(cases, file), '')
    }
    const result = spawnSync(process.execPath, [runner, '--case', 'DS-999'], { encoding: 'utf8' })
    expect(result.status).toBe(1)
    expect(result.stderr).toContain(message)
    expect(result.stdout).toBe('')
  } finally {
    rmSync(root, {
      recursive: true,
      force: true,
    })
  }
})

it.each(['[]', '[null]', '{}'])('rejects invalid phases %s before starting a driver', phases => {
  const report = mkdtempSync(join(tmpdir(), 'papercut-phases-'))
  try {
    const file = join(report, 'case.mjs')
    writeFileSync(file, 'export const phases = ' + phases)
    const result = spawnSync(process.execPath, [fileURLToPath(new URL('../smoke-worker.mjs', import.meta.url)), 'unused', report, file], { encoding: 'utf8' })
    expect(result.status).toBe(1)
    expect(result.stderr).toContain('must export a nonempty array of phase functions')
    expect(result.stdout).not.toContain('passed')
  } finally {
    rmSync(report, {
      recursive: true,
      force: true,
    })
  }
})
