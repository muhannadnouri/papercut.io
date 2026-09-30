import { spawnSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { ROOT } from './lib/paths.js'

const fixturePath = join(ROOT, 'scripts/fixtures/search-v2/corpus.json')
const fixtureBytes = readFileSync(fixturePath)
const fixture = JSON.parse(fixtureBytes)
const scratch = mkdtempSync(join(tmpdir(), 'papercut-search-eval-'))
const rawPath = join(scratch, 'native.json')
const outputPath = resolve(process.argv[2] ?? join(ROOT, 'dist/search-v2-evaluation.json'))

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: ROOT, env, stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`${command} failed with exit code ${result.status}`)
}

function percentile(values, fraction) {
  if (values.length === 0) return null
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.ceil(fraction * sorted.length) - 1]
}

function quality(queries) {
  let documents = 0
  let passages = 0
  let documentHits = 0
  let passageHits = 0
  let reciprocalRank = 0
  let passageReciprocalRank = 0
  let ndcg = 0
  let passageNdcg = 0
  let answered = 0
  let passageQueries = 0

  for (const row of queries) {
    const labels = row.labels.filter((item) => item.grade > 0)
    if (labels.length > 0) {
      answered++
      documents += labels.length
      documentHits += labels.filter((item) => row.ids.slice(0, 10).includes(item.id)).length
      reciprocalRank += reciprocal(row.ids, labels, (item) => item.id)
      ndcg += discountedGain(row.ids, labels, (item) => item.id)
    }
    const sectionLabels = labels.filter((item) => item.section !== undefined)
    if (sectionLabels.length > 0) {
      passageQueries++
      passages += sectionLabels.length
      const found = row.passages.map((item) => `${item.document}:${item.section}`)
      passageHits += sectionLabels.filter((item) => found.slice(0, 10).includes(`${item.id}:${item.section}`)).length
      passageReciprocalRank += reciprocal(found, sectionLabels, (item) => `${item.id}:${item.section}`)
      passageNdcg += discountedGain(found, sectionLabels, (item) => `${item.id}:${item.section}`)
    }
  }
  return {
    queries: queries.length,
    judgedQueries: answered,
    document: {
      labeledRelevant: documents,
      recallAt10: documents ? documentHits / documents : null,
      mrrAt10: answered ? reciprocalRank / answered : null,
      ndcgAt10: answered ? ndcg / answered : null,
    },
    passage: {
      judgedQueries: passageQueries,
      labeledRelevant: passages,
      recallAt10: passages ? passageHits / passages : null,
      mrrAt10: passageQueries ? passageReciprocalRank / passageQueries : null,
      ndcgAt10: passageQueries ? passageNdcg / passageQueries : null,
    },
  }
}

function reciprocal(found, labels, key) {
  const accepted = new Set(labels.map(key))
  const index = found.slice(0, 10).findIndex((id) => accepted.has(id))
  return index < 0 ? 0 : 1 / (index + 1)
}

function discountedGain(found, labels, key) {
  const grades = new Map(labels.map((item) => [key(item), item.grade]))
  const gain = found.slice(0, 10).reduce((sum, id, index) =>
    sum + (2 ** (grades.get(id) ?? 0) - 1) / Math.log2(index + 2), 0)
  const ideal = [...grades.values()].sort((a, b) => b - a).slice(0, 10)
    .reduce((sum, grade, index) => sum + (2 ** grade - 1) / Math.log2(index + 2), 0)
  return ideal ? gain / ideal : 0
}

assert.equal(reciprocal(['miss', 'hit'], [{ id: 'hit' }], (item) => item.id), 0.5)
assert.equal(discountedGain(['direct', 'useful'], [
  { id: 'direct', grade: 2 }, { id: 'useful', grade: 1 },
], (item) => item.id), 1)
assert.equal(percentile([5, 1, 3], 0.5), 3)

try {
  run(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['test', '--', 'src/utils/searchEvaluation.test.ts'])
  run('cargo', ['test', '--offline', '--manifest-path', 'src-tauri/Cargo.toml', 'search_v2_evaluation', '--lib'], {
    ...process.env,
    PAPERCUT_SEARCH_EVAL_OUTPUT: rawPath,
  })
  const native = JSON.parse(readFileSync(rawPath))
  const byId = new Map(fixture.queries.map((query) => [query.id, query]))
  const rows = native.query_results.map((row) => {
    const query = byId.get(row.id)
    if (!query) throw new Error(`Unlabeled query ${row.id}`)
    return {
      id: row.id,
      mode: query.mode,
      split: query.split,
      language: query.language,
      category: query.category,
      rationale: query.rationale,
      labels: query.relevant,
      ids: row.first.ids,
      passages: row.first.passages,
      candidateDocuments: row.first.measurements.candidate_documents,
      totalDocuments: row.first.total_documents,
      firstMs: row.first.measurements.total_ms,
      warmMs: row.warm.map((sample) => sample.measurements.total_ms),
      firstPhasesMs: row.first.measurements,
    }
  })
  const grouped = (field) => Object.fromEntries(
    [...new Set(rows.map((row) => row[field]))].sort()
      .map((value) => [value, quality(rows.filter((row) => row[field] === value))]),
  )
  const phases = ['db_ms', 'candidate_ms', 'verification_ms', 'exact_evidence_ms', 'result_evidence_ms', 'term_matches_ms', 'result_ms', 'total_ms']
  const warmPhases = Object.fromEntries(phases.map((field) => {
    const samples = native.query_results.flatMap((row) => row.warm.map((sample) => sample.measurements[field]))
    return [field, { p50: percentile(samples, 0.5), p95: percentile(samples, 0.95) }]
  }))
  const commit = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8' })
  const report = {
    sourceCommit: commit.status === 0 ? commit.stdout.trim() : null,
    corpusVersion: native.corpus_version,
    corpusSha256: createHash('sha256').update(fixtureBytes).digest('hex'),
    device: {
      os: native.os,
      arch: native.arch,
      build: native.build,
      cpu: process.platform === 'linux'
        ? readFileSync('/proc/cpuinfo', 'utf8').match(/^model name\s*:\s*(.+)$/m)?.[1] ?? null
        : null,
      memoryMethod: native.peak_rss_kib == null ? 'unavailable' : 'Linux process VmHWM; includes the test binary and fixture creation',
      peakRssKiB: native.peak_rss_kib,
    },
    corpus: { documents: native.documents, sections: native.sections, sqliteBytes: native.sqlite_bytes, queries: rows.length },
    timingMethod: 'First run after fixture creation and five repeats, each reopening SQLite; OS page cache is not cleared. Milliseconds are native search pipeline timings.',
    metricsNote: 'Grades are 0 irrelevant, 1 useful, 2 direct. Labels are incomplete; unlabeled returns are scored as 0 for nDCG. Empty-relevance cases are contract checks and excluded from ranking averages.',
    metrics: { all: quality(rows), byMode: grouped('mode'), bySplit: grouped('split'), byLanguage: grouped('language'), byCategory: grouped('category') },
    performance: { firstMs: { p50: percentile(rows.map((row) => row.firstMs), 0.5), p95: percentile(rows.map((row) => row.firstMs), 0.95) }, warmPhases },
    queries: rows,
  }
  mkdirSync(dirname(outputPath), { recursive: true })
  writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`)
  console.log(`Search evaluation: ${outputPath}`)
  console.log(`Document Recall@10 ${report.metrics.all.document.recallAt10.toFixed(3)}; warm p95 ${report.performance.warmPhases.total_ms.p95.toFixed(2)} ms`)
} finally {
  rmSync(scratch, { recursive: true, force: true })
}
