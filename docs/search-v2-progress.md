# Search v2 progress

Branch: `feature/search-v2`. Plan: the locally supplied `Papercut_Search_v2_Codex_Implementation_Plan.md` handoff at the repository root. This file records completed stages; the current product search behavior remains the control until a later stage passes its gate.

## Stage 0 — Baseline

Status: complete. Source commit: `8be9952fe02a10921ea4955df204b115268bc686`. Corpus: [`scripts/fixtures/search-v2/corpus.json`](../scripts/fixtures/search-v2/corpus.json), version 1; its SHA-256 is in the [baseline report](search-v2-baseline.json).

Run `npm run test:search-eval` to generate a fresh report in `dist/search-v2-evaluation.json`. This command checks frontend query parsing, imports the synthetic corpus through the actual Rust store path into a disposable SQLite database, and calls the same native orchestration as the app. Pass an output path only when intentionally saving a comparison, e.g. `npm run test:search-eval -- docs/search-v2-stage1.json`. The checked-in baseline is `docs/search-v2-baseline.json`.

The corpus has 24 documents and 53 indexed sections, including one 24-section document, a PDF page locator, English and Arabic, mixed script, repeated terms, quotes, and misleading metadata. Thirty labeled queries use stable IDs: 23 development cases and 7 held-out cases. Each record includes language, query category, intended native clauses, optional filter scope, relevance grades, and a labeling rationale. The Rust check asserts phrase/filter exclusions, result deduplication and limits, deterministic ties, selected evidence counts and locations, and stable results over six runs. The TypeScript check asserts that every display query parses into the native clauses recorded in the fixture.

The baseline yields document Recall@10 **31/37 = 0.838**, MRR@10 **0.828**, and nDCG@10 **0.815**. English recall is **26/29 = 0.897**; Arabic is **3/6 = 0.500**. The seven held-out queries have **9/9 recall**, so they are a useful guard but do not establish general quality. Three queries have passage labels (four relevant passages); their Recall@10, MRR@10 and nDCG@10 are all **1.0**. One no-answer query is a contract check and is omitted from ranking averages. Category results and all query IDs/results are in the report.

Measured misses: `en-typo`, `en-concept`, `ar-mark-gap`, `ar-tatweel-gap`, `ar-alef-gap`, and one additional relevant document in `en-stem`. The first five are known recall gaps; `en-stem` shows that the existing English Porter tokenizer does not bridge every related form. Ranking is already strong on this small corpus, with `en-title-rank` placing a PDF body mention ahead of the labeled title result. Stage 1 should test whether field weights improve that example and other ranking cases without letting misleading titles dominate.

On a Linux x86-64 debug build with an Intel Core Ultra 5 325, the first run after fixture creation had p50 **1.08 ms** and p95 **2.03 ms** native pipeline time; five repeated runs per query had p50 **1.12 ms** and p95 **2.03 ms**. The detailed phase timings, database size and peak process memory method are recorded in the report. The operating-system page cache was not cleared; these are first-run and warm-repeat timings, **not true cold-disk or low-device measurements**. At this scale the latency variation is noise, not evidence of improvement. This tiny synthetic corpus cannot establish performance on hundreds of books or mobile devices. The earlier 500-document / 100,000-section gate in [`user-document-search.md`](user-document-search.md) remains the large-corpus gate.

**Measurement rules:** Recall@10 is the fraction of labeled relevant documents/passages in the first ten. MRR@10 is the mean reciprocal rank of the first labeled relevant hit, with zero for a miss. nDCG@10 uses grades 1 useful and 2 direct. Labels are incomplete, so unjudged results score zero in nDCG and recall can overstate real-world quality. Do not tune on the held-out set; add more blinded labels and a realistic large corpus before claiming a user-visible win. Passage metrics include only queries with section judgments. Device/process memory is a whole-test peak, not per-query allocation.

