import { spawnSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { isDeepStrictEqual } from 'node:util'
import { ROOT } from './lib/paths.js'

if (process.argv[2] === '--library') {
  runLibraryEvaluation()
  process.exit(0)
}
if (process.argv[2] === '--prepare-library') {
  runLibraryPreparation()
  process.exit(0)
}
if (['--review', '--review-comparison', '--check-review'].includes(process.argv[2])) {
  checkReviewPreparation()
  if (process.argv[2] !== '--check-review') runReviewPreparation()
  process.exit(0)
}

function privatePath(path, existing = true) {
  const full = existing
    ? realpathSync(path)
    : join(realpathSync(dirname(resolve(path))), basename(path))
  const local = relative(realpathSync(ROOT), full)
  if (local === '' || (!isAbsolute(local) && local !== '..' && !local.startsWith(`..${sep}`))) {
    throw new Error('private evaluation inputs and output must be outside the checkout')
  }
  return full
}

function reviewPacket(report) {
  if (!Array.isArray(report.queries) || !report.queries.length) throw new Error('expected production query report')
  const seen = new Set()
  const key = { version: 1, snapshotSha256: report.snapshotSha256, sourceCommit: report.sourceCommit,
    modelSha256: report.modelSha256, int8ModelSha256: report.int8ModelSha256,
    inputReportSha256: report.inputReportSha256, broaderReportSha256: report.broaderReportSha256,
    hybridEvidencePolicy: report.hybridEvidencePolicy,
    rerankingModelSha256: report.reranking?.modelSha256, queries: [] }
  const grades = { version: 1, queries: [] }
  const lines = [
    '# Private search relevance review', '',
    'Grade the document and the displayed evidence separately: **0 irrelevant, 1 useful, 2 directly relevant**. Leave a grade **null** when unsure; an unjudged candidate is not irrelevant. Add a short reason in `grades.json`, or reply with the candidate code and your grades.', '',
    'Judge the intended information need, not just word presence. For a misspelling, judge the intended topic. If excerpts are insufficient, inspect the full source using its title and section/page; installed documents can be read in Papercut. Do not infer document relevance from a title alone.', '',
    'Each question pools up to ten results per available channel: direct, Broader, explicit spelling retry, semantic, hybrid, and reranked semantic, including both embedding variants when supplied. Candidate order is shuffled deterministically and ranks/channels are hidden. A separate `key.json` records their original ranks. Avoid opening that key until grading is complete.', '',
    'These bounded excerpts are not whole documents. Evidence grade covers the expanded excerpts together; the separately shown lead has its own grade. This pool cannot establish library-wide recall or a held-out semantic gain.', '',
    'Grade the result-card lead excerpt separately from its expanded evidence. The lead may differ from the first expanded passage. Lead and expanded-evidence grades must not be inferred from each other.', '',
  ]
  const md = (text) => text.replace(/\s+/g, ' ').replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/[\\`*_{}[\]()#!|]/g, '\\$&')
  const sourceText = (excerpt) => excerpt.replace(/<\/?mark>/g, '').replace(/&(amp|lt|gt|quot|#39);/g,
    (_, entity) => ({ amp: '&', lt: '<', gt: '>', quot: '"', '#39': "'" })[entity])
  const locator = ({ sectionIndex, pageIndex, byteStart, byteEnd, evidenceKind }) => ({ sectionIndex, pageIndex, byteStart, byteEnd, evidenceKind })
  const renderPassage = (passage) => {
    if (typeof passage.excerpt !== 'string' || !Number.isInteger(passage.sectionIndex) || passage.sectionIndex < 0 ||
      (passage.pageIndex != null && (!Number.isInteger(passage.pageIndex) || passage.pageIndex < 0)) ||
      (passage.sectionTitle != null && typeof passage.sectionTitle !== 'string') ||
      ((passage.byteStart != null || passage.byteEnd != null) && (!Number.isInteger(passage.byteStart) ||
        !Number.isInteger(passage.byteEnd) || passage.byteStart < 0 || passage.byteEnd <= passage.byteStart))) {
      throw new Error('invalid source locator or excerpt')
    }
    lines.push(`Section index ${passage.sectionIndex}${passage.sectionTitle ? ` · ${md(passage.sectionTitle)}` : ''}${passage.pageIndex == null ? '' : ` · Page ${passage.pageIndex + 1}`}`, '')
    if (passage.evidenceKind === 'semantic') lines.push('Conceptual source context; literal counts and highlights do not apply.', '')
    // Render source as inert code, including any authored HTML, after removing only native marks.
    lines.push(...sourceText(passage.excerpt).split(/\r?\n/).map((line) => `    ${line}`), '')
  }
  for (const [index, row] of report.queries.entries()) {
    if (!row || typeof row.id !== 'string' || !row.id.trim() || seen.has(row.id) || typeof row.query !== 'string') {
      throw new Error('invalid or duplicate query ID')
    }
    seen.add(row.id)
    const candidates = new Map()
    for (const [channel, response] of [['direct', row.response], ['broader', row.broaderResponse], ['retry', row.retry?.response], ['semantic', row.semanticResponse], ['hybrid', row.hybridResponse],
      ['semantic-int8', row.semanticInt8Response], ['hybrid-int8', row.hybridInt8Response],
      ['reranked-semantic', row.rerankedSemanticResponse], ['reranked-semantic-int8', row.rerankedSemanticInt8Response]]) {
      if (row.request?.exactPhrases?.length && (channel.includes('semantic') || channel.includes('hybrid')) && response != null) {
        throw new Error('mandatory quotation cannot receive semantic candidates')
      }
      if (channel !== 'direct' && response == null) continue
      if (!Array.isArray(response?.results)) throw new Error('expected native result evidence')
      const channelIds = new Set()
      for (const [rank, hit] of response.results.slice(0, 10).entries()) {
        if (typeof hit.documentId !== 'string' || !hit.documentId || channelIds.has(hit.documentId) ||
          typeof hit.title !== 'string' || typeof hit.excerpt !== 'string') throw new Error('invalid or duplicate native hit')
        channelIds.add(hit.documentId)
        const candidate = candidates.get(hit.documentId) ?? { hit, evidenceSets: [], channels: [], order: createHash('sha256').update(JSON.stringify([row.id, hit.documentId])).digest('hex') }
        const passages = hit.passages?.length ? hit.passages.slice(0, 3) : [hit]
        if (hit.semanticEvidence != null) {
          const source = hit.semanticEvidence
          const context = source.passages?.length ? source.passages : [source]
          if (!channel.includes('hybrid') || source.documentId !== hit.documentId || context.length > 2 ||
            context.some((passage) => passage.documentId !== hit.documentId || !Number.isInteger(passage.byteStart) || !Number.isInteger(passage.byteEnd))) {
            throw new Error('invalid hybrid semantic context')
          }
          passages.push(...context.map((passage) => ({ ...passage, evidenceKind: 'semantic' })))
        }
        const signature = JSON.stringify([hit, ...passages].map(({ excerpt, sectionIndex, pageIndex, byteStart, byteEnd }) => ({ excerpt: sourceText(excerpt), sectionIndex, pageIndex, byteStart, byteEnd })))
        let evidence = candidate.evidenceSets.findIndex((set) => set.signature === signature)
        if (evidence < 0) {
          evidence = candidate.evidenceSets.length
          candidate.evidenceSets.push({ signature, lead: hit, passages })
        }
        candidate.channels.push({ channel, rank: rank + 1, evidenceSet: evidence + 1 })
        candidates.set(hit.documentId, candidate)
      }
    }
    const question = `Q${String(index + 1).padStart(2, '0')}`
    lines.push(`## ${question}: ${md(row.query)}`, '')
    if (!candidates.size) lines.push('No candidates to grade; no relevance judgment is inferred.', '')
    const mappings = []
    const judgments = []
    const ordered = [...candidates.values()].sort((a, b) => a.order.localeCompare(b.order))
    for (const [position, { hit, channels, evidenceSets }] of ordered.entries()) {
      const code = `${question}-C${String(position + 1).padStart(2, '0')}`
      lines.push(`### ${code} · ${md(hit.title)}`, '')
      const sets = []
      for (const [setIndex, { lead, passages }] of evidenceSets.entries()) {
        const evidenceCode = `${code}-E${setIndex + 1}`
        lines.push(`Evidence set ${evidenceCode}`, '')
        lines.push('Lead excerpt (result card)', '')
        renderPassage(lead)
        lines.push('Lead excerpt grade: ___ · Reason: ___', '', 'Expanded evidence', '')
        for (const passage of passages) renderPassage(passage)
        sets.push({ code: evidenceCode, leadEvidenceGrade: null, evidenceGrade: null, notes: '' })
        lines.push('Evidence set grade: ___ · Reason: ___', '')
      }
      lines.push('Document grade: ___ · Overall evidence grade: ___ · Reason: ___', '')
      mappings.push({ code, documentId: hit.documentId, channels,
        locators: evidenceSets[0].passages.map(({ sectionIndex, pageIndex }) => ({ sectionIndex, pageIndex })), evidenceSets: evidenceSets.map(({ lead, passages }, index) => ({
        code: sets[index].code, leadLocator: locator(lead), locators: passages.map(locator),
      })) })
      judgments.push({ code, documentGrade: null, evidenceGrade: null, notes: '', evidenceSets: sets })
    }
    key.queries.push({ id: row.id, query: row.query, request: row.request, suggestedQuery: row.response?.suggestedQuery,
      directTotal: row.response?.totalDocuments, retryTotal: row.retry?.response?.totalDocuments, candidates: mappings })
    grades.queries.push({ id: row.id, candidates: judgments })
  }
  return { review: `${lines.join('\n')}\n`, key, grades }
}

function runReviewPreparation() {
  const comparison = process.argv[2] === '--review-comparison'
  if (process.argv.length !== (comparison ? 6 : 5)) throw new Error('usage: search-eval.mjs --review REPORT_JSON NEW_OUTPUT_DIRECTORY or --review-comparison FLOAT32_JSON INT8_JSON NEW_OUTPUT_DIRECTORY')
  const input = privatePath(process.argv[3])
  const output = privatePath(process.argv[comparison ? 5 : 4], false)
  if (existsSync(output)) throw new Error('review directory must be new')
  const bytes = readFileSync(input)
  let report = JSON.parse(bytes)
  if (comparison) {
    const right = readFileSync(privatePath(process.argv[4]))
    report = comparisonReport(report, JSON.parse(right))
    report.inputReportSha256 = { float32: createHash('sha256').update(bytes).digest('hex'),
      int8: createHash('sha256').update(right).digest('hex') }
  }
  const packet = reviewPacket(report)
  const reportSha256 = createHash('sha256').update(comparison ? JSON.stringify(report) : bytes).digest('hex')
  packet.key.reportSha256 = reportSha256
  packet.grades.reportSha256 = reportSha256
  mkdirSync(output, { mode: 0o700 })
  try {
    for (const [name, data] of [['review.md', packet.review], ['grades.json', JSON.stringify(packet.grades, null, 2)], ['key.json', JSON.stringify(packet.key, null, 2)]]) {
      writeFileSync(join(output, name), data, { flag: 'wx', mode: 0o600 })
    }
  } catch (error) {
    rmSync(output, { recursive: true, force: true })
    throw error
  }
  console.log(`Private grading packet: ${packet.grades.queries.length} questions, ${packet.grades.queries.reduce((sum, row) => sum + row.candidates.length, 0)} candidates; all grades unjudged.`)
}

function comparisonReport(float32, int8) {
  if (float32.modelQuantization !== 'none' || int8.modelQuantization !== 'dynamic-int8' ||
    !float32.modelSha256 || !int8.modelSha256 || float32.modelSha256 === int8.modelSha256) {
    throw new Error('expected distinct float32 and dynamic INT8 model reports')
  }
  for (const field of ['corpusSha256', 'snapshotSha256', 'lexicalSha256']) {
    if (typeof float32[field] !== 'string' || !/^[a-f0-9]{64}$/.test(float32[field]) || float32[field] !== int8[field]) {
      throw new Error('comparison inputs must share corpus, snapshot and lexical hashes')
    }
  }
  for (const field of ['documents', 'passages', 'semanticEvidencePolicy', 'hybridEvidencePolicy', 'broaderReportSha256']) {
    if (!isDeepStrictEqual(float32[field], int8[field])) throw new Error('comparison source or evidence policies differ')
  }
  if ([float32, int8].some((report) => report.queries?.some((row) => row.rerankedSemanticResponse != null)) &&
    (!float32.reranking?.modelSha256 || !int8.reranking?.modelSha256)) throw new Error('comparison reranking identity missing')
  for (const field of ['modelSha256', 'scoreType', 'documents', 'passagesPerDocument', 'pairTokens', 'aggregation', 'batchSize', 'intraThreads']) {
    if (!isDeepStrictEqual(float32.reranking?.[field], int8.reranking?.[field])) throw new Error('comparison reranking policies differ')
  }
  if (!Array.isArray(float32.queries) || !Array.isArray(int8.queries) || float32.queries.length !== int8.queries.length) {
    throw new Error('comparison question pools differ')
  }
  const right = new Map(int8.queries.map((row) => [row.id, row]))
  if (right.size !== int8.queries.length) throw new Error('duplicate comparison question')
  return { ...float32, int8ModelSha256: int8.modelSha256, queries: float32.queries.map((row) => {
    const other = right.get(row.id)
    if (!other || ['query', 'request', 'response', 'broaderResponse', 'retry', 'excluded'].some((field) => !isDeepStrictEqual(row[field], other[field]))) {
      throw new Error('comparison question, constraints or native controls differ')
    }
    if (row.request?.exactPhrases?.length && [row.semanticResponse, row.hybridResponse, row.rerankedSemanticResponse,
      other.semanticResponse, other.hybridResponse, other.rerankedSemanticResponse].some((value) => value != null)) {
      throw new Error('mandatory quotation cannot receive semantic candidates')
    }
    return { ...row, semanticInt8Response: other.semanticResponse, hybridInt8Response: other.hybridResponse,
      rerankedSemanticInt8Response: other.rerankedSemanticResponse }
  }) }
}

function checkReviewPreparation() {
  const hit = (id) => ({ documentId: id, title: 'Navigation guide', excerpt: '<mark>Compass</mark> &amp; <script>source text</script>', sectionIndex: 0, pageIndex: 2 })
  const report = { queries: [
    { id: 'navigation', query: 'compas navigation', response: { results: [hit('a')] }, retry: { response: { results: [hit('a'), hit('b')] } } },
    { id: 'empty', query: 'unknown', response: { results: [] } },
    { id: 'many', query: 'archive', response: { results: Array.from({ length: 12 }, (_, i) => hit(`d${i}`)) } },
  ] }
  const packet = reviewPacket(report)
  assert.deepEqual(packet, reviewPacket(report), 'packet ordering must be repeatable')
  assert.equal(packet.grades.queries[0].candidates.length, 2, 'deduplicate across direct and retry')
  assert.equal(packet.grades.queries[1].candidates.length, 0)
  assert.equal(packet.grades.queries[2].candidates.length, 10, 'bound each channel to ten')
  assert.deepEqual(packet.key.queries[0].candidates.find((row) => row.documentId === 'a').channels,
    [{ channel: 'direct', rank: 1, evidenceSet: 1 }, { channel: 'retry', rank: 1, evidenceSet: 1 }])
  assert.ok(packet.grades.queries.every((row) => row.candidates.every((item) => item.documentGrade === null && item.evidenceGrade === null)))
  assert.ok(packet.review.includes('    Compass & <script>source text</script>'), 'source HTML must be inert')
  assert.ok(!packet.review.includes('<mark>') && !packet.review.includes('rank:'))
  assert.throws(() => reviewPacket({ queries: [report.queries[0], report.queries[0]] }), /duplicate query/)
  assert.throws(() => reviewPacket({ queries: [{ id: 'bad', query: 'archive', response: { results: [{ ...hit('a'), sectionIndex: -1 }] } }] }), /locator/)
  assert.throws(() => reviewPacket({ queries: [{ id: 'duplicate', query: 'archive', response: { results: [hit('a'), hit('a')] } }] }), /duplicate native hit/)
  const semantic = { ...hit('a'), excerpt: 'Authored compass passage', byteStart: 10, byteEnd: 30 }
  const pooled = reviewPacket({ queries: [{ ...report.queries[0],
    semanticResponse: { results: [semantic, hit('c')] }, hybridResponse: { results: [semantic] },
  }] })
  const mapping = pooled.key.queries[0].candidates.find((row) => row.documentId === 'a')
  assert.equal(pooled.grades.queries[0].candidates.length, 3)
  assert.equal(mapping.evidenceSets.length, 2, 'retain different lexical and semantic evidence')
  assert.equal(mapping.channels.find((row) => row.channel === 'hybrid').evidenceSet, 2, 'deduplicate identical semantic/hybrid evidence')
  assert.ok(pooled.grades.queries[0].candidates.every((row) => row.evidenceSets.every((set) => set.evidenceGrade === null)))
  assert.ok(pooled.grades.queries[0].candidates.every((row) => row.evidenceSets.every((set) => set.leadEvidenceGrade === null)))
  assert.equal(mapping.evidenceSets[1].locators[0].byteStart, 10)
  assert.equal(mapping.evidenceSets[1].leadLocator.byteStart, 10)
  const expanded = [{ ...hit('a'), excerpt: 'A compass needle aligns with magnetic north.', sectionIndex: 1 }]
  const distinctLeads = reviewPacket({ queries: [{ ...report.queries[0],
    retry: undefined,
    response: { results: [{ ...hit('a'), passages: expanded }] },
    broaderResponse: { results: [{ ...hit('a'), excerpt: 'Navigation index', sectionIndex: 4, passages: expanded }] },
  }] })
  const leads = distinctLeads.key.queries[0].candidates.find((row) => row.documentId === 'a').evidenceSets
  assert.equal(leads.length, 2, 'identical expanded passages must not hide different result-card leads')
  assert.deepEqual(leads.map((set) => set.leadLocator.sectionIndex), [0, 4])
  assert.deepEqual(leads[0].locators, leads[1].locators)
  assert.ok(distinctLeads.review.includes('Lead excerpt (result card)') && distinctLeads.review.includes('Lead excerpt grade: ___'))
  assert.throws(() => reviewPacket({ queries: [{ ...report.queries[0], response: { results: [{
    ...hit('a'), sectionIndex: -1, passages: expanded,
  }] } }] }), /locator/, 'validate the actual lead even when expanded passages are valid')
  assert.throws(() => reviewPacket({ queries: [{ ...report.queries[0], semanticResponse: { results: [{ ...semantic, byteEnd: 0 }] } }] }), /locator/)
  const supportedHit = { ...hit('a'), semanticEvidence: semantic }
  const supported = reviewPacket({ queries: [{ ...report.queries[0], hybridResponse: { results: [supportedHit] } }] })
  const supportSet = supported.key.queries[0].candidates.find((row) => row.documentId === 'a').evidenceSets[1]
  assert.equal(supportSet.locators.length, 2, 'retain lexical evidence and conceptual context separately')
  assert.equal(supportSet.locators[1].evidenceKind, 'semantic')
  assert.ok(supported.review.includes('Conceptual source context; literal counts and highlights do not apply.'))
  const nativePassages = [0, 1, 2].map((sectionIndex) => ({ ...hit('a'), sectionIndex }))
  const complete = reviewPacket({ queries: [{ ...report.queries[0], hybridResponse: { results: [{
    ...supportedHit, passages: nativePassages,
    semanticEvidence: { ...semantic, passages: [semantic, { ...semantic, sectionIndex: 3 }] },
  }] } }] })
  const completeSet = complete.key.queries[0].candidates.find((row) => row.documentId === 'a').evidenceSets[1]
  assert.equal(completeSet.locators.length, 5, 'bound context to three lexical plus two semantic passages')
  assert.equal(nativePassages.length, 3, 'review preparation must not mutate the native passages')
  assert.throws(() => reviewPacket({ queries: [{ ...report.queries[0], hybridResponse: { results: [{ ...supportedHit,
    semanticEvidence: { ...semantic, documentId: 'b' } }] } }] }), /hybrid semantic/)
  assert.throws(() => reviewPacket({ queries: [{ ...report.queries[0], hybridResponse: { results: [{ ...supportedHit,
    semanticEvidence: { ...semantic, passages: [semantic, semantic, semantic] } }] } }] }), /hybrid semantic/)
  assert.throws(() => reviewPacket({ queries: [{ ...report.queries[0], hybridResponse: { results: [{ ...supportedHit,
    semanticEvidence: { ...semantic, byteEnd: 0 } }] } }] }), /locator/)
  assert.throws(() => reviewPacket({ queries: [{ ...report.queries[0], response: { results: [supportedHit] } }] }), /hybrid semantic/)
  const left = { ...report, corpusSha256: 'a'.repeat(64), snapshotSha256: 'b'.repeat(64), lexicalSha256: 'c'.repeat(64),
    modelQuantization: 'none', modelSha256: 'float32', queries: [{ ...report.queries[0], semanticResponse: { results: [semantic] } }] }
  const right = { ...left, modelQuantization: 'dynamic-int8', modelSha256: 'int8', queries: [{ ...left.queries[0],
    semanticResponse: { results: [{ ...semantic, excerpt: 'Different source context' }, hit('d')] } }] }
  const combined = reviewPacket(comparisonReport(left, right))
  assert.equal(combined.grades.queries[0].candidates.length, 3, 'include both variants without duplicate documents')
  const shared = combined.key.queries[0].candidates.find((row) => row.documentId === 'a')
  assert.equal(shared.evidenceSets.length, 3, 'keep distinct variant excerpts independently judgeable')
  assert.ok(shared.channels.some((row) => row.channel === 'semantic-int8'))
  assert.ok(combined.grades.queries[0].candidates.every((row) => row.documentGrade === null && row.evidenceSets.every((set) => set.evidenceGrade === null)))
  const reranking = { modelSha256: 'reranker', documents: 10, passagesPerDocument: 2, pairTokens: 512 }
  const rerankedLeft = { ...left, reranking, queries: [{ ...left.queries[0], rerankedSemanticResponse: left.queries[0].semanticResponse }] }
  const rerankedRight = { ...right, reranking, queries: [{ ...right.queries[0],
    rerankedSemanticResponse: { results: [...right.queries[0].semanticResponse.results].reverse() } }] }
  const reranked = reviewPacket(comparisonReport(rerankedLeft, rerankedRight))
  assert.equal(reranked.key.rerankingModelSha256, 'reranker')
  assert.deepEqual(reranked.grades, combined.grades, 'reranking identical evidence must not duplicate the blinded pool or assign grades')
  assert.equal(reranked.key.queries[0].candidates.find((row) => row.documentId === 'a').channels.find((row) => row.channel === 'reranked-semantic-int8').rank, 2)
  assert.ok(!reranked.review.includes('rerankScore') && !reranked.review.includes('rank:'))
  assert.throws(() => comparisonReport(rerankedLeft, { ...rerankedRight, reranking: { ...reranking, pairTokens: 256 } }), /reranking policies/)
  assert.throws(() => comparisonReport({ ...rerankedLeft, reranking: undefined }, { ...rerankedRight, reranking: undefined }), /reranking identity/)
  assert.throws(() => reviewPacket({ queries: [{ ...rerankedLeft.queries[0], request: { exactPhrases: ['silver compass'] }, semanticResponse: null }] }), /quotation/)
  const broad = reviewPacket({ queries: [{ ...report.queries[0], broaderResponse: { results: [hit('e')] } }] })
  assert.equal(broad.grades.queries[0].candidates.length, 3)
  assert.equal(broad.key.queries[0].candidates.find((row) => row.documentId === 'e').channels[0].channel, 'broader')
  const sameText = reviewPacket({ queries: [{ ...report.queries[0], broaderResponse: { results: [{ ...hit('a'), excerpt: hit('a').excerpt.replace(/<\/?mark>/g, '') }] } }] })
  assert.equal(sameText.key.queries[0].candidates.find((row) => row.documentId === 'a').evidenceSets.length, 1, 'highlight changes must not duplicate displayed evidence')
  assert.throws(() => comparisonReport(left, { ...right, corpusSha256: 'd'.repeat(64) }), /hashes/)
  assert.throws(() => comparisonReport(left, { ...right, hybridEvidencePolicy: { maximumSemanticPassages: 1 } }), /evidence policies/)
  assert.throws(() => comparisonReport(left, { ...right, queries: [{ ...right.queries[0], request: { mode: 'broader' } }] }), /constraints/)
  assert.throws(() => comparisonReport(left, { ...right, queries: [{ ...right.queries[0], response: { results: [] } }] }), /controls/)
  assert.throws(() => comparisonReport({ ...left, queries: [left.queries[0], { ...left.queries[0], id: 'second' }] },
    { ...right, queries: [right.queries[0], right.queries[0]] }), /duplicate/)
  const quoted = { ...left, queries: [{ ...left.queries[0], request: { exactPhrases: ['silver compass'] } }] }
  assert.throws(() => comparisonReport(quoted, { ...right, queries: [{ ...right.queries[0], request: quoted.queries[0].request }] }), /quotation/)

  const aligned = embeddingInputs({ snapshotSha256: 'same-source', queries: [{ id: 'scope', query: 'compass',
    request: { documentUrls: ['doc:a'], exactPhrases: ['silver compass'] }, ids: ['a'], response: { results: [hit('a')] },
  }] }, [{ id: 'a', url: 'doc:a', sections: [{ text: 'Authored source', page_index: 2 }] }])
  const corpus = JSON.parse(aligned.corpusBytes)
  assert.deepEqual(corpus.queries[0].scope, ['a'])
  assert.deepEqual(corpus.queries[0].phrases, ['silver compass'])
  assert.equal(corpus.documents[0].sections[0].page_index, 2)
  assert.equal(aligned.lexical.corpus_sha256, createHash('sha256').update(aligned.corpusBytes).digest('hex'))
  assert.throws(() => embeddingInputs({ queries: [{ request: { documentUrls: ['missing'] } }] }, []), /unknown document/)

}

function embeddingInputs(report, documents) {
  const urls = new Set(documents.map((doc) => doc.url))
  if (urls.size !== documents.length || new Set(documents.map((doc) => doc.id)).size !== documents.length) {
    throw new Error('duplicate embedding document identity')
  }
  for (const row of report.queries) {
    if (row.request.documentUrls?.some((url) => !urls.has(url))) throw new Error('embedding scope contains an unknown document')
  }
  const corpus = { snapshotSha256: report.snapshotSha256, documents, queries: report.queries.map((row) => ({
    id: row.id, query: row.query, category: 'development', phrases: row.request.exactPhrases,
    scope: row.request.documentUrls ? documents.filter((doc) => row.request.documentUrls.includes(doc.url)).map((doc) => doc.id) : [],
    relevant: [],
  })) }
  const corpusBytes = `${JSON.stringify(corpus)}\n`
  const lexical = { corpus_sha256: createHash('sha256').update(corpusBytes).digest('hex'),
    snapshotSha256: report.snapshotSha256, queries: report.queries }
  return { corpusBytes, lexical }
}

function runLibraryPreparation() {
  if (process.argv.length !== 5) throw new Error('usage: search-eval.mjs --prepare-library CORPUS_JSON NEW_EXTERNAL_DIRECTORY')
  const input = privatePath(process.argv[3])
  const output = privatePath(process.argv[4], false)
  if (existsSync(output)) throw new Error('prepared library directory must be new')
  mkdirSync(output, { mode: 0o700 })
  try {
    run('cargo', ['test', '--offline', '--manifest-path', 'src-tauri/Cargo.toml',
      'search_v2_prepare_library', '--lib', '--', '--ignored'], {
      ...process.env, PAPERCUT_SEARCH_EVAL_INPUT: input, PAPERCUT_SEARCH_EVAL_PREPARED_LIBRARY: output,
    })
  } catch (error) {
    rmSync(output, { recursive: true, force: true })
    throw error
  }
  console.log('External evaluation library prepared through the production document store.')
}

function runLibraryEvaluation() {
  if (process.argv.length !== 6 && !(process.argv.length === 8 && process.argv[6] === '--embedding-input')) {
    throw new Error('usage: search-eval.mjs --library LIBRARY_SQLITE QUERIES_JSON OUTPUT_JSON [--embedding-input NEW_EXTERNAL_DIRECTORY]')
  }
  const [library, queries, output] = process.argv.slice(3, 6).map((path, index) => privatePath(path, index !== 2))
  if (output === library || output === queries) throw new Error('output must not overwrite an input')
  if (existsSync(output)) throw new Error('output must be a new file')
  const embeddingDirectory = process.argv[7] ? privatePath(process.argv[7], false) : null
  if (embeddingDirectory && existsSync(embeddingDirectory)) throw new Error('embedding input directory must be new')
  const temp = mkdtempSync(join(tmpdir(), 'papercut-library-eval-'))
  try {
    const requests = join(temp, 'requests.json')
    const native = join(temp, 'native.json')
    const env = {
      ...process.env,
      PAPERCUT_SEARCH_EVAL_QUERIES: queries,
      PAPERCUT_SEARCH_EVAL_REQUESTS: requests,
      PAPERCUT_SEARCH_EVAL_DB: library,
      PAPERCUT_SEARCH_EVAL_SNAPSHOT: join(temp, 'snapshot.sqlite3'),
      PAPERCUT_SEARCH_EVAL_OUTPUT: native,
      ...(embeddingDirectory ? { PAPERCUT_SEARCH_EVAL_CORPUS: join(temp, 'documents.json') } : {}),
    }
    run(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['test', '--', 'src/utils/searchEvaluation.test.ts'], env)
    run('cargo', ['test', '--offline', '--manifest-path', 'src-tauri/Cargo.toml',
      'search_v2_library_evaluation', '--lib', '--', '--ignored'], env)
    const report = JSON.parse(readFileSync(native, 'utf8'))
    const input = new Map(JSON.parse(readFileSync(queries, 'utf8')).map((query) => [query.id, query]))
    const parsed = new Map(JSON.parse(readFileSync(requests, 'utf8')).map((row) => [row.id, row.request]))
    for (const row of report.queries) {
      row.query = input.get(row.id).query
      row.request = parsed.get(row.id)
    }
    const commit = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8' })
    report.sourceCommit = commit.status === 0 ? commit.stdout.trim() : null
    report.queriesSha256 = createHash('sha256').update(readFileSync(queries)).digest('hex')
    report.snapshotSha256 = createHash('sha256').update(readFileSync(env.PAPERCUT_SEARCH_EVAL_SNAPSHOT)).digest('hex')
    if (embeddingDirectory) {
      const documents = JSON.parse(readFileSync(env.PAPERCUT_SEARCH_EVAL_CORPUS, 'utf8'))
      const { corpusBytes, lexical } = embeddingInputs(report, documents)
      mkdirSync(embeddingDirectory, { mode: 0o700 })
      try {
        writeFileSync(join(embeddingDirectory, 'corpus.json'), corpusBytes, { flag: 'wx', mode: 0o600 })
        writeFileSync(join(embeddingDirectory, 'lexical.json'), `${JSON.stringify(lexical)}\n`, { flag: 'wx', mode: 0o600 })
      } catch (error) {
        rmSync(embeddingDirectory, { recursive: true, force: true })
        throw error
      }
    }
    writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 })
    console.log(`Read-only production evaluation: ${report.queries.length} queries; report written outside checkout.`)
  } finally {
    rmSync(temp, { recursive: true, force: true })
  }
}

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
      suggestedQuery: row.first.suggested_query ?? undefined,
      passages: row.first.passages,
      candidateDocuments: row.first.measurements.candidate_documents,
      totalDocuments: row.first.total_documents,
      firstMs: row.first.measurements.total_ms,
      warmMs: row.warm.map((sample) => sample.measurements.total_ms),
      firstPhasesMs: row.first.measurements,
    }
  })
  const currentById = new Map(rows.map((row) => [row.id, row]))
  const baseline = JSON.parse(readFileSync(join(ROOT, 'docs/search-v2-baseline.json')))
  for (const previous of baseline.queries) {
    assert.ok(byId.has(previous.id), `missing baseline query ${previous.id}`)
    const current = currentById.get(previous.id)
    if (!current) continue // A selected evaluation split may omit this query.
    for (const label of previous.labels.filter((item) => item.grade > 0 && previous.ids.slice(0, 10).includes(item.id))) {
      assert.ok(current.ids.slice(0, 10).includes(label.id), `${previous.id}: lost baseline relevant document ${label.id}`)
    }
  }
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
