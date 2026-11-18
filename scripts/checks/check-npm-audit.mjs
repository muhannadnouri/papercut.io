import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const advisory = 'https://github.com/advisories/GHSA-fc5q-39xc-vjpf'
// shortcut: temporary dev-tooling exception, remove when a patched picomatch ships.
const expires = Date.parse('2026-12-01T00:00:00Z')

export function checkAudit(report, packages, now = Date.now()) {
  const { vulnerabilities, metadata, auditReportVersion, error } = report
  if (error || auditReportVersion !== 2 || !vulnerabilities ||
      typeof vulnerabilities !== 'object' || Array.isArray(vulnerabilities) ||
      !packages || typeof packages !== 'object') {
    throw new Error('Invalid npm audit report or lockfile')
  }

  const entries = Object.entries(vulnerabilities)
  for (const severity of ['info', 'low', 'moderate', 'high', 'critical']) {
    if (metadata?.vulnerabilities?.[severity] !== entries.filter(([, entry]) => entry?.severity === severity).length) {
      throw new Error(`Invalid npm audit count for ${severity}`)
    }
  }
  if (metadata.vulnerabilities.total !== entries.length) {
    throw new Error('Invalid npm audit total')
  }

  function tracesOnlyToException(name, visited = new Set()) {
    const entry = Object.hasOwn(vulnerabilities, name) ? vulnerabilities[name] : undefined
    if (visited.has(name) || entry?.name !== name || entry.severity !== 'high' ||
        !Array.isArray(entry.nodes) || !entry.nodes.length ||
        !entry.nodes.every(node => packages[node]?.dev === true) ||
        !Array.isArray(entry.via) || !entry.via.length) {
      return false
    }
    if (name === 'picomatch') {
      return entry.isDirect === false && entry.nodes.every(node => /(?:^|\/)node_modules\/picomatch$/.test(node) &&
        packages[node].version === '4.0.7') &&
        entry.via.length === 1 && entry.via[0]?.url === advisory &&
        entry.via[0].dependency === name && entry.via[0].severity === 'high'
    }
    const next = new Set(visited).add(name)
    return entry.via.every(cause => typeof cause === 'string' && tracesOnlyToException(cause, next))
  }

  const blocked = []
  const excepted = []
  for (const [name, entry] of entries) {
    if (entry.severity === 'high' || entry.severity === 'critical') {
      const target = now < expires && tracesOnlyToException(name) ? excepted : blocked
      target.push(name)
    }
  }
  return {
    blocked,
    excepted,
  }
}

if (import.meta.main) {
  try {
    const result = spawnSync('npm', ['audit', '--audit-level=high', '--json'], {
      encoding: 'utf8',
      maxBuffer: 16 * 1024 * 1024,
      timeout: 120_000,
    })
    if (result.error || ![0, 1].includes(result.status)) {
      throw result.error ?? new Error(`npm audit exited with ${result.signal ?? result.status}`)
    }
    process.stdout.write(result.stdout)
    process.stderr.write(result.stderr)
    const report = JSON.parse(result.stdout)
    const { packages } = JSON.parse(readFileSync('package-lock.json', 'utf8'))
    const { blocked, excepted } = checkAudit(report, packages)
    if (result.status === 1 && !blocked.length && !excepted.length) {
      throw new Error('npm audit failed without a high or critical finding')
    }
    if (excepted.length) {
      console.warn(`Temporary development dependency exception until 2026-12-01 UTC for ${advisory}: ${excepted.join(', ')}`)
    }
    if (blocked.length) {
      throw new Error(`Blocking npm audit findings: ${blocked.join(', ')}`)
    }
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