No ranking, query, UI, index, or source-navigation behavior changed in Stage 0. The evaluation uses a temporary database and does not open the installed app's library. Branch checkout alone does not isolate app data for manual Tauri runs; use a separate test library when testing future schema or index changes.

### Verified search map

| Boundary | Current path and behavior |
| --- | --- |
| Entry and parsing | `src/App.tsx` → `src/hooks/useSearch.ts`; `src/utils/phraseSearch.ts::parseSearchQuery` separates unquoted words and quoted clauses. |
| Bundled content | `src/hooks/usePagefind.ts` loads the build-time Pagefind index; `src/utils/phraseSearch.ts` verifies quoted source text. The Stage 0 quality corpus measures uploaded SQLite results only; the frontend parser and existing Pagefind tests remain separate checks. |
| Upload IPC | `src/uploads/DocumentUploads.ts::searchUploadedDocuments` → `src-tauri/src/document_uploads/commands.rs::document_uploads_search` → `search.rs::search_uploads`; blocking work and progress stages are already established. |
| Schema and lifecycle | `src-tauri/src/document_uploads/store.rs::open_db_in`, `upsert_document`, `delete_document_rows`; `pdf/index.rs` finalizes page text through the store. FTS5 columns: two unindexed IDs, then indexed title, heading, text; tokenizer: `porter unicode61 remove_diacritics 1`. Titles repeat per section row. |
| Query and ranking | `search/query.rs` handles token bounds, punctuation/hyphen aliases, safe FTS literals, and normalized phrase comparison. `search.rs::document_candidates` uses unweighted BM25, lower scores first, best section per document, and existing stable tie breaks. All unquoted terms are required; cross-section hits use document-level clause intersection. No edit-distance typo correction exists. |
| Evidence and serialization | `search.rs` verifies quoted phrases against source sections before limiting, then returns actual phrase counts or bounded passages, twelve section bins and per-term matching-section counts. Rust DTOs in `types.rs` serialize camelCase; `src/hooks/useSearch.ts` maps them to `src/types/search.ts`. Ordinary section counts are not word-occurrence counts. |
| Display and navigation | `src/components/SearchResults/SearchResults.tsx` renders evidence and lazy concordance; `src/utils/searchOpenTarget.ts`, `src/components/DocumentViewer/readerTarget.ts`, `readerTextRanges.ts`, and `src/viewers/pdfSearchTarget.ts` resolve sections, text and PDF pages. DOM offsets are mapped UTF-16 code units; current IPC uses IDs/text rather than raw offsets. |
| Optional model precedent | `src-tauri/src/native_tts/engine/model.rs` downloads/verifies local TTS assets; compatibility with an embedding runtime is unproven and belongs to a later experiment. |

Relevant checks: `npm run test:search-eval`; `cargo test --offline --manifest-path src-tauri/Cargo.toml document_uploads::search --lib`; `npm test -- src/hooks/useSearch.test.ts src/hooks/useSearchConcurrency.test.ts src/utils/phraseSearch.test.ts src/components/SearchResults/SearchResults.test.tsx src/viewers/pdfSearchTarget.test.ts`; `npm run build:typecheck`; `npm run lint`; `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`.

Validation on this checkout: all **125 frontend tests** passed; TypeScript, ESLint, Rust formatting, and the evaluation command passed. In the full Rust library suite, **132 tests passed**, **one optional sample-EPUB test was ignored**, and **two library-transfer socket tests were blocked by the filesystem/network sandbox**. Those two tests passed separately with local socket access. No search test failed.

## Next: Stage 1 — BM25 field weights

Try a few small title/heading/body ratios on the development set, then check the held-out set once. The FTS column order means any BM25 call needs weights for both unindexed ID columns as well as title, heading and text. Use the existing result order and stable ties. Inspect repeated titles, misleading metadata and section-level evidence before accepting a weight. Ship only if ranking improves without violating phrase, filter, count, locator, or latency contracts; otherwise record the result and skip the change.
