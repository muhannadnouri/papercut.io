import assert from 'node:assert/strict'
import { test } from 'node:test'
import { checkAudit } from '../check-npm-audit.mjs'

const beforeExpiry = Date.parse('2026-11-30T23:59:59Z')
const atExpiry = Date.parse('2026-12-01T00:00:00Z')

function fixture() {
  const vulnerabilities = {
    picomatch: {
      name: 'picomatch',
      severity: 'high',
      isDirect: false,
      nodes: ['node_modules/picomatch'],
      via: [{
        dependency: 'picomatch',
        severity: 'high',
        url: 'https://github.com/advisories/GHSA-fc5q-39xc-vjpf',
      }],
    },
    vite: {
      name: 'vite',
      severity: 'high',
      isDirect: true,
      nodes: ['node_modules/vite'],
      via: ['picomatch'],
    },
  }
  const report = {
    auditReportVersion: 2,
    vulnerabilities,
    metadata: {
      vulnerabilities: {
        info: 0,
        low: 0,
        moderate: 0,
        high: 2,
        critical: 0,
        total: 2,
      },
    },
  }
  const packages = {
    'node_modules/picomatch': {
      version: '4.0.7',
      dev: true,
    },
    'node_modules/vite': { dev: true },
  }
  return {
    report,
    packages,
  }
}

test('accepts the original development-only advisory and its dependent finding before expiry', () => {
  const { report, packages } = fixture()
  assert.deepEqual(checkAudit(report, packages, beforeExpiry), {
    blocked: [],
    excepted: ['picomatch', 'vite'],
  })
})

test('blocks the advisory and dependent finding at expiry', () => {
  const { report, packages } = fixture()
  assert.deepEqual(checkAudit(report, packages, atExpiry).blocked, ['picomatch', 'vite'])
})

test('blocks a production copy, another version, or another advisory on picomatch', () => {
  for (const change of [
    ({ packages }) => {
      delete packages['node_modules/picomatch'].dev
    },
    ({ packages }) => {
      packages['node_modules/picomatch'].version = '4.0.6'
    },
    ({ report }) => {
      report.vulnerabilities.picomatch.via.push({ url: 'https://example.invalid/other-advisory' })
    },
  ]) {
    const context = fixture()
    change(context)
    assert.deepEqual(checkAudit(context.report, context.packages, beforeExpiry).blocked, ['picomatch', 'vite'])
  }
})

test('blocks an independent advisory and dependent findings with mixed causes', () => {
  const { report, packages } = fixture()
  report.vulnerabilities.other = {
    name: 'other',
    severity: 'critical',
  }
  report.metadata.vulnerabilities.critical = 1
  report.metadata.vulnerabilities.total = 3
  report.vulnerabilities.vite.via.push('other')
  assert.deepEqual(checkAudit(report, packages, beforeExpiry).blocked, ['vite', 'other'])
})

test('rejects invalid reports and accepts a clean audit after expiry', () => {
  const { report, packages } = fixture()
  assert.throws(() => checkAudit({ error: { code: 'E503' } }, packages), /Invalid npm audit/)
  assert.throws(() => checkAudit({
    ...report,
    vulnerabilities: {},
  }, packages), /Invalid npm audit/)
  report.metadata.vulnerabilities.high = 0
  report.metadata.vulnerabilities.total = 0
  assert.deepEqual(checkAudit({
    ...report,
    vulnerabilities: {},
  }, packages, atExpiry), {
    blocked: [],
    excepted: [],
  })
})
