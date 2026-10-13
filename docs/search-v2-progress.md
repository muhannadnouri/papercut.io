# Search v2 progress

Branch: `feature/search-v2`. Plan: the locally supplied `Papercut_Search_v2_Codex_Implementation_Plan.md` handoff at the repository root. This file records completed stages; the current product search behavior remains the control until a later stage passes its gate.

## Current decision — Stage 7 opt-in integration approved

**On 2026-10-09 the user authorized Stage 7 development with a default-off semantic toggle.** Lexical search remains the default. The user chose a separate labeled conceptual-results section alongside existing All words/Broader results, preserving their semantics. Only a user who enables semantic search should receive conceptual results or a visible lexical fallback when the provider/index is unavailable, incomplete or fails. This is authorization to integrate, not a claim of release acceptance or an independently demonstrated overall ranking win. Cross-language ranking is a lower product priority; same-language relevance, required quotes, filters, source evidence and resource safety remain requirements. Retain the model-free improvements and isolated experimental controls. The rejected reranker remains removed. Earlier sections below record historical decisions and deferrals, not the current authorization.

The [conceptual contract review](#stage-6--conceptual-contract-review-2026-10-08)
retains semantic-first ordering with native Broader evidence as the experimental
candidate. It recovers missing displayed answer clauses in familiar controls,
but leaves mixed-language ranking and unavailable-answer failures unresolved.
The bounded model/fusion comparisons are complete; do not schedule another sweep.
The [native reload check](#native-provider-reload-check--2026-10-08) adds bounded
same-process lifecycle evidence, including higher reload memory peaks; it does
not close native stability or device responsiveness.
The [command loading check](#native-command-loading-check--2026-10-08) verifies
status, early cancellation and reactivation through actual handlers on the mock
runtime; actual WebView startup/recovery acceptance remains open.
The [cache interruption check](#cache-process-interruption-check--2026-10-08)
covers abrupt termination and fresh-process retry at the installer boundary
using tiny assets. The [real-cache command restart](#real-cache-command-restart--2026-10-08)
adds fresh-process command recovery with pinned weights; upstream transfer,
WebView and mobile acceptance remain open. The
[optimized native check](#optimized-native-validation--2026-10-08) passes the
existing provider, Arabic audio and command-recovery checks in release tests;
it leaves the original crash and resource/device caveats unresolved.
Earlier, the user reported all five consolidated smoke groups passed, including
the explicitly compiled model-cache UI, and an earlier full CI pass. Device
details and individual smoke logs were not inspected. Full CI for the dated
2026-10-10 source is recorded below; current-source acceptance remains open.
The earlier smoke/cache checks preceded the new indexing commands and do not
validate semantic ranking or mobile provider runtimes.

The [model-free acceptance review](#model-free-acceptance-review--2026-10-08)
confirms the candidate passes fresh default-feature and frontend regressions
without FastEmbed. The automated review is complete; normal-build platform
readiness is recorded in the [normal packaging review](#normal-packaging-and-platform-readiness--2026-10-08).
Normal and experimental-cache app smoke is user-reported passed on the tested
machine. Other device/provider acceptance and semantic quality remain open.

| Gate | Current evidence and decision |
| --- | --- |
| Model-free search | Ranking, Broader mode, source evidence, authored-word spelling help, Arabic normalization and query corrections are implemented on this branch. A 500-document / 100,000-section saturated-match native probe passes common-query timing thresholds on this development host; it does not establish weak-device suitability. The user reports ordinary-search/reader/audio smoke passed. General synonym expansion is unclaimed; hard-coded word pairs were removed. |
| Embedding baseline | Cached FP32/INT8 providers, anchored passages, incremental SQLite indexing, read-only queries and document-level fusion remain experimental. The standalone report now includes an evidence-preserving semantic-first response alongside the unchanged semantic/RRF controls. The comparison accepts verified Small/Base metadata and dimension-safe 384/768-vector indexes; the app now also offers optional Base FP32 and Large FP32 with dimension-safe 768/1,024-vector indexes. Small retains its existing FP32/INT8 selections and identities. Native primary evidence and separate conceptual context are preserved. Semantic ordering does not enforce unquoted names/dates and must not redefine All words. |
| Representative quality | The [public human-label comparison](#stage-6--public-human-label-comparison-2026-10-08) supports same-language conceptual passage ranking. The [wider-pool check](#stage-6--frozen-wider-pool-check-2026-10-08) and [fresh-query verification](#stage-6--fresh-query-fusion-verification-2026-10-08) confirm Broader fusion can replace positive first results with judged negatives and reduce known-positive recall. Keep semantic-only ordering as the conceptual control; the fresh English All words result cautions against a universal semantic-first policy. Base's mixed gain and higher cost do not establish an overall acceptance win. Prior cross-language/entity/date and missing-answer-span failures remain. Public passage labels do not establish private-book, displayed-answer or full-library quality. The original independent packet remains unjudged; product acceptance remains open. |
| Cached reranker/context augmentation | Rejected: a cross-language gain introduces a same-language date regression; combined-context and explicit-exclusion controls do not justify a repair. Its live CLI/module are removed. |
| Resource and platform readiness | The app now uses official Libtashkeel 1.5.0's public inference interface with ORT `rc.13`, a checksum-verified bundled model and no package fork. Linux native tests and a production Debian package pass; the historical shared-runtime experiment reused the app adapter with FastEmbed/sherpa and is now retired in favor of app-native checks. Provider-enabled Windows/macOS/Android builds and iOS simulator archive/device linking now pass CI, as recorded below. Linked initialization is runtime-tested on Linux and the iOS arm64 simulator; Android x86_64 shared-runtime FP32/INT8 inference and clean exit now pass the bounded mobile review below. Mobile app linking remains separate from inference evidence. Unix build copies now replace shared libraries atomically. The earlier native crash remains unexplained, and measured allocator retention prevents promising immediate RSS recovery on disable. Desktop FP32/INT8 initialization and the bounded provider lifecycle now pass on Linux, Windows and both macOS architectures in the verified run below. Android arm64/physical-device execution, weak reference hardware, responsiveness and app-level lifecycle/fallback validation remain open. |
| Production optional hybrid | Stage 7 development is explicitly authorized with a default-off toggle and lexical fallback only within the enabled semantic path. App indexing, saved opt-in/lifecycle controls, native retrieval and the separate React conceptual-results/fallback path are implemented below. Functional settings no longer require Developer mode. Quality, resource and platform limits remain release gates. |

**Automated acceptance was recorded for the 2026-10-10 source; current-source
CI and interactive/product release gates remain open.** See the
[current consolidation](#stage-78--branch-acceptance-consolidation--2026-10-12) and
the dated [final review below](#stage-7--final-automated-branch-acceptance-review--2026-10-10). Native retrieval,
functional Settings and the separate conceptual-results UI are implemented.
Lexical remains default; native shipping scripts now include provider capability
without enabling saved user consent. The subsequent
[keyword-results cleanup](#stage-7--keyword-results-and-starter-eligibility--2026-10-11) has local
frontend validation, followed by the
[activity and results presentation pass](#stage-7--compact-search-activity-and-results-copy--2026-10-11);
the [Search Mode heading cleanup](#stage-7--search-mode-heading-cleanup--2026-10-11)
and the [conceptual summary cleanup](#stage-7--conceptual-results-summary--2026-10-11),
followed by the [settings readiness cleanup](#stage-7--settings-readiness-and-copy--2026-10-11),
also have focused frontend/browser validation. The subsequent
[stored-work and conditional layout pass](#stage-7--stored-work-guards-and-conditional-settings--2026-10-11)
adds native storage-status and frontend validation. The subsequent [optional Base/Large pass](#stage-7--optional-base-and-large-models--2026-10-11) adds locally tested model choices. The [main-content evidence pass](#stage-7--main-content-evidence-and-passage-roles--2026-10-12) adds structural cleanup with preserved source anchors. The [natural-boundary pass](#stage-7--sentence-and-paragraph-windows--2026-10-12) adds bounded sentence context within those source regions. The subsequent [passage-window evaluation](#stage-7--passage-window-quality-and-cost-review--2026-10-12) finds a bounded local usefulness gain while leaving backmatter classification, missing context and unsupported-query failures open. The [PDF backmatter correction](#stage-7--pdf-backmatter-classification--2026-10-12) addresses the measured reference/index role gaps without claiming an unsupported-query relevance gain. The [alternative-passage review](#stage-7--alternative-passages-and-weak-matches--2026-10-12) rejects an unhelpful expansion and leaves selection/unsupported-query quality open. The subsequent [candidate-coverage diagnosis](#stage-7--candidate-coverage-diagnosis--2026-10-12) distinguishes two selection failures from an explanation missing from the bounded pool; it justifies a representation experiment, not a shipped reranker. The subsequent [adjacent-body experiment](#stage-7--bounded-adjacent-body-embedding-experiment--2026-10-12) reduces derived vector storage and improves one fresh first-result bundle, but does not justify shipping grouping or establish a general relevance win. The [global candidate-selection feasibility pass](#stage-7--bounded-global-candidate-selection-feasibility--2026-10-12) establishes complete reference witnesses within a fixed global pool, justifying a bounded local selector trial with an explicit evidence-token budget; it does not demonstrate improved ranking. The subsequent [reranker input gate](#stage-8--bounded-reranker-input-gate--2026-10-12) rejects full-bundle single-pair scoring because the cached model cannot accept that evidence intact; compact source-aware inputs remain an experiment, not a shipping change. The subsequent [compact-window trial](#stage-8--compact-scoring-window-trial--2026-10-12) also fails its frozen size/evidence gate; keep the reranker rejected and address the measured source-role gap before further selector work. The subsequent [wrapped-index correction](#stage-7--wrapped-pdf-index-entries-and-cross-references--2026-10-12) fixes that measured role gap with generic guards and a PDF-only upgrade; model-ranking acceptance remains open. The [updated-index retrieval regression](#stage-7--updated-pdf-role-retrieval-regression--2026-10-12) preserves retained results and native contracts but shows no new relevance gain. The [fresh same-language review](#stage-7--fresh-same-language-relevance-review--2026-10-12) finds useful first passages for all six fully supported questions, but only five complete answers with nearby text; unsupported nearest matches and one context-selection gap remain. The [focused selection diagnosis](#stage-7--fresh-result-selection-and-context-diagnosis--2026-10-12) finds complete source witnesses inside the fixed global pool with unchanged nearby context; choosing them remains unsolved. The subsequent [context-capable scorer input gate](#stage-8--context-capable-scorer-input-fit-gate--2026-10-12) preserves all tested full evidence pairs within frozen token bounds, permitting only a controlled inference/resource trial; relevance and runtime compatibility are untested. The subsequent [native scorer cost gate](#stage-8--context-capable-scorer-native-cost-gate--2026-10-12) validates intact long-input inference but stops before broader grading because one warm pair exceeds the whole-query budget; keep this scorer unintegrated. The recorded full CI predates these UI, model and content changes. The first
integrated scale probe found and fixed repeated common-phrase FTS scans without
changing ranking, quote rules or coverage. Interactive lifecycle/results smoke
remains explicitly deferred by the user; the earlier five-group pass predates
this UI. Native stability, allocator retention, reference hardware, platform
runtimes and representative same-language quality remain release gates.

## Stage 7 — Combined settings cleanup — 2026-10-12

User approved removing both background-work notices and simplifying index
maintenance; the subsequent styling feedback applies to both removal buttons.
Download and indexing progress retain their explicit Cancel Download/Stop
Indexing controls. Persistent header activity and native job lifetimes are
unchanged; closing Settings still does not cancel work.

Library Index keeps its outer disclosure but no longer nests Index Details.
Document coverage stays visible, and a nonzero active passage count appears as
muted supporting text. The empty state omits zero passages and the duplicate
No Saved Index sentence. Refresh Status shares the existing responsive removal
row with Remove Index; Build/Update Index stays separate and primary. Inline
confirmation, focus return and busy/stored-work guards remain intact.

Remove Model Files, Remove Index and their confirmation buttons use the app's
standard tonal surface and transparent border with danger text. Hover does not
add a pink fill/outline; keyboard focus retains a visible danger-colored ring.
Disabled removal remains muted. Search-owned CSS and unused disclosure/notice
translations are cleaned up across all eight locales. No native, runtime,
dependency or CI change is introduced.

Validation: all 217 frontend/tooling tests across 50 files, locale parity,
typecheck, lint, frontend production build and diff checks pass. Existing tests
cover maintenance dispatch/adjacency, zero-count presentation, progress and
cancellation without notices, and busy/removal guards. An isolated actual-component
browser preview with synthetic IPC confirms light/dark tonal styling, desktop
maintenance alignment, confirmation-cancel focus return, visible keyboard focus,
empty-index presentation and 320px Arabic RTL stacking without page overflow.
This is frontend evidence, not installed-app or physical-device smoke.

Smoke remains explicitly deferred, including header activity after Settings
closes. Use the updated Settings Action Presentation Follow-Up when testing
resumes. Next review the user's current CI result against its exact source SHA;
no fresh provider matrix is justified solely by this presentation change.

## Stage 7 — Settings action layout and download status — 2026-10-12

User approved the presentation follow-up; installed-app smoke stays deferred.
Download/Verify and Remove Model Files share a spaced action row on wide views,
with preparation at the leading edge and removal at the trailing edge. Narrow
views stack them. Confirmation remains inline below that row, names the actual
removal action, and retains the stored-file/index, busy, active-model and repeat
removal guards. Dismissing confirmation returns focus to the removal trigger,
not the newly adjacent preparation button.

Search-owned CSS keeps removal's danger text/border/background and visible focus
in both themes even when general Settings hover rules load later. Disabled
removal is muted. Cancel Download and Stop Indexing remain neutral secondary
controls; removal is recoverable by downloading/rebuilding, without deleting
imported documents. Consent, download/index dispatch and cancellation cleanup
behavior are unchanged.

Active cache work suppresses idle absent/incomplete/present recovery instructions.
Byte-transfer events show downloading; explicit existing-file checks show
verification, and removal has its own status. Because the native event contract
has no separate verification phase, unknown/final pending work uses honest
preparation/verification wording rather than claiming installation at 100%.
Only the status line announces updates; byte ticks are not a live region.
Reopening uses the active download's model/format label, even when the selected
format differs. One muted background sentence remains beside progress and
Cancel Download only during active work. All eight locales include the clarified
labels; no native event/schema/runtime/dependency or CI change is introduced.

Validation: 217 frontend/tooling tests across 50 files, locale parity, typecheck,
lint, frontend production build and diff checks pass. The focused UI regressions
cover active/terminal cache copy, verification, cancellation guards, unrelated
progress, format identity and action-specific removal confirmation. An isolated
actual-component browser fixture with synthetic IPC checks desktop action
separation, light/dark danger and confirmation focus, active transfer copy and
320px Arabic RTL stacking without page overflow. This is visual/frontend evidence,
not installed WebView, network download, screen-reader or physical-device acceptance.

The existing CI run was already underway before this UI commit; review its exact
source SHA when supplied and do not call it coverage of this later delta. No new
provider matrix or model-inference run is required for this presentation-only
change. Push this commit, review CI evidence, and use the updated consolidated
smoke checklist when the user resumes testing. No smoke case is marked passed.

## Stage 7/8 — Branch acceptance consolidation — 2026-10-12

**Decision: retain the implemented default-off semantic feature and close the
tested scorer route under its frozen resource policy. Current merge/release
acceptance remains open; no further speculative implementation is scheduled.**
This pass reconciles the original plan, latest application boundaries and retained
evidence. It is documentation/acceptance tracking, not a new whole-branch audit,
relevance comparison, CI verification or smoke run.

Reviewed source `e6fe8cc716d4ea6724cdec7ca654fc9ec3c6ff5e`; application code is
unchanged since `5b29ab4`. The last verified full/inference CI in the record is
`38110242011` on `1f020d3`. It predates the later UI, model-choice, source-role,
window and nearby-text changes. Preserve that historical result without calling
current-source automated acceptance complete.

| Original plan stage | Current disposition |
| --- | --- |
| 0–4: lexical baseline/ranking, Broader, typo help, Arabic forms | Implemented with native/frontend regression evidence. These remain independent of semantic consent; current UI/source navigation smoke is still pending. |
| 5: controlled aliases | Intentionally deferred following the user's removal of all hard-coded word pairs. No maintained synonym dictionary or universal synonym coverage is promised. |
| 6: embedding evaluation | Bounded same-language usefulness and resource evidence exist, with provisional private grading and retained failures. Representative independent quality and reference-device evidence remain open. |
| 7: optional integration | User-authorized implementation includes explicit downloads/indexing, saved default-off consent, separate conceptual results and lexical fallback. Shipping capability is integrated; current CI, installed-app/source upgrade smoke and platform/resource/product gates remain open. |
| 8: measured optimization | Candidate-selection failure justified the experiment. The compact policies and tested long-context FP32 CPU scorer fail their gates; none is integrated. Stop this route under the tested limits; no ANN/model/cap/thread sweep is justified. |

The [current branch acceptance summary](search-v2-branch-review.md#current-acceptance-consolidation--2026-10-12)
now separates implemented contracts, evidence, known quality limitations and
remaining gates. The [smoke checklist](search-v2-smoke.md) opens with current status,
shipping build commands and a six-round off/on run order. Remove obsolete
provider-free artifact guidance and duplicated historical CI narrative from that
checklist; keep the dated evidence in this progress log. Preserve every existing
manual checkbox and follow-up case, including schema-11 PDF upgrades and model
switching. No case is newly marked passed, and no smoke is required during this
user-deferred pass.

The fresh development batch's 5/6 complete expanded supported answers and 0/4
useful unsupported-question results remain limits, not representative acceptance.
Complete source witnesses for the remaining supported case are present in a
bounded pool, but the tested scorer does not meet query cost. Neither opt-in nor
passing input/runtime checks proves a better selector or calibrated no-answer
behavior. Base/Large remain optional embedding choices, with no blanket
platform-quality/resource claim from Small FP32/INT8 CI.

Validation: current build/CI/model/opt-in/results/index-policy boundaries are
read against the original plan; documentation links, preserved checkbox text,
build commands, source-tree identity and diff checks pass. No application,
runtime, dependency, CI or private-corpus file is changed. Memsearch recall,
expansion and memory append/readback work. Unchanged application suites and
platform builds are not rerun for this documentation-only pass.

**Next:** push and run the existing `validation_scope=full` workflow with
`provider_inference=true` once for the current native/model source, then review
that actual run/SHA. Keep inference optional for routine CI. Resume the
consolidated installed-app smoke only when the user is ready; preserve any
physical-device, independent-quality and signed-distribution deferrals honestly.
Fix observed defects rather than start another automatic optimization pass.
No push, dispatch, merge or release is performed here.

## Stage 8 — Context-capable scorer native cost gate — 2026-10-12

**Decision: do not integrate the tested FP32 CPU scorer.** Initialization and
intact long-input inference pass, but one warmed pair exceeds the frozen budget
for an entire bounded query. Stop before broader relevance grading; no ranking
benefit or regression outcome is established. The earlier tokenizer fit remains
valid, and current cosine selection is unchanged.

Before downloading or running weights, freeze the same 24-candidate,
8,192-token pair and 65,536-token query limits, CPU inference with two intra-op
threads, batch size one, a five-second warm whole-query target, a 60-second
session-load limit and a 6 GiB combined process-RSS ceiling. A watchdog also
limits an individual inference phase/query to 30 seconds. These are conservative
development-trial decision limits, not measured user tolerance or promises for
all hardware. The gate order permits retained/fresh relevance testing only after
resource checks pass. Do not change limits, truncate evidence or drop candidates
after seeing the result.

Download the previously pinned `rozgo/bge-reranker-v2-m3` export at
`fbd57b17b4db111a9d16813bb08b4c804fac18e9` into a disposable directory. Verify
both its 108,479-byte ONNX graph and 2,271,088,656-byte external data file against
the published LFS SHA-256 and sizes before native loading. No private query or
source text is sent to the download host. Reuse cached FastEmbed 7.1.0/ORT
`rc.13` Rust libraries and the existing Linux shared runtime; the actual loaded
ONNX Runtime reports **1.27.0**. No new dependency, compatibility adapter or
shipping model is introduced.

The native child keeps the existing verified Small FP32 embedding provider
loaded and initialized while loading/scoring with the cross-encoder. Both use
two intra-op threads. Explicitly set the scorer limit to 8,192, then disable
truncation/padding and validate the tokenizer boundaries before inference.
Select the largest frozen pair by token count, without consulting its judgment;
its **2,611 tokens** include the full query/title/heading/source-context union.
Run the same pair twice at batch size one. This is an input/cost probe of a
retained development pair, not a fresh quality case or the full focused query.

| Native Linux check | Observed result |
| --- | --- |
| Scorer session initialization in a fresh process | 1.834 seconds |
| First intact 2,611-token pair | 10.470 seconds |
| Warm repeat of that same pair | 10.389 seconds |
| Combined child-process peak RSS with embedding provider present | 2.981 GiB |
| Inference output/repeat | Finite, identical scalar score |
| Native teardown | Exit 0; no watchdog termination |
| Five-second whole-query resource gate | Failed by one warm pair alone |
| Full 24-candidate query, retained controls and fresh relevance grading | Not run after the cost failure |

The load measurement has a warm filesystem/page cache after asset download and
verification; it is not a true cold-storage benchmark. Peak RSS is the observed
short probe's coexisting-process peak, not incremental scorer memory, total
system memory including file cache, or a worst-case full-query result. The
embedding session is idle during scoring; foreground reader/audio contention is
not tested. Cached Rust library builds are reused rather than a packaged release
app; the native ORT library performs inference. These measurements do not prove
all queries, platforms, accelerators or quantized variants would have the same
cost. No 8,192-token graph inference is attempted: the boundary controls validate
tokenizer/guard behavior, while actual graph inference covers 2,611 tokens.

The repeated pair already costs more than the allowed entire query, so the
frozen gate stops before the focused global pool, name/date/exclusion controls
or fresh same-language batch. There is no complete-query latency extrapolation,
new selector ranking, relevance grade or answerability cutoff. The scalar score
is not used as confidence. Historical unsupported-query and passage-selection
failures remain; acceptable short-probe memory and clean exit do not override
the query-time failure.

Validation: original input/source/model/runtime hashes pass before and after
execution; linked-library resolution and its version are recorded. Assert-based
checks cover input limits, finite/deterministic output, watchdog time/RSS bounds,
expected gate failure and absence of downstream query/grading phases. Private
reports/helpers and model assets remain outside the checkout. The owned probe
executable is removed. No app/index/runtime/dependency/CI change occurs; diff
checks pass and unchanged application suites are not rerun. Memsearch recall,
expansion and memory append/readback work.

**Next:** close this optional scorer route under the current policy and
consolidate branch acceptance around the implemented default-off semantic
feature, its known relevance limits and the pending smoke checklist. A slower
optional scorer, acceleration or another precision/runtime would require a
separate product/resource decision and evidence; no automatic model, candidate,
thread or clipping sweep is justified. Do not present this result as Stage 7
quality acceptance or a completed relevance comparison. Current-revision CI,
independent quality, physical-device performance and app acceptance remain open.
Consolidated smoke remains deferred.

## Stage 8 — Context-capable scorer input-fit gate — 2026-10-12

**Decision: one local scorer passes the bounded tokenizer/input gate with full
candidate evidence. Proceed only to a controlled native inference and resource
trial; no ranking improvement or shipping integration is accepted.** The cached
512-token scorer and its failed compact-window policy remain rejected. Current
cosine selection, app models, source roles and nearby-context behavior are unchanged.

The single candidate is `BAAI/bge-reranker-v2-m3`, pinned at
`953dc6f6f85a1b2dbfca4c34a2796e7dde08d41e`. Its
[publisher tokenizer configuration](https://huggingface.co/BAAI/bge-reranker-v2-m3/blob/953dc6f6f85a1b2dbfca4c34a2796e7dde08d41e/tokenizer_config.json)
permits 8,192 tokens; the
[model configuration](https://huggingface.co/BAAI/bge-reranker-v2-m3/blob/953dc6f6f85a1b2dbfca4c34a2796e7dde08d41e/config.json)
has 8,194 positions with the matching padding/position offset. This is a joint
query-plus-document limit, including special tokens. Download only pinned
configuration, tokenizer and model-card metadata, verifying regular Git blobs
and LFS sizes/SHA-256 where applicable. No graph or weights are downloaded and
no model session is initialized.

Freeze before counting:

- At most **24 candidates**, **8,192 tokens per complete pair** and **65,536 total
  pair tokens per query**, with batch size one reserved for a future inference
  trial. These are input bounds, not measured latency or memory guarantees.
- Pair the raw query with the original title, anchor heading and full ordered
  source/context union. Merge overlapping spans only within the same original
  source section; preserve all source bytes and locators. Apply no summaries,
  clipping, extra prompts, grouping or judgment-based input selection.
- Disable truncation and padding for counts. Reject any oversized pair or query
  group before inference; never silently shorten evidence or drop a candidate.
  Retained complete-witness records must fit with their original source clauses.

| Frozen input set | Intact pairs fitting | Largest pair | Known complete witnesses fitting |
| --- | --- | --- | --- |
| Archived ungrouped global pools: 24 questions × 24 candidates | 576 / 576 | 2,611 tokens | 15 / 15 |
| Current native document results: 12 questions × 2 results | 24 / 24 | 2,364 tokens | 5 / 5 |
| Current focused global pool: 24 candidates | 24 / 24 | 2,393 tokens | 4 / 4 |
| Retained name/date/exclusion controls | 140 / 140 | 157 tokens | Not a witness comparison |
| Previously authored English/Arabic mirror controls | 8 / 8 | 339 tokens | Four positive controls fit |

All **624 candidate pairs** and **148 additional controls** fit. The largest
actual query group contains **53,418 tokens**, below the frozen total budget;
all 37 candidate/query groups also satisfy the candidate cap. All 24 known
candidate-witness records retain complete evidence. These records overlap prior
development judgments and are not 24 independent questions or new relevance
wins. Archived pools are input stress cases, not recomputed current-schema
retrieval. The twelve current questions supply document winners, not a fresh
global 24 for each question. Controls reuse assistant-provisional judgments;
this pass provides no held-out or independent quality evidence.

The installed FastEmbed 7.1.0 already lists this model through the
[`rozgo/bge-reranker-v2-m3` ONNX export](https://huggingface.co/rozgo/bge-reranker-v2-m3/tree/fbd57b17b4db111a9d16813bb08b4c804fac18e9),
pinned separately at `fbd57b17b4db111a9d16813bb08b4c804fac18e9`. Its tokenizer
file differs in bytes from the publisher's file; it is not treated as an
identical artifact. Both verified metadata sets declare the same input limit.
Actual encodings agree on every tested token ID, offset and sequence boundary;
Rust and Python token IDs agree for all 772 inputs and three boundary controls.
This establishes tested tokenizer compatibility, not graph equivalence or ORT
runtime compatibility. FastEmbed's default limit is 512; a later runner must
set the intended limit explicitly and precheck full inputs before its truncating
encoding path. Merely selecting this model would not preserve the evidence.

Validation includes exact original UTF-8/source-span bindings, frozen input and
metadata hashes, deterministic repeat encodings and intact end offsets for both
parts of every pair. Assert-based helpers check 8,192-token acceptance,
8,193-token rejection, empty inputs, Arabic offsets, the total-query boundary
and rejection of a 25th candidate. All 140 retained control counts reproduce
prior native counts; their historical ranking failures remain unresolved.
Owned tokenizer executables are removed. Reports/helpers and private inputs
remain outside the checkout, with no app/index/dependency/CI change. Memsearch
recall, expansion and memory append/readback work. Diff checks pass; unchanged
application suites are not rerun for this evaluation/documentation pass.

**Cost caveat:** the [BGE publisher documentation](https://bge-model.com/tutorial/5_Reranking/5.2.html)
lists 568 million parameters. The pinned publisher weight-file metadata lists
about 2.27 GB; the ONNX export lists a similarly sized external data file. Neither
is downloaded or validated here. Disk size is not peak RAM. This is substantially
larger than the rejected compact scorer, and processing tens of thousands of
pair tokens per query may be too costly even with batch size one. Reranking is
query-time work, not a one-time indexing expense. No quantization, weak-device,
mobile, latency or memory claim follows from input fit.

**Next:** one bounded offline native quality/cost trial, starting with pinned
ONNX graph/external-data integrity, initialization and full-length input checks.
Freeze resource limits, CPU threads, batch size and relevance criteria before
measurement. Compare against unchanged cosine selection, retain name/date/
exclusion controls and include fresh same-language cases. Measure cold load,
warm query latency and peak memory, including coexistence with the embedding
provider. Reject regressions, clipping or unacceptable cost before considering
integration; no broader model sweep or shipping change is justified yet.
Current-revision CI, independent quality, physical-device performance and app
acceptance remain open. Consolidated smoke remains deferred.

## Stage 7 — Fresh result selection and context diagnosis — 2026-10-12

**Decision: the remaining fully supported development case fails at passage
selection, not source ingestion or nearby-context construction.** A fixed global
pool already contains complete source-backed bundles under the existing context
rules. Do not widen nearby text, increase candidate caps, rebuild embeddings or
add private query rules to repair this case. Choosing the complete evidence still
requires a demonstrated selector; no shipping behavior changes in this pass.

Before retrieval, freeze the previously graded question, its essential reference
clauses, updated disposable index, cached Small FP32 model and a global budget of
24 body candidates. Order by unchanged cosine with stable document/source ties;
apply no per-document quota, overlap removal, grouping or label-based selection.
Every diagnostic candidate uses its original span and the actual native
nearby-context function. This is a diagnosis of an existing development failure,
not a new held-out quality comparison. Private inputs and source identities stay
outside tracked files; live app data is untouched.

The native scan checks all 5,708 indexed vectors; 4,183 are eligible body
candidates. Complete frozen-reference witnesses occur at global positions
**13, 17, 18 and 19**. Each independently carries both the requested explanation
and its remedy in its primary passage plus unchanged nearby text. Their evidence
is verified against exact original source paragraphs and essential clauses; it
is never assembled from unrelated candidates. These four witness judgments are
assistant-provisional and do not exhaustively label all 24 candidates or establish
that position 13 is the earliest possible complete answer.

| Focused development check | Result |
| --- | --- |
| Current selected primary plus nearby text | Useful but incomplete, unchanged |
| Complete reference witnesses within the frozen global 24 | Four verified individual bundles |
| Extra context, changed source roles or new embeddings needed for those witnesses | None |
| Achieved improvement from a new selector | Not tested; no selector added |

The current per-document winner comes from a different part of the document,
outside the explanation's nearby radius. Its paragraph earns the higher cosine
score, so it wins before nearby evidence is loaded. At the lower-ranked reference
anchors, the existing nearby text supplies every essential clause within its
normal limits. The missing explanation is therefore neither absent from the
index nor clipped out of those candidate bundles. A global pool provides a useful
selection opportunity; it does not itself choose a better result. Raising the
visible document limit cannot change which passage wins inside this document.

The four verified witness bundles contain 3,097–4,178 source characters each.
All 24 candidates together carry 128,180 source/context characters, including
repeated nearby evidence. Feeding that entire material to a scorer is a material
input cost despite the bounded candidate count. These are source-size counts,
not model-token counts, query-latency measurements or device-memory benchmarks.
They do not establish compatibility with the rejected cached 512-token scorer.

Validation:

- All **12** archived native responses reproduce exactly, including scores,
  document order, original spans and nearby text; deterministic repeats pass.
  The focused bounded pool also repeats exactly. Read-only connections leave the
  copied database byte-identical, with no index update or live-library mutation.
- An independent full-sort reconstruction checks every vector checksum, dimension,
  source-input hash and body role and matches every top-24 coordinate and score.
  Scalar float32-to-float64 accumulation follows the native operation order.
  All **174** returned primary/context slices preserve original UTF-8 text,
  headings, page locators, body membership and existing context limits.
- Frozen question/result/grade/policy, model and restored-source hashes pass.
  Witness clause bindings and evidence hashes pass. Temporary native probe edits
  are restored byte-for-byte before execution, and owned executable copies are
  removed afterward. Private reports and assert-based helpers remain outside the
  checkout. Formatting/diff checks pass; unchanged application suites are not
  rerun. Memsearch recall, expansion and memory append/readback work.

**Next:** the original plan's optional Stage 8 selection gate is now justified for
this measured failure: evaluate one bounded local scorer that can see complete
candidate evidence, keeping current cosine selection as the control. First
establish its actual input-window fit, preserved source clauses and explicit
candidate/token budget before inference; then require retained relevance and
entity/date/exclusion controls, fresh same-language cases and measured resource
cost before considering integration. The cached reranker and its failed compact
window policy remain rejected; do not repeat a cap sweep or ship an oracle-selected
reference. Unsupported-query behavior is unchanged, and no universal confidence
cutoff is validated. Current-revision CI, independent quality, physical-device
performance and app acceptance remain open. Consolidated smoke remains deferred.

## Stage 7 — Fresh same-language relevance review — 2026-10-12

**Decision: useful retrieval is demonstrated in this small fresh batch, but
complete primary evidence and unsupported-query behavior remain quality gaps.**
Keep the shipping model and selection policy unchanged. This is an evaluation
and documentation pass; it adds no runtime code, dependency, maintained probe or
CI job. Live app data is untouched.

Freeze 12 new English questions and source-based answer criteria before native
retrieval: six fully supported, two with a supported explanation but an
unreported requested measurement, and four unsupported by the source snapshot.
None repeats the preceding retained packet's queries. Use SQLite's backup API to
copy the updated disposable index and reuse the checksum-verified cached Small
FP32 model. The actual shipping query path searches all imported documents with
a two-document limit and no changed ranking, extra candidate pool or model sweep.

Grade all 24 returned document bundles against those criteria on a three-point
scale: 0 means irrelevant or only related to the topic; 1 means useful but
incomplete; 2 means complete source support for the requested clauses and
qualifications. Mixed questions cannot earn a complete grade when their requested
measurement is absent. Review primary passages and expanded nearby text
separately. Shuffle bundles and mask document titles, result rank and similarity
scores; freeze judgments before unmasking. The same assistant authored the
questions and graded them, so these remain **provisional development judgments,
not independent quality evidence**. Source-selected questions on an existing
small corpus do not establish representative same-language performance.

| Fresh native relevance check | Result |
| --- | --- |
| Fully supported: useful first primary passage | 6/6 |
| Fully supported: complete first primary passage | 2/6 |
| Fully supported: useful first passage plus nearby text | 6/6 |
| Fully supported: complete first passage plus nearby text | 5/6 |
| Fully supported: complete evidence in either returned document | 5/6 |
| Partially supported: useful first passage plus nearby text | 2/2 |
| Partially supported: requested measurement supplied | 0/2 |
| Unsupported: useful evidence in either returned document | 0/4 |
| Unsupported: still returned two nearest documents | 4/4 |

Nearby text completes three answers whose primary passages omit a requested
clause. One fully supported question still receives useful advice without its
causal explanation; the frozen reference shows that explanation elsewhere in
the document, outside the returned nearby text. This is a remaining selection
and context-coverage failure, not evidence that the source lacks the answer.
An unsupported measurement question retrieves numbers for different conditions
and a different metric; those numbers do not answer it. Conceptual retrieval
still returns nearest passages when the library cannot support the question.

Useful fully supported results score higher than unsupported results in this
packet, but one useful partially supported result scores below an unsupported
result. A universal similarity cutoff would therefore discard useful partial
evidence if tuned to suppress all four negatives. This packet does not justify
such a cutoff, query-specific rules, a larger model or restarting the rejected
reranker. There is no controlled before/after comparison on these fresh questions,
so the counts establish neither a new relevance gain nor a regression rate.

Validation:

- The native cached-provider probe passes all 12 queries and deterministic,
  read-only repeats. Each ready query scans 5,708 passages with full index coverage.
  The copied database remains byte-identical and all logical rows remain equal to
  the updated source index. No synchronization or live-library mutation occurs.
- All **170** returned primary/context spans reconstruct exact original UTF-8
  slices with matching headings and page locators, inside body ranges. Nearby
  text stays within the existing block and byte limits. Frozen question, result,
  grade, model and restored-source hashes are checked.
- Temporary native access edits are restored byte-for-byte before execution;
  owned probe executables are removed afterward. Private questions, references,
  judgments, reports and assert-based helpers stay outside tracked files.
  Formatting and diff checks pass. Unchanged application suites are not rerun.
  Memsearch recall, expansion and memory creation/readback work.

**Next:** diagnose the one incomplete fully supported result against a fixed,
bounded candidate pool: establish whether a source-complete witness is available
and whether selection or nearby-context construction loses it before proposing a
change. Treat this now-graded packet as a development regression set; freeze new
quality criteria before any later selection trial. The counts grade full primary
text and expanded nearby text, not the always-visible clamped preview. Current
revision full CI, independent quality, other languages, physical-device cost and
interactive reader/settings acceptance remain open. The
[consolidated smoke packet](search-v2-smoke.md#pdf-backmatter-follow-up--2026-10-12)
remains deferred; this run closes none of its interactive checkboxes.

## Stage 7 — Updated PDF role retrieval regression — 2026-10-12

**Decision: retain the PDF role correction. The disposable upgrade preserves
retrieval contracts and existing answerable results; it demonstrates no new
relevance gain.** No ranking policy, model choice, UI, dependency, maintained
probe or CI job changes in this pass. Live app data is untouched.

SQLite's backup API creates separate baseline and update copies of the
retained source/index snapshot, including committed WAL contents. The same
cached Small FP32 model hashes, 24 questions, prior judgments and native
selection policy are frozen before execution. These are **reused development controls**:
16 answerable questions and eight unsupported questions, not a fresh or
independently graded acceptance set. Compare the unchanged shipping primary
passage and nearby text with identical query vectors across both copies.

The update runs the actual schema-10-to-11 migration, saved-geometry role
backfill and explicit semantic synchronization. Before synchronization, a scoped
PDF returns partial coverage and no conceptual results or query inference; the
unaffected EPUB checkpoint stays ready. Exactly one PDF source section changes
from body to auxiliary. Canonical source rows, headings, page locators, original
UTF-8 text, EPUB roles and both literal/search-form FTS projections remain exact.
All vector changes are confined to that corrected section.

The explicit PDF update processes 2,151 passages, reusing **2,150 vectors** and
encoding **one** changed auxiliary passage. Two prior passage spans are replaced
by one, so the total index changes from 5,709 to 5,708 passages. The EPUB requires
no synchronization work. An unchanged resume performs no tokenization, inference,
passage processing or writes. One ordered, warmed development-host update takes
about 6.3 seconds including backfill and synchronization; this is not a paired
performance comparison, cold-start measurement or device benchmark.

| Retained native relevance check | Before | After |
| --- | --- | --- |
| Useful first primary passage | 13/16 | 13/16 |
| Complete first primary passage | 6/16 | 6/16 |
| Useful first passage plus nearby text | 14/16 | 14/16 |
| Complete first passage plus nearby text | 11/16 | 11/16 |
| Complete evidence in either returned document | 12/16 | 12/16 |
| Useful evidence for unsupported questions | 0/8 | 0/8 |

All 24 result lists, original spans, nearby text, document order and scores are
identical; only the scanned-passage count changes. Exact retained evidence reuses
its frozen assistant-provisional judgments; no new evidence is assigned a grade.
The corrected page does not appear in updated primary results or nearby text.
It was not a winning native primary result in this packet before the correction,
so this run does **not** demonstrate removal of a previously displayed bad answer.
The earlier diagnostic pool's source-role error is corrected; incomplete body
answers and irrelevant nearest results for unsupported queries remain.

Validation:

- The real cached-provider native probe passes all 24 paired queries and **99**
  scope, quote and cancellation checks. Each document filter matches its full-scope
  result; empty scope returns no result or inference. Repeats are deterministic,
  cancellation during retrieval returns an error, and subsequent queries succeed.
  Query paths leave the database unchanged.
- A frozen required quote has FTS postings only in the newly auxiliary section.
  It still qualifies that PDF and returns a body passage; a missing required quote
  and an empty scope return no conceptual results. Exact quote/source eligibility
  is preserved without requiring the quote in the displayed passage.
- **723** returned primary/context spans, including quote controls, reconstruct
  exact original UTF-8 slices with matching headings/page locators and bounded
  body context. Source/model hashes, the frozen policy and the prior ungrouped
  native baseline match; all 613 copied geometry files retain their source hashes.
  These are locator checks, not an interactive reader-navigation test.
- Temporary probe access/source edits are restored byte-for-byte, and owned native
  executables are removed. Private inputs, reports, grades and assert-based helpers
  stay outside tracked files. Formatting/diff checks pass. Unchanged application
  suites are not rerun; the preceding correction's full suites remain the recorded
  suite evidence. Memsearch recall, expansion and memory creation/readback work.

**Next:** a bounded fresh same-language relevance review on the updated index,
with questions and source-grounded criteria frozen before retrieval. Separate
answerable, partial-evidence and unsupported cases; grade the actual displayed
passage plus nearby text. This can guide the next quality decision without
restarting the rejected reranker, increasing candidate caps or shipping grouping.
Assistant grading remains provisional. Current-revision full CI, independent
relevance, physical-device performance and app acceptance remain open. The
[consolidated smoke packet](search-v2-smoke.md#pdf-backmatter-follow-up--2026-10-12)
remains deferred; this automated run closes none of its interactive checkboxes.

## Stage 7 — Wrapped PDF index entries and cross-references — 2026-10-12

The shared PDF source-role classifier now recognizes cued index-entry wraps and
parenthesized publisher cross-references. It joins classification-only copies
when a page-number continuation, hyphenated label or unfinished parenthesis
produces a valid complete entry. Recognized `(see …)` / `(see also …)` references
contribute to navigation density only on an independently qualified index page;
they do not qualify a page alone. These are publisher syntax cues, not subject
aliases or spelling overrides. Existing minimum numeric-entry, multiple-page-list,
parenthetical-qualifier and alphabetical-order requirements remain.

Long prose, sentence punctuation (including Arabic/CJK) and whitespace-only numeric
columns still block whole-page classification. Split numeric column headings also
remain body content. Zero/malformed page lists, incomplete cross-references and
sparse/ambiguous layouts do not gain index status merely from wrapping. Uncued
wraps, wraps beyond two visual lines and other-language reference syntax remain
conservative limitations; a headerless table formatted exactly like a qualified
index remains ambiguous.
Canonical reader text, extraction order, original UTF-8 offsets and literal
search eligibility remain unchanged. Only derived body/auxiliary roles change.

Upload schema **11** queues existing PDF roles for the established restartable
saved-geometry backfill, including libraries already on schema 10. It preserves
EPUB/HTML roles; absent or mismatched sidecars conservatively keep source text.
The existing semantic freshness triggers mark affected PDF checkpoints stale.
**Explicitly update the semantic index before affected PDFs supply conceptual
results again.** Ordinary keyword search remains available. Compatible vectors
can be reused; the model/window identity is unchanged, with no model download or
unrelated EPUB-only rebuild. The upgrade does not start embedding work itself.

Validation:

- The new generic wrapped-index regression fails on the old classifier and passes
  after the correction. It checks page-list/parenthesis/hyphen wraps, cross-reference
  density, multilingual source preservation, prose, long lines, numeric headings,
  malformed lists and insufficient cross-reference-only pages. Existing sparse,
  reversed-list, table, reference and source-offset tests still pass.
- The expanded schema-10 upgrade regression verifies PDF-only invalidation,
  saved-geometry reclassification, unchanged source rows/page locators and EPUB
  roles, missing-sidecar fallback and idempotent reopen. A stale scoped PDF returns
  no conceptual results or query inference before its explicit update. Literal index
  terms and reference quotes remain searchable; required quotes in auxiliary text still
  qualify a document whose conceptual result uses body prose after explicit update.
- A private native source replay checks 613 captured PDF pages against original
  text and geometry hashes. Exactly one previously missed index-style page changes
  from body to auxiliary; the other 612 recorded role projections are identical.
  Canonical source text matches on every page and all derived ranges preserve UTF-8
  boundaries. This verifies the measured classification correction, **not a new
  model-ranking, relevance or unsupported-query win**. No weights are loaded and
  live app data is untouched; private files/helpers remain outside the checkout.
- Complete local Rust suites pass: **183 default-feature tests** (five opt-in
  checks ignored) and **241 native/provider-enabled tests** (ten opt-in checks
  ignored). Existing localhost tests require execution outside the socket-restricted
  sandbox; those reruns pass. Formatting and diff checks pass. No React, dependencies,
  maintained scripts, model choices or CI jobs change.

**Next:** a bounded native retrieval regression after explicitly updating a
**disposable** index. Check that the corrected auxiliary page no longer wins over
body evidence, retained answerable controls keep their useful source passages,
and quotes, filters and source navigation remain intact. Report unsupported
queries honestly; this correction does not solve answer absence or selection of
incomplete body passages. Do not restart the rejected cached-reranker/window/model
sweep or ship experimental grouping. The [consolidated smoke packet](search-v2-smoke.md#pdf-backmatter-follow-up--2026-10-12)
now includes this upgrade and remains deferred. Current-revision CI, independent
relevance, physical-device performance and app acceptance remain open.

## Stage 8 — Compact scoring-window trial — 2026-10-12

**Decision: reject this compact single-window policy. Both input validity and
required evidence preservation fail the frozen pre-inference gate.** Do not run
inference, retune the rule after seeing results or integrate the cached reranker.
This experiment changes no shipping code, model, dependency, index or CI check.
The native response and experimental grouping remain unchanged.

Before construction, freeze the existing global 24 candidates per query across
24 reused development questions, one 512-token scoring pair per candidate and a
source-only construction rule. Within
each captured body/context union, choose a local sentence seed with FTS5 BM25;
use the existing Porter/Unicode tokenizer and safely quoted OR query terms,
without a word list or reference labels. Equal-score ties prefer original member
span overlap, then source proximity; an unmatched query uses those source ties.
Include the original document title and the seed's actual heading. Add neighboring
units alternately after and before the seed while each complete unit fits. A
blocked side stays blocked; do not skip ahead to known reference evidence.

Units use the production punctuation/paragraph boundary heuristic projected onto
existing bounded source spans. They retain exact UTF-8 locators, but are not a
guarantee of complete linguistic sentences. Count query, metadata, source text and
special tokens with the cached model's actual tokenizer, without truncation or
padding. An oversized seed is an explicit failure, not a silently removed
candidate. The compact scoring text is assessed separately from its unchanged,
larger display bundle. Policy and source/model inputs were hashed before results;
source-only judgments were recorded before any inference, which was never run.

| Frozen feasibility check | Current control | Grouped trial |
| --- | --- | --- |
| Compact candidate pairs within 512 tokens | 569/576 | 569/576 |
| Previously identified complete display-bundle witnesses | 15/16 | 16/16 |
| Those witnesses still complete in compact scoring text | 10/16 | 10/16 |
| Required complete-witness lower bound before inference | 11/16 | 12/16 |

Across both variants, 1,138 of 1,152 pairs fit; their median is 498 tokens and
maximum is 512. The other 14 pairs require 877–884 tokens even before neighbor
expansion. **These are repeated appearances of one index-style source block,
not 14 distinct boundary failures.** That captured block remains classified as
body content despite its auxiliary layout. Its wrapped entries and cross-references
also contain too few recognized sentence boundaries for this scoring policy.
The conservative page classifier does not cover every such layout; this is a
source-role gap to investigate with generic examples, not a reason to weaken
prose/table guards or hide arbitrary numeric text.

Assistant provisional clause review finds 20 of the 31 selected complete
witnesses remain complete; eleven become useful but partial. A lexical seed can
favor topic vocabulary and fill its budget with surrounding prose while omitting
an essential mechanism or qualification. This is not a new ranking result.
Only previously identified complete witnesses were graded; alternative compact
witnesses elsewhere in the pool were not exhaustively labeled. The failed lower
bound does not prove that every compact representation must fail, or that all
answers inherently need more than 512 tokens.

Four fresh authored English/Arabic identity/date/approval cases, each with a
mirrored exclusion negative, were frozen before construction. All eight short
windows fit and retain the intended clauses, including the explicit exclusions.
These near-duplicate construction controls fit intact: they neither stress
long-window compaction nor demonstrate ranking, independent quality or a repair
for the cached model's earlier name/date/exclusion failures. No model weights
were loaded, and no new relevance ordering, inference latency or reranker memory
measurement is claimed. Constructing all 1,152 diagnostic windows took about
6.3 seconds on this host; this is not an app query-latency or device benchmark.

Validation: frozen policy/input/model and review hashes pass; all 1,152 candidate
windows and eight authored controls reconstruct deterministically. Token counts,
source ordering, exact original slices, body-range membership and 15,039 selected
source units pass checks, alongside multilingual/UTF-8, decimal, no-hit and
oversized-seed self-checks. The earlier independent global full-sort reference,
vector checksums, unchanged source rows and 8,224 captured evidence slices still
pass. Private reports/helpers remain outside tracked files. App suites are not
rerun because shipping behavior is unchanged; diff checks pass. Memsearch recall,
expansion and memory creation/readback work; automatic index refresh is unclaimed.

**Next:** investigate and correct the measured PDF source-role gap for wrapped
index entries and cross-references using generic layouts, with retained prose,
numeric-table and exact-source-offset safeguards. Verify derived-index invalidation
before shipping any classification change. Do not start another cached-reranker,
window-size or model sweep from this result; optional Stage 8 optimization has
not earned integration. Grouping stays experimental. Lexical default, explicit
downloads/indexing and opt-in fallback remain unchanged. Consolidated smoke stays
deferred; independent relevance, current-revision CI, physical-device performance
and app acceptance remain open.

## Stage 8 — Bounded reranker input gate — 2026-10-12

**Decision: reject full-bundle, single-pair reranking at the cached model's
512-token limit. Do not run inference or integrate a reranker from this pass.**
The preceding global pool establishes useful candidate coverage, but that does
not mean its complete source evidence fits a small cross-encoder. This rejects
one input policy, not the possibility of useful compact scoring windows.

The frozen policy keeps the same global 24 candidates, original member spans
and nearby-context union. Its scoring input includes the document title, anchor
heading and every ordered evidence span, with separate source locators retained.
Count the query and pair special tokens using the cached quantized mMARCO MiniLM
model's actual tokenizer. Its configured maximum is 512 pair tokens. Disable
truncation and padding; reject an oversized input rather than silently dropping
text or candidates. Model, input and policy hashes were frozen before token
analysis. No new model, download, dependency, retrieval or index is introduced.

The cached model's earlier name/date and exclusion failures remain rejection
controls; the previous implementation remains removed. Before analyzing the new
pool, the tokenizer reproduces all 140 archived regression-pair token counts
from the original native scoring report. This verifies input counting across
the diagnostic runtime, **not a new inference replay or a fix for those failures**.
Exact-boundary self-checks accept 512 tokens and reject 513 without truncation;
empty pairs are rejected and Arabic pair encoding is checked for deterministic,
nonempty source offsets.

| Input-budget check across 24 reused development questions | Current control | Grouped trial |
| --- | --- | --- |
| Full candidate bundles within 512 pair tokens | 41/576 | 36/576 |
| Previously identified complete source-witness bundles within the limit | 0/15 | 0/16 |
| Median full-bundle pair tokens | 2,068 | 2,068 |
| Largest full-bundle pair | 2,611 | 2,611 |
| Existing embedding-input-only pairs within the limit | 576/576 | 576/576 |
| Median embedding-input-only pair tokens | 289 | 302 |
| Largest embedding-input-only pair | 352 | 352 |

Overall, 1,075 of 1,152 full pairs exceed the limit. None of the 31 selected
complete witnesses fits intact. These witnesses contain surrounding source text
as well as essential clauses: **this does not prove that each answer itself
needs more than 512 tokens, or that no compact alternative candidate exists**.
Do not reinterpret this representation failure as a new retrieval/ranking loss.

Exact existing embedding input is a frozen diagnostic ablation, not an accepted
replacement for complete evidence. It fits all pairs, but six previously reviewed
inputs from eleven identical-input controls omit essential clauses supplied by
their nearby context, while the full source bundles are complete. Those grades
and source-input hashes are reused unchanged. Thus omitting context cannot be
assumed to preserve the coverage that justified the trial. No exhaustive
embedding-input-only quality or alternative-witness labeling is claimed.

The pre-inference gate fails, so no new ranking, relevance gain, inference
latency, model-load cost or peak reranker memory is reported. Fresh quality cases
and native scoring-regression replay remain conditional on a valid input policy;
they are not performed or represented as passing here. Increasing the input
length is not a setting-only remedy for this cached model's positional limit.
The prior rejected model also cannot be accepted merely because a later input
representation fits.

Validation: all frozen input/model/source hashes and the preceding native pool's
independent ordering, checksums, original source rows and 8,224 evidence slices
remain valid. All 1,152 full/core pair counts are repeated with truncation disabled;
the result bounds, witness membership and identical-input omission controls pass.
Reports and generic reproduction/self-check code remain private; no temporary
app edit, new maintained script or CI check is needed. Diff checks pass. App
suites are not rerun because shipping behavior is unchanged. Memsearch recall,
expansion and memory creation/readback work; this does not claim an automatic
memory-index refresh.

**Next:** one source-aware compact scoring-window experiment. Freeze a generic
construction rule and actual-tokenizer budget before results, preserve exact
source locators, and check required answer clauses separately from surplus
context. Do not choose windows using reference labels, silently truncate them,
expand the global pool, add private word rules or start another model sweep.
Only an input policy that preserves useful evidence justifies the bounded
selection comparison and its fresh relevance, name/date/exclusion and resource
checks. Keep grouping experimental and the native response unchanged. Lexical
default, explicit downloads/indexing and opt-in fallback remain unchanged.
Consolidated smoke stays deferred; independent relevance, current-revision CI,
physical-device performance and app acceptance remain open.

## Stage 7 — Bounded global candidate-selection feasibility — 2026-10-12

**Decision: candidate coverage justifies one bounded local selection/reranking
trial under Stage 8; it does not justify shipping grouping or a reranker.** The
grouped pool contains a demonstrated complete answer bundle for all 16 known
answerable development cases, including the intended reference explanation
missing from the control pool. Choosing that evidence remains unsolved. No application code, indexing
policy, model choice, dependency or CI check changes in this pass.

The policy was frozen before retrieval: keep exactly the best 24 available body
candidates across the entire ready library, ordered by cosine and stable original
document/source ties. There is no per-document quota or overlap deduplication;
repeated source windows consume slots. The streaming selector retains at most
24 scores while scanning every vector. The control and previous grouped index
use identical captured Small FP32 query vectors, 384 dimensions and source data.
No new encoding, download or indexing occurs; live application data is untouched.

Each candidate carries its exact original member spans and unchanged production
nearby context anchored at the first member. A grouped candidate can contain up
to four source members; the original 320-token/4,096-character embedding-input
limits remain unchanged. Nearby context retains its ten-excerpt/8,192-character
ceiling. Evidence merges overlapping spans only within the same source block,
retaining separate locators across blocks, with a 12,288-character upper bound
per candidate. Complete-answer judgments consider one candidate bundle at a
time, never combine unrelated candidates into an answer, and require every
essential reference clause rather than merely a related heading or paragraph.

Both preceding question packets are now reused development controls: 16
answerable cases and eight unsupported requests. Their questions and reference
clauses were frozen before this pass. Thirty-one complete source witnesses were
checked against those clauses and exact original text; clipped reference edges
in two cases were checked directly for all essential clauses. Judgments and
evidence digests were frozen after review. These are agent-reviewed reference
witnesses, **not independent relevance acceptance or exhaustive labels for all
1,152 candidates**. Private source identities, questions and reports remain
outside tracked files.

| Development measure | Current control | Grouped trial |
| --- | --- | --- |
| Demonstrated complete individual bundle somewhere in the global 24 | 15/16 | 16/16 |
| Previously graded first returned document bundle is complete | 11/16 | 12/16 |
| Queries whose global pool contains only one of the two documents | 5/24 | 6/24 |

The first-result row repeats the preceding native-response grades; no new
selector improves them here. Reference-witness coverage is a demonstrated lower
bound, not proof that the remaining control case has no alternative answer in
any candidate. The newly recovered grouped witness is at global position nine;
its earlier per-document raw position five used a different ordering. Other
previously incomplete cases have complete witnesses at positions six to sixteen.
Thus a bounded selector has useful evidence to choose, but topic similarity and
first-result order remain insufficient.

Several complete witnesses depend on nearby source text beyond the embedded
input. This matters before reranking: a small cross-encoder cannot be assumed to
see all the answer clauses if that text is silently truncated. Across each
24-candidate pool, overlapping evidence repeats a median 34.2% of characters in
the control and 29.5% in the grouped trial. The median numbers of candidates
overlapping an earlier candidate's source members are three and 3.5, respectively,
with a maximum of seven. These are redundancy measurements, not answer synthesis
or a retrospectively tuned deduplication policy. A global pool can also exclude
an entire document; the small two-document snapshot does not establish general
library coverage. All eight unsupported requests still receive nearest
candidates. No new abstention policy or relevance threshold is validated.

| Development-host measurement | Current control | Grouped trial |
| --- | --- | --- |
| Median instrumented warm candidate scan | 16.02 ms | 11.99 ms |
| Largest per-query median scan | 19.04 ms | 17.59 ms |
| Median native evidence loading | 1.13 ms | 1.19 ms |
| Median source-evidence characters across the 24 candidates | 148,410 | 155,784.5 |
| Largest observed source-evidence characters across the pool | 222,142 | 222,142 |

These release-build measurements alternate variant order with two warm repeats
per query, summarizing the 24 per-query medians. Scan time includes diagnostic
input/vector integrity checks; evidence loading is measured separately. Captured
query vectors exclude query encoding, provider activation and any reranker.
This uses different instrumentation/build conditions from the preceding index
experiment; compare variants within this pass, not absolute times across passes.
The 2.73-second child process peaks at 143,020 KiB RSS without loading an embedding
provider. That includes diagnostic plans and test infrastructure; it is not
whole-app, reranker, mobile or weak-device memory evidence. Feeding every complete
bundle to a reranker would still be substantial work despite the 24-candidate
count: an explicit per-candidate token budget must be measured with that model's
actual tokenizer.

Validation: the native probe passes all 24 paired queries and reproduces the
actual current control response exactly. Repeated pools are deterministic and
read-only. An independent full-sort reconstruction matches every bounded native
top-24 sequence and score. Frozen policy/input/model digests, vector dimensions
and checksums, unchanged source rows, all 8,224 returned source/context slices,
UTF-8 locators/body roles and evidence bounds pass. Witness membership and exact
source-evidence digests also pass. Temporary generic native probe edits are
restored byte-for-byte and its owned executable removed. Rust formatting and
diff checks pass. Application suites are not rerun because shipping behavior is
unchanged. Grouped migrations, quote/filter/cancellation integration, auxiliary
fallback, preview behavior and larger-library coverage are not validated here.

**Next:** one bounded local selector/cross-encoder trial. First freeze its
per-candidate input budget and verify that the source-anchored answer clauses
survive the proposed input construction; then compare achieved relevance,
latency and peak memory against the unchanged native response. Retain previous
entity/date and rejected-reranker regression controls, add fresh quality cases,
and keep the trial offline with explicit asset acquisition. If input truncation
loses the required evidence or ranking gains do not justify cost, reject the
trial rather than enlarge caps or start another model sweep. Honest multi-span
evidence/preview and shipping lifecycle integration remain separate requirements.
No ANN or new CI checks follow from these scan timings. Lexical default, explicit
downloads/indexing and opt-in fallback remain unchanged. Consolidated smoke
stays deferred; current-revision CI, physical-device performance and independent
relevance acceptance remain open.

## Stage 7 — Bounded adjacent-body embedding experiment — 2026-10-12

**Decision: retain grouping as an experimental candidate for a bounded selection
comparison; do not integrate it into app indexing yet.** One fresh first-result
bundle improves, derived vector storage falls, and a previously buried reference
group becomes retrievable much earlier. The three incomplete development cases
remain incomplete at the first result. This is a promising representation signal,
not a demonstrated overall quality win or a release decision.

One policy was frozen before retrieval: greedily join at most four consecutive,
complete, single-window body blocks under the same nonempty authored heading.
Only non-page source sections qualify. Stop at a document/heading/page boundary,
source-order gap, auxiliary or mixed-role block, multiple existing windows, or
the existing input limit. Retokenize the complete joined input, including the
existing title/heading and encoder prefix, against the 320-token/4,096-character
ceilings; never truncate it to fit. All other existing passages remain unchanged.
Each member retains its original source block and UTF-8 byte range; a group is
never represented as one fabricated contiguous range across different blocks.

Both variants use the same disposable source snapshot, pinned cached Small FP32
provider, 384 dimensions, complete fresh encodes and identical diagnostic SQLite
schema. The control reproduces actual native retrieval. Query input/vector,
cosine/body preference and stable source/document ties are shared. Each variant
returns one best candidate per document. Live application data is untouched;
private source identities, questions, reference text and reports stay untracked.
No new model, dependency, shipping code or CI checks are introduced.

There are 842 merged groups. Candidate count falls from 5,709 to 4,177. The
page-based document retains all 2,152 candidates; the short-block document falls
from 3,557 to 2,025. Every original source unit remains represented exactly once
in the ordered plan, including unchanged pre-existing overlapping windows.

The response **projects the first original member as its primary anchor**, keeps
all group-member locators separately, and attaches unchanged production nearby
context to that anchor. Thus primary grades below measure this proposed display
projection, not the relevance of the entire embedded group. A shipping design
would need honest multi-span evidence and a useful preview; a heading-only first
member cannot stand in for a complete multi-paragraph answer. Full nearby-bundle
grades do not validate the current collapsed card preview.

Eight fresh answerable questions and four unsupported requests, with reference
clauses and rubric, were frozen before retrieval. The preceding eight-answerable/
four-unsupported packet is retained separately as development diagnosis.
Ninety-six document bundles were graded with variant, rank and score hidden;
43 exact prior-evidence grades were reused, 17 identical new bundles share grades,
and 36 unique bundles were read and graded. Grades were frozen before unblinding.
The same agent authored and graded the fresh questions: **provisional evidence,
not independent acceptance**. Grade 0 supplies no requested component, 1 supports
part of the answer, and 2 supplies all essential clauses; topic similarity alone
is insufficient.

| Measure | Current control | Grouped trial |
| --- | --- | --- |
| Development first primary supplies an answer component | 7/8 | 6/8 |
| Development first primary is complete | 4/8 | 4/8 |
| Development first primary plus nearby text is complete | 5/8 | 5/8 |
| Fresh first primary supplies an answer component | 6/8 | 5/8 |
| Fresh first primary is complete | 2/8 | 2/8 |
| Fresh first primary plus nearby text is complete | 6/8 | 7/8 |
| Fresh complete bundle somewhere among returned documents | 7/8 | 7/8 |
| Unsupported requests with a supported answer component, either packet | 0/8 | 0/8 |

The fresh gain promotes the document with complete evidence above a partial
answer in another document. It does not recover an answer absent from the
returned documents. The trial's first anchor is only a heading, while its grouped
input and attached nearby text contain the requested explanation. The same
heading-first projection also weakens one development primary without completing
its bundle. All unsupported requests still receive nearest results; zero supported
answers is a failure to answer, **not successful abstention**. No confidence
threshold or weak-match rejection policy follows from these grades.

A post-grading raw-candidate diagnosis explains why grouping remains worth a
selection comparison. In wrong-topic development case B, three related reference
blocks individually rank 157, 212 and 247 in the unchanged all-body raw ordering;
their joined input ranks 5 in the grouped ordering. In partial case C, reference
blocks previously at raw ranks 23, 27 and 29 are represented in groups at ranks
7 and 16. These are per-document raw ranks before overlap removal, distinct from
the preceding coverage table. They locate reference-containing inputs, **not
complete-bundle oracle coverage or achieved first-result wins**. Case B's first
wrong-topic result is unchanged. Grouping helps candidate availability without
by itself solving selection; no caps were raised or policy tuned to these ranks.

| Development-host cost | Current control | Grouped trial |
| --- | --- | --- |
| Derived vector database, excluding source/model files and sidecars | 11,726,848 bytes | 8,585,216 bytes |
| Full encode and persistence, excluding planning/import | 232.52 s | 217.56 s |
| Median instrumented warm scan | 386.90 ms | 296.18 ms |
| Largest per-query median instrumented scan | 388.54 ms | 297.72 ms |
| Peak child-process RSS during index build | 1,504,448 KiB | 1,504,696 KiB |

Derived storage falls about 26.8%, encode/persist time 6.4%, and instrumented scan
time 23.4%; there is no observed memory reduction. Planning took a separate
16.13-second process, including provider activation and both plans. Index builds
ran once each in control-then-group order; these are not repeated end-to-end
indexing benchmarks. Import/extraction and shipping checkpoints are excluded.
Scans alternate order with four warm samples per query/variant; the reported
median is across the 24 per-query medians. They include diagnostic input/vector
integrity checks and exclude shared query encoding (median 9.80 ms), so they are
not app response timings. Host RSS includes the provider and diagnostic plans;
no mobile, weak-device, Base/Large or whole-app performance claim is made.
Incremental cost remains open: editing a paragraph can shift later greedy groups
under its heading and invalidate additional embedding inputs.

Validation: native planning, both full indexing runs and all 24 paired queries
pass. Control primary spans, text, order, nearby context and scores reproduce
actual native results; repeated paired queries are stable and read-only. Vector
checksums/dimensions, input hashes, frozen policy/packet/plan/grade/model digests,
original source rows and every returned UTF-8 slice/locator/context limit pass.
Merged inputs satisfy complete-body, single-window, heading/order and input-bound
assertions. Temporary generic probe edits are restored byte-for-byte and its
owned executable removed. Rust formatting and diff checks pass. Application
behavior is unchanged, so application suites are not rerun. This trial does not
implement or validate grouped migrations, resume/checkpoint invalidation,
quote/filter/cancellation integration or a grouped-results UI.

**Next:** a bounded global candidate-selection feasibility pass, keeping the
current app response as the baseline and this grouped index as an experimental
control. First establish complete anchored evidence in a fixed candidate pool;
then compare a minimal selector or small local cross-encoder only if candidate
coverage justifies it, with measured quality, latency and memory as required by
Stage 8. Resolve multi-source evidence/preview semantics before any shipping
change. Do not inflate per-document caps, retune groups to familiar questions,
add a model sweep or introduce ANN from this result. Default-off semantics,
explicit downloads/indexing and lexical fallback are unchanged. Consolidated
smoke stays deferred; current-revision CI, physical-device performance and
independent relevance acceptance remain open.

## Stage 7 — Candidate-coverage diagnosis — 2026-10-12

**Decision: separate passage-selection failures from missing reference coverage;
do not ship a larger results disclosure or a reranker from this diagnosis.**
Two incomplete cases have complete evidence deeper in existing candidates. The
third case's intended explanation is outside the tested pool, even though its
source paragraphs are correctly stored and indexed. A reranker cannot promote
an explanation it never receives.

This reuses the preceding frozen eight-answerable/four-unsupported packet,
reference clauses, grades, disposable index and pinned Small FP32 model. It is
a development diagnosis, not a new held-out comparison or independent acceptance.
Private source identities, questions, references and raw reports remain untracked;
live application data was not modified. No reimport, passage embedding or index
update was required.

Before retrieval, diagnostic caps were fixed at 3, 10, 25 and 50 distinct body
passages **per document**. Both eligible documents are inspected, so the largest
pool is up to 100 candidates overall; this is not a scalable global reranking
budget. Ranking uses unchanged cosine scores with stable source-order ties.
Greedy deduplication excludes intersecting byte spans within a source block,
not semantic repetition. Existing primary spans, document order and scores are
preserved. For each bounded candidate, the probe obtains its actual production
nearby context, with unchanged source boundaries and clipping limits.

Coverage is checked against essential reference clauses in exact source slices.
Merely hitting a reference block, or combining arbitrary distant passages, does
not count as a complete candidate bundle. The three remaining cases are:

| Diagnostic case | Existing candidate coverage | Implication |
| --- | --- | --- |
| Partial evidence A | Candidate 3's nearby text contains the complete explanation. The directly answering primary passage ranks 7 among distinct spans, or 8 before overlap removal. | Useful evidence is available; selecting the highest-scoring primary misses it. The previous two-extra comparison supplied those extra primary passages without their own nearby context. |
| Wrong-topic case B | The intended explanation's six short body paragraphs rank 58–979 among distinct passages. None occurs in the top 50 or their bounded nearby text. Inspected alternate explanations provide partial evidence; the useful document still ranks second. | Both document order and reference coverage need attention. This is not a missing import/vector or a reader-offset fault. Reranking the tested pool cannot recover that reference explanation. |
| Partial evidence C | Candidate 13's nearby text supplies all required components. The first direct reference paragraph ranks 20 and also needs adjacent text for completeness; other reference paragraphs rank 24 and 26. | Useful evidence is available below ten candidates. One paragraph's score does not establish completeness of a multi-part explanation. |

The five previously complete cases retain their complete baseline evidence.
Potential coverage of the **frozen reference explanations**, across both documents,
is therefore:

| Evidence pool | Cases with a complete reference explanation in at least one bundle |
| --- | --- |
| Current primary plus nearby text | 5/8 |
| Top 3 candidates with each candidate's nearby text | 6/8 |
| Top 10 with nearby text | 6/8 |
| Top 25 with nearby text | 7/8 |
| Top 50 with nearby text | 7/8 |

These are oracle-selection coverage bounds, **not achieved first-result grades**.
No selector has been demonstrated to choose the right bundle, and all top-50
alternatives have not received exhaustive relevance labels. Absence of the frozen
reference explanation is not proof that no other source could answer the question.
Displaying all candidates would add substantial irrelevant and repeated material;
the earlier three-passage expansion remains rejected. All four unsupported
queries retain their identical unsupported nearest results. Wider coverage is
not an abstention policy, and the overlapping score ranges still do not justify
a universal confidence threshold.

The missing reference group consists of short adjacent body blocks under the
same authored heading. Current embeddings represent each block independently;
nearby text is attached only after selection. This makes bounded grouping of
adjacent body text at embedding time a relevant next hypothesis. The diagnosis
does not prove fragmentation is the sole cause or that grouping will improve
ranking. Existing title/heading context is already present; adding it again is
not a proposed fix. Do not relabel intentional exercises as auxiliary content or
add subject-specific keywords to force these cases upward.

Validation: all twelve real-provider native queries pass against the existing
5,709-passage index. Full production responses, including primary text, source
spans, scores, document order and nearby context, are identical to the preceding
control. The first three distinct spans also match the preceding comparison.
Vector checksums/dimensions and source-input hashes are checked during scanning;
8,023 candidate/context excerpts pass exact UTF-8 slicing, body membership,
original locator and context-bound checks. Frozen packet, grade, source and model
digests match; authoritative source rows, roles and locators remain unchanged.
The native probe checks read-only query behavior. Temporary generic probe edits
were restored byte-for-byte and its owned executable removed. Rust formatting
and diff checks pass. Application behavior is unchanged, so the application
suites were not rerun; this probe is not a latency/memory or device benchmark.
No UI, runtime, dependency, model option, shipping script or CI job changes.

**Next:** compare one bounded adjacent-body representation against the unchanged
control using the cached model and disposable indexes. Preserve source locators,
body/heading boundaries, input limits, quotes and scope; measure index/query cost
and regressions as well as reference recovery. Treat this packet as development
evidence and use fresh questions for any quality-win claim. Only consider a
bounded local reranker trial once useful candidates are demonstrated, with a
separate global budget and measured quality/resource cost, as required by Stage 8
of the original plan. Do not raise caps to known reference ranks, launch another
model sweep, or add ANN without a measured bottleneck. Default-off semantics and
explicit downloads/indexing remain unchanged. Consolidated interactive smoke
remains deferred; current-revision CI, physical-device performance and independent
relevance acceptance remain open.

## Stage 7 — Alternative passages and weak matches — 2026-10-12

**Decision: do not add the tested three-passage expansion or a score-based
rejection rule.** The expansion adds reading burden without improving the
provisional answer grades. The remaining failures concern selecting the right
explanation and distinguishing a related passage from a supported answer.
They are not solved by displaying more nearest neighbors.

The earlier temporary evaluation snapshots were no longer available. A new
owned, disposable snapshot was prepared using the current PDF.js page-layer
helper, native PDF content-role classifier and native EPUB parser/persistence.
Application data was not modified. Both candidates use that same snapshot,
the pinned cached Small FP32 model and the current passage policy. This is a
fresh paired quality check, not a before/after comparison with earlier snapshots.
Private inputs, reference clauses, queries and raw reports remain untracked.

Eight fresh answerable questions and four unsupported requests were frozen,
with reference clauses and the grading rubric, before retrieval. The unsupported
set includes two requests near the source domain and two unrelated requests.
Grade 0 means no requested answer component, 1 means supported but incomplete
evidence, and 2 means all essential requested components are supplied. Topic
similarity alone is not useful evidence. Passage bundles were presented in
shuffled document order with scores/ranks hidden; grades were frozen before
unblinding. The same agent prepared and graded the packet, so this remains
**provisional evidence, not independent quality acceptance**.

The control is actual native retrieval: one best passage per document plus its
bounded nearby body text. The candidate retains that exact primary passage,
score, document order and context, adding at most two further body passages per
document. Candidates are sorted by existing cosine scores with source-order
ties; greedy selection rejects intersecting byte spans in the same source block.
Nonoverlapping spans can still repeat a topic or text already shown as context.
Each extra has its own original source locator; nothing is concatenated into a
new embedding, and no index, model or document-ranking policy changes.

| Evidence available in the first document result | At least partial useful evidence | Complete requested evidence |
| --- | --- | --- |
| Full primary passage alone | 7/8 | 4/8 |
| Primary plus existing nearby text | 7/8 | 5/8 |
| Same evidence plus two distinct extra passages | 7/8 | 5/8 |

Across both returned documents, all eight answerable cases have at least partial
evidence, but only five have complete evidence under either expanded policy.
One useful document ranks second behind a wrong-topic result; two other first
results remain incomplete. An exercise-like passage can outrank the concrete
explanation. These observations do not justify treating all exercises or
questions as auxiliary content: they can themselves be intentional search targets.

Of the 32 additional passages across answerable cases and both documents, 25
supply no requested answer component. Across the full packet, nine of 48 extras
are entirely within text already provided by the baseline. The two extras add
about 1,760 characters per document at the median, with about 1,208 characters
not already present in the baseline; the observed maximum added text is 2,507
characters per document. These are observed payload/reading costs, not new
production limits or latency/memory benchmarks. Adding two passages is not a
substitute for improving selection.

All four unsupported requests still receive nearest matches, with no useful
requested evidence in either document or expansion. Across all returned document
candidates, useful-evidence cosine scores span approximately 0.846–0.895 and
irrelevant scores 0.745–0.856. They overlap. The highest unsupported first-result
score is less than 0.001 below a useful partial first result. Document-score gaps
also overlap: approximately 0.004–0.052 for answerable queries and 0.012–0.051
for unsupported queries. Gaps between distinct passage scores likewise overlap.
Neither a large gap nor a high cosine score proves that a source answers the
question. No threshold was fitted or shipped from this small packet.

Validation: the temporary native import, real-provider indexing and query probes
all pass. The comparison checks exact primary spans/scores against production
retrieval, vector checksums/dimensions, source-input hashes, authoritative UTF-8
slices and body membership for every candidate/context excerpt, context bounds,
and read-only query behavior. Stored source text, roles and locators remain
identical after evaluation. The packet and grade digests remain frozen. Temporary
probe edits were restored byte-for-byte and the owned executable removed.
Rust formatting and diff checks pass. The unchanged application suites were not
rerun for this documentation-only pass. No UI, runtime, dependency, model option,
shipping script or CI job is added.

**Next:** perform a bounded candidate-coverage diagnosis for the incomplete and
wrong-topic cases: check where the reference explanations appear deeper in the
existing vector results before choosing a selection change. Only consider local
reranking if a bounded pool actually contains useful explanations and ordering
is the measured problem; a reranker cannot recover missing candidates. Do not
ship a larger disclosure, exclusion-word list, universal cutoff, model sweep or
ANN index on this evidence. Keep default-off semantics, explicit preparation,
source anchors, filters and mandatory quotes. The consolidated context/reader,
lifecycle, RTL and responsiveness smoke remains deferred. Current-revision CI,
physical-device performance and independent relevance acceptance remain open.

## Stage 7 — Bounded neighboring source context — 2026-10-12

Conceptual cards now have a collapsed **Passage and Nearby Text** disclosure.
It shows the full matched passage without the preview's four-line clamp, plus
nearby main-content excerpts in original source order. Each excerpt retains its
own section/page and exact UTF-8 byte span, and opens that source through the
existing reader target helper. Nearby text is explicitly labeled as context,
not another relevance-ranked match. Authored markup is escaped, not executed.
The collapsed preview and one-result-per-document ordering remain unchanged.

Native retrieval reads this context within the existing read-only query
snapshot, cancellation/deadline checks and final freshness check. It considers
the matching body range's remaining text and at most four neighboring source
blocks in each direction. Each direction allows at most five separate excerpts
and 4,096 characters; each excerpt allows at most 2,048 characters. Total nearby
text is therefore at most ten excerpts / 8,192 characters per returned document.
Auxiliary spans are omitted, an auxiliary-only block stops traversal, and an
auxiliary-only primary match has no body context attached. Disjoint ranges
remain separate. These are bounded source neighborhoods, not inferred chapter
boundaries; nearby topics may differ and clipping may cut a word or clause.

Context is never concatenated into an embedding input, scored independently,
or counted as an additional document. Source text, vectors, model/window
identity, required-quote document eligibility, filters, lexical results and
fallback remain unchanged. No schema upgrade, model download or index rebuild
is required for this addition. An outstanding PDF-role update from the preceding
pass still requires its explicit index preparation. The React code and styles
remain isolated under `src/search-semantic`; all eight locales include the new
labels. No dependencies, model options, shipping scripts or CI jobs were added.

Validation:

- 240 provider-enabled native tests pass (ten explicit probes ignored). Generic
  context fixtures check source order, original Unicode slices and locators,
  same-block prefix/suffix text, four-block distance and character/excerpt limits,
  auxiliary boundaries and mixed body/note content, scope isolation and read-only
  retrieval. Existing quote, cancellation, freshness and fallback regressions pass.
- 182 provider-disabled native tests pass (five ignored). All 216 frontend tests
  pass; connected tests cover escaped full text, independent reader targets,
  disabled opening controls and a disclosure outside the preview button.
  Typecheck, locale parity, focused ESLint, Vite build, Rust formatting and diff
  checks pass. The Vite build retains its existing chunk-size advisory.
- A temporary generic browser preview verifies expansion at desktop and
  390-pixel widths, no horizontal overflow or nested buttons, and keyboard opening
  of a neighboring block with its own section/page/text target. This is component
  evidence, not native WebView, PDF/EPUB navigation or physical-device acceptance.
  Temporary preview files and probe edits are removed/restored before committing.
- Real cached Small FP32 retrieval on an owned disposable snapshot preserves every
  primary passage and score in the frozen regression packet. Source digests and
  exact context slices/body roles pass. No passage inference or writes occur on
  unchanged preparation/resume; model inputs and stored vectors are unchanged.
  One ordered development-host run has approximately 326 ms median query time
  and a largest observed serialized response of about 17.6 kB; this is not a
  paired performance benchmark or mobile resource claim.
- Provisional grading of the first result **with expanded evidence** supplies
  missing clauses in two of eight answerable cases: useful evidence rises from
  6/8 to 7/8 and direct/complete evidence from 5/8 to 7/8. Primary-only grades
  remain unchanged. One answer is still outside the context bounds, and all four
  unsupported questions still return irrelevant nearest matches. This is a
  reused regression packet with agent grades, not independent quality acceptance.
  Private questions, documents and raw grading artifacts remain untracked.

**Next:** review a small alternative-passage candidate set and weak/unsupported
matches against fresh judgments before adding ranking or rejection logic. A
universal cosine cutoff remains unjustified. Keep source anchors and input limits,
explicit preparation and default-off semantics. The consolidated reader, lifecycle,
RTL and responsiveness smoke remains deferred, with a new context follow-up in
[the smoke checklist](search-v2-smoke.md#neighboring-context-follow-up--2026-10-12).
Current-revision CI and physical-device performance remain unverified.

## Stage 7 — PDF backmatter classification — 2026-10-12

The shared PDF classifier now recognizes additional structural reference/index
layouts without a subject-word list or document-specific overrides. Printed
references can qualify without explicit HTTP/HTTPS/DOI text when consecutive
numbered entries contain repeated dated, quoted titles after author separators.
Continuation lines belong to their original numbered entry. Existing linked
reference and dot-leader navigation rules remain available.

Dense subject indexes require several independent cues: short alphabetic labels,
valid comma/range page lists, repeated parenthetical qualifiers and a majority of
ascending adjacent labels. Sentence-like prose, long lines and numeric column
headings block whole-page index classification. These conservative rules leave
ambiguous or unsupported layouts as main content; they are not a general PDF
layout parser. A headerless table formatted exactly like an index remains an
ambiguous case. Reader text, extraction order, UTF-8 offsets and literal FTS
eligibility/counts remain unchanged. Mixed reference pages retain preceding
body prose. References and indexes stay searchable, rather than being deleted.

Upload schema version 10 queues existing PDF roles for the existing restartable
backfill from saved geometry. It preserves EPUB/HTML roles, including structural
roles that cannot be recovered from legacy rendered HTML. Missing or mismatched
sidecars retain authored text conservatively. Existing semantic freshness
triggers invalidate the affected PDF checkpoints; an explicit Build / Update
Index is required before those documents can supply conceptual results again.
Compatible vectors may be reused. The semantic model/window identity is
unchanged, so this does not force an unrelated EPUB-only index rebuild or model
download. Ordinary keyword search remains available during preparation.

Validation:

- 239 provider-enabled native tests pass (10 explicit probes ignored), and 181
  provider-disabled tests pass (five ignored). Generic fixtures cover printed
  and bare-domain references, mixed body/reference pages, dense subject indexes,
  Unicode page-list offsets, numeric tables/column headings, prose quotations,
  sparse and reversed lists, and malformed page references.
- A version-9 upgrade regression checks PDF-only invalidation, unchanged source
  rows/locators and EPUB roles, literal quoted citation eligibility, missing
  sidecar fallback, explicit semantic update and an idempotent reopen. A required
  citation quote can still select the document while its conceptual result uses
  body text, preserving the existing quote contract.
- A real cached Small FP32 run on a disposable native snapshot reuses the frozen
  regression packet from the preceding review. All eight answerable cases retain
  their previous returned passages and grades. The targeted reference/index hits
  now yield body passages, but all four unsupported questions still yield
  irrelevant results. This establishes the classification correction, not a new
  relevance win or independent quality acceptance. EPUB roles remain unchanged;
  canonical source digests and exact returned UTF-8 spans pass. Unchanged resume
  performs no inference or passage writes. Private details remain untracked.
- Temporary probe edits are removed before the normal suites; formatting and
  diff checks pass. No frontend, dependencies, models, shipping scripts or CI
  jobs change in this pass.

**Next:** evaluate bounded neighboring context or a small alternative-passage
set for answers split across original blocks, retaining individual source
anchors and input limits. A score threshold still needs broader calibration;
this pass does not justify one. Interactive smoke remains deferred, including
the PDF upgrade/explicit index-update checks in the consolidated checklist.
Physical-device, independent-quality and current-revision CI gates remain open.

## Stage 7 — Passage-window quality and cost review — 2026-10-12

This pass evaluates the preceding window change; it adds no product code or new
CI harness. Separate disposable native indexes compare the fixed-token method
from `801949d` with sentence-aware windows from `a0222f6`, keeping source text,
current body/auxiliary classification, titles, retrieval and pinned Small FP32
assets constant. Installed app data is untouched. Production EPUB parsing
matches saved source text exactly; PDF role backfill uses saved page geometry,
so this is not a fresh PDF extraction/OCR check.

Eight fresh answerable questions and four unsupported questions, with
source-grounded expectations, were frozen before retrieval. A familiar control
is excluded from fresh metrics. The rubric was fixed before inspection:
3 = direct answer with adequate evidence, 2 = useful partial answer,
1 = related topic without an answer, 0 = unrelated/unsupported.
All returned passages were graded with variant, rank and score hidden; judgments
were frozen before unblinding. These are implementing-agent provisional grades,
not independent labels. Grades concern full native passages, not the rendered
four-line cards. Private source identities, questions and excerpts remain outside
tracked files.

| Measure | Fixed windows | Sentence-aware windows |
| --- | --- | --- |
| Useful first result on fresh answerable cases | 5 / 8 | 6 / 8 |
| Direct first answer | 4 / 8 | 5 / 8 |
| At least one useful result | 6 / 8 | 7 / 8 |
| Unsupported questions still returning irrelevant results | 4 / 4 | 4 / 4 |
| Cold derived-index rebuild | 240.4 s | 247.1 s |
| Unchanged resume, without inference or passage writes | 9.9 ms | 9.7 ms |
| Warm native query median | 312.5 ms | 316.0 ms |
| Sampled full-process peak RSS | About 1.43 GiB | About 1.43 GiB |

Across the positive document-passage comparisons, three grades improve,
thirteen tie and none regress. One improvement changes first-result usefulness:
the sentence-aware selection includes a remedy omitted by the fixed window.
The other improvements affect secondary results. Passage count rises 1.2%,
rebuild time 2.8% and whole SQLite file size 0.44%. The observed query median
rises 1.1%; this is one ordered run per variant, not a statistically established
latency change. Both native probes used debug builds and the existing two-thread
CPU provider on the development host. Queries were OS-warm after indexing; RSS
was sampled over parsing, model loading, indexing and retrieval. These figures
do not establish production or weak-device performance.

The remaining failures have distinct causes:

- Some PDF bibliography and alphabetical-index passages still lie inside stored
  main-content ranges. Body-first retrieval cannot repair a mistaken source role.
  The current citation detector requires explicit HTTP/HTTPS/DOI text, and the
  navigation detector requires dot leaders; other reference/index layouts escape
  these conservative rules. Authored text and literal citation search must remain
  available when improving classification.
- Some useful answers or qualifications occupy adjacent original blocks; one
  selected block describes a request while the linking explanation is elsewhere.
  Sentence cuts inside a single block cannot recover missing neighboring evidence.
  Another useful answer ranks behind a related but non-answering passage.
- Unsupported questions always receive nearest passages under the current
  document-level retrieval contract. Useful and irrelevant scores overlap, so a
  universal cosine cutoff cannot separate all reviewed answers from all bad hits.
  Similarity is not answer confidence; this packet does not justify tuning a
  suppression threshold or requiring lexical overlap.
- React still clamps conceptual previews to four lines without an inline full-text
  expansion. A full native passage grade does not prove that its answer is visible
  on the search screen; expanding a preview alone cannot supply omitted evidence.

Both probes preserve the canonical source digest and verify every returned UTF-8
slice against its original source. Unchanged resume also asserts no retokenization,
inference or passage writes. Temporary probe source edits were restored byte-for-
byte before the normal provider-enabled suite passed: 236 tests, 10 explicit
probes ignored. Formatting and diff checks pass. No dependencies, frontend,
model manifests, shipping scripts or CI jobs change in this pass.

**Decision and next:** retain sentence-aware windows as a bounded local
improvement, without claiming overall quality acceptance. Next repair the generic
PDF backmatter classification gaps with guarded fixtures for ordinary prose,
numeric tables and literal citations. Then evaluate bounded neighboring context
or a small alternative-passage set with preserved individual source anchors;
do not blindly join pages or exceed input limits. Reuse this frozen packet as
regression evidence and add fresh cases for later acceptance. No new model sweep,
reranker or ANN implementation is justified here. Indexing is incremental for
unchanged source/model/policy identities; imports, edits and identity changes can
require more derived work. Interactive smoke remains deferred. No new CI,
native WebView, independent grading, additional-model/language quality or
physical-device performance evidence is claimed.

## Stage 7 — Sentence and paragraph windows — 2026-10-12

Semantic windows now prefer sentence ends or explicit paragraph breaks after
at least half the available window. Context overlap prefers a sentence start
near the existing 40-token target and cannot exceed half the window; small
character-limited windows reduce that target to keep advancing. The total input
still includes the E5 prefix, title and heading within the 320-token and
4,096-character ceilings. Complete inputs are retokenized before inference;
oversized metadata fails explicitly rather than being silently truncated.

The isolated passage helper recognizes common Latin, Arabic and CJK terminal
punctuation, retains closing quotation marks/brackets and skips dots inside
decimal numbers and URLs. Long sentences prefer whole-word cuts; unbroken text
falls back to token edges. SentencePiece offsets may include leading whitespace,
so overlap retains the token containing the selected start instead of skipping
the sentence's first word. Boundary lookups use sorted ranges rather than
repeatedly searching all boundaries for every passage.

Each window remains inside one original section/page and body/auxiliary region.
Original UTF-8 offsets, reader locators, authored text, role preference, document
ordering, required quotes and filters remain intact. This pass does not join
adjacent sections/pages or expand the frontend preview clamp. Lexical search,
default-off consent and explicit download/build controls are unchanged.

The policy identity advances to `chunks-v2`. Old vectors are incompatible and
the enabled semantic path falls back to keywords until an explicit index rebuild.
The rebuild replaces only derived semantic data; it neither rewrites sources nor
downloads another model. Natural boundaries can produce more passages and more
indexing work than fixed token cuts; this is not a claim of unchanged index cost.

Validation:

- 236 provider-enabled native regressions pass, with 10 explicit probes ignored
  by default; 178 provider-disabled regressions pass, with five explicit probes
  ignored. New generic cases cover sentence overlap, whitespace-bearing tokens,
  closing quotes, LF/CRLF paragraph breaks, Arabic/CJK/emoji UTF-8 spans, numeric/URL
  punctuation, forced cuts/progress, invalid offsets and policy mismatch followed
  by explicit rebuild with source preservation.
- Actual cached Small FP32/INT8 provider probes on this Linux development host
  check long English, Arabic and CJK windows against the real tokenizer and
  complete-input limits, and run inference. Existing SQLite quote/filter,
  cancellation/checkpoint and lexical-preservation controls also pass.
- The existing isolated Small FP32 command probe passes explicit indexing,
  source-locator verification, scoped/quoted queries, checkpoint reuse, title
  invalidation and removal through Tauri handlers on MockRuntime. It does not
  launch the WebView.
- Formatting and diff checks pass. There are no dependencies, frontend, model
  manifest, shipping-script or CI workflow changes.

**Limits and next:** punctuation heuristics are not a language-aware sentence
parser: abbreviations, missing punctuation and oversized sentences can still
produce imperfect boundaries. Section/page and role edges remain hard limits.
These controls establish valid windows, not better full-library rankings; no
fresh relevance comparison, Base/Large platform sweep or physical-device latency
claim is made. Next rebuild a disposable index and evaluate displayed passages
on fresh questions before deciding whether cross-section context, multiple
passages or a calibrated weak-match policy is justified. Use the existing full
CI with provider inference after pushing. Interactive smoke stays deferred;
the consolidated checklist includes this rebuild and source-navigation follow-up.

## Stage 7 — Main-content evidence and passage roles — 2026-10-12

This corrective pass addresses auxiliary content dominating previews without
adding another model, reranker, synonym list or CI harness. Source text and
section/page locators remain intact. The role classifier uses document structure
and PDF geometry, rather than subject words or per-document overrides.

HTML/EPUB imports retain safe EPUB/DPUB note, bibliography and navigation roles
through both sanitizers. Headings remain searchable but are auxiliary standalone
passages; body paragraphs retain their source spans. PDFs retain canonical
extraction order while deriving main-content spans from repeated page-edge
furniture, smaller bottom notes, dense dot-leader navigation and bracketed
citation lists with links. Numeric tables alone do not qualify as navigation.

Keyword evidence prefers matching body text, including within a mixed PDF page.
Citation-only queries retain auxiliary evidence, and authored exact-phrase
previews keep their existing path. Literal eligibility, scope, quote rules and
match-count queries are unchanged; notes are not deleted or excluded from FTS.
Conceptual windows split at body/auxiliary boundaries before tokenization and
keep offsets into the original UTF-8 source. Retrieval chooses the highest-scoring
main passage within each document, falling back to auxiliary passages when that
document has no main content. Document ordering still uses the selected cosine
score; there is no new confidence threshold or multi-passage result policy.
Required quotes still constrain document eligibility, so the displayed conceptual
body passage can be elsewhere than a required quote in a note.

Upgrade behavior:

- Upload schema 9 adds derived `body_ranges` metadata. Restartable background
  backfill handles one document per transaction without inference, rewriting
  reader text or replacing section IDs. An explicit semantic build also finishes
  pending role backfill before taking its source snapshots.
- Saved HTML is reparsed only when section order and text match. Saved, validated
  PDF page layers are reused only when reconstructed text matches exactly.
  Missing or mismatched source sidecars conservatively retain all text.
- Previously stripped EPUB role markup cannot be recovered from saved HTML;
  re-import those EPUBs to obtain reliable note classification. Unmarked content
  is not silently guessed from vocabulary.
- The passage policy identity changes. Existing semantic vectors are incompatible
  and queries fall back to keywords until the user explicitly rebuilds the index.
  Metadata changes invalidate coverage/checkpoints. Downloads and indexing stay
  explicit; lexical remains available with the toggle off.

Validation:

- 232 provider-enabled native regressions pass (10 explicit probes ignored by
  default); 174 provider-disabled regressions pass (five explicit probes ignored).
  New generic cases cover independent EPUB/DPUB roles and sanitizer round trips,
  inherited roles, canonical PDF text, Unicode offsets, numeric-table protection,
  body previews, citation-only/exact searches, auxiliary-only fallback, metadata
  invalidation and restartable HTML/PDF backfill with absent-source fallback.
- Actual cached Small FP32/INT8 lifecycle and SQLite controls pass on this Linux
  development host, including real tokenizer windows split at role boundaries,
  exact source offsets, quotes, filters, cancellation and lexical preservation.
- A local saved-page check confirms byte-identical source reconstruction while
  classifying page furniture and auxiliary lists. This is extraction evidence,
  not a fresh full-library ranking comparison or physical-device acceptance.
- The existing optimized retrieval probe passes with 200 documents and 100,000
  synthetic 384-dimensional passages: broad/common-phrase scans take about
  0.20–0.24 seconds here, a 500-passage scope about 1.5 ms. This excludes model
  inference, uses OS-warm fixture pages and does not establish weak-device latency.
- Formatting and diff checks pass. No dependency, frontend, shipping-script,
  workflow or model manifest changes are part of this pass.

**Limits and next:** PDF classification is heuristic: unusual notes, unmarked
references or layouts can remain unclassified, and smaller bottom captions may
be treated as auxiliary. Main-content preference can suppress a useful note
when body passages exist; exact note searches remain available lexically. Token
windows still lack complete paragraph/sentence and cross-section context, and
unrelated conceptual neighbors are not yet suppressed. Next address those
passage/context boundaries, then evaluate fresh relevance before introducing a
calibrated weak-match policy. Push and validate this revision through existing
CI; interactive smoke remains explicitly deferred, including re-import/rebuild
and source-navigation checks in the consolidated checklist.

## Stage 7 — Optional Base and Large models — 2026-10-11

The user requested two additional capacities, preserving both Small choices.
Settings now separates Model from Model Format: multilingual-e5-small offers FP32
and dynamic INT8; multilingual-e5-base and multilingual-e5-large offer FP32.
Lexical remains default and semantic search remains opt-in. Selection starts no
work. Downloads and index builds still require explicit action; model/precision
changes require turning search off and reset download consent. Only the selected
model is loaded. Desktop controls share a row and stack on narrow screens;
resource and rebuild guidance is translated in all eight locales.

A focused native asset manifest holds each pinned revision, byte count, SHA-256
and dimension. Base installs 1,127,143,136 bytes and emits 768 dimensions; Large
installs 2,252,993,226 bytes and emits 1,024. Large's external ONNX weights are
verified and passed through the installed FastEmbed public initializer API.
There are no dependency, ORT, Libtashkeel, shipping-script or CI changes, and
model weights remain outside tracked files. Existing Small preferences, cache
paths and encoder identities remain unchanged. Base/Large use separate caches;
removing a selected cache leaves other models' files intact.

Index/query callers read the active checksummed encoder identity and dimension
together. Incompatible vectors cannot be reused or searched: switching models
requires explicitly rebuilding the derived semantic index. The app keeps one
semantic index, so switching back after replacement requires rebuilding again.
Source documents, reader content and lexical search remain intact. Opening
settings, choosing a model and restarting do not rebuild or download; a saved
opt-in reloads only the selected cached model.

Validation:

- 215 frontend tests across 50 files pass. New choice/consent tests cover the two
  additional capacities, FP32-only precision, no implicit setup, saved selection
  and restart behavior. Lint, locale parity, TypeScript and Vite build pass.
- 221 native tests pass; 10 existing explicit probes remain ignored by default.
  The manifest regression protects Small's exact identity and all four selections;
  a Small → Base → Large → Small index regression requires explicit rebuilds,
  rejects incompatible coverage/vectors and preserves source text. Cache IPC
  exposes the four choices without creating model files. Provider-disabled
  native compilation also passes.
- Real pinned Base and Large lifecycle probes pass on this Linux development
  machine, including factory loading, finite normalized vectors of the expected
  dimensions, repeatability, English/Arabic input and prefix/size limits. The
  existing native SQLite controls pass with each actual model: filters, required
  quotes, cancellation/checkpoint reuse, incompatible-index lexical fallback and
  preserved sources. Large's verified external-weight loading succeeds.
- The debug probe peaks at about 2.85 GiB for Base and 5.18 GiB for Large; bounded
  test durations are 18.78/36.56 seconds. These are test-process measurements,
  not product query latency or guarantees for other hardware. Larger does not
  establish better relevance; previous mixed Base comparisons remain relevant.
- Browser checks of the actual settings component with synthetic IPC confirm
  explicit download/enable, active selection locks, readable desktop controls
  and Arabic dark layout at 320px with no horizontal overflow. Temporary preview
  files, tab and server are cleaned up. Native WebView smoke is not claimed.

**Next:** push and run the existing full CI with provider inference enabled;
its default probes remain Small FP32/INT8, avoiding multi-gigabyte downloads on
every run. Base/Large checks can be selected explicitly through the existing
ignored regression for later platform/resource validation. Rebuild with
`npm run desktop` when ready and resume the consolidated smoke checklist,
including explicit model switching/rebuilds. Smoke stays deferred at the user's
request. Representative same-language relevance and physical-device memory/
responsiveness remain acceptance work before claiming a larger-model quality win.

## Stage 7 — Stored-work guards and conditional settings — 2026-10-11

The user approved removal guards, conditional off-state layout and contextual
cleanup controls. Native cache status now distinguishes complete files from any
stored final/staging cache (`hasStoredFiles`); native index status distinguishes
retained preparation/checkpoints (`hasStoredIndex`) from compatible ready coverage.
Empty retained SQLite tables/triggers after removal are not saved work. These are
additive read-only status fields; no schema migration, ranking, download/indexing
policy, dependencies or CI jobs change. Existing native cleanup stays idempotent.

Model removal guards apply at confirmation and invocation, including incomplete
files, and refresh after completion prevents repeated empty removal. Index removal
uses stored work rather than ready-passage count, so disabled/incompatible encoders
and partial checkpoints remain cleanable. Remove Model Files and Remove Index now
sit inside their respective sections, with a separator, consequence text and inline
confirmation. The separate Manage Storage card is removed. Confirm/cancel moves
focus to a surviving control before the confirmation or empty off-index disappears.

Installed unloaded models collapse to format/Files Stored; missing/incomplete files
and relevant model operations/errors expand. When off, an empty index is hidden and
stored preparation is collapsed as Saved Index · Search Off. Enabled, running,
removing or failed index states expand; diagnostics survive disabling. Ready status
and coverage share a wrapping row, spacing is modestly reduced, and recovery details
remain under Technical Details. All eight locales and the consolidated smoke list
are updated. Opening controls still starts no download, activation or index job.

Validation: **212 frontend tests across 50 files** and **219 native library tests**
with `native-tts-shared,experimental-search-embeddings` pass; ten existing opt-in
native tests remain ignored. Native storage regressions cover partial final/staging
files, initialized work without ready coverage, cleanup, retained empty tables and
rebuild. Frontend regressions cover repeat removal, partial cleanup, stored-work
guards, contextual controls, empty/collapsed off-state and pending/error visibility.
Lint, eight-locale parity, TypeScript, production build and formatting pass. The
native suite's loopback-fixture tests needed execution outside the socket-restricted
sandbox; all passed there. Actual-component simulated-IPC browser checks verify
model cleanup/focus, off-index removal with focus on the surviving toggle, compact
English desktop and Arabic320px dark wrapping with visible Back/Done. Fixtures and
preview server are removed from the checkout; no app data or real models were
modified. These are not native smoke or screen-reader/device acceptance.

Next: push and review current-revision full CI with provider inference enabled;
when the user is ready, rebuild with `npm run desktop` and run the updated
[consolidated semantic off/on smoke checklist](search-v2-smoke.md). Interactive
smoke remains explicitly deferred; release gates remain open.

## Stage 7 — Settings readiness and copy — 2026-10-11

The approved task view now presents **Semantic Search**, **Download Model / Manage
Model**, then **Library Index**. The switch remains off and disabled without model
files, with its prerequisite and status linked accessibly. Downloads retain fresh
per-format consent and an explicit installer click; index build/update remains
explicit. The model installer stays owned by SearchModelSetup, inserted between
SemanticSearchSetup's existing toggle and index panels without taking over their
state. Native commands, opt-in persistence, retrieval and CI are unchanged.

A small shared observed-state function replaces the generic enabled promise in
general Settings and the toggle card. Loading, indexing/cancelling, incomplete
coverage, empty/checking library and keyword-only fallback remain distinct from
**On · Ready**. Failed/stale encoder state, index removal and `0 / 0` cannot claim
readiness. Inactive index controls explain that a loaded model is required; they
do not tell an already-enabled user to enable again.

Technical copy now describes local processing, device-dependent resources, retained
files/work and saved opt-in reloads without development-host RAM figures or CI
claims. Those measurements remain in developer docs. INT8's result/performance
caveat stays beside its format choice. A short closing-Settings notice appears
only beside active downloads/indexing and points to header progress/cancellation;
it makes no promise about app exit or mobile suspension. Expanded **Index Details**
uses a horizontal count/action row on wide screens and stacks on narrow screens.
All eight locales are updated; styles remain in the component's own stylesheet.

Validation: **208 frontend tests across 50 files**, lint, locale parity, TypeScript
and the production frontend build pass. Regressions protect observed-state
precedence in both Settings views, prerequisite guards, panel order, explicit
operation ownership and conditional background guidance. A browser fixture using
the actual dialog/setup components with simulated IPC verifies off/missing model,
loading, failure, download and indexing states; only status/cache reads occur on
opening. Keyboard disclosure focus, desktop count/action alignment, narrow Arabic
stacking/dark mode, no horizontal overflow and visible dialog actions pass. This
is presentation evidence, not native smoke or screen-reader/device acceptance.

Next: push this commit and review current-revision CI; when the user is ready,
rebuild with `npm run desktop` and run the updated consolidated semantic off/on
[smoke checklist](search-v2-smoke.md). Smoke remains explicitly deferred and all
interactive release gates remain open.

## Stage 7 — Conceptual results summary — 2026-10-11

The user-approved summary now uses **Conceptual Results**, a localized count such
as **2 documents**, and **Related passages from your imported documents.** The count
represents displayed cards, not a known total beyond the native response limit of
50. The imported-only restriction remains explicit without repeating **Your Library**
in the heading. Complete healthy indexing coverage stays in Search settings;
partial fallback shows **Documents ready for this search** coverage, retains lexical
fallback guidance and continues withholding partial conceptual cards.

The existing full explanation and quoted-clause rule move unchanged into a native
collapsed **How Matching Works** disclosure. Its keyboard focus is visible and its
text wraps using the component's own CSS. All eight locales include the new copy
and appropriate document-count plural forms. Retrieval, filters, quote eligibility,
model consent, native behavior and CI are unchanged.

Validation: all **205 frontend tests across 50 files**, lint, locale parity,
TypeScript and the production frontend build pass. A focused regression distinguishes
displayed-card count from indexed-document coverage and protects empty/loading/
partial states. An actual-component browser fixture verifies English zero/one/two
counts, disabled silence, partial fallback without cards, and keyboard disclosure.
English desktop and 320-pixel Arabic/RTL views have no horizontal overflow; the
Arabic disclosure wraps and remains translated. Browser checks are not native
smoke or actual screen-reader acceptance.

Next: push this presentation commit and review current-revision CI. When the user
is ready, rebuild with `npm run desktop` and run the updated
[consolidated smoke checklist](search-v2-smoke.md), including semantic off/on and
the latest settings/activity/results changes. Smoke remains explicitly deferred;
all checklist boxes and interactive release gates remain open.

## Stage 7 — Search Mode heading cleanup — 2026-10-11

The user-approved cleanup visually hides the existing native fieldset legend in
SearchBar's own stylesheet. The localized **Search Mode** accessible name, radio
choices, keyboard behavior and mode-dependent helper text remain intact. No
query, filter, index, preference or conceptual-results behavior changes.

Validation: all **four focused SearchBar tests**, lint, locale parity, TypeScript
and the production frontend build pass. Existing assertions now also protect the
legend and input/helper association. A browser fixture using the actual SearchBar
confirms the group name remains in the accessibility tree while the legend is
visually clipped; arrow keys switch the selected radio and helper copy. English
desktop and 320-pixel Arabic/RTL layouts have no horizontal overflow. Native app
smoke and actual screen-reader testing remain user-deferred.

**Recommendation recorded in that pass:** simplify the conceptual-results summary.
Use **Conceptual Results** as the title and a count such as **2 documents** for
displayed cards, without implying a known total beyond the response limit. Retain
the imported-document restriction in brief supporting copy rather than repeating
**Your Library** in the heading. Hide healthy complete indexing coverage from
results; keep coverage visible when it explains a fallback/problem and retain full
details in Search settings. Partial coverage must still withhold conceptual cards
and explain lexical fallback. Move the detailed quote rule into an accessible
**How Matching Works** disclosure while keeping a short explanation visible.
This follows [NN/G's minimalist design guidance](https://www.nngroup.com/articles/aesthetic-minimalist-design/)
and [progressive disclosure](https://www.nngroup.com/articles/progressive-disclosure/);
it is a design recommendation, not a user-test finding. The screenshot's current
labels are unchanged in that commit; the recommendation is implemented in the
[following summary pass](#stage-7--conceptual-results-summary--2026-10-11).
Resume the [consolidated smoke checklist](search-v2-smoke.md)
after rebuilding with `npm run desktop` when the user is ready; all boxes remain open.

## Stage 7 — Compact search activity and results copy — 2026-10-11

The header now uses one compact activity button with a decorative spinner,
phase label and chevron alongside the unchanged Settings control. It opens the
existing Search settings view directly from home or the reader. During observed
native indexing, it shows completed-document coverage such as **3 / 5**; these
counts are neither a time estimate nor a job percentage. Before native reservation,
old coverage is hidden. Cancellation has its own pending phase.

App-owned indexing outcomes survive reader/settings navigation within the same
process. **Indexing Complete** and **Indexing Cancelled** expire after eight
seconds; **Indexing Failed** and **Search Status Unavailable** remain actionable
with **Review** until opened or another accepted action starts. Reviewing dismisses
the header notice while retaining the settings diagnostic. Only the native
cancellation result is classified as cancellation, so a real failure racing a
cancel request remains a failure. A failed final status refresh does not claim
that indexing failed or that the library is ready. Generation guards suppress
late outcomes after disable; repeated successful jobs reset the notice timer.
This adds no persistent job history or terminal download/activation notices.

The stable live region announces phases rather than byte/document ticks. The
existing reduced-motion spinner rule is retained, and the full accessible name
includes coverage and action even when narrow labels ellipsize. The Settings gear
stays at the logical end; home reserves a toolbar row and narrow reader layouts
put activity below Back. Styles remain in the component's own stylesheet.

The requested results cleanup removes the redundant **Keyword Results** heading,
its unused caller prop and locale key. Ordinary cards retain their summary count.
The separate conceptual explanation now uses normal-weight, wrapping text with
its own style, and its heading/count can wrap independently. Retrieval, quote
rules, consent and conceptual/keyword separation are unchanged.

Validation: **204 frontend/tooling tests in 50 files**, lint, locale parity,
TypeScript and the frontend build pass. Focused regressions cover activity phases,
quiet announcements, native coverage, outcomes, cancellation/failure races,
timer reset/expiry, late completions and forwarding to existing settings controls.
A browser fixture using the actual components checks desktop/light/dark views,
320-pixel English/Arabic home and reader layouts, keyboard management access,
wrapped conceptual copy and absence of the redundant heading. It caught and
resolved narrow-reader crowding of Back. Fixture statuses/cards are synthetic;
this is not native inference, screen-reader or installed-app smoke evidence.
The narrow PDF toolbar layout also retains its existing fullscreen controls-only
boundary: activity styles do not reveal the hidden settings/Back row in fullscreen.

**Next:** review the updated [smoke checklist](search-v2-smoke.md) after rebuilding
with `npm run desktop`, when the user is ready. Smoke remains explicitly deferred;
all checklist boxes remain open. The visible Search Mode label has only been
reviewed: recommend hiding it while retaining the radio group's accessible label
and helper text in a small follow-up. No new CI jobs or dependencies are needed.

## Stage 7 — Keyword results and starter eligibility — 2026-10-11

The source-group review is implemented: keyword cards now form one list with one
summary count and a small **Starter Document** badge on bundled cards. Counts
show **Showing N of M** when display limits leave matches undisplayed. Existing
document include/exclude controls retain source selection; the search scope and
default remain unchanged. The opt-in conceptual channel stays separate, with
**Keyword Results** replacing the visible jargon label. English result headings
and action labels use Title Case; all eight locales have matching keys, and the
unused source-group headings are removed.

The screenshot mismatch was reproduced against the actual built starter index.
A fresh Pagefind instance, without the startup discovery search, still returns
unrelated candidates for a generic query such as `orchard lantern survey` and
highlights short fragments rather than the requested words. The problem is loose
candidate eligibility, not demonstrated startup cache contamination. The shared
search hook now intersects those candidates with Pagefind's quoted-term matches
for each distinct unquoted term before scope/count/display limits. Missing terms
reject the candidate; empty intersections stop further checks. Original candidate
order and Pagefind's language/stemming behavior remain in use. Authored quoted
phrases still receive the existing separate source-text verification. Broader
continues to exclude starter documents, and uploaded/native/conceptual retrieval
is unchanged. No Pagefind package, index configuration, word-pair overrides or
new search engine is introduced.

This is presentation consolidation plus an All Words eligibility correction,
not globally comparable relevance scores: uploaded results still precede starter
results, and each provider retains its ordering. Required-term lookups add indexed
Pagefind calls for nonempty candidates; they do not fetch every source document
or encode text. No latency guarantee for a larger starter corpus is claimed.

Validation: **199 frontend/tooling tests in 50 files**, lint, locale parity,
TypeScript and the frontend build pass. New regressions cover required-term
intersection, deduplication, early rejection, retained candidate order, phrase-only
separation, a spelling retry after rejecting loose starter hits, unified counts,
source badges, scope and capped totals. Existing quote, concurrency, Broader,
evidence/navigation and conceptual-channel checks remain passing.
A browser harness invoking the actual helper and built Pagefind index rejects
three loose candidates, retains two positive controls and three stemming controls,
and returns zero for an empty scope. Rendered English/Arabic cards and badges
fit 320-pixel views without horizontal overflow; filtered/keyword-only and separate
conceptual presentation were checked. Uploaded/conceptual cards in that preview
are synthetic; no real model/native inference or installed-app smoke is claimed.

**Next:** rebuild with `npm run desktop` and review the combined presentation,
then resume the updated [smoke checklist](search-v2-smoke.md) when ready. Verify
All Words rejects unrelated starter cards and still admits intended terms/phrases
under include/exclude filters. Smoke remains user-deferred and all checklist
boxes remain open. No new CI matrix or automatic model tuning is needed.

## Stage 7 — Search settings readiness — 2026-10-11

The follow-up screenshot review found contradictory verification guidance while
the model was already loaded and prominent setup actions after indexing was
complete. The task view now uses the active provider identity for **Model Loaded**;
file presence alone never claims verification. A loaded model collapses into
**Manage Model** with its format/status summary. Download permission and the
installer action remain visible only while semantic search is off and that
format is not loaded. Explicit per-format consent remains required because the
installer can fetch missing files even when the button says **Verify Cache**.
Turn off before format/verification/removal maintenance; reopening an inactive
cache still distinguishes presence from an observed successful verification.

Active use replaces the numbered setup headings with **Semantic Search** and
**Library Index**. **Ready for conceptual search** requires a selected active
provider, nonempty complete document coverage, no indexing and no current error.
Current coverage has no Build action. Incomplete coverage offers **Build Index**
or **Update Index**, reusing compatible checkpoints through the existing native
command. Document coverage stays visible; passage totals and **Refresh Status**
move under **Index Details**. Failed, running, empty or inactive states never
claim readiness. Downloads, activation and indexing remain explicit; no Rust,
controller, dependency, runtime, shipping or CI behavior changes.

English Search settings buttons/headings use Title Case; all eight locales have
matching translated state/action keys. The Search task view owns
`SearchModelSetup.css`, and the header activity component owns
`SearchActivity.css`; `AppSettings.css` retains only shared Settings styles.
Native disclosures, existing dialog focus handling, cleanup confirmations and
cancellation/error paths remain in use.

Validation: **195 frontend/tooling tests in 50 files**, lint, locale parity,
TypeScript and the frontend build pass. Added regressions distinguish file
presence/verification/loading, restore explicit consent after disable, and cover
current/partial/empty/failed/inactive/running index actions and readiness. Existing
consent, cancellation, fallback and stale-response checks remain passing.
Browser previews using mocked native state cover ready/partial/setup views,
English/Arabic, 320-pixel widths and a 1024-pixel ready view. Layout checks find
no horizontal overflow and keep Back/Done inside the viewport. Keyboard access
to native disclosures was checked. These previews do not establish installed-app,
actual WebView zoom, screen-reader or physical-device acceptance.

**Next:** review the rebuilt layout with `npm run desktop`, then resume the
updated [smoke checklist](search-v2-smoke.md) when ready. Smoke remains explicitly
user-deferred; no checkbox is marked passed. The earlier full CI acceptance
predates this UI-only delta. No additional native/model matrix or automatic
ranking pass is justified by these presentation changes.

## Stage 7 — Search settings UX cleanup — 2026-10-11

Following the user-approved UX review, general Settings now shows a compact
semantic-search summary and **Manage** entry. A dedicated, wider task view reuses
the existing dialog with a scrolling body and persistent header/Back/Done controls.
Three panels separate model download, opt-in and library preparation. Detailed
resource/compatibility and recovery explanations are collapsed under **Technical
details**; destructive model/index cleanup lives under **Manage storage**, with
consequence text and explicit confirmation. Source documents are never removed.
Cleanup remains possible for incomplete model caches and staging files.

Empty libraries now request imports rather than showing `0 / 0` as readiness;
index preparation is disabled until searchable documents exist. Labels distinguish
missing files, unverified file presence, verified completion, model loading,
indexing, ready coverage and lexical fallback. Background download/activation/
indexing activity offers **Manage** beside Settings, including while the view is
closed or the reader is open. The App-owned controller observes existing download
start/terminal events and retains bounded, busy-only polling; byte-progress events
do not trigger extra status IPC. No new native commands, job manager or idle poll.

All eight UI locales are updated. Default-off consent, per-format download
permission, explicit activation/indexing, cooperative cancellation and separate
conceptual results retain their existing contracts. No Rust/runtime/model,
dependency, shipping command or CI change is made.

Validation: **192 frontend/tooling tests in 50 files**, lint, locale parity,
TypeScript and the frontend build pass. Focused regressions cover Settings
navigation, cleanup confirmation, empty/partial/ready index states, activity
labels and download events/cleanup without idle polling; existing consent,
cancellation, fallback and stale-response tests remain passing. An actual browser
preview with mocked native state was inspected in English/Arabic at desktop and
320-pixel viewport widths: no horizontal overflow, persistent navigation,
keyboard disclosure/focus wrapping and Escape restoration. These are UI-preview
checks, not installed-app, screen-reader or physical-device acceptance.

**Next:** review this rebuilt layout and resume the updated consolidated
[smoke checklist](search-v2-smoke.md) when ready. Smoke remains user-deferred;
no checkbox is marked passed. The previous full CI acceptance is evidence for its
recorded revision, not this UI delta. No additional native inference matrix or
model tuning is needed for a React/layout-only change.

## Stage 7 — final automated branch acceptance review — 2026-10-10

**Automated acceptance is complete; final merge/release acceptance remains open.**
[Full run 38110242011](https://github.com/muhannadnouri/papercut.io/actions/runs/38110242011)
passes on `1f020d31a34f3a7943772f9fa806391f060d49b6`. All ten jobs succeed with
the four redundant build definitions absent. Shipping package uploads, AppImage
media and both macOS runtime checks pass. Default/provider Rust builds and tests
remain separate; all six runtime targets complete both formats' index/query and
lifecycle controls with successful native exit. The remaining shipping steps
therefore supply the required inference prerequisites without the second builds.
Inference/model downloads remain manual and default-off in routine CI.

Verified run evidence: **184 frontend/tooling tests in 48 files**, **162 default
Rust tests (five ignored)**, **217 provider Rust tests (ten ignored)**, plus the
explicit real-model test on Linux, Windows, macOS Intel/ARM, Android x86_64 emulator
and iOS arm64 simulator. Lint, frontend build, security audits and packaging jobs
also pass. Run metadata, job steps/logs and artifact metadata bind this evidence
to the reviewed source; packages are not downloaded or installed in this review.
Prior Linux extracted-package contents/relocation evidence remains separate.

The [final review](search-v2-branch-review.md#final-acceptance-review--2026-10-10)
builds on the original branch review and rechecks its corrective delta and connected
App/hooks, native retrieval/storage, runtime and shipping boundaries. Remote and
local `main` still match merge base `8be9952fe02a10921ea4955df204b115268bc686`.
The branch comparison is **95 files, 23,079 added / 323 removed text lines**, plus
the unchanged upstream model asset. No new reproducible corrective defect is
identified in these re-reviewed flows; this is not an independent second audit of
every line or a claim that passing tests prove all product behavior.

The original plan's phrase/filter/evidence, opt-in/lifecycle and platform contracts
remain the acceptance basis. The user's later separate conceptual-results choice
supersedes trial rank fusion; lexical modes are not redefined. Keep the frozen
retrieval experiment outside the shipping/CI graph, the lexical evaluator and
mobile runner, and the official Libtashkeel adapter/model/license. No further
model comparison, optimization framework or Stage 8 work is justified now.

**Next:** resume the consolidated [smoke checklist](search-v2-smoke.md) when the
user is ready, focusing on the Stage 7 lifecycle/results and installed-package
checks added since the earlier reported pass. It remains explicitly deferred;
no checkbox is marked passed. Correct obsolete instructions that called ordinary
shipping builds provider-free: current native shipping commands include capability,
with semantic use still default-off. Representative same-language relevance,
physical/reference-device resources, minimum-OS behavior and signed Apple
distribution remain unaccepted. The earlier unexplained Linux crash and native
allocator retention remain recorded limits; this green run does not explain them.
Do not start another automatic tuning/CI pass merely to continue the roadmap.
This review changes docs only; no push, dispatch, merge or release is performed.

## Stage 7 — shipping CI acceptance and consolidation — 2026-10-10

[Full run 38107442282](https://github.com/muhannadnouri/papercut.io/actions/runs/38107442282)
completed successfully on shipping-integration source
`e6869601e4c9eae0c5cd22884b8914df0fb7c4f5`. All ten jobs pass. Ordinary native
shipping scripts produce Linux AppImage/Debian, Windows MSI/NSIS, macOS Intel/ARM
DMG and Android arm64 APK artifacts; unsigned iOS simulator/archive and device
linking pass. AppImage media and both macOS bundle-runtime checks pass. Artifact
metadata was reviewed; this follow-up does not download, install or launch them,
and unsigned iOS checks are not signed IPA acceptance.

The optional inference steps actually ran on Linux, Windows, both macOS
architectures, Android x86_64 emulator and iOS arm64 simulator. Each log completes
FP32 and INT8 index/query and lifecycle controls, and exits successfully. Default
Rust coverage passes **162 tests (five ignored)**; provider-enabled coverage
passes **217 tests (ten ignored)**, followed by the explicit real-model test on
each runtime. These are bounded native checks, not WebView or physical-device
acceptance. The Android exit/Windows loader corrections remain in place.

With shipping builds accepted, remove **four redundant workflow step definitions**:
the Windows/macOS unbundled provider build, second Android APK and second iOS
simulator/device builds. This removes **six build executions per full run** because
the desktop step ran on three matrix targets. Existing shipping steps prepare the
same required runtimes, including Android's x86_64 libraries and iOS's simulator
slice. No new job, matrix, npm command, Tauri config or cache framework is added.
Default/provider Rust compilation/regressions, package uploads/runtime checks,
manual `provider_inference` gates, model sharing and failure diagnostics remain.
No application, runtime, model, signing or user-consent behavior changes.

Fresh local validation passes the existing **nine shipping-command regressions**
and **six mobile-runner self-tests**. A parsed workflow comparison confirms exactly
those four steps are removed and every other job, condition, matrix, upload and
inference command is unchanged; YAML and all **38 Bash-compatible run blocks**
parse. Documentation links, unchanged smoke checkboxes and diff checks pass.
Native inference and platform builds are not rerun locally in this deletion-only
pass; the confirming remote run remains necessary.

**Next:** push this consolidation and run `validation_scope=full` with
`provider_inference=true` once to verify the remaining shipping steps alone supply
all inference prerequisites. Keep inference optional for routine CI. Then complete
the final branch acceptance review and the consolidated interactive smoke when
the user resumes it. Smoke remains explicitly deferred; physical/reference-device
resources, representative same-language relevance and signed Apple distribution
remain open. No push, dispatch, release or merge is performed here.

## Stage 7 — provider in shipping builds — 2026-10-10

**Finding 3's build integration is implemented; platform package acceptance remains
open.** On top of source `d5df49e`, existing desktop/native Android scripts now
select `native-tts-shared,experimental-search-embeddings`; native iOS export and
unsigned device/simulator checks select the linked provider alongside native TTS.
Release already invokes these scripts, so no release workflow, npm script, Tauri
config, global Cargo default, runtime/model dependency or signing setting changes.
The shared runtime staging and iOS archive-only correction stay intact.

Semantic search still requires saved user opt-in; fresh/invalid preferences are
disabled. Building capability does not activate/download/index models. No embedding
weights ship. Explicit static desktop and no-native mobile alternatives remain;
Android fully custom arguments retain explicit feature selection. The small
shipping-command regression captures real script invocations without toolchains,
downloads or signing, covering three desktop platforms, Android native/custom/no-native
paths, iOS export/archive and the device helper's packaged frontend protocol.

Fresh checks: **184 tests in 48 files**, including **nine shipping-command cases**,
lint, locale/TypeScript/Vite/frontend build and Rust formatting. **Ten provider
regressions pass (three asset/IPC tests ignored)** with runtime loading pointed
at the extracted Debian payload, including default-off/no-startup-I/O, truthful
capability, command validation, cancellation and cache boundaries. No installed
app data or private corpus is used; ignored inference checks are not newly run.

Local Linux `npm run desktop -- --bundles deb`, `--bundles rpm` and
`--bundles appimage` all build successfully using the shipping script. Archive inspection confirms compiled
provider/FastEmbed code, unchanged shared runtime bytes and upstream license,
with no embedding weights. Relocated payloads resolve their packaged sherpa/ORT
with development library-path overrides unset. AppImage extraction does not launch
the app; its existing GStreamer media check passes. Its two runtime file locations
come from the existing resource/dependency bundling layout and contain matching
bytes; file copies do not establish multiple loaded ORT instances.

Only CI comments change: uploaded native packages now include provider capability.
No job/step/matrix expansion or dispatch is added. Retain the temporary duplicate
provider builds until the new shipping-package run is accepted, then remove the
obsolete unbundled desktop/second APK/second simulator/device checks while keeping
default/provider Rust coverage and explicit inference. Prior passing platform
inference runs do not certify these newly integrated distribution packages.

**Next:** push and run `validation_scope=full` to validate Windows/macOS installers,
Android APK and unsigned iOS simulator/device linking through existing shipping
entry points. Review the resulting artifacts/runtime checks before consolidating
CI variants. Signed/notarized Apple distribution, physical/reference-device
resources, representative same-language quality and the user-deferred consolidated
interactive smoke remain release gates. No publish, push or release dispatch is
performed by this pass.

## Stage 7 — branch review corrections — 2026-10-10

**Findings 1 and 2 are fixed; finding 4's compatibility crate is removed.** The
[branch review](search-v2-branch-review.md) retains its original source comparison
and evidence, with corrective status and the remaining shipping gate.

- Conceptual submissions retain one running IPC and one latest eligible pending
  request. Once the old call settles, success or failure, only the latest request
  starts; obsolete completion cannot publish results. Disable, invalid input,
  pending lexical scope, provider preparation/unavailability and unmount discard
  queued work. Exact phrases/scopes and independent lexical results remain intact.
  No polling, unbounded queue or new native cancellation command is added.
- Pre-7 migration clears only normalized FTS; pre-8 migration clears only spelling
  vocabulary/checkpoints in the schema transaction. Older-app edits cannot leave
  stale derived rows marked ready. Backfill reads authored sources; normal schema-8
  restart still resumes committed batches. No source, original FTS, library
  placement or semantic data is deleted.
- Removed all four `experiments/search-v2-embeddings/compat/` files (**2,028 lines**).
  Historical four-order compatibility evidence stays in Git; use app-native checks
  in the [provider guide](search-embedding-provider.md#runnable-checks), rather than a
  second Cargo/runtime graph. The published Libtashkeel dependency, production
  adapter/shared initializer and model/license remain. Other retrieval experiments
  stay frozen outside the app/CI graph; evaluator/mobile tooling remains.

The single-slot frontend and stale-projection Rust regressions fail before their
fixes and pass afterward. Fresh validation: **175 frontend tests in 47 files**,
**162 default Rust tests (5 ignored)**, **217 provider-enabled Rust tests
(10 ignored)**, lint, TypeScript and Rust formatting. The new Rust test covers
schema 6 and 7 title edits with populated derived tables, new/old Arabic title
matches and vocabulary, source/placement preservation and a subsequent schema-8
restart; existing interrupted-batch tests remain. No installed app data, private
corpus, model download, new CI step or packaging change is part of this pass.

**Follow-up:** finding 3 is implemented in the [shipping pass above](#stage-7--provider-in-shipping-builds--2026-10-10);
validate its remaining platform packages before collapsing redundant packaging
variants. Retain default/provider regression boundaries and explicit inference checks.
Smoke remains user-deferred for the consolidated final pass; real-device resource,
representative same-language quality and interactive acceptance remain open. No
push, workflow dispatch, release or merge is performed.

## Stage 7 — branch boundaries, CI and release review — 2026-10-10

The [full Ponytail branch review](search-v2-branch-review.md) compares committed
`878fbbf` with current `main`/merge-base `8be9952`; unrelated working files are
excluded. It finds two corrective blockers: a replacement conceptual request
gets stuck after native single-slot rejection, and returning from the older app
can incorrectly reuse stale normalized FTS/vocabulary after a title edit.
Fresh focused frontend checks pass **33 tests in six files**, while an isolated
single-slot hook reproduction fails latest-query recovery. A disposable SQL
replay reproduces the migration issue; it is not a real-app downgrade test.

Keep the lexical evaluator/generic fixture, test-only production evaluator and
mobile inference runner. Retire the separate `compat/` crate in the corrective
pass; retain the other standalone retrieval controls as frozen developer
benchmarks outside mandatory CI and product packaging. No source is deleted in
this review. Official Libtashkeel and the shared Arabic-TTS adapter/model remain.

Normal release scripts still omit the provider. Address that deliberate boundary
before a semantic-feature release, keeping runtime user consent default-off.
Once shipping feature packages are validated, fold duplicate packaging variants
into one shipping pass per target; keep deterministic default/provider tests and
the existing explicit model-runtime path. No new CI layer, workflow dispatch,
model sweep, runtime change or production fix is part of this documentation pass.

**Follow-up:** findings 1 and 2 are fixed and finding 4 is retired in the
[corrective pass above](#stage-7--branch-review-corrections--2026-10-10).
Finding 3 is the later shipping-package gate, not an instruction to enable or
publish unaccepted packages now. Smoke stays user-deferred for the consolidated
final pass; quality, physical/reference-device resources and real-app acceptance
remain open. Stage 8 has no new measured justification.

## Stage 7 — native indexing/query checks without new CI jobs — 2026-10-10

**Decision: extend the existing ignored provider regression, keeping CI orchestration
unchanged.** Its first lifecycle cycle now exercises the production semantic store,
retrieval and lexical search paths against disposable generic English/Arabic
documents, once for each FP32/INT8 format. Later reload cycles retain the existing
provider checks. No new job, workflow input, build variant, npm script, dependency
or model asset is added. The existing mobile runner requires an index/query
completion marker for each format as well as the passing test and normal exit.
Its owned temporary directory also contains the SQLite fixtures, so cleanup covers
native aborts and timeouts.

The added assertions cover:

- Cancellation after a committed indexing checkpoint, writer-slot cleanup,
  partial-index availability, checkpoint reuse on resume and skipping unchanged
  documents without tokenization/inference.
- Document scopes, empty scopes, result limits and mandatory English/Arabic
  quotes; a title-only phrase cannot satisfy a required body quote.
- Conceptual provenance and source-byte evidence, with no lexical match count
  invented for conceptual results; retrieval performs no database writes.
- Cooperative query cancellation, incompatible-index rejection before inference
  and provider disable at the encoding boundary preventing result publication.
- Unchanged production lexical results with a missing/partial/incompatible index,
  after cancellation or opt-out, and after derived-index removal.

These are shared backend checks using actual tokenization, model vectors and
SQLite. They do not execute mobile Tauri IPC, the WebView, the React fallback
banner or interactive reader/audio flows. The authored Arabic control checks the
query contract, not representative book relevance. Cooperative cancellation still
cannot interrupt an ORT call already in progress.

Local Linux release validation passes the extended exact native test with both
formats (**one passed, 2.56 seconds**, excluding compilation), **12 semantic tests
with one scale test ignored** in each default/provider configuration, and **six
mobile-runner self-checks**. Workflow parsing, existing shell syntax/change
detection and diff checks pass. The workflow itself is unchanged. This host has
no Android emulator/NDK or Xcode; target execution was pending at implementation
time and is now verified below. Run `38090360569` validates the earlier provider
lifecycle only.

### Mobile indexing/query acceptance verified — 2026-10-10

[Run 38094061234](https://github.com/muhannadnouri/papercut.io/actions/runs/38094061234)
is a successful first-attempt manual **mobile** run on
**`f42af57c76fc7bbf82ae183b33cfd55bc776d247`**. Both mobile checkout logs bind
that exact source. All five executed jobs pass: change detection, shared checks,
security audit, Android and iOS. Desktop and SILMA packaging are skipped; this is
not a new full-platform run.

| Runtime and job | Verified result of the extended native test |
| --- | --- |
| [Android API 35 / x86_64 emulator](https://github.com/muhannadnouri/papercut.io/actions/runs/38094061234/job/114337900314) | Release shared runtime: both FP32/INT8 index/query and lifecycle markers; **one passed, 18.42 seconds**, normal exit and successful runner cleanup/emulator termination. No destroyed-mutex/exit-134 abort. |
| [iOS arm64 simulator](https://github.com/muhannadnouri/papercut.io/actions/runs/38094061234/job/114337900263) | Release linked runtime: both formats complete index/query and lifecycle checks; **one passed, 28.39 seconds**, normal exit and successful owned simulator/fixture cleanup. |
| [Linux x64 shared checks](https://github.com/muhannadnouri/papercut.io/actions/runs/38094061234/job/114336224669) | Debug shared runtime: the same extended test passes both formats, **one passed, 46.67 seconds**. |

These completion markers follow the indexing/query assertions listed above;
they are not merely activation markers. Runner success also requires a zero
native exit and completed cleanup. Whole-test times include verification,
inference, indexing/query and lifecycle work; they are not per-query benchmarks
or comparative platform performance measurements.

Shared checks also pass **168 frontend tests in 47 files**, **six runner checks**,
**161 default Rust tests (5 ignored)** and **216 provider Rust tests (10 ignored)**,
plus lint, dependency audits and frontend build. Normal/provider Android arm64
APK builds and iOS simulator archives/unsigned device links pass. The separate
cached IPC and scale tests remain ignored; they were not exercised on mobile.

**Decision:** close the bounded backend indexing/query gate on these emulator/
simulator runtimes, including unchanged lexical availability after semantic
failure or opt-out. No code fix, additional CI job or model tuning is justified
by this result. This does not validate mobile IPC/React fallback, UI navigation,
representative relevance, Android arm64/physical devices, minimum supported OS,
device resource costs or long-running native stability. Interactive smoke stays
explicitly deferred, and the earlier unexplained native-crash/allocator limits
remain open.

### CI maintenance policy

Keep deterministic model-free contract and runner tests in ordinary CI. Keep
real-model initialization/index/query checks together in the existing explicit
`provider_inference` path, sharing assets and one native test process per target.
Rerun relevant platform scopes after provider/runtime changes and before accepting
a provider release. Manual testing covers physical-device resources, foreground
responsiveness, reader/RTL/audio behavior and full-app lifecycle. It does not
replace deterministic filter, quote, cancellation or fallback regressions. Before
merge, review the workflow for redundant build/setup steps; historical scale/model
experiments should remain explicit developer checks rather than more mandatory
jobs. Additional coverage should extend existing tests before adding orchestration.

**Follow-up:** the [branch review](search-v2-branch-review.md) above completes the
implementation/removal, CI duplication and release-packaging review. Its
corrective findings precede further acceptance; normal packages still omit the
provider. Preserve the explicit runtime checks and deferred smoke boundary.
This run remains bounded native evidence, not merge or release approval.

## Stage 7 — mobile provider inference harness — 2026-10-10

**Decision: execute the existing desktop provider regression on disposable mobile
emulator/simulator runtimes, only through explicit manual CI opt-in.** After
pushing this commit, dispatch CI on `feature/search-v2` with
`validation_scope=mobile` and `provider_inference=true`. Full scope with that
flag also runs the checks. The flag still defaults to false; ordinary PR and
scheduled CI downloads no embedding weights and boots no emulator for this test.

The Linux checks job reuses the production installer and manifest to verify both
FP32/INT8 caches, then uploads a **test-only** `search-provider-validation-models`
artifact with one-day retention. The mobile jobs download this same roughly
594 MiB asset set into runner temporary directories. Each target provider
verifies the asset sizes/hashes again before loading. No new model, downloader,
production command, upstream library modification or npm script is introduced.
Normal APK uploads still precede the provider checks and omit embedding support.

| Target | Added execution check | Acceptance status |
| --- | --- | --- |
| Android API 35, x86_64 emulator | Release library test with `native-tts-shared,experimental-search-embeddings`, prepared x86_64 sherpa/ORT shared libraries, KVM and 4 GiB emulator RAM | Both FP32/INT8 formats pass and exit normally in run `38090360569` on `955fe5b`, whole native test 21.06 seconds. Existing arm64 APK builds remain separate evidence. |
| iOS arm64 simulator on the macOS runner | Release library test with `native-tts-ios,experimental-search-embeddings-linked`, prepared simulator static archives, simulator SDK and an ad-hoc signed executable copy | Both FP32/INT8 formats pass again in run `38090360569` on `955fe5b`, whole native test 16.26 seconds. Physical-device acceptance remains open. |

The earlier next-step wording proposed an Android **arm64** emulator. The current
Linux runner instead supports a practical accelerated **x86_64** check. This does
not close Android arm64 inference acceptance; an arm64 runner/device is still
needed. iOS selects an installed iPhone/runtime pair rather than hard-coding a
runner device name. Its test deployment target matches the native simulator
archive's 15.1 minimum; this does not validate minimum supported app OS versions.
The execution methods follow the [Android emulator action](https://github.com/ReactiveCircus/android-emulator-runner)
and [Rust simulator testing guidance](https://rustc-dev-guide.rust-lang.org/tests/running.html#testing-on-emulators).

`scripts/check-mobile-provider-inference.mjs` selects exactly one Cargo JSON
library-test artifact and runs only the ignored provider regression. A passing
exit must also include one completed lifecycle for **each** format and exactly
one passed test. Input-prefix/size/token limits, finite normalized 384-vectors,
English query/passage and Arabic query inference, repeat/distinct-input controls,
disable, failed activation and cancelled publication reuse the existing test.
Downloads are disabled on the target; inherited format overrides are removed.
Android addresses only the action's emulator serial and owns a UUID directory
under `/data/local/tmp`; iOS creates, boots, shuts down and deletes only its own
simulator. Staging, inference and cleanup commands have host timeouts; Android
also bounds the remote native child to ten minutes. Native loader/test output
remains in the job log. Cleanup runs after staging, signing and inference failures.

Initial local validation passed **five runner self-checks**, including real subprocess
failure/timeout propagation, malformed/ambiguous build logs, missing format/test
completion, wrong Android ABI, simulator selection, isolated settings and cleanup
after failures. Workflow parsing, actual Bash syntax/change-detection checks and
diff checks pass. The same runner self-checks run in ordinary shared CI without
assets or emulators. This Linux host has no installed emulator/NDK or Xcode;
No target inference pass was claimed before the CI reviews below.

**Next:** push and dispatch the manual mobile inference run above, then review
both target logs against this commit. Compilation/link success alone is not
inference acceptance. Afterwards retain Android arm64/physical-device execution,
resource/lifecycle, native stability, representative quality and app acceptance
gates. Interactive search/index/fallback/reader/RTL/audio smoke stays explicitly
deferred; remind the user at the consolidated final smoke pass.

### Mobile inference first CI result and Android archiver correction — 2026-10-10

[Run 38079859346](https://github.com/muhannadnouri/papercut.io/actions/runs/38079859346)
checks out **`55d4fe05e91b24562079dbe82b7ad7faa9f5b897`** in both mobile jobs.
Shared checks, Linux inference and the verified-model artifact upload succeed.
Both normal/provider Android arm64 APK builds and the model-artifact download
succeed. The [Android regression compile step](https://github.com/muhannadnouri/papercut.io/actions/runs/38079859346/job/114296361112)
then fails in `ring v0.17.14`: `cc-rs` cannot find
`x86_64-linux-android-ar`. Emulator setup/inference are skipped. This is a missing
archive-tool configuration in the new direct Cargo step, not evidence of an ORT
initialization or model-inference failure.

The direct compile step now exports target-specific
`AR_x86_64_linux_android` and `RANLIB_x86_64_linux_android` pointing to the existing
NDK's `llvm-ar` and `llvm-ranlib`. Compiler/linker selections, runtime libraries,
models and app builds are unchanged. Target-specific variables avoid changing
host build tools. This follows the [NDK build-system guidance](https://developer.android.com/ndk/guides/other_build_systems).
No alias, extra tool installation, runtime upgrade or dependency edit is needed.

The [iOS job](https://github.com/muhannadnouri/papercut.io/actions/runs/38079859346/job/114296361062)
passes its normal/provider simulator archives and unsigned device links, then
compiles and executes the exact provider test with the simulator's linked
runtime. Both format logs record activation readiness, completed inference
controls, disable and lifecycle completion; **one test passes, 11.01 seconds**.
That duration covers asset verification and the bounded native lifecycle, not
compilation, simulator boot or app query latency. The runner completes cleanup
and the job succeeds. This closes bounded iOS simulator provider inference for
this revision, not physical-device costs, integrated search/audio or UI smoke.

The new regression executes the actual Android compile-step Bash with a Cargo
environment-capture stub, including an NDK path containing spaces. It fails on
the original missing archiver and passes after the fix; all **six** runner
self-checks pass locally. A temporary Rust probe using locked `cc 1.2.56`
confirms both target-specific overrides select the supplied LLVM tools.
Workflow parsing/guards, Bash syntax/change detection and diff checks pass.
There is still no local Android NDK/emulator run; Android target compilation and
inference require fresh CI on this correction.

**Next:** push and start a **new** manual CI run with `validation_scope=mobile`
and `provider_inference=true`. Rerunning the failed job at the old SHA cannot
exercise this fix. Review Android compilation, both-format inference and cleanup;
retain the verified iOS result while checking for regressions. Android arm64 and
physical-device memory/responsiveness/lifecycle, representative quality and
native-stability gates remain open. Interactive smoke stays explicitly deferred.

### Mobile provider inference acceptance review — 2026-10-10

[Run 38090360569](https://github.com/muhannadnouri/papercut.io/actions/runs/38090360569)
is a successful first-attempt manual **mobile** run on
**`955fe5b2d08a945f99279c6ac4f9fa69bbd489e5`**. Both mobile checkout logs bind the
result to the Android environment-lifetime correction. All five executed jobs
succeed: change detection, shared checks, security audit, Android and iOS.
Desktop packaging and SILMA runtime packaging are skipped for this scope; their
previous results are separate evidence, not rerun here.

| Target and job | Verified native result |
| --- | --- |
| [Android API 35 / x86_64 emulator](https://github.com/muhannadnouri/papercut.io/actions/runs/38090360569/job/114326912562) | Release shared-runtime test: both FP32/INT8 formats complete; **one passed, 21.06 seconds**, normal process exit and runner cleanup. The previous destroyed-mutex/exit-134 abort does not recur. |
| [iOS arm64 simulator](https://github.com/muhannadnouri/papercut.io/actions/runs/38090360569/job/114326912526) | Release linked-runtime test: both formats complete; **one passed, 16.26 seconds**, normal process exit and owned simulator cleanup. |
| Linux x64, shared checks | Explicit cached provider test repeats both formats and exits normally: **one passed, 44.64 seconds**. |

Both mobile logs identify the exact ignored provider regression and record
activation readiness, completed inference controls, disable and lifecycle
completion for each format. The bound source checks finite normalized vectors,
English/Arabic inputs, repeatability, distinct inputs, input limits, failed-load
cleanup and cancelled INT8 publication. Targets use verified temporary assets
with downloads disabled. Whole-test durations include verification and lifecycle;
they are neither query benchmarks nor comparisons of platform performance.

Normal/provider Android arm64 APK builds, iOS simulator archives and unsigned
device links also succeed. These builds do not execute an Android arm64 or iOS
physical-device provider. Shared checks pass **168 frontend tests in 47 files**,
**six runner regressions**, **161 default Rust tests (5 ignored)** and
**216 provider-feature Rust tests (10 ignored)**; the cached provider check is
then explicitly exercised. Lint, dependency audits and frontend build succeed.
The model-dependent index/IPC checks remain ignored in this CI invocation.

**Decision:** close the bounded mobile provider initialization/inference/clean-exit
gate for these tested runtimes. The Android guard is validated by this run, not
by a captured teardown stack; the original call chain remains unconfirmed.
Retain the guard until a compatible upstream fix is adopted and target-validated.
This does not resolve the earlier unexplained Linux crash, prove long-running
stability, or accept integrated app search, model quality or device resource costs.
No additional runtime change or model comparison is justified by this pass.

**Next recommended automated pass:** extend mobile native coverage to the
semantic indexing/query path using disposable generic documents and existing
index/retrieval modules. Check compatible incremental indexing, quote and document
scope enforcement, cancellation/disable and fallback availability. The existing
Linux-only cached IPC tests cannot be described as mobile acceptance. Keep
Android arm64/physical-device execution, minimum-OS coverage, memory/CPU/thermal
costs and foreground reader responsiveness open. Interactive search/index/reader/
RTL/audio smoke remains explicitly deferred; remind the user for the consolidated
final pass. No release or merge acceptance is declared.

### Android inference completes but native exit aborts — 2026-10-10

[Run 38082869212](https://github.com/muhannadnouri/papercut.io/actions/runs/38082869212)
checks out **`00706794964fd3366931348e74d3f361fd69152a`**. The Android archiver
correction works: target compilation and emulator boot succeed. Both FP32/INT8
activation, inference controls, disable and lifecycle markers complete; the
exact test reports **one passed in 23.45 seconds**. The process then aborts with
`FORTIFY: pthread_mutex_lock called on a destroyed mutex`, and `adb` returns 134.
The runner correctly fails the job despite the passing assertions. Android
provider acceptance remains open. The iOS simulator repeats both formats and
exits successfully (**one passed, 26.86 seconds**); shared checks and security
also pass. Whole-test durations include verification/lifecycle, not app latency.

ORT rc.13's global environment exit hook is the leading suspect, not a proven
stack diagnosis: the job has no native backtrace. Upstream documents a
[CPU teardown failure](https://github.com/pykeio/ort/issues/614) and
[environment-release ordering failures](https://github.com/pykeio/ort/issues/609),
addressed by its newer manual-environment API. Those reports support the
hypothesis; they do not establish this Android crash's exact call chain.

The shared initializer now retains **one Android-only `Arc<Environment>`** for
the process lifetime through the existing public API. This keeps rc.13's exit
hook from releasing the last environment reference after native globals have
been destroyed. Model sessions still unload on disable; the guard holds no model
and is created only on first native use, including Arabic preprocessing. Other
platforms retain their existing initialization. No dependency version, vendor
source, runtime library or search default changes. This bounded workaround
should be removed after an upstream teardown fix is adopted and target-validated.
Android failure handling also captures the emulator's crash-buffer log before
cleanup; diagnostic failure cannot turn the failed test into success.

All **six runner self-checks** pass, including a successful-looking test summary
followed by exit 134, bounded crash capture, failed diagnostics and owned cleanup.
Workflow guards/Bash syntax/change detection and Rust formatting pass. The locked,
offline Linux release provider suite passes **three tests**, with the asset-dependent
native test ignored; this is not a target inference run. A temporary Linux probe
compiles the actual Android-conditional initializer against pinned ORT and verifies exactly one additional environment owner, with repeated init
adding none; the pre-change source fails that ownership assertion. This checks
the public API/lifetime guard, **not Android execution or the crash's cause**.
Fresh target CI is required to validate normal native exit with the guard.

**Next:** push this correction and start a **new** manual CI run with
`validation_scope=mobile` and `provider_inference=true`. Require both model
formats, zero process exit and cleanup; inspect the captured native stack if it
still aborts. Android arm64/physical-device costs, native stability, representative
quality and app acceptance remain open. The earlier unexplained Linux crash is
not resolved by this Android workaround. Interactive smoke stays deferred.

## Stage 7 — explicit provider runtime checks — 2026-10-10

**Decision: reuse the ignored real-model provider regression for explicit desktop
runtime validation, with model installation available only by manual opt-in.**
This extends initialization/inference evidence beyond the successful build/link
checks below; it does not change the model, runtime, search behavior or release
package features.

The existing workflow now accepts `provider_inference` (boolean, default `false`).
After pushing, dispatch CI on `feature/search-v2` with
`validation_scope=desktop` and `provider_inference=true`. This runs the native
test on Linux, Windows x64 and both macOS architectures using the libraries
already prepared by each platform build. Linux reuses the debug test build;
Windows/macOS use release tests. Normal artifact uploads precede those latter
checks, and still omit the provider. No new npm command is required.

The test remains ignored in ordinary CI. Explicit inference jobs set
`PAPERCUT_EMBEDDING_TEST_DOWNLOAD=1` and reuse the production pinned, size/SHA-256
verified installer through a mock app handle. Assets live under the runner's
temporary directory, cached by OS and asset-manifest hash, outside app data.
Both retained formats total about **594 MiB** on a cold cache; intact caches are
verified without fetching. Without the explicit flag, the test only reads a
supplied cache. No download is added to PR, push or scheduled CI.

| Host/runtime | Initialization and inference evidence |
| --- | --- |
| Linux x64, existing shared ORT | Both FP32/INT8 formats pass in run `38075038695`, debug test, 45.37 seconds. Earlier local release/debug checks also pass. |
| Windows x64, release runtime DLLs | Both FP32/INT8 formats pass in run `38075038695`, release test, 11.93 seconds. The Common Controls manifest fix clears the pre-harness loader failure. |
| macOS Intel / Apple Silicon, prepared shared libraries | Both FP32/INT8 formats pass on both hosts in run `38075038695`, release tests, 11.20 / 12.08 seconds respectively. |
| Android / iOS | Android API 35/x86_64 and iOS arm64 simulator FP32/INT8 provider inference, clean process exit and cleanup pass in `38090360569` on `955fe5b`. Integrated indexing/query, Android arm64 and physical-device acceptance remain open. |

Times above cover the complete regression, including model installation/cache
verification, activation and lifecycle checks; they exclude compilation and are
not query latency, app responsiveness or comparable cross-platform benchmarks.

The reused regression validates activation/warm-up, finite normalized
384-dimensional English query/passage and Arabic query vectors, repeated-input
stability, disable/unload, failed activation cleanup and cancelled publication.
A new assertion rejects reuse of one vector for different inputs. Linux also
checks that exactly one ORT shared image is mapped. This does not measure
same-language relevance, cross-language accuracy or immediate RSS recovery.
The local installer-enabled run uses disposable copies of verified assets and
makes no upstream transfer; it verifies warm-cache installer reuse, not a new
cold-download result.

The focused provider regressions also pass: **10 tests, 3 ignored**. Workflow
YAML/guard/cache/path assertions, Bash syntax, previous platform-helper command
assertions, Rust formatting and diff checks passed before the manual run.
Desktop execution results are verified in the acceptance review below.

**Next:** desktop inference is now verified in the acceptance review below.
Prepare actual mobile runtime/device checks and retain resource/quality
acceptance gates. Consolidated Stage 7 UI smoke remains explicitly deferred. Build or vector checks alone do
not justify Stage 8 optimization or provider release acceptance.

### First runtime run — Windows loader correction

[Run 38066316034](https://github.com/muhannadnouri/papercut.io/actions/runs/38066316034)
checks pushed **`ff124375abe15c02e6916982698e288e6a278c51`**. Inspected Linux
and Apple Silicon logs show both FP32 and INT8 activation, inference controls,
disable/failure/cancellation checks and the exact ignored test passing. Their
whole test times are 72.07 and 25.04 seconds, including test-time installation
and verification; these are not query latency measurements. Intel inference is
still running at review time and is not counted as passed. Mobile jobs are
skipped by this desktop run.

The [Windows log](https://github.com/muhannadnouri/papercut.io/actions/runs/38066316034/job/114256554769)
shows normal packaging/uploads and the provider app build succeed. The provider
library test compiles, then its executable exits with **`0xc0000139`
(`STATUS_ENTRYPOINT_NOT_FOUND`) before the harness prints `running 1 test`**.
The unused scanner imports are warnings; no model activation or inference occurs
on Windows in this run. The log does not identify the offending DLL/export.

The existing Sherpa build helper copies its DLL set into the profile root and
`examples`, but omits `deps`, where Cargo puts this test executable.
[Windows DLL search order](https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-search-order#standard-search-order-for-unpackaged-apps)
checks the executable directory and system folders before `PATH`. Thus the CI
PATH prefix alone does not guarantee that this executable uses the prepared
runtime rather than a stale adjacent or system copy. This is a verified staging
gap and a plausible explanation for the loader failure, not proof of the exact
DLL selected by that failed process.

**Correction: the same helper now copies the matching runtime DLL set into
`deps` as well, overwriting stale copies, and reports all three destinations.**
No new runtime, package dependency, Libtashkeel patch, app initializer change or
cache purge is introduced. A local assertion invoking the actual copy/helper
functions fails before the correction and passes afterwards. It covers debug
and release target-qualified layouts, app/example/test destinations, stale-file
replacement and exclusion of import libraries. The rebuilt focused provider
suite also passes **10 tests, 3 ignored**, along with app Rust formatting, the
changed helper formatting and diff checks. Standalone vendor formatting retains
an unrelated pre-existing match-arm difference. This checks filesystem staging
on Linux, not Windows loader execution.

**Next:** push the correction and dispatch a fresh desktop CI run with
`provider_inference=true`; rerunning the old SHA will not include it. Inspect
Windows startup and both format results, and the final Intel outcome. If the
loader failure persists, capture the missing export and selected DLL before
considering a runtime/version change. Mobile inference, native stability/resource
limits and consolidated Stage 7 UI smoke remain open; smoke stays deferred.

### Windows staging rerun — diagnostics required

[Run 38070213129](https://github.com/muhannadnouri/papercut.io/actions/runs/38070213129)
uses pushed **`4feb99530d20d52becdde34985ec4e4bbb37d0f3`**. Its
[Windows log](https://github.com/muhannadnouri/papercut.io/actions/runs/38070213129/job/114267436470)
confirms DLL staging into `release/deps`, successful compilation, and the same
pre-harness `STATUS_ENTRYPOINT_NOT_FOUND` failure. **The staging correction does
not resolve the failure; its earlier shadowing explanation remains unproven.**
No Windows model inference passes. Linux and Apple Silicon steps succeed;
Intel is still running in this rerun at review time. The completed first run's
Intel log now also confirms both formats pass, with 20.55 seconds for the whole
test including installation/verification, not query latency.

Local inspection downloads the official 1.13.4 Windows shared release library
archive into temporary storage and matches its release asset SHA-256
`f923e5eacb6bca83914d89cb31afa579e11eeaff9af39f8ead82ad19f44b2c9f`.
LLVM PE inspection confirms all declared Sherpa Rust binding functions exist
in the C API DLL, the C++ DLL's named C API imports exist, and the C API's ORT
ordinal 1 corresponds to exported `OrtGetApiBase`. These are checks on the
official archive, not proof of the failed runner's imported functions, selected
DLLs or Windows system exports. The failing test executable is not available
locally, so the missing export cannot yet be identified.

**Next bounded change: failure-only diagnostics in the existing Windows CI
job.** The inference step has an ID, and a following step runs only if that
specific step fails. It uses installed Visual Studio `vswhere`/`dumpbin` to log
SHA-256 hashes and PE import tables for the test executables and adjacent DLLs,
plus DLL export tables. It does not rerun tests, alter DLLs, waive failure,
download new tools or change package/model/runtime configuration. Cached test
executables may also be listed; bind the current one to Cargo's failed-command
path in the same log. Missing binaries/tooling or inspection errors fail the
diagnostic step explicitly.

Workflow YAML and assertions verify the specific failure guard, preserved test
failure, output selection and retained default-off/normal-package boundaries;
diff checks pass. Windows PowerShell/dumpbin execution remains a CI check, not
a locally exercised capability. No further provider tuning or dependency bump
is justified without the missing-export evidence.

**Next:** push and dispatch fresh desktop CI with `provider_inference=true`.
If Windows fails, inspect `Diagnose Windows provider loader failure` for the
actual imports, hashes and exports before selecting a corrective change. Keep
smoke deferred; Windows acceptance remains open and this is a diagnostics
commit, not a claimed fix for the loader failure.

### Windows PE diagnosis — Common Controls manifest correction

[Run 38072112454](https://github.com/muhannadnouri/papercut.io/actions/runs/38072112454)
is bound to **`3e9d0eef5ab5a749c35d4b52ad820b23869de2fa`**. The
[Windows diagnostic step](https://github.com/muhannadnouri/papercut.io/actions/runs/38072112454/job/114273094762)
succeeds after the same pre-harness failure. The actual failed
`app_lib-c3632dcbcb43fde0.exe` imports `TaskDialogIndirect` from `comctl32.dll`,
and its PE section summary has no resource section. Adjacent ORT and Sherpa
C API DLL hashes match the official archive inspected above, and their required
native exports are present. The scanner import warnings are nonfatal.

**Identified build defect:** Tauri's generated resource is linked to app binaries,
not this library unit-test executable. The test retains the dialog dependency,
whose `TaskDialogIndirect` requires
[Common Controls v6](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-taskdialogindirect).
Tauri's own source documents the same `STATUS_ENTRYPOINT_NOT_FOUND` test failure
and [explicitly embeds a manifest](https://github.com/tauri-apps/tauri/blob/dev/examples/api/src-tauri/build.rs).
This explains why ordinary Windows packaging can succeed while the native
library test never reaches its harness. It is a specific manifest-selection
defect; Windows execution must still confirm that correcting it clears this run's
loader error and exposes no further failures.

The app build script now uses Tauri's public `WindowsAttributes` API to omit
its resource-embedded manifest **only for Windows/MSVC**, then supplies
`/MANIFEST:EMBED` and `/MANIFESTINPUT` through Cargo's general linker arguments.
This reaches the library test as well as the app, rather than duplicating a
manifest resource in the app. `windows-app-manifest.xml` matches the installed
Tauri default byte-for-byte: the same Common Controls v6 dependency. Icons,
version resources and other-platform behavior remain intact. CI change detection
includes this build input. No dependency/runtime version, upstream library fork,
model, retrieval or opt-in policy change is needed.

Local assertion checks execute the actual build-script control flow against a
Tauri stub for Windows/MSVC, Windows/GNU, Linux, macOS, Android and iOS. A real
Cargo probe confirms the general linker arguments reach a library unit test.
A Windows PE link using LLVM tools verifies manifest resource ID 24, the v6
identity and retention of an existing resource; the control without embedding
has no manifest. This is cross-link evidence, **not execution with Microsoft's
linker or the Windows loader**. The existing focused embedding suite passes
**10 tests, 3 ignored**; workflow guards, actual Bash change detection, Rust
formatting and diff checks pass. No redundant frontend/ranking or model-inference
rerun is needed for a Windows-only link correction.

Linux inference and Apple Silicon inference succeed in this run; the Intel job
is cancelled before its inference step, so its earlier successful evidence is
retained without claiming a fresh pass. Mobile jobs are skipped in desktop scope.

**Next:** push this correction and dispatch a **new** CI run on the updated
`feature/search-v2` revision with `validation_scope=desktop` and
`provider_inference=true`. Confirm Windows reaches the test harness and passes
both FP32/INT8 initialization/inference/lifecycle checks. A rerun of the old SHA
cannot test this fix. Keep interactive smoke deferred and the mobile runtime,
resource/reference-device, quality and original native-stability gates open.

### Desktop provider runtime acceptance verified — 2026-10-10

**The Windows pre-harness loader failure is closed on the tested CI runner.**
[Run 38075038695](https://github.com/muhannadnouri/papercut.io/actions/runs/38075038695)
is a successful first-attempt manual desktop run on
**`982e3f91b102190751a3dd67c579d6069e9deb7a`**, matching both local HEAD and the
pushed `feature/search-v2` ref at review time. The change-detection log confirms
`validation_scope=desktop`; all four native inference logs confirm the explicit
model-installation flag and exact ignored test execution.

| Runtime check | Verified result, whole test duration |
| --- | --- |
| [Windows x64](https://github.com/muhannadnouri/papercut.io/actions/runs/38075038695/job/114281810262), release | Harness starts; FP32 and INT8 activation, inference and lifecycle complete; one test passes, 11.93 seconds. |
| [Linux x64](https://github.com/muhannadnouri/papercut.io/actions/runs/38075038695/job/114280156701), debug | Both formats complete; one explicit test passes, 45.37 seconds. |
| [macOS Intel](https://github.com/muhannadnouri/papercut.io/actions/runs/38075038695/job/114281810369), release | Both formats complete; one test passes, 11.20 seconds. |
| [macOS Apple Silicon](https://github.com/muhannadnouri/papercut.io/actions/runs/38075038695/job/114281810245), release | Both formats complete; one test passes, 12.08 seconds. |

Each log records one cycle per format, activation readiness, completed inference
controls, disable and lifecycle completion. The bound test source checks finite
normalized 384-dimensional outputs, English query/passage and Arabic query input,
repeat-input equality, distinct-input vectors, input limits, active-model removal/
switch protection, failed activation cleanup and cancelled INT8 publication.
Linux additionally asserts a single mapped ORT image. This is a bounded provider
regression using MockRuntime, not a full app search/index or audio test.

The manifest correction is now validated by actual Windows build and test
execution. Ordinary Windows MSI/NSIS packaging and the subsequent provider app
build also pass; failure-only diagnostics are correctly skipped. No ORT upgrade,
libtashkeel fork or additional loader change is required for this incident.
The earlier native crash and measured allocator retention remain separate open
issues.

All seven executed jobs succeed; Android, iOS and SILMA packaging are correctly
skipped for desktop scope. Shared checks pass **168 frontend tests in 47 files**,
**161 default Rust tests (5 ignored)** and **216 provider-feature Rust tests
(10 ignored)**; the asset-dependent provider test is then explicitly exercised
as above. Lint, dependency-audit steps, frontend build, download-monitor and SILMA
self-test steps succeed. Passing audits retain existing informational warnings;
this review does not claim a warning-free dependency tree. Normal desktop
packaging, Linux media verification, both macOS runtime-bundle checks and normal
artifact uploads pass. Uploaded packages still omit the embedding feature.
No artifacts were downloaded, installed or launched during this review.

**Next:** execute the mobile provider harness added above on an
Android x86_64 emulator and iOS arm64 simulator with the prepared platform
runtimes and verified assets; Android arm64 execution remains open. Keep model
downloads confined to opt-in validation and use disposable data. Simulator/emulator results must be followed by physical
mobile/reference-device memory, foreground responsiveness and lifecycle checks;
they cannot establish those costs. Preserve the Stage 7 quality and source/
filter/quote/navigation gates. No Stage 8 optimization or extra model sweep is
justified by this pass. Consolidated interactive search/index/fallback/EPUB/RTL/
audio smoke remains user-deferred; remind at final acceptance. A documentation-
only commit does not require another desktop inference run.

## Stage 7 — cross-platform provider build checks — 2026-10-09

**Decision: extend the existing platform jobs with explicit provider-enabled app
builds; keep normal package artifacts and the default-off setting unchanged.**
Linux provider compilation/regressions already pass remotely. This pass adds:

| Target | Added check | Status |
| --- | --- | --- |
| Windows x64 | Tauri app build with shared native TTS and the dynamic provider, without bundling | Passed on `0796645` |
| macOS Intel / Apple Silicon | Same app build on both existing matrix runners | Passed on `0796645` |
| Android arm64 | Debug APK build with shared native TTS and the dynamic provider | Passed on `0796645` and `771b034` |
| iOS arm64 simulator | App archive with native TTS and the linked provider | Passed on `771b034`; export collision resolved |
| iOS arm64 device | Release static library followed by unsigned generic-device Xcode linking with native TTS and the linked provider | Passed on `771b034` |

The checks reuse the existing build helpers, runtime preparation, caches and
platform gates. Desktop and Android checks run after normal artifact uploads,
since provider builds reuse executable/APK output paths. No provider artifact
is published. The ordinary build commands and release workflow keep their
original features. No new npm script, model download, runtime version, package
fork or generated Apple project change is needed.

The existing `ios:ci:device` helper now accepts optional `--features` through
Node's strict argument parser. It always retains `native-tts-ios` and
`tauri/custom-protocol`, prepares the device static archives, stages `libapp.a`
and suppresses a second Tauri Rust build during unsigned Xcode linking. The
simulator helper already forwards extra features to Tauri, so it is unchanged.

Local validation passes an assert-based command self-check with native build
tools mocked: normal/provider feature forwarding, device/static-library staging,
simulator versus device runtime paths, unsigned-link flags, invalid arguments,
unsupported-host rejection and stopping on Cargo failure. Workflow YAML parsing,
added shell syntax, platform selection and upload-before-provider assertions
also pass. These checks exercise command construction, **not foreign-target
compilation or model inference**. Only the Linux host is available locally;
Windows/macOS/Android/iOS builds must run on their existing CI runners.

The initial next action was a full CI run; its outcomes and the corrective pass
are recorded below. A successful
build would close a compile/link gap, not validate runtime initialization,
model activation, mobile performance or same-language relevance. Provider
runtime/device acceptance, the original crash/retention caveats and the explicitly
deferred Stage 7 lifecycle/results/RTL/navigation/responsiveness smoke remain
open. Stage 8 optimization is still not justified.

### First cross-platform run and iOS export correction

[Full run 38009770905](https://github.com/muhannadnouri/papercut.io/actions/runs/38009770905)
tests **`07966456a72ba2af035c4ef61ef242a4a0c1bbe2`**, matching the committed
provider-build changes. Attempt 1 is a `workflow_dispatch`; the change-detection
log confirms `VALIDATION_SCOPE: full`. All nine other reported jobs succeed;
**only `build-ios` fails**. The Windows, both macOS and Android provider steps
pass, as do normal builds/artifact uploads and Linux provider checks.

The iOS job passes its normal simulator and unsigned device builds. During the
provider simulator step, Rust finishes its release compilation and Xcode reports
`BUILD SUCCEEDED`. The fatal error follows archiving: Tauri tries to rename the
archived `Papercut.app` into the simulator export directory occupied by the
first build, then fails with `Directory not empty (os error 66)`. Its error
message names the source archive path. The provider device step is skipped.
Missing Metal-toolchain Swift search paths and the 15.1-versus-15.0 simulator
deployment warning are nonfatal in this log; they are not evidence of a provider
runtime failure or a reason to change ORT/Libtashkeel here.

**Fix: `ios:ci` now passes `--archive-only` in the existing shared iOS helper.**
CI needs compilation and final app linking, not a simulator app export. The
pinned [Tauri CLI path](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/mobile/ios/build.rs#L428-L461)
still archives the app, then returns before the export/rename. Both normal and
provider simulator checks use that path. Release IPA/init commands keep their
behavior. No output deletion, cache reset, dependency update or generated Xcode
project change is needed.

The command assertion fails against the pre-fix helper and passes after the
change. Mocked checks also confirm the simulator target/runtime and native/provider
features are retained, release/init commands do not acquire archive-only, and
device/Android commands and workflow boundaries still pass. JavaScript syntax
and diff checks pass. These were local command regressions; the corrected Xcode
archive and previously skipped provider device build needed the remote rerun
recorded below.

The corrective pass requested `validation_scope=mobile` to cover both iOS
checks and Android, along with the shared checks job. Passing desktop jobs did
not need repeating for this iOS-only helper change. The resulting verification
follows; provider inference/device/resource acceptance remains separate, and
the interactive Stage 7 smoke stays explicitly deferred.

### Mobile correction — verified 2026-10-10

[Mobile CI run 38023521669](https://github.com/muhannadnouri/papercut.io/actions/runs/38023521669)
passes on **`771b034dabb414d07d61e830d76eb801943b18a6`**, matching the local
review source and GitHub's pushed `feature/search-v2` ref. Attempt 1 is a
`workflow_dispatch`; the change-detection log confirms `VALIDATION_SCOPE: mobile`.
All five executed job summaries succeed. Desktop and SILMA packaging are
intentionally skipped by this scope, not failed or newly retested.

The inspected iOS log confirms that both normal and provider simulator commands
finish with `build/app_iOS.xcarchive`. Both unsigned generic-device builds report
`BUILD SUCCEEDED`, including the previously skipped provider-enabled device
step. The normal/provider Android steps and normal APK upload also pass.
The shared checks report **168 frontend tests / 47 files**, **161 default Rust
tests / 5 ignored**, and **216 provider Rust tests / 10 ignored**; dependency
audits, lint and frontend/model-monitor/SILMA checks pass. The earlier Swift
search-path and 15.1-versus-15.0 deployment warnings persist without failing
the archive/device checks; this does not establish support on the minimum OS.

**Decision: the sequential simulator export failure is resolved, and the added
other-platform provider compile/link checks are verified across the two runs.**
Windows/macOS evidence remains the successful `0796645` full run; this mobile
rerun validates the scoped iOS helper correction on `771b034`. Normal package
artifacts still omit the provider and the semantic toggle stays default-off.
No embedding weights, artifact installation, app launch or inference/device
runtime checks are added by this review. Asset-dependent and scale tests remain
explicitly ignored in ordinary CI.

**Next:** provider initialization/cached-model inference against each platform's
packaged runtime, real-device resources/lifecycle and representative product
quality remain acceptance work. Keep the consolidated Stage 7 UI smoke deferred
as requested; native crash/retention and weak-reference-device caveats remain.
No further build-matrix rerun is needed merely for this evidence-only docs
commit, and no Stage 8 optimization is justified by these build results.

## Stage 7 — opt-in provider CI checks — 2026-10-09

**Decision: enforce the Linux provider-enabled code path in the existing checks
job; normal package builds and the default-off product setting stay unchanged.**
The prior green full run validated normal configurations but omitted this path.
Two steps now follow the retained default Rust tests in `.github/workflows/ci.yml`:

- Build the non-test native library with
  `native-tts-shared,experimental-search-embeddings` and the locked dependencies.
- Reuse the existing Linux runtime-copy helper after Cargo prepares sherpa/ORT,
  then run the complete library suite with those features and the shared runtime
  on `LD_LIBRARY_PATH`.

The steps inherit the existing `code_checks` gate: they run whenever that job
runs, including full/manual checks and code-changing PRs. No new workflow,
dispatch option, npm script, Tauri configuration, crate/runtime version or model
download is added. Normal desktop/mobile build commands still omit embedding
features. The existing checks timeout remains; the first remote run completes
within it, as recorded below.

Local execution passes the non-test provider library build and **216 tests /
10 ignored**, with the same commands using cached dependencies offline. Existing
regressions cover provider capability/default-off behavior, request validation,
activation generations, installer integrity/interruption, index invalidation,
source/quote/scope contracts and stale query handling. This is automated fixture
coverage, not real-model inference. Asset-dependent inference/IPC/audio, external
library and scale probes remain explicitly ignored; no private corpus or model
weights are required by ordinary CI. Previously recorded real-cache checks
remain prior evidence rather than new runs.

Workflow YAML and added Bash syntax checks pass using installed tools; assertions
confirm ordering after the default suite, the existing checks gate, and unchanged
normal package feature boundaries. These remain local checks from the preceding
pass; no local test reruns are claimed in the remote review below.

### First remote provider run — verified

[Full CI run 38005733746](https://github.com/muhannadnouri/papercut.io/actions/runs/38005733746)
passes on **`9bfe0307e767ad523f80201d5afc024305ed9b9a`**, matching the local
review source and GitHub's pushed `feature/search-v2` ref. Attempt 1 starts
2026-10-09 23:42:30 UTC and is complete by 2026-10-10 00:07:02 UTC. Inspected
all ten successful job summaries and the complete checks/change-detection logs;
the latter confirms `VALIDATION_SCOPE: full`.

The new provider library build passes in **94 seconds**. The runtime-copy/helper
and regression step pass in **24 seconds**, including test compilation; the
suite reports **216 passed / 10 ignored**, with 1.43 seconds of test execution.
The complete checks job takes **8 minutes 34 seconds**, within its unchanged
30-minute timeout. These are CI step/test durations, not app load, indexing or
search latency. The retained default suite reports **161 passed / 5 ignored** and frontend
**168 passed / 47 files**. Dependency audits/lint/build checks and normal Linux,
Windows, macOS x64/arm64, Android and unsigned iOS simulator/device builds pass;
Linux media and both macOS runtime verification steps succeed.

**Decision: Linux provider-enabled non-test compilation and fixture regressions
are now verified remotely.** The ignored model/asset/external-library/scale
checks remain unexecuted by this run; it establishes neither real-model inference
nor provider-enabled packages/mobile/other-desktop runtime acceptance. Normal
package features and the default-off product behavior remain unchanged. No
artifact download/installation, app launch, push or dispatch is performed by
this review, and no interactive smoke checkbox is marked passed.

**Next:** provider-enabled platform linking/device/runtime and representative
product-scope quality/resource checks remain the useful acceptance work. Keep
the consolidated rebuilt-app smoke explicitly deferred as requested. The
earlier crash/allocator-retention caveats remain; no Stage 8 model/ANN work is
justified. This evidence-only documentation follow-up does not require repeating
the CI matrix merely to change the review commit SHA.

## Stage 7 — verified full CI acceptance — 2026-10-09

**Decision: the CI revision gap is closed; normal-platform automated validation
passes for the integrated candidate. Stage 7 release acceptance remains open.**
The user supplied [CI run 38002904236](https://github.com/muhannadnouri/papercut.io/actions/runs/38002904236).
GitHub reports completed/success, attempt 1, on
**`022ea959e5c05f84e71f0b19ae68dd6df45bb87c`**, matching local HEAD at review
and GitHub's current `feature/search-v2` ref. This revision includes all five
Stage 7 implementation commits and the earlier acceptance review. The run
started 2026-10-09 23:08:29 UTC and completed by 23:30:36 UTC. Unlike the older
run below, it validates the integrated source revision.

Inspected the run/ref responses, all ten GitHub-reported jobs and their step
summaries, full change-detection/checks logs, and artifact metadata. The dispatch
log explicitly records **`VALIDATION_SCOPE: full`**. All ten jobs succeed,
including the audit action's separate security check. Platform-specific steps
skipped on other operating systems are expected; no required platform job was
skipped.

| Current-revision CI area | Verified evidence |
| --- | --- |
| Frontend and normal native checks | Lint, locale/type/Vite build, **168 frontend tests / 47 files**, model-download monitor self-test, SILMA self-test and **161 default Rust tests (5 ignored)** pass. npm audit reports zero vulnerabilities; Rust audit and inactive-waiver graph check succeed. |
| Normal desktop packaging | Linux, Windows, macOS x64 and arm64 builds succeed. Linux AppImage media verification and both macOS bundled-runtime verifications succeed. |
| Normal mobile builds | Android native-TTS APK and unsigned iOS simulator/device native-TTS builds succeed. These are build checks, not device launches or listening tests. |
| Published artifacts | Metadata lists eight unexpired run/SHA-bound artifacts: Linux AppImage/deb, Windows MSI/NSIS, macOS x64/arm64 DMG, Android APK and SILMA CPU runtime. No artifact was downloaded, installed or launched in this review; iOS evidence is the unsigned build steps. |

**Scope of the pass:** the existing workflow still tests Rust without embedding
features and builds normal TTS configurations. It compiles the integrated React
UI for normal packaging and tests provider-unavailable/default lexical contracts
in the checks job. It does **not** run the real embedding provider on those platform
runtimes. The dependency audit's `--all-features` graph inspection does not
change that boundary. Prior local Linux provider-enabled 216-test, cached
FP32/INT8 handler, Wry release-build and synthetic scan checks remain separate
prior evidence, not newly executed remote checks.

Rechecked the original Stage 7/8 acceptance requirements: default-off semantic
search, independent conceptual results and lexical fallback remain the approved
candidate. Current-SHA normal CI is now satisfied. The remaining gates are the
user-deferred lifecycle/conceptual reader/RTL/keyboard/recovery/responsiveness
smoke, representative product-scope relevance, provider-enabled platform/device
validation and reference-hardware resource safety. The earlier crash, allocator
retention and INT8 portability caveats remain unresolved. No new source change
or Stage 8 model/ANN work follows from this green run.

**Next:** keep smoke deferred as explicitly requested and retain the existing
[focused checklist](search-v2-smoke.md) for the consolidated rebuilt-app pass.
The next useful automated pass is targeted provider-enabled CI coverage using
existing build prerequisites, without enabling the provider in ordinary packages.
Platform/device runtime and product-scope evidence remain separate; normal CI
alone does not justify widening
availability or marking Stage 7 shipped. This follow-up changes documentation
only, so it does not require another full build matrix solely to refresh the
review commit SHA. Rerun relevant CI when executable/configuration changes occur.
No tests were rerun locally, push or CI dispatch performed, or unrelated files
included.

## Stage 7 — acceptance review and CI binding — 2026-10-09

This section records the earlier review before the user supplied the
[current-revision full run](#stage-7--verified-full-ci-acceptance--2026-10-09).
Its revision-gap finding is historical and resolved above.

**Decision at completion: retain the integrated opt-in candidate; Stage 7 release
acceptance remains open.** Reviewed local source `f0cae5b`, the original Stage 7/8 plan,
current workflow/build entry points, opt-in/query boundaries and existing
regression/quality/resource evidence. No new model, runtime, ranking policy or
optimization is justified. Two obsolete native module comments now describe the
integrated path; executable behavior is unchanged.

The user reports pushing and a successful full CI run. The latest successful
branch run returned by GitHub is
[CI run 37991207464](https://github.com/muhannadnouri/papercut.io/actions/runs/37991207464),
created 2026-10-09 21:04:50 UTC, on **`bd8162b223265da576eedb58a99e22f9aa346962`**.
Inspected all nine jobs and their step summaries: checks, Linux/Windows/macOS
x64/arm64 builds, Android native-TTS APK, iOS unsigned simulator/device builds,
SILMA runtime and change detection all succeed. Linux media and macOS runtime
verification steps succeed. No artifacts were downloaded or installed, and full
job logs and the exact dispatch input were not inspected.

Both GitHub's branch-ref response and a fresh read-only `git ls-remote` still
return **`bd8162b`** for `origin`'s `feature/search-v2`. Local `f0cae5b` is five
commits ahead: semantic storage, lifecycle controls, native query, React results
and the measured phrase fix. No run was returned for that local SHA. Therefore
this verified green run supports the earlier normal-build candidate, **not the
new integrated Stage 7 commits**. Reconcile the push/run SHA before recording
current-commit remote acceptance; the user's report is retained without silently
binding it to a different revision. The workflow/build scripts are unchanged
between these revisions.

Full CI's regular Rust test command has no embedding features. Desktop scripts
select normal TTS features, Android selects `native-tts-shared`, and iOS selects
`native-tts-ios`; none enables the embedding provider. The all-features dependency
graph audit is not an all-features runtime/build test. Even a green run on the
current SHA would validate normal platform configurations and frontend behavior,
not provider-enabled cross-platform linking, lifecycle or resource use.

| Acceptance area | Review finding and remaining evidence |
| --- | --- |
| Product choice and plan | Default-off toggle, lexical default and independent conceptual section match the approved product decision. Separate channels intentionally replace the original trial fusion step; rejected RRF/reranker sweeps are not unfinished implementation. |
| Source/filter/quote contracts | Existing native tests and real cached handlers cover constrained selection, source spans/hashes, readiness, invalidation/removal/deletion, disable and deterministic best-document ranking. Actual reader targeting and UI races remain in deferred smoke. |
| Optional lifecycle and fallback | Explicit download/cache verification, saved consent, explicit resumable index jobs and enabled-path fallback are implemented. No automatic download/indexing from search. Hook tests and mock handlers do not establish WebView restart/recovery acceptance. |
| Quality at product scope | Earlier human-labeled public passage evidence supports same-language concepts; familiar/provisional grades do not independently establish displayed-answer/full-library quality. The independent product-scope gate remains open. Unquoted entity/date constraints are not promised; lexical research modes remain available. |
| Resource safety | The 100,000-passage synthetic scan justifies the completed phrase fix, not ANN. It excludes inference. Previous native activation/reload peaks and retention still apply; weak-device responsiveness, sustained CPU contention, battery/thermal cost and the original crash remain unresolved. |
| Platform/build scope | Normal earlier-revision platform CI is verified. Current Linux provider compilation and cached handlers are locally tested; provider-enabled installers, Windows/macOS/Android/iOS runtime/device acceptance remain unverified. `compiled` reports capability, not certification. The retained INT8 export's portability is unverified. |
| Interactive acceptance | Earlier five smoke groups passed by user report before Stage 7. New lifecycle/conceptual navigation, RTL, keyboard, responsiveness and recovery groups remain explicitly deferred; no checkbox is marked complete. |

**Next at completion:** push the outstanding local commits to the intended branch and bind a
successful full run to their SHA. Keep interactive smoke deferred as requested;
when ready use [the existing focused groups](search-v2-smoke.md), then review
product-scope relevance and provider/device evidence before wider availability.
Normal builds continue omitting the provider. Do not mark Stage 7 shipped or
start speculative Stage 8/model work merely to fill the deferred interval.

Validation in this review: source/workflow/plan inspection, GitHub job-step
summaries, remote-ref comparison, formatting and diff checks. No executable
behavior changed, so the passing native/frontend checks recorded in the prior
passes are not repeated or relabeled as fresh runs. No push or CI dispatch is
performed by this review.

## Stage 7 — native scan and acceptance checks — 2026-10-09

The ignored `conceptual_query_scale` check creates **200 generic documents /
100,000 passages / 384 dimensions** in disposable SQLite storage. It runs the
production read-only retrieval with synthetic unit vectors, checks scope,
required phrases, scanned/result counts, deterministic ties, one query encoding
and read-only access. No private documents, model downloads or user library
writes are involved. The fixture's source and derived index occupy **259,915,776
bytes**; this is not a vectors-only size estimate.

A saturated common-phrase query initially took **3,416.7 ms**, versus **154.4 ms**
for a warm unrestricted scan. Each document repeated the same FTS lookup. The
native query now uses bound, uncorrelated SQLite `IN` subqueries, reusing each
phrase's candidate set before authored-source verification. Scoped readiness
counts remain independent of phrase matches; body-token boundaries, hyphen
aliases, exact-source verification, best-passage ordering and lexical behavior
are retained. SQLite manages candidate sets; Rust does not retain an unbounded
posting list. Common phrases still incur a library-wide FTS candidate lookup,
even with a narrow vector scope.

The focused optimized store run passes **10 checks**, including the scale probe.
After the fix the same common-phrase case takes **189.9 ms**; unrestricted scans
remain about **155 ms**, a 500-passage scope takes **1.3 ms**, and a missing phrase
returns without encoding/scoring in **0.55 ms**. These are development-host,
OS-warm synthetic-vector measurements, excluding model inference and indexing.
They justify this one fix, not an ANN index, universal latency claim, quality
claim or weak-device acceptance. The ten-second cooperative budget remains.

The existing real-cache command check also covers the response states consumed
by React: missing index before build, partial/empty after title invalidation,
updated source after rebuild, incompatible/empty after explicit index removal,
ready/empty after deleting the only source, and a recoverable error after
provider disable. Explicit cached FP32/INT8 runs use disposable app data and
actual handlers; these checks do not start a WebView.

Both optimized cached runs pass: **2.11 s FP32 / 1.40 s INT8** for the complete
handler/index lifecycle test, including local cache copying and activation;
these are not query-only timings. Full native suites pass **161 default tests
(5 ignored) / 216 provider-enabled tests (10 ignored)**. The final scale-only
rerun reports 160.8/156.2 ms first/warm, 1.39 ms scoped, 193.8 ms common phrase
and 0.59 ms missing phrase, with **26,616 KiB process RSS high water**. That RSS
includes fixture construction and all scans **without a model**, not the real
provider's much larger activation/reload footprint or foreground app memory.

The final normal Linux shared-TTS **Debian package builds successfully** using
the existing offline desktop command. Its 41,763,686-byte archive contains the
matching three prepared runtime libraries and original Libtashkeel license,
with no optional embedding cache assets. The extracted binary resolves sherpa
and ORT through its relative resource path without installation or app launch.
The normal dependency tree still excludes FastEmbed; no new runtime, package
fork, configuration or npm script is added. Locale/TypeScript/Vite checks run as
part of the builds, retaining the existing large-chunk warning. React is
unchanged; its prior 168 tests remain evidence, not a fresh test-run claim.

The explicit provider-feature Wry release app also builds with
`native-tts-shared,experimental-search-embeddings --no-bundle`, using the existing
shared runtime and npm Tauri entry point. This validates compilation/linking;
no provider-enabled installer, WebView launch or new interactive smoke is claimed.

**Next:** keep the provider behind its explicit build feature and opt-in toggle.
The remaining acceptance work is rebuilt-app lifecycle/conceptual-result smoke,
existing full/platform CI after pushing, and reference-device/platform resource
checks. Smoke remains user-deferred: use [the consolidated checklist](search-v2-smoke.md)
when ready; automated handlers and packaging do not replace reader/RTL/UI checks.
No new model/fusion sweep or further speculative optimization is scheduled.

## Stage 7 — opt-in conceptual-results UI — 2026-10-09

`src/search-semantic/useConceptualSearch.ts` independently queries the native
command; `ConceptualResults.tsx` renders a separate **Conceptual results · Your
Library** section. The lexical hook only exports submission identity/scope;
`App` composes the optional boundary through a SearchTab results slot. Removing
the semantic feature does not require rewriting lexical ranking/cards. The
existing All words/Broader results and counts stay separate, and semantic work
does not delay their completion. No dependency, runtime, build-script or native
retrieval changes are made in this pass.

No semantic queries or fallback notice while off or with empty/invalid input.
When enabled, queries preserve parsed unquoted text, every authored quoted clause
and include/exclude/empty document scope. Filter changes wait for the matching
lexical submission, avoiding duplicate native reservations. Repeated submissions
have unique identity; old query, scope, source, model or index state is hidden
immediately, and late completion after disable/unmount is ignored. A microtask
skips the retired StrictMode effect before invoking native work. These are UI
freshness guards, not interruption of an already running native call.

Ready results show plain authored passages and section metadata, without HTML
injection, query marks, similarity-as-confidence, literal counts or concordance.
The existing section/page/text target opens the displayed source, rather than
substituting the query or a required quote found elsewhere. UTF-8 byte offsets
are not passed as DOM offsets. Existing legacy/block/normalization navigation
limits still need rebuilt-app checks; a best-effort section/page jump is not
proof of exact passage highlighting on every viewer.

Missing/incompatible/partial indexes and unavailable/preparing/failed providers
show enabled-path lexical fallback with recovery guidance. Partial results are
not displayed: continue lexical search across the scope until indexing is
complete. The query's scoped coverage is shown when known; result count means
shown documents, not all possible conceptual matches. Ready empty results are
separate from failure. Unquoted names/dates are conceptual input, and quoted
clauses remain required at document scope even if the displayed passage differs.

Settings exposes the existing installer plus **Enable semantic search (optional)**,
which still defaults off, requires an explicit verified cached activation and
explicit index build/resume. Download consent remains separate. Normal builds
cannot enable a missing provider but can turn off a saved opt-in; browsers offer
no native setup. Eight locales describe the functional path. Linux-first evidence
does not imply Android/iOS or weak-device acceptance.

Validation: frontend **168 tests pass / 47 files**, typecheck, eight-locale parity,
scoped ESLint and Vite production build pass (existing large-chunk warning retained).
Eight added regressions cover opt-in/capability boundaries, required phrases and
scope forwarding, filter/submission races, stale A→B→A and source/disable/unmount,
StrictMode, fallback vs empty, plain-text provenance/reader targets, and disabling
a saved preference without a provider. Existing lexical and lifecycle tests pass.
Rust is unchanged; the prior native **161/216** suites and real cached FP32/INT8
handler checks remain the native evidence, not rerun claims in this UI-only pass.
No new model/fusion sweep or WebView/mobile/performance acceptance is claimed.

**Next:** rebuilt-app lifecycle and conceptual-result smoke, then acceptance and
packaging/resource checks against the now-functional opt-in path. The user has
kept smoke deferred; remind them and use the focused checklist rather than claiming
that the earlier cache-only acceptance covers this UI. Push the local commit and
rerun the existing platform/full CI when ready. Keep the feature/default-build
boundary until release gates are met.

## Stage 7 — native conceptual retrieval — 2026-10-09

At completion of this earlier pass, `document_uploads/semantic/query.rs` added
one independent read-only command; React still returned lexical results. Ordinary builds and inactive providers fail
recoverably, without activating/downloading a model or initializing storage.
No dependency, runtime, config or script changes are needed. The existing lexical
query limits, safe FTS phrase construction and authored-source verifier are reused.

Scope and every required quoted clause constrain documents before scoring or the
result limit. Empty scope means no documents; absent scope means all uploads.
Body-token candidates exclude title-only/substrings, and authored verification
rejects stemming-only phrases while preserving punctuation/line-wrap aliases.
Quotes retain document scope, so clauses can occur in different sections and the
chosen conceptual passage need not be the literal phrase location. Unquoted
names/dates/words remain conceptual input, not new mandatory terms.

One best passage ranks each document by cosine descending, with deterministic
source-order/document-ID ties and at most 50 retained documents. Repeated chunks
do not create extra votes. Results carry plain actual source text, conceptual
provenance, section/page locators and validated section-local UTF-8 byte spans;
there are no fabricated marks/counts. The later UI must use the existing reader
section/page/text target boundary rather than treating bytes as DOM offsets.

Only compatible completed documents are visible. Missing/incompatible/partial/
ready states and scoped readiness counts support the later enabled-path fallback;
unavailable-index counts do not establish coverage. Corrupt vectors/source hashes
fail closed. Snapshot plus fresh-generation checks reject edits, deletion, index
removal or model changes during scoring. One independent query slot and a
cooperative ten-second budget constrain concurrency/waiting, but cannot interrupt
a native call or a document's phrase verification. Result memory is bounded;
exact scoring still scans compatible scoped vectors linearly. This is not a
large-library latency, relevance-threshold or weak-device acceptance claim.

Validation: default Rust **161 pass / 4 ignored** and shared-TTS/provider **216
pass / 9 ignored**. Five added native tests cover scope/quote starvation, body
word boundaries and hyphen aliases, partial/unknown compatibility, Unicode spans,
corruption/limits, source edit/deletion/removal races and query-slot cleanup.
Default-off command checks preserve no-storage behavior. The existing cached
FP32 and INT8 command-index checks now also query through the actual handler,
verify returned spans against stored text and check empty scope, in disposable
app data with no downloads. Default/provider non-test Cargo checks, formatting
and diff checks pass. These are structural/handler checks, not a new relevance
comparison, query-latency benchmark or WebView navigation test. React is unchanged;
its suite was not rerun in this native-only pass.

**Next at completion of this earlier pass:** wire the native query through the isolated React semantic boundary into
a separate labeled conceptual-results section, with stale-result protection,
existing filters/required phrases, actual reader targets and visible fallback
only when opted in. Move preparation controls out of Developer mode when that
path is functional. Remind the user of the deferred lifecycle plus new query/UI
smoke then; keep quality, platform, native stability and memory gates open.

## Stage 7 — saved opt-in and index lifecycle — 2026-10-09

The isolated `src/search-semantic/` hook/component has one owner in `App`, shared
by Settings in both library and reader layouts. The existing model-cache panel
shares its format selection and installer; no new dependency, runtime, build
script or configuration is introduced. Controls stay under Developer mode until
the separate conceptual-results path is implemented.

The device-local preference defaults off. Enable activates a verified cached
format, with no automatic download. A saved opt-in reloads that cache on restart;
index synchronization remains explicit. Invalid preferences fail closed; failed
persistence refuses enabling, while disabling still stops native work and warns
about persistence failure. Build/resume, cancel, remove and coverage refresh are
explicit actions. Completed-document/passages and encoded/reused counts are
shown honestly. No ranking or literal-query semantics change in this pass.

Native disable signals the index writer before waiting on model cleanup. Job
slots stay busy through cleanup and checkpoints remain reusable. Status and
download-cancel lock waits use the blocking pool; job progress is sampled after
coverage work. UI generation guards reject stale completion after disable,
native disable generations also prevent a queued loader from starting/publishing
after an early disable; duplicate/conflicting actions are refused, and polling does not pile up status
requests while a native call is slow. Closing Settings or entering the reader
does not detach the App-owned lifecycle. Allocator retention and native-call
delay remain limits, not fixed by the toggle.

Validation: frontend **160 tests pass** across 45 files, including saved opt-in,
StrictMode/restart, failure/retry, cancellation races, persistence failure,
coverage and controls; typecheck, locale parity, scoped ESLint and Vite production
build pass (existing large-chunk warning retained). Default Rust **156 pass / 4
ignored** and shared-TTS/provider **211 pass / 9 ignored**, with local loopback
access for fixture servers. The new native regression proves provider disable
cancels an in-flight index slot without prematurely releasing it; a queued-load
regression proves a superseded activation never reaches its loader. Explicit cached
FP32 and INT8 command-index checks pass in disposable app data, with no downloads.
Default and provider non-test Cargo checks pass. New interactive checks are in
the [focused smoke follow-up](search-v2-smoke.md#stage-7-preparation-controls--new-follow-up);
the user's earlier smoke pass predates these controls.

**Next at completion of this earlier pass:** bounded native conceptual retrieval for compatible completed documents,
existing scope and required-phrase verification, real source spans/reader targets,
then a separate labeled conceptual-results section and visible enabled-path
lexical fallback. Move functional opt-in controls out of Developer mode when
that path is usable. Keep All words/Broader literal results independent; do not
add another model/fusion sweep or advertise untested platforms.

## Stage 7 — App index foundation, 2026-10-09

The optional Rust boundary lives in `document_uploads/semantic.rs` and
`document_uploads/semantic/store.rs`, with narrow provider methods in
`search_embeddings.rs` and explicit handler/state registration in `lib.rs`.
Lexical query, import, title-edit and deletion implementations are unchanged.
No new dependency, build script, configuration or React setting is added.

Four commands expose explicit build, status, cancel and removal. Builds require
an already verified, active provider; they never download or activate one.
Normal builds reject builds/removal recoverably, and status does not initialize
semantic storage. Even an enabled provider build only creates semantic tables
on an explicit indexing request. The existing UI does not call these commands.

The index shares the upload SQLite database so source mutations and invalidation
are transactional. Triggers remain effective if a later normal build edits the
library. They mark modified documents unavailable until synchronization; deletion
cascades derived rows. Document generations prevent stale publication after
edits, deletion/recreation, removal/rebuild or switching compatible identities
away and back. Incompatible explicit builds discard only derived vectors;
unknown schemas fail closed rather than being overwritten.

Identity includes the verified encoder and every model/tokenizer/config asset
checksum, vector dimension, extraction/source-normalization policy, E5 prefix
and versioned 320-token/40-token-overlap chunk policy. Source content hashes
include the exact embedded title, heading and body. Section-local UTF-8 spans
use the actual tokenizer; oversized metadata/passages fail explicitly instead
of silently truncating. Current source sections retain reader/page locators.

One writer runs on the blocking pool. It reads one document at a time, performs
native inference outside SQLite write transactions, checkpoints individual
passages and publishes only a completed document. Retry/restart reuses validated
unchanged vectors, including identical reimports with new section row IDs.
Partial checkpoints do not count as available documents. Obsolete spans are
pruned when a document finishes. Dimensions, finite unit-normalized values and
stored vector checksums are validated before reuse/publication. The existing
storage preflight supplies reserve/headroom; SQLite storage errors still fail
recoverably if the estimate is exceeded. Cancellation is cooperative, not native
call interruption. Provider status/disable can wait for a section's tokenization
or one inference. Per-document source retention and foreground resource limits
still require realistic device measurements.

Validation: six storage/job tests and a default-off IPC test cover checkpoint
recovery, unchanged reuse, source mutation races, format/removal roundtrips,
cascade deletion, UTF-8/vector corruption, unknown schemas and job cleanup.
The default library suite has **155 passing tests** and four ignored tests
(the two loopback checks pass separately outside the socket-restricted sandbox).
The shared-TTS/provider suite passes **209**, with nine opt-in tests ignored.
The new ignored cached-model command check is then run explicitly for both
FP32 and INT8 in disposable app data. It exercises real English/Arabic/Mandarin
tokenization/overlap, native encoding, indexing, unchanged-build skip, title
invalidation, removal/rebuild and document deletion. This is structural/runtime
evidence, not relevance grading or an app/device responsiveness measurement.

**At this pass's completion, next was:** add the separate React semantic setting/hook and native preference/job
lifecycle: default off, explicit model choice/activation, indexing progress and
coverage, cancellation on disable and restart recovery. Then integrate bounded
semantic retrieval in the user-selected separate conceptual-results section,
with explicit provenance and visible lexical fallback for the enabled path.
Keep All words/quotes/filter behavior explicit and unchanged in the
default lexical path. Do not infer supported-platform or quality acceptance from
the authorization to proceed. Do not schedule another broad model/fusion sweep.

## Model-free acceptance review — 2026-10-08

**Decision: retain the model-free release candidate; do not claim release
acceptance or begin Stage 7.** Reviewed the original handoff's stages, hard
contracts and regression matrix against current query construction, retrieval,
source evidence, rendering, migration and tests on `d3c999c`. No search code or
new test is needed. The original plan permits this outcome independently of
whether embeddings ever ship.

| Candidate behavior | Existing checks and remaining boundary |
| --- | --- |
| Stage 1 field weighting and short-query adjacency | Stable ordering, metadata/body distinctions and mandatory-phrase controls pass. These reorder lexical candidates; they do not recover no-overlap concepts. |
| Stage 2 Broader | Coverage tiers, missing terms, quotes, scope and deduplication pass. Relaxation is explicit, Your Library only, for 3–12 distinct expressions; other query lengths keep all terms required. |
| Stage 3 spelling assistance | Authored-vocabulary, protected-input and complete scoped-retry checks pass; starter hits do not hide the retry. No silent replacement, general dictionary or spelling-variant guarantee. |
| Stage 4 Arabic search forms | Source offsets, mark/tatweel/alef policy, literal quotes and restart/replacement/delete migration checks pass. Authored text stays intact; Arabic morphology and all orthographic variants are unclaimed. |
| Query/evidence corrections | Long-query tail clauses, oversize rejection, nested quotes, cross-section passages, count/locator bounds and stale-request UI checks pass. Visual reader/navigation acceptance remains deferred. |
| Stage 5 / later semantic work | Hard-coded related-word pairs are removed. General aliases are intentionally deferred; the provider and isolated index/ranking experiment do not participate in app search. |

Fresh default-feature native tests pass **148**, with four opt-in tests ignored.
The complete frontend suite passes **147 tests in 43 files**. The existing
evaluation/parser checks pass on the unchanged version-5 authored fixture:
25 documents, 54 sections and 41 requests; document Recall@10 **41/45**, MRR@10
**0.917**, nDCG@10 **0.912**. All five labeled passages remain retrieved.

On the same **30 original requests and original labels**, Recall@10 is
**34/37**, versus **31/37** at baseline; no previously found relevant label is
lost. Only the three intended Arabic recoveries and title-ranking case change
result ID lists/order. This is retained regression evidence, not a new gain or
representative quality claim. The four fixture misses are related wording,
conceptual no-overlap and two literal typos with explicit verified retries.
Assisted retries are not counted as direct recall. Warm debug native p95 is
**3.47 ms** on this tiny fixture, excluding IPC/rendering and genuine cold disk.

The tracked upload/search implementation and relevant frontend sources are
byte-identical to `6f9fa7a`, the source of the existing 500-document /
100,000-section release probe. Reuse that bounded development-host scale
evidence rather than rerunning an unchanged workload. Real-book vocabulary
pressure, reference hardware and actual app responsiveness remain unmeasured.

Locked offline Linux dependency graphs for default and normal shared-TTS builds
exclude FastEmbed and experimental embedding features. Normal TTS keeps the
official preprocessor/runtime dependencies; this review does not remove those
branch changes or prove their non-Linux packaging. TypeScript, full ESLint,
eight-locale parity, production Vite and diff checks pass; the existing bundle
size advisory remains. Pagefind implementation/index generation, installed app
data, model downloads and production ranking are untouched. Source bindings,
reports and logs are retained privately, with no private-library details tracked.

**Next:** review normal-build packaging and supported-platform build readiness,
using the existing scripts and available toolchains. Missing toolchains/devices
must remain explicit gaps, not passing results. Do not add another search/model
tuning pass to fill the deferred interval. Consolidated rebuilt-app
search/EPUB/RTL/audio/model-control smoke remains user-deferred; remind at final
acceptance. That smoke plus representative device/platform evidence precedes
model-free release acceptance. Stage 7 additionally needs a quality win and
compatible product indexing/fallback/lifecycle evidence.

## Normal packaging and platform readiness — 2026-10-08

**Decision: normal Linux Debian packaging passes on this host; retain the
model-free candidate and leave platform/app acceptance open.** Reused the
existing desktop entry point on `35eb0ae`:

```sh
CARGO_NET_OFFLINE=true npm_config_offline=true npm run desktop -- --bundles deb
```

The normal frontend, starter-index and native bundle steps complete using cached
dependencies/runtime files. No script, configuration, dependency or product-code
change is needed. Pagefind generation here is the existing packaging step, not a
search change. Optimized native compilation takes 44.94 seconds; this is not
whole-build, import or search latency.

The resulting `Papercut_1.9.2_amd64.deb` is **41,725,840 bytes**. Its app matches
the current compiled binary apart from Tauri's expected `UNK` → `DEB` bundle
marker. Packaged sherpa/ORT and Libtashkeel license bytes match the cached source
files. The app's relative resource runpath and sherpa's sibling-library rpath
resolve both native libraries from an extracted package without
`LD_LIBRARY_PATH`. The ELF loader's list mode verifies resolution without running
application main; nothing is installed or launched. The compiler fingerprint
contains normal shared-TTS/preprocessing features and no experimental embedding
feature. No embedding model-cache files are packaged.

The app references GLIBC versions through **2.39**; packaged ORT and sherpa
reference through **2.16 / 2.17** respectively. These symbol references and
successful resolution on this development host do not establish distribution
compatibility, installer/startup success or device responsiveness. Retain exact
package, build logs, fingerprint, hashes and inspection results privately.

| Target / configuration | Evidence from this pass and remaining work |
| --- | --- |
| Linux x86-64 normal shared-TTS Debian | Build/content/relocated-loader checks pass. Install, WebView, reader/audio and actual app smoke remain deferred. |
| Linux AppImage | Existing CI build/media verifier reviewed; no AppImage produced or verified in this pass. |
| Windows x86-64; macOS x86-64/arm64 | Existing desktop CI matrix uses normal shared-TTS builds and runtime preparation. No corresponding host/toolchain or fresh build result here. |
| Android arm64 native TTS | Generated Gradle project and cached Java/native files exist; this host has no Android Rust target or NDK. Build not attempted; do not treat cached libraries as APK validation. |
| iOS arm64 simulator/device native TTS | Generated Xcode project exists. Linux cannot build it; the existing wrapper's macOS/Xcode guard rejects this host as expected. No archive/device result. |
| Experimental provider configurations | Normal packaging jobs do not enable them. Prior Linux provider tests do not prove other-platform build or lifecycle acceptance. |

The four build/check entry scripts pass Node syntax checks. Reviewed desktop,
Android and iOS preparation through feature selection, runtime paths and the
existing CI jobs. CI's unsigned iOS simulator/device checks are distinct from
the signed release/upload workflow. Missing toolchains are recorded rather than
downloaded merely to manufacture a local matrix. Build/config/workflow source
bindings remain unchanged; no installed library, optional model download,
publication or push occurs.

**Next:** after the user's push, obtain normal-build results through the existing
CI workflow on this branch: `validation_scope=full`, or separate `desktop` and
`mobile` runs. No workflow is dispatched in this pass. Review failed jobs before
making platform fixes; passing artifact builds still need actual device/app
acceptance. Consolidated search/EPUB/RTL/audio/model-control smoke remains
explicitly deferred; remind when the user resumes final acceptance. Do not start
Stage 7 or another model/search tuning pass while waiting for this evidence.

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
| Query and ranking | `search/query.rs` handles token bounds, punctuation/hyphen aliases, safe FTS literals, and normalized phrase comparison. `search.rs::document_candidates` uses BM25, lower scores first, best section per document, and existing stable tie breaks. Stage 1 gives title/heading/text weights of 5/3/1 for imported documents. Short unquoted searches additionally prefer adjacent body phrases within existing scope/coverage tiers. Default mode requires all unquoted terms; Stage 2's uploaded-only option allows measured missing terms at document scope. Stage 3 offers bounded authored-word typo retries after an empty uploaded search. |
| Evidence and serialization | `search.rs` verifies quoted phrases against source sections before limiting, then returns actual phrase counts or three body-scored passages, twelve section bins and per-term matching-section counts. Retained FTS excerpts use up to 64 tokens; section cards open their selected evidence target. Rust DTOs in `types.rs` serialize camelCase; `src/hooks/useSearch.ts` maps them to `src/types/search.ts`. Ordinary section counts are not word-occurrence counts. |
| Display and navigation | `src/components/SearchResults/SearchResults.tsx` renders evidence and lazy concordance; `src/utils/searchOpenTarget.ts`, `src/components/DocumentViewer/readerTarget.ts`, `readerTextRanges.ts`, and `src/viewers/pdfSearchTarget.ts` resolve sections, text and PDF pages. DOM offsets are mapped UTF-16 code units; current IPC uses IDs/text rather than raw offsets. |
| Optional model precedent | `src-tauri/src/native_tts/engine/model.rs` downloads/verifies local TTS assets; compatibility with an embedding runtime is unproven and belongs to a later experiment. |

Relevant checks: `npm run test:search-eval`; `cargo test --offline --manifest-path src-tauri/Cargo.toml document_uploads::search --lib`; `npm test -- src/hooks/useSearch.test.ts src/hooks/useSearchConcurrency.test.ts src/utils/phraseSearch.test.ts src/components/SearchResults/SearchResults.test.tsx src/viewers/pdfSearchTarget.test.ts`; `npm run build:typecheck`; `npm run lint`; `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`.

Validation on this checkout: all **125 frontend tests** passed; TypeScript, ESLint, Rust formatting, and the evaluation command passed. In the full Rust library suite, **132 tests passed**, **one optional sample-EPUB test was ignored**, and **two library-transfer socket tests were blocked by the filesystem/network sandbox**. Those two tests passed separately with local socket access. No search test failed.

## Stage 1 — BM25 field weights

Status: implemented on this branch, pending manual library testing. Imported-document FTS5 has two `UNINDEXED` ID columns followed by indexed title, heading, and text. Both candidate and passage queries now use `bm25(uploaded_document_fts, 0, 0, 5, 3, 1)`; lower scores rank first. The existing best-section-per-document aggregation and stable newest-import/document-ID ties remain. There is no schema migration or change to bundled Pagefind search. Repeated titles contribute to each section row, but a document takes its best section score rather than summing every row.

The development-only trials of title/heading/text ratios **3/2/1**, **5/3/1**, and **10/5/1** produced identical rankings and metrics across all 23 development queries. We chose the plan's modest **5/3/1** seed because stronger weights showed no development benefit. A single full-corpus check after that choice changed only `en-title-rank`: the direct title match `a017` moved from second to first, ahead of a weaker PDF body mention. The misleading-title query `en-title` still puts substantive `a001` first. All 30 queries retained result membership, candidate counts, deduplication, and source locators. The same corpus SHA-256 (`bebf246ea039f2f49a3d0eb143f63919a0b2bd5dab0a5aca09d9f8602568e4b4`) makes the [Stage 0 baseline](search-v2-baseline.json) directly comparable.

Document Recall@10 stayed **31/37 = 0.838** and MRR@10 stayed **0.828**. Overall nDCG@10 moved from **0.8154** to **0.8216**; held-out nDCG@10 moved from **0.9745** to **1.0000**, entirely from that one query. Development nDCG@10 remained **0.7648**. Passage metrics stayed at **1.0** on the three labeled cases. This is a narrow controlled ranking improvement, not evidence of a broad quality gain. The Stage 0 note already identified the held-out title case, so this held-out check was a regression guard rather than a blinded generalization estimate. More independently labeled title, heading, and long-book cases and manual testing are needed.

Warm pipeline p50/p95 on the tiny fixture measured **1.02/1.90 ms**, versus **1.12/2.03 ms** at baseline. The difference is too small and noisy to count as a speed gain; it only shows no obvious regression on this machine. The 500-document / 100,000-section reference-device gate and isolated app-data testing remain outstanding. `PAPERCUT_SEARCH_EVAL_SPLIT=dev npm run test:search-eval -- /tmp/search-v2-dev.json` runs the development subset; omit the variable for all queries. The new ranking assertions check both the specific-title and misleading-title cases. Validation: **24 Rust search tests**, **125 frontend tests**, TypeScript, ESLint, and Rust formatting passed.

## Stage 2 — Explicit broader lexical retrieval

Status: implemented on this branch, pending manual and large-library testing. **All words** remains the default and keeps its existing SQLite and Pagefind behavior. **Broader · Your Library** is an explicit upload-only choice; it does not search starter documents or change Pagefind. Switching modes reruns a submitted query. Quoted phrases stay mandatory and are verified against source text before limiting results.

For three or more distinct unquoted terms, broader mode requires at least half of them, with a minimum of two, at **document** scope. One- and two-term queries keep all-terms matching, which avoids admitting documents on one common word. Up to twelve indexed query terms are considered, using the existing parser and FTS aliases. Candidates are ordered by matched-term coverage, then the existing BM25/import-time/ID order. Matching terms and explicitly missing terms appear on result cards; title-only matches can have zero body-section occurrences without being called missing. Existing result and passage limits still bound evidence output. There is no index migration or new dependency.

Corpus [version 2](../scripts/fixtures/search-v2/corpus.json) adds **nine development queries**: eight broader cases and one default-mode guard, for **39 total** queries (32 development, 7 held out). Its SHA-256 is `03cd2273d95afb3365cd61f9b83daf642f161e7f85eb0fc73184ec53617a7714`. All **30 original queries** produced exactly the same result IDs, order, passage locators, and candidate/total counts as the Stage 1 report. The eight new broader cases found **6/6 labeled relevant documents** in five judged queries; three no-answer checks rejected a missing quote, a one-word-only match, and a two-term partial match. A 24-section document retained one result, three bounded passages, twelve position bins, and honest missing-term evidence. These small, synthetic, mostly scoped cases demonstrate the behavior, not general search precision. The new corpus changes the aggregate denominator, so its overall score should not be compared directly with the version 1 baseline.

On the 24-document fixture, broader queries had warm p50/p95 **1.81/3.07 ms**. This is a functional smoke measurement, not a 500-document / 100,000-section performance result; the established reference-device gate remains. The Rust path still scans candidate matches before applying the visible-result limit, so common terms in a large library need manual/device measurement before release. Run `npm run test:search-eval` for the complete report. Validation: **24 Rust search tests**, **128 frontend tests**, TypeScript, ESLint, locale parity, Rust formatting, and the Vite build passed.

## Stage 4 — English/Arabic search-form normalization

Status: implemented on this branch, pending manual and large-library testing. The existing English Porter FTS index and Pagefind path stay intact. Arabic-containing uploaded queries use a second FTS5 index with a version 1 search form: remove tashkeel (`U+064B`–`U+065F`, `U+0670`) and tatweel (`U+0640`), and fold `آ`, `أ`, `إ` to bare `ا`. Ya/alef-maqsura, ta-marbuta/ha, and other letters remain distinct. These choices can admit spelling-related false positives; quotations are still verified against authored section text. English punctuation and PDF hyphen handling already covered the measured fixtures, so this stage adds no new English rewrite.

Both FTS tables are derived from the same authored title, heading, and section text in the import transaction. Schema version 7 and `search_form_version=1` mark the policy. Existing libraries get an additive shadow table, then a background worker fills at most 64 missing sections per transaction. A restart resumes missing rows; import, title edit, OCR replacement, and deletion update both indexes. Until the backfill commits `search_form_ready=1`, Arabic queries use the original index, so search stays available with its previous recall. The original index is not rebuilt or rewritten. On the branch, manually test against a separate copy of app data: switching branches does not isolate `io.papercut.desktop` storage.

The normalized index nominates sections and ranks with the same 5:3:1 BM25 weights. Result excerpts and term text are remapped to authored characters before marking, including skipped Arabic marks and tatweel; section ordinals and PDF page indices still come from `uploaded_sections`. Normalized byte offsets never become reader offsets. A source-verified quotation remains literal and may reject a section nominated by the forgiving index. The reader's own highlight behavior is unchanged.

On the unchanged 39-query corpus, Arabic document Recall@10 rose from **3/6 to 6/6**: `ar-mark-gap`, `ar-tatweel-gap`, and `ar-alef-gap` now return their labeled documents. Overall recall is **40/43 = 0.930**, versus **37/43 = 0.860** before this stage on the same corpus. The 30 original queries differ from the Stage 0 baseline only in those three recovered Arabic cases and Stage 1's title ranking; English and held-out results retain their previous IDs and order. The added checks cover authored Arabic excerpts, quote rejection despite a forgiving candidate, mixed/emoji character mapping, PDF page locators after replacement, and a simulated interrupted legacy-index rebuild. Warm p50/p95 was **1.23/2.11 ms** on the tiny synthetic corpus; it is not a large-library performance claim. A second FTS table increases storage and indexing work, so the existing 500-document / 100,000-section gate and manual testing remain required.

Validation: **103 upload-module Rust tests passed**, one optional sample-EPUB test ignored; search evaluation, 16 targeted Search Results/reader tests, TypeScript, ESLint, and Rust formatting passed. The evaluation used a disposable SQLite database and did not open installed app data.

## Stage 3 — conservative typo suggestions

Status: implemented after Stage 4 on this branch, pending manual and large-library testing. Stage 4 moved ahead because the baseline showed three Arabic normalization misses versus one English typo miss. For an uploaded **All words** search with no native matches, the native worker may now propose one authored English spelling as an explicit retry. It never replaces the query, changes counts, or creates a result for the miss. The retry remains visible even when starter results exist; choosing it submits the corrected query through the ordinary search path, so excerpts and locators come from actual matching text. Pagefind code and search behavior remain unchanged.

Schema version 8 adds a small per-document vocabulary of distinct authored ASCII English words from title, headings, and body; it does not use Porter-stemmed `fts5vocab` terms. Imports/reimports, title edits, and deletions update the vocabulary transactionally. Existing libraries build it one whole document per background transaction, restart from missing documents after interruption, and keep normal search available while suggestions wait for `spelling_ready=1`. The indexed lookup narrows by the first two letters and word length before a one-edit insertion/deletion/substitution check. At most four unquoted words, 256 vocabulary candidates, eight verified alternatives, and one changed word per retry are considered. No suggestion runs for quotes, broader mode, Arabic/mixed script, uppercase names, digits, hyphenated identifiers, or short words. A lowercase rare name can still receive an optional suggestion; it is never silently corrected. The two-letter prefix and candidate caps deliberately leave some typos unresolved.

Corpus [version 3](../scripts/fixtures/search-v2/corpus.json) keeps the same 24 documents and 39 queries, adding a labeled suggestion contract to `en-typo`; SHA-256: `7baf5966937e3583bb4c29228c304e779e0531a7c8021038dd01ac0d60ea9dbe`. `enviroment` remains a zero-result literal search and suggests `environment`, which then finds the labeled `a009` document when explicitly retried. All 39 literal result-ID lists match Stage 4, so direct Recall@10 stays **40/43 = 0.930**; the assisted retry is reported separately rather than inflating that metric. A second synthetic case checks `collonialism` → `colonialism`, actual-source highlighting, quote/name/identifier/script exclusions, and a no-suggestion case against 1,024 distractor vocabulary entries. The legacy migration test covers restart, reimport, title change, and deletion. Warm p50/p95 was **1.15/2.12 ms** on the tiny fixture. Vocabulary size, background rebuild time, and suggestion latency still need the existing large-library/device gate.

Validation: **105 upload-module Rust tests passed**, one optional sample-EPUB test ignored; 16 targeted UI/evaluation tests, the search evaluation, TypeScript, ESLint, and Rust formatting passed. Evaluation used disposable SQLite data, not installed app data.

## Stage 5 — controlled related wording

Status: the one-off related-form retry was prototyped, then removed after review. It recovered one labeled fixture document, but each additional mapping required another word pair in product code. Moving those pairs into a file would preserve the same manual maintenance problem. The response field, UI retry, and mapping code are gone; literal results and the existing spelling correction path remain. No general synonym or spelling-variant expansion is claimed. The `en-stem` related-form miss and `en-concept` no-overlap miss remain useful evaluation cases. Stage 6 can test whether a local model helps with conceptual retrieval; any future general spelling-variant method needs separate precision and performance evidence.

## Corrective pass — visible uploaded spelling help

Manual testing with a larger uploaded library exposed a combined-results problem: an uploaded **All words** miss could have a verified spelling retry, yet a Starter Document hit concealed it. The hook and results component now preserve and display that retry even when starter results exist. A generic fixture uses `calender meeting agenda` → `calendar meeting agenda`; the typo stays a literal miss until the user chooses the corrected search. Pagefind code and behavior are unchanged. Starter hits can still have different matching semantics from uploaded All words results, so the combined result label needs separate consideration if that work is authorized later.

No word pair from the manually imported library is stored in source, fixtures, or this progress record. The earlier hard-coded related-form pair was also removed. A generic spelling variant such as `color`/`colour` remains an open recall case. Dictionary-like overrides are not a scalable search strategy; optional corpus-vocabulary suggestions or local embeddings should be evaluated against independently judged queries before either becomes a feature.

Corpus [version 5](../scripts/fixtures/search-v2/corpus.json) adds one synthetic document and two development queries: a three-word typo and its correctly spelled control. SHA-256: `d5f3e1012a0be0e72be64fd19e6a34335af2e879b111fd31b7e62450a7df9781`. The 25-document, 54-section, 41-query fixture reports direct Recall@10 **41/45 = 0.911**. The previous 39-query corpus was **40/43 = 0.930**; the new denominator includes one assisted-only relevant label, so these percentages are not a like-for-like quality comparison. The frontend regression checks that a verified uploaded retry remains visible beside starter hits. Tiny-fixture timings do not establish large-library performance.

The larger library provides exploratory cases, not held-out relevance judgments. Before Stage 6, collect independent judgments for representative queries including spelling variants, exact names/phrases, Arabic, and conceptual misses. Stage 6 remains an optional local embedding experiment; Stage 7 depends on measured quality and device results. Isolated app data and reference-device checks remain necessary before considering a release.

## Model-free decision gate before Stage 6

Status: synthetic regression comparison complete; independent relevance and reference-device gates remain open.

On the 30 queries shared by the original baseline and corpus version 5, result IDs and order changed in only four cases: three intended Arabic recoveries and one intended title-ranking reorder. This is a like-for-like regression comparison; the 41-query aggregate above is not comparable to the 30-query baseline. The evaluation command now fails if an original query disappears or a document labeled relevant and found in the original baseline falls out of the top ten. It still reports new gains and assisted retries separately. The remaining synthetic misses are a related-form case, a conceptual no-overlap case, and two literal typo cases. No general synonym or conceptual retrieval claim follows from the model-free work.

The next evaluation needs independently judged queries and relevant passage/document IDs from a representative library, with exact names, quotes, Arabic, spelling variants, conceptual no-overlap questions, and irrelevant neighbors. Keep private titles, text, and raw queries outside tracked fixtures and reports. Record only aggregate quality and device measurements here. Freeze the judgments before comparing lexical-only, embedding-only, and hybrid results; keep the existing synthetic suite as a regression check, not the Stage 6 quality gate. The model experiment remains conditional on those judgments and a realistic library-size performance baseline.

## Stage 6 — isolated desktop embedding probe

Status: inference and synthetic comparison working; relevance, storage, and reference-device gates open. [`experiments/search-v2-embeddings`](../experiments/search-v2-embeddings) is a standalone Rust executable, not part of the Tauri app. It uses FastEmbed 7.1.0 with `intfloat/multilingual-e5-small`; its ONNX Runtime dependency stays separate from Papercut's existing TTS/runtime build. No app dependency, search index, result UI, or default behavior changed. The model and any private inputs stay outside the repository.

The model card specifies `query:`/`passage:` prefixes, 384-dimensional vectors, mean pooling, L2 normalization, and a 512-token maximum. The executable verifies finite, unit-length 384-dimensional outputs. It makes section-anchored passages using the model tokenizer, a 320-token budget including heading/prefix, and 40-token overlap, retaining source section and byte spans. It ranks documents by their best passage score, then compares the existing lexical report with semantic-only and simple document-level rank fusion. Quoted queries are excluded from this prototype comparison because semantic similarity cannot satisfy source-verified phrases. The original probe repeated indexing; the aligned comparison pass below adds an immutable external development cache. It still does not serve app search results. The nested-quote parsing defect found during evaluation is addressed in the corrective pass below. [Model card](https://huggingface.co/intfloat/multilingual-e5-small) · [FastEmbed model support](https://docs.rs/fastembed/7.1.0/fastembed/enum.EmbeddingModel.html).

On the generic version 5 fixture, 25 documents produced 54 passages. Across 28 judged unquoted queries, lexical Recall@10 was **31/35**, semantic **35/35**, and hybrid **35/35**; top-one relevant-hit counts were **25/28**, **27/28**, and **28/28** respectively. The sole conceptual query moved from **0/1** to **1/1** at Recall@10. Model load took about **3.4 s**, embedding those passages about **1.1 s**, and semantic query embedding plus in-memory scan had about **11 ms p95** on this Linux development machine. Raw vectors were **82,944 bytes**; the downloaded model cache occupied about **465 MiB**. These are debugging measurements, not evidence of a library-scale quality win: ten results across 25 documents is an easy target, labels are incomplete, and the query timing excludes lexical retrieval, disk reads, and rendering.

Run the experiment after `npm run test:search-eval` with `FASTEMBED_MODEL_DIR` set to an explicit cached model snapshot outside the checkout and `cargo run --offline --manifest-path experiments/search-v2-embeddings/Cargo.toml -- scripts/fixtures/search-v2/corpus.json dist/search-v2-evaluation.json`. The snapshot must contain `onnx/model.onnx`, `tokenizer.json`, `config.json`, `special_tokens_map.json`, and `tokenizer_config.json`. Model loading uses these local files directly; the automatic model-download feature is disabled. Set `SEARCH_V2_CANDIDATES_JSON` to a new external JSON path for source-linked candidates and `SEARCH_V2_VECTOR_CACHE` to an external cache path to reuse compatible passage vectors. For the next gate, the user will supply real questions; candidate results will be prepared locally for human relevance grades, with private questions and document details outside tracked files. Then measure SQLite vector storage/cold scans and weak-device responsiveness. Do not integrate this crate into the app or upgrade Papercut's ONNX Runtime solely on the synthetic result.

## Corrective pass — nested quotes and production-path evaluation

Status: implemented and checked; manual reader smoke testing and independent relevance grades remain open. The shared frontend parser now retains inner quotation marks within one required phrase instead of splitting the phrase and treating intervening words as ordinary terms. Straight outer quotes with inner curly quotes, curly outer quotes with inner straight quotes, and nested curly quotes are supported. Delimiter pairs must balance; mismatched or unfinished nesting produces the existing quote error. Multiple separate quoted clauses retain their existing meaning. No escaping syntax for nested straight quotes is introduced.

The parser and backend checks use independent navigation examples. They verify the full phrase, the surrounding unquoted words, malformed nesting, source verification against an otherwise identical passage without inner quotation marks, authored excerpt marks, section locators, and a spelling retry that retains the original document filter. The source file stays unchanged across snapshot creation and searching; the snapshot stays unchanged across searching. A write through the read-only connection is rejected.

The existing evaluation command now accepts private libraries and questions through the real frontend parser and uploaded Rust search pipeline. It opens the input through a read-only SQLite connection, creates a consistent temporary snapshot with `VACUUM INTO`, and searches that snapshot without schema migrations or readiness changes. SQLite may require access to WAL shared-memory metadata even for a read-only connection; a filesystem sandbox can therefore prevent opening an installed library. Use a consistent exported database in that case, including committed WAL contents. Do not copy a database while it has active writes.

```sh
npm run test:search-eval -- --library /path/outside/checkout/search.sqlite3 /path/outside/checkout/queries.json /path/outside/checkout/report.json
```

The input is a nonempty JSON array, for example `[{"id":"navigation","query":"archive \"silver compass\"","mode":"all"}]`. `mode` defaults to `all`; `broader` is also supported. Optional `documentUrls` must be a nonempty list of document URLs. Result limits match the upload requests from the UI: 50 unscoped or 100 scoped. Inputs and reports must resolve outside the checkout; the report's parent directory must already exist and its filename must be new. The command rejects in-checkout paths and existing outputs before evaluation, then uses exclusive creation to prevent overwriting another file.

The private report contains parsed requests, document IDs, native counts, source excerpts, passage/section/page locators, and phase timings. An offered spelling correction is searched separately with the same scope and limit; its results never inflate the direct result list. HEAD revision and input/snapshot hashes identify the run, which uses the current working-tree code. It measures uploaded search only, excluding starter search, IPC/rendering time, and true cold-disk performance. Private results receive no inferred relevance grades. Reports remain outside version control.

Validation: **140 frontend tests**, **28 Rust search checks**, TypeScript, ESLint, Rust formatting, the synthetic evaluation, and private path/overwrite guards passed. The original baseline regression guard passed and generic direct Recall@10 remained **41/45 = 0.911**. A private production-path run also completed against a verified external copy; the installed source database remained byte-for-byte unchanged. Private queries, document details, and results are absent from tracked examples and documentation.

Next: smoke-test a nested quotation, an ordinary quotation, and the explicit spelling retry in the rebuilt app; verify each opens at the displayed source evidence. Prepare and freeze human relevance grades before any proximity-ranking trial or lexical/semantic/hybrid comparison. Persisted vectors, incremental indexing, cold scans, and reference-device responsiveness are still Stage 6 work. Stage 7 remains conditional on a measured quality benefit and acceptable device costs; this corrective pass changes neither ranking nor semantic integration.

## Stage 6 — private relevance review preparation

Status: grading packet generation implemented; provisional assistant grades recorded at the user's request, with independent relevance validation pending. **The user deferred the preceding pass's app smoke tests. Begin the next pass with a reminder to test nested quotes, ordinary quotes, the explicit spelling retry, and opening their displayed source locations.** Automated checks do not close this manual gate.

The existing evaluator can now prepare a review packet from a private production-path report without rerunning search or inference:

```sh
npm run test:search-eval -- --review /path/outside/checkout/report.json /path/outside/checkout/new-review-directory
npm run test:search-eval -- --check-review
```

The report and output directory must resolve outside the checkout. The parent directory must exist, and the output directory must be new. `review.md` presents questions and up to ten direct candidates plus ten explicit spelling-retry candidates per question, deduplicated by document and ordered by a deterministic hash rather than retrieval rank. It displays at most three native excerpts per candidate with authored section/page locators. Source text is rendered as inert code; native highlight marks are omitted. There is no new app UI, dependency, model run, or database access in this step.

`grades.json` starts with separate `documentGrade` and `evidenceGrade` values of `null`, plus a free-text reason. Use **0 irrelevant, 1 useful, 2 directly relevant**; leave uncertain candidates unjudged. Document relevance can require inspecting the full source. Evidence relevance judges the displayed excerpts together and is not a per-passage judgment. Candidate codes map back to document IDs, locators, and direct/retry ranks in `key.json`; avoid reading that key before grading. Input-report hashes bind the grades and key to the same saved run. On platforms that honor POSIX modes, the new directory is owner-only and files are owner-readable/writable.

This prepares a lexical baseline, not a scored semantic comparison. Earlier semantic candidate exports used a different document pool and omit their best-passage evidence; combining their ranks with this baseline would give an invalid comparison. Align the document pool, questions, filters, and source locators before pooling channels. Previously inspected questions remain exploratory/development cases, even with display order hidden. An independent untouched set is still needed for the held-out Stage 6 quality gate. A top-ten candidate pool also cannot establish library-wide recall, and null judgments must not be treated as irrelevant.

Validation: the runnable review checks cover deduplication across direct/retry results, preserved ranks in the separate key, repeatable presentation, per-channel limits, empty pools, unjudged defaults, inert source HTML, malformed locators, and duplicate query/hit rejection. Private path/overwrite and input-validation checks passed; a private packet was generated with matching report hashes and source mappings. The existing synthetic evaluation and baseline regression guard passed with direct Recall@10 unchanged at **41/45 = 0.911**; ESLint and diff checks passed. Private questions, titles, excerpts, packets, and grades remain outside tracked files.

The user delegated grading to the assistant. Provisional document and displayed-evidence grades were recorded with per-candidate reasons, explicit query interpretations, and assessment provenance. Source inspection covered the displayed passages, their full source sections, and additional matching-section contexts from the saved library copy. It was not an exhaustive reading of every document. Grades were written before consulting the packet's rank key, but earlier conversation context may already expose rankings; this is not a blind independent evaluation. The original unjudged sheet was preserved privately. No private questions, document details, judgments, or assessment files were added to tracked sources.

The direct review pool contained **33 candidates**: **18 directly relevant documents**, **14 useful background documents**, and **one irrelevant document** under the stated interpretations. Displayed evidence received **six direct**, **19 useful**, and **eight irrelevant** grades. **Twelve of the 18 directly relevant documents** had displayed evidence below grade 2. Two spelling-retry documents were judged separately and excluded from those direct totals. These are pooled development judgments, not library-wide recall, a held-out score, or a semantic comparison.

The main observed weakness is evidence presentation: short excerpts cut away the relationship between terms, and bibliographic entries or heading fragments can displace substantive body text. The uploaded path currently requests 18-token FTS snippets and retains three passages ordered by BM25; those choices warrant a bounded evidence-quality experiment. An unrelated co-occurrence within one section also shows why proximity alone cannot guarantee conceptual relevance.

Next: improve passage selection and excerpt context first, using the provisional grades as development diagnostics. Preserve document membership, literal counts, filters, and authored locators; rerun regression checks and regrade the displayed evidence. Then assess a modest phrase/proximity ranking preference if justified. Independent relevance validation and the deferred app smoke checks remain open. Stage 6 vector persistence, incremental indexing, cold scans, and reference-device measurements can be assessed separately; production hybrid search remains conditional on quality and device gates.

## Corrective pass — source evidence and excerpt context

Status: implemented on `feature/search-v2`; manual app and reference-device checks remain open. This supersedes Stage 1's shared candidate/passage score policy. Document ranking still uses title/heading/body weights **5/3/1**; passage selection uses **0/0/1**, so metadata alone cannot boost an evidence passage's score. BM25 length normalization still applies, and short citations can still win. No citation blacklist or semantic relevance guarantee is implied.

Ordinary original-index excerpts now request up to **64 FTS tokens**, using [SQLite's native fragment selection](https://www.sqlite.org/fts5.html#the_snippet_function). The existing matching-section scan retains its 18-token distribution anchors. Longer context is fetched in one batch for at most three retained passages per visible document, plus its initial candidate excerpt, using actual FTS row IDs rather than assuming they equal source section IDs. Exact-phrase evidence and Arabic authored-text restoration retain their existing excerpt policies.

A section card's excerpt, heading, section/page target, and result ID now refer to its selected body evidence. This can choose a different opening section without changing document membership or order. Cross-section cards keep their existing explanation and expose all three passages in Explore; the former display filter could hide a passage whose excerpt was absent from the main card. Counts, distribution targets, term comparisons, and document filters keep their existing meanings.

Same-pool provisional assistant regrading froze document grades and the original query interpretations. Across **33 direct candidates**, directly relevant evidence increased from **6 to 11**, useful evidence changed from **19 to 15**, and irrelevant evidence from **8 to 7**. Six evidence grades improved; none declined. Directly relevant documents with weaker displayed evidence decreased from **12 to 7**. Two explicit retry candidates retained their grades and remain outside those totals. These are development diagnostics, not independent held-out validation, a recall gain, or a semantic comparison. Some citations, disconnected co-occurrences, and missing explanatory context remain unresolved.

Private before/after checks preserved document IDs/order, totals, per-term coverage, and complete distribution objects. Both verified quotation responses were identical. **264** excerpt/locator pairs were checked against source sections; unquoted excerpts were checked as authored substrings, while exact evidence retained its verified canonical display. The installed database remained byte-for-byte unchanged. Private inputs, passages, grades, and reports remain outside tracked files.

Warm measurements used debug builds on the development machine, six runs per variant with the first discarded, the same copied database and parsed requests, and fresh SQLite connections with a warm OS cache. The busiest query's median native time increased from approximately **113 to 154 ms**, with evidence construction increasing from **54 to 95 ms**. Its serialized native response grew from approximately **114 to 155 kB** (compact UTF-8 JSON). This is an ongoing query/payload cost, not an indexing cost. Bounded context generation limits extra excerpt work; these timings do not establish a latency improvement or a cold-disk, rendering, mobile, or weak-device pass. An additional per-term body-coverage prototype was removed because it added work without improving the reviewed grades beyond the simpler change.

Validation: **29 native search checks** passed, with the private-library check run separately; **32 targeted frontend result, hook, parser, and PDF-target checks**, TypeScript, ESLint, formatting, and the synthetic baseline regression checks passed. Generic direct Recall@10 remains **41/45 = 0.911**. The new generic regression covers misleading metadata, retained context, stemmed highlights, selected document identity/filter/counts, paired PDF targets, differing FTS/source IDs, and the metadata-only fallback. The existing rendering check now protects all three cross-section passages.

Next: rebuild and smoke-test nested/ordinary quotations, the explicit spelling retry, longer result-card excerpts, all three cross-section Explore passages, and opening their authored HTML/PDF source targets. Then assess a small phrase/proximity document-ranking trial against frozen development judgments; avoid treating ordinary stemmed-word co-occurrence as a conceptual answer. Stage 6 still needs an aligned lexical/semantic candidate pool with source evidence, independent questions, persisted/incremental vectors, realistic cold scans, and reference-device measurements before Stage 7 hybrid integration.

## Corrective pass — optional body-phrase ordering

Status: implemented on `feature/search-v2`, pending manual/device and independent quality validation. **The user deferred the preceding evidence pass's smoke tests. Remind them at the start of the next pass:** nested/ordinary quotes, explicit spelling retry, longer card excerpts, all three cross-section Explore passages, and HTML/PDF source jumps. Add a short compound query to check the new ordering in All words and Broader modes. These checks remain a release gate; they did not block this reversible ranking pass.

For **two to six unquoted terms**, with at least two same-section candidates, search makes one additional indexed body-phrase lookup. It reuses the existing FTS phrase/hyphen aliases and Arabic search-form normalization, then stably prefers those documents. Broader-mode document coverage remains first; BM25 and import/ID ties remain within each tier. Default-mode cross-section results retain their eligibility and existing lower group. The preference runs before the visible limit, so a different document may enter a limited result list without changing eligibility or totals. Requests containing any required quotation retain their previous ranking.

This signal measures adjacent indexed tokens in the supplied order, including existing stemming and hyphen aliases; it is not a newly verified literal quotation. It changes neither literal counts nor evidence construction. Metadata-only phrases do not qualify for the body signal. It does not resolve synonyms, unrelated stemmed-word co-occurrences, or no-overlap conceptual questions.

A private development probe compared this phrase preference with SQLite's [native NEAR grouping](https://www.sqlite.org/fts5.html#fts5_near_queries). In the frozen pool, phrase preference improved one short compound question; the longer cross-section questions had no qualifying signal. Wider proximity gave a slightly larger improvement on that same question, but the simpler phrase lookup was selected without introducing another query-expression builder or distance setting. Broader proximity scoring remains deferred until a larger independently judged set warrants it.

The production-path rerun used the frozen assistant document grades from the preceding review. For the one changed seven-candidate question, **nDCG@10 increased from 0.951 to 0.966**, with directly relevant documents occupying all three leading positions rather than two. The other six questions retained their ordering, including the empty direct result and the verified quotations. All **33 direct review candidates** retained their grades; two explicit retry candidates were unchanged. Every native per-document result, including counts, passages, distribution, term comparisons and authored locators, was identical. No new top-ten candidate needed a judgment. This is a modest same-pool development improvement, not independent held-out validation, a recall gain, or a semantic benefit. The installed database and frozen grades remained unchanged; private questions/results remain outside tracked files.

Alternating before/after debug binaries used the same copied library and parsed requests, six runs each with the first discarded, warm OS cache and fresh SQLite connections. Candidate-stage medians increased by approximately **0.1–1.1 ms** on eligible questions. The changed question's total median was approximately **22.1 to 23.0 ms**; the busiest question was approximately **152.0 to 154.3 ms**. These exclude IPC/rendering and do not close cold-disk or reference-device gates. No inference or indexing was needed for this preference.

Validation: the development split passed before the selected policy was checked against the complete synthetic regression set. Generic direct Recall@10 remains **41/45 = 0.911**; **30 native search checks**, the separately invoked private-library evaluation, production parser checks, formatting and diff checks passed. One new generic integration check exercises the real orchestration: a body phrase outranks misleading metadata, existing joined-compound aliases/stemming still work, per-document evidence remains identical, a PDF page target stays paired, All words retains cross-section candidates, Broader coverage retains priority, document filters hold, and required-quote order/counts remain unchanged.

Next: return to Stage 6 by aligning lexical/semantic/hybrid candidate exports to the same document pool and questions, retaining source-linked best-passage evidence and reusing embeddings across runs. The previous selected semantic export cannot establish a full-library comparison. Grade that development comparison, then use untouched questions and complete persistence/incremental/cold/device gates before deciding on Stage 7 integration. Carry the deferred smoke reminder into that next pass.


## Stage 6 — aligned source-linked comparison

Status: development experiment only; app smoke, independent relevance, vector lifecycle, cold/disk and reference-device gates remain open. **The user again deferred app smoke tests. Start the next pass with the pending reminder:** nested/ordinary quotes, explicit spelling retry, longer excerpts, three cross-section Explore passages, paired HTML/PDF source jumps, and a short compound query in both search modes.

The production evaluator can export embedding inputs from the same read-only snapshot used for lexical search:

```sh
npm run test:search-eval -- --library /external/search.sqlite3 /external/queries.json /external/native.json --embedding-input /external/new-input-directory
FASTEMBED_MODEL_DIR=/external/cached-model-snapshot SEARCH_V2_VECTOR_CACHE=/external/vectors.json SEARCH_V2_CANDIDATES_JSON=/external/new-candidates.json cargo run --release --offline --manifest-path experiments/search-v2-embeddings/Cargo.toml -- /external/new-input-directory/corpus.json /external/new-input-directory/lexical.json
npm run test:search-eval -- --review /external/new-candidates.json /external/new-review-directory
```

Inputs retain production-parsed questions, mandatory phrases, document filters, authored sections and PDF pages. With every question scoped, the export contains the union of those document scopes; otherwise it contains the whole snapshot. Unknown scope identities are rejected rather than broadening retrieval. Corpus hashes bind the exported lexical ranks to the source pool; the probe rejects mismatched questions, constraints, evidence IDs and out-of-scope ranks. Direct spelling retries remain separate. Quoted questions retain their verified lexical control and are excluded from semantic/hybrid retrieval.

The earlier chunker inherited the inference tokenizer's 512-token truncation. Long sections could therefore lose source text after their first window. Source tokenization now disables truncation/padding before producing overlapping chunks, and checks each prefixed passage against the 320-token budget. Inference keeps its bounded tokenizer. The known-tail multilingual regression verifies contiguous coverage, UTF-8 source byte spans, token limits and PDF page pairing. The original probe's passage counts and quality results are preliminary; they do not establish whole-section semantic coverage.

The experiment reuses an immutable whole-probe JSON vector cache outside the checkout. Its SHA-256 key includes actual model/tokenizer artifact hashes, pooling/normalization/runtime policy, chunking/prefix version, and complete generated passage content/spans. Reuse validates count, 384 dimensions, finite values and unit norms. Incompatible/corrupt caches fail explicitly; the caller chooses a new path or removes the old cache. Query changes can reuse passage vectors. This deliberately small development cache is not transactional SQLite persistence, resumable/incremental indexing, streamed retrieval, or an app feature; interrupted writes are rejected on the next read.

Semantic results export their actual best source passage with document identity, section/page, UTF-8 byte bounds and score. Hybrid uses document-level reciprocal rank fusion with the existing 50-candidate channels and k=60 seed; scores and ties remain deterministic. Combined hits preserve available native lexical evidence; otherwise they retain the semantic source passage. The separate semantic export remains available for grading, without silently replacing stronger lexical evidence. The pooled review now includes up to ten candidates per direct/retry/semantic/hybrid channel, deduplicates documents, and retains distinct evidence sets for the same document. Each set can receive its own evidence grade. Retrieval ranks/channels stay in the separate key, and all initial grades are null. Semantic excerpts contain authored text and no invented literal match counts or highlights.

The private development run aligned **25 selected documents, 2,804 sections and seven supplied questions** through actual URL scopes on the saved library snapshot. This is a biased selected pool, not a full-library or held-out evaluation; lexical statistics still come from the library's FTS index. Five unquoted questions were compared; two quotation questions remained identical lexical controls. The source-complete chunker produced **3,504 passages**. Initial release-build passage embedding took approximately **608 seconds** with two inference threads. Raw float32 vectors occupy **5.13 MiB**; the simple JSON development cache occupies **15.59 MiB**.

The user had delegated grading to the assistant. The pooled packet contained **59 question/document candidates and 83 distinct evidence sets**. All received provisional grades after displayed-source and additional source-context inspection. The **23 existing document judgments** present in this pool stayed frozen; new document judgments were entered before computing channel metrics. Prior context exposed some results, and these are not blind independent labels or exhaustive document reviews. Correcting hybrid evidence preservation retained document ranks/grades and required grading one newly displayed lexical set. All private details, questions, reports and judgments remain outside tracked files.

| Development channel | Mean document nDCG@10 | Direct documents in pooled top tens | Direct displayed evidence in pooled top tens |
| --- | --- | --- | --- |
| Lexical direct | 0.565 | 12 / 23 returned | 6 / 23 |
| Lexical with the explicit spelling retry for its empty direct question | 0.701 | 14 / 25 returned | 7 / 25 |
| Semantic | 0.820 | 16 / 50 returned | 8 / 50 |
| Hybrid | 0.924 | 17 / 50 returned | 9 / 50 |

These means cover the same **five unquoted development questions** and use the shared judged candidate pool for ideal ordering. They are not library-wide recall, independent quality estimates, or precision at a common returned-list length. Reporting the explicit retry separately avoids presenting an already assisted lexical recovery as a wholly new semantic win. Semantic ranking was worse than lexical on one question; hybrid slightly worsened the strongest lexical question (**1.000 to 0.997**). Unquoted semantic retrieval deliberately does not enforce lexical All words eligibility, so it cannot silently replace that product contract.

The key remaining weakness is evidence: **20 of 50** semantic and hybrid displayed sets were judged irrelevant, compared with **four of 23** direct lexical sets. Short navigation/citation fragments can gain similarity from repeated title/heading context, and broad conceptual neighbors can lack the requested relationship. Better document ordering therefore does not establish better reader-facing answers. The next quality experiment should test that source-selection weakness against these frozen development grades before any production integration.

A fresh cached release process reused vectors with identical semantic ranks/passages and unchanged document fusion order. Cache loading/validation took about **0.05 seconds**; model/artifact loading took **1.7–2.1 seconds**, and recreating anchored chunks took **1.4–2.2 seconds**. Query embedding, in-memory scan and fusion/evidence together measured roughly **9–15 ms** per unquoted question in these warm runs. These are five samples, not a stable device p95 or full hybrid latency; lexical retrieval, IPC/rendering and cold disk are excluded. A cached subprocess peaked at approximately **1.42 GiB RSS**, including model startup, the whole probe source, vectors and serialization. That measured startup cost needs investigation before a weak-device decision. The JSON cache and whole-source loading are deliberately probe-scale choices, not scalable production storage.

Validation: **30 native search checks**, production parser/synthetic baseline guards (direct Recall@10 **41/45**), **two ordinary experiment checks and the separately invoked cached-model long-section check**, pooled-review/export checks, negative scope/question/hash guards, ESLint and both Rust formatting/diff checks passed. Cache reuse retained semantic ranks and evidence; combined hits preserve their complete native evidence when available. **77 semantic byte-span/source/page pairs** were verified against authored sections. Private inputs and source database copies were not modified. Manual reader smoke checks and independent English/Arabic, exact-name/date, no-overlap and irrelevant-neighbor judgments remain open.

Next: make a bounded semantic evidence-selection experiment and investigate model startup memory, then obtain untouched questions for independent validation. Incremental SQLite persistence, restart/deletion/update behavior, cold loading and reference-device costs remain Stage 6 gates before any Stage 7 decision. Remind the user of the deferred app smoke checks at the start of the next pass.

## Stage 6 — bounded semantic evidence and startup-memory probe

Status: implemented in the isolated desktop experiment only. This supersedes the preceding experiment's maximum-cosine excerpt selection; document ranking, native lexical evidence and app behavior retain their preceding policies. **Deferred app smoke checks remain open:** quotes, explicit spelling retry, longer excerpts, all three cross-section Explore passages, HTML/PDF source jumps, and a short compound query in both modes. Carry this reminder into the next pass.

Semantic document ranking still uses each document's maximum passage cosine. Evidence now uses its highest-scoring passage with at least **32 body model tokens**, excluding title/heading/prefix tokens. A document with no qualifying passage falls back to its original best short passage. This is a small context heuristic, not a citation blacklist, answer detector or relevance guarantee; a concise correct answer can lose to a weaker longer passage. The threshold is a development seed, not a tuned product setting. Independently test concise answers and multilingual content before accepting it. Body-token counts are derived from the existing untruncated tokenizer windows; embedded text and source spans are unchanged, so existing vectors remain compatible. Reports record the evidence policy, the selected passage's `cosine`, and the original ranking passage's `documentCosine` separately. Hybrid continues to copy complete available native lexical evidence.

The same five unquoted development questions retained all document membership/order and frozen document grades. Six semantic excerpts changed: **four improved from irrelevant to useful, two retained useful grades, none declined** under provisional assistant regrading of displayed source/context. Semantic irrelevant evidence fell from **20/50 to 16/50**; hybrid irrelevant evidence from **20/50 to 17/50**. Direct-answer evidence remained **8/50 semantic and 9/50 hybrid**. Document nDCG remained **0.820 semantic and 0.924 hybrid**. This improves displayed background context in a reused selected pool, not independent quality, conceptual recall or demonstrated answer correctness. Unrelated conceptual neighbors and incomplete requested relationships still remain. Private sources, questions, excerpts and grades stay outside tracked files.

The memory trial used ONNX Runtime's existing [weight-prepacking option](https://github.com/microsoft/onnxruntime/blob/main/include/onnxruntime/core/session/onnxruntime_session_options_config_keys.h), through the installed FastEmbed session configuration. Four alternating fresh release subprocesses per variant used the same input and vector cache; the first per variant was discarded. On this development machine with a warm OS cache and two inference threads, median peak RSS was **1,483,572 KiB enabled versus 1,483,408 KiB disabled**, effectively unchanged. Median query embedding/scan/fusion time across the remaining fifteen question samples was approximately **13.3 versus 21.7 ms**. Median complete subprocess time was approximately **4.97 versus 4.71 seconds**, and model-load time **2.03 versus 1.94 seconds**; these few noisy samples do not establish a startup-speed benefit. Ranked documents and selected spans matched; maximum selected-passage score difference was about **2.4 × 10⁻⁷**. Existing passage vectors were reused for this diagnostic, so it is not a fresh-index equivalence or quantization-quality check. **The switch was removed**: no retained runtime/configuration, dependency, model identity or cache-key change.

A separate temporary Linux `/proc/self/status` phase probe located the high-water mark during session construction: RSS rose from approximately **14 MiB before reading the model** to **463 MiB after reading it**, then reported **1,449 MiB peak / 984 MiB current** after session construction. Current RSS was approximately **1,002 MiB after queries**. Cache loading did not materially change the observed resident level. This localizes the cost to model/session initialization; it does not identify every allocator copy or prove a file loader alone would fix it. FastEmbed's installed explicit-artifact constructor accepts model bytes and calls `commit_from_memory`. Diagnostic instrumentation stayed outside the retained implementation. No cold-disk, weak-laptop, phone, indexing-peak or production-runtime compatibility claim is made.

Validation: **four experiment checks**, including the explicitly enabled cached-model long-section check, and pooled-review checks passed. The new generic regression protects evidence selection, the 31/32-token boundary, short-only fallback, unchanged document order, scope, source/PDF target pairing, correct passage score, and unchanged vector serialization. The final release rerun reused the existing cache and retained the trial's source excerpts/ranks. **77 semantic/hybrid byte-span/source/page pairs** were verified, quotes retained their lexical controls, native evidence remained identical, and all document grades stayed frozen. Rust formatting and diff checks passed. No private corpus examples or word-pair overrides were added.

Next: test **one justified quantized export of the same model** against this float32 baseline, with a separate artifact identity/cache and explicit quality, source-evidence, startup-memory and latency comparison. Do not start a model leaderboard or production integration. Untouched independent questions, especially concise answers, names/dates, English/Arabic, no-overlap and irrelevant-neighbor controls, are still needed. SQLite vector persistence, incremental update/delete/restart behavior, bounded/cancellable disk scans and cold/reference-device measurements remain Stage 6 requirements before a Stage 7 go/no-go decision. Remind the user of the deferred app smoke checks at the next pass.

## Stage 6 — same-model INT8 comparison

The isolated experiment now accepts `SEARCH_V2_MODEL_QUANTIZATION=dynamic-int8`; absent or `none` retains float32 policy. Unsupported values fail explicitly. This is an explicit experiment selection, not an app search mode or production default. Existing FastEmbed/ONNX dependencies and the app runtime stay unchanged.

The comparison uses the publisher's [multilingual-e5-small ONNX artifacts](https://huggingface.co/intfloat/multilingual-e5-small/tree/614241f622f53c4eeff9890bdc4f31cfecc418b3/onnx) at revision **614241f622f53c4eeff9890bdc4f31cfecc418b3**, with identical tokenizer/config files, query/passage prefixes, mean pooling, L2 normalization, 384 dimensions and source chunking. The float32 artifact is **470,268,510 bytes**, SHA-256 `ca456c06b3a9505ddfd9131408916dd79290368331e7d76bb621f1cba6bc8665`; the publisher's `model_qint8_avx512_vnni.onnx` is **118,346,824 bytes**, SHA-256 `dd476dd0c2514e9b9be83aeb3853fac0763e0bdf4a71645407587d77c48a2d88`. Both downloaded files matched the pinned publisher metadata. The selected artifact occupies the probe's existing `onnx/model.onnx` path in a separate external model directory; no automatic model resolution/download or additional model family was introduced.

[Dynamic quantization](https://onnxruntime.ai/docs/performance/model-optimizations/quantization.html) derives activation ranges at inference time. The installed FastEmbed dynamic policy forbids splitting one multi-text call into internal batches. The probe instead embeds **one passage per call** for dynamic INT8, giving each passage a stable range independent of indexing peers. Float32 retains **16-passage batches**. An outer bounded loop now releases each batch's token-level outputs before proceeding; the old all-passage call retained every internal batch until pooling. A generic cached-model check crosses the float32 batch boundary and verifies exact agreement with the original call. For INT8 it verifies identical vectors after reordered/resumed indexing. It also exercises English and Arabic retrieval against an unrelated source, and existing checks retain source-tail/UTF-8/page coverage.

The dynamic model identity includes its per-text inference policy and actual artifact hashes. Its passage cache is distinct from float32; cached float32 vectors must not be searched with INT8 query vectors. Output reports record the quantization policy. Model quantization does **not** quantize the stored vectors: both remain normalized float32 vectors. Changing the model or compatible preprocessing requires fresh passage embeddings.

The preceding `/tmp` model/vector/review artifacts were unavailable when this pass began. A new fixed **25-document, 3,418-section** selected development pool was rebuilt from a read-only copy of the installed library, using the same seven supplied questions and production parsing. Selection used bounded round-robin direct/retry candidates, so it is biased toward lexical results, not a representative held-out corpus. Both formats use the same hash-bound corpus, lexical report, constraints and source spans. Earlier grades were not reconstructed or assumed; this comparison cannot claim a longitudinal nDCG change from the preceding pool. Private queries, sources and examples remain outside tracked files.

The source-complete chunker generated **4,365 passages**. The release comparison used Linux x86_64 on an Intel Core Ultra 5 325, two inference threads, a warm OS cache and fresh processes. This CPU reports AVX2 but not AVX512 VNNI; the selected publisher export ran successfully here. Its filename alone does not establish portability or performance on other CPUs, mobile devices or the app's runtime. Both formats used the installed experimental FastEmbed **7.1.0 / ort 2.0.0-rc.13**. No dependency upgrade was needed.

| Development resource measurement | Float32 | Dynamic INT8 |
| --- | --- | --- |
| Model artifact | 448.48 MiB | 112.86 MiB |
| Initial passage embedding | 335.13 s | 103.53 s |
| Median fresh cached-process peak RSS | 1,485,456 KiB (1.42 GiB) | 799,928 KiB (0.76 GiB) |
| Median complete cached-process time | 2.13 s | 1.87 s |
| Median model/session load time | 0.97 s | 0.73 s |
| Median query embedding + scan + fusion/evidence | 6.94 ms | 4.49 ms |
| Raw float32 passage vectors | 6.39 MiB | 6.39 MiB |
| JSON development vector cache | 19.42 MiB | 19.42 MiB |

Four alternating cached subprocesses per format were measured; the first per format was discarded. Peak RSS uses Linux child-process high-water accounting from a fresh wrapper process. Query medians combine fifteen samples per format from the five unquoted questions; they exclude lexical retrieval, IPC, rendering and cold disk. Initial indexing was one development run per format, with some float32 overlap with targeted checks/builds, not a controlled throughput benchmark. The observed approximately **46% lower startup peak**, **75% smaller artifact** and faster initial embedding justify retaining this explicit experiment. They do not close device, cold-start or production-latency gates.

A temporary `/proc/self/status` phase probe again localized the high-water mark to session construction. Float32 reported approximately **1,450 MiB peak / 985 MiB current** immediately after session creation and **1,004 MiB current** after queries; INT8 reported approximately **781 MiB peak / 652 MiB current**, then **670 MiB current** after queries. These are per-process readings, not cumulative sibling-child accounting. Session memory remains substantial. The probe was removed from the retained code; no bespoke loader or runtime tuning switch was added.

The pooled review contained **63 question/document candidates**. Float32 judgments were entered before reviewing INT8 changes; shared document grades stayed fixed, while new candidates and changed evidence sets were graded from displayed source and additional context before calculating scores. These are provisional assistant judgments, not blind human labels or exhaustive reviews. Both required-quotation questions remained identical lexical controls and were excluded from the five-question semantic comparison. All rows below cover fifty returned items and use the same combined judged pool for ideal ordering.

| Development channel | Mean document nDCG@10 | Direct documents | Direct displayed evidence | Irrelevant displayed evidence |
| --- | --- | --- | --- | --- |
| Float32 semantic | 0.851 | 17 / 50 | 8 / 50 | 17 / 50 |
| INT8 semantic | 0.888 | 17 / 50 | 8 / 50 | 15 / 50 |
| Float32 hybrid | 0.948 | 17 / 50 | 12 / 50 | 15 / 50 |
| INT8 hybrid | 0.963 | 17 / 50 | 11 / 50 | 13 / 50 |

Document ranking was comparable or slightly better on these aggregate development scores, but **INT8 was not regression-free**. Semantic top-ten membership overlapped on **46/50** items and hybrid on **47/50**; ordering and some excerpts changed. One previously direct excerpt became partial context, reducing hybrid direct evidence from twelve to eleven; one semantic question's document score also declined slightly. Lower irrelevant-evidence totals do not erase that regression or establish no-overlap recall. Keep float32 as the experiment's default and INT8 as an explicit candidate; neither has production approval.

Validation: **six experiment checks** passed with each explicitly supplied model, including policy validation, bounded float32 batching equivalence, INT8 reordered/resumed-vector stability, generic English/Arabic retrieval, source-tail coverage and the existing evidence/scope/cache checks. Review self-checks and the production parser/read-only evaluation checks passed. Separate negative runs rejected unsupported/non-Unicode policies and a float32-cache/INT8-model mismatch without rewriting the cache or creating output. Cached runs retained each format's exact ranks, selected spans and native controls; **144 semantic/hybrid source-byte/section/page pairs** were verified against authored text. Available native lexical evidence remained identical in hybrid output. Formatting and diff checks passed.

The fixed experiment snapshot remained hash-identical throughout. A final installed-file copy differed in bytes on two database pages, while its schema and every table's logical rows matched the initial snapshot; no cause is attributed. All evaluator/model work used external copies, and this pass made no installed-library writes. Models, fixed inputs, caches, reports, grades and hash manifests were preserved in an owner-only local data directory outside the checkout, avoiding dependence on `/tmp` surviving the next session.

Next: remain in **Stage 6**. Add the smallest SQLite vector-persistence experiment with content/model identity, unchanged-vector reuse, update/delete/restart checks and bounded/cancellable disk scanning; retain this same-model comparison as a baseline. Prepare fresh generic English/Arabic, concise-answer, exact-name/date, no-overlap and irrelevant-neighbor cases **for the user's review**, as requested, without tuning on their held-out labels. Independent quality, cold/reference-device costs and deferred app smoke checks remain prerequisites for a Stage 7 decision. At the next pass, remind the user to check quotes, explicit spelling retry, longer excerpts, all three Explore passages, paired HTML/PDF source jumps and a short compound query in both modes.

## Stage 6 — SQLite vector lifecycle and streamed scanning

The isolated experiment now accepts `SEARCH_V2_VECTOR_SQLITE` as an alternative to its JSON development cache. It reuses **rusqlite 0.32.1 with bundled SQLite**, already used by the app, within the separate experiment crate. There is no app schema migration, search-mode change, new database service or ANN dependency. Default float32 and explicit INT8 policies retain their existing identities and source-evidence rules.

```sh
FASTEMBED_MODEL_DIR=/external/cached-model-snapshot \
SEARCH_V2_VECTOR_SQLITE=/external/experiment-vectors.sqlite3 \
SEARCH_V2_CANCEL_FILE=/external/cancel-experiment \
SEARCH_V2_CANDIDATES_JSON=/external/new-candidates.json \
cargo run --release --offline --manifest-path experiments/search-v2-embeddings/Cargo.toml -- /external/corpus.json /external/lexical.json
```

Select either SQLite or `SEARCH_V2_VECTOR_CACHE`, not both. Models, stores, cancellation markers and candidate reports remain outside the checkout, and output paths must not collide. SQLite files are created with owner-only permissions on Unix. Existing unrelated databases, unsupported schema versions and incompatible model identities are rejected without replacing them. The schema records a model/preprocessing fingerprint and dimension, stable passage IDs, document IDs, section/page locators, source byte bounds, body-token counts, embedded-content hashes and little-endian float32 vector blobs. Each decoded vector is checked for 384 dimensions, finite values and unit norm.

A passage ID binds the stable document identity to section index and byte bounds, rather than its position in the exported document list. Equal embedded text under the same model reuses the stored vector. Document reordering and page-only changes refresh locators without inference; text/title/heading changes re-embed affected passages. Changed section boundaries can invalidate further passages. Model/tokenizer/chunking/prefix policy changes require a separate index. This index represents **exactly the supplied corpus snapshot**: passages absent from that export are removed. A filtered subset must not be passed as an authoritative whole-library deletion list in a future app integration.

Synchronization uses one transaction. It embeds only missing/changed passages in bounded batches, commits complete updates/deletions together, and rolls back failures or cancellation. Unchanged rows are not rewritten. This is an atomic snapshot experiment with one writer, not a background-job scheduler or resumable per-document importer; an interrupted new transaction must redo its uncommitted embeddings while retaining the preceding committed snapshot. SQLite reuses freed pages after deletion; this pass does not promise immediate file shrinkage.

Query retrieval streams one validated vector at a time through the same document-ranking/evidence reducer as the in-memory path, with a **2 MiB SQLite page-cache setting**. Maxima still require O(documents) state and exact scans are O(passages); no approximate index was added. The cancellation marker is optional: creating it aborts between SQLite indexing batches, between vector rows, between questions and before report publication. Errors checking the marker are propagated. An active inference call or SQLite operation must finish before the next check, so this is cooperative cancellation rather than a hard latency bound. Failed scans do not return partial candidates.

Source JSON parsing, tokenization and the complete authored corpus/chunk metadata still load in memory on each run. The experiment now avoids materializing every vector for SQLite retrieval, but it is **not yet a standalone search process that loads only winning source passages**. Model startup also remains substantial. Measure source loading, cold disk and reference-device behavior before considering production; warm vector streaming alone does not close those gates.

Fresh review material was prepared separately: **14 new questions and 12 invented sources**, including English/Arabic paraphrases, concise answers, name/date distractors, required quotes, cross-language retrieval, conceptual neighbors and a no-answer control. These cases remain **unrun and ungraded for the user's review**. They contain no private-library examples and are not a substitute for independent real-library relevance judgments. No policy was tuned on them.

The fixed preceding **25-document / 4,365-passage** snapshot was indexed into a separate SQLite file for each model. Every stored vector matched its preceding JSON vector byte-for-byte, and all stored passage IDs, content hashes, document/section/page locators and UTF-8 byte spans were checked against authored sections. Initial and cached semantic/hybrid membership, order, scores and excerpts retained their exact preceding values; lexical evidence and quote controls were unchanged. **100 distinct selected semantic source-span/question pairs** were also verified. No relevance regrading was needed for unchanged evidence, and this persistence pass makes no quality-gain claim; the preceding INT8 excerpt regression remains open.

| Warm development measurement | Float32 JSON | Float32 SQLite | INT8 JSON | INT8 SQLite |
| --- | --- | --- | --- | --- |
| Vector store on disk | 19.42 MiB | 8.96 MiB | 19.42 MiB | 8.96 MiB |
| Median cache load or snapshot synchronization | 37.44 ms | 79.34 ms | 36.75 ms | 77.35 ms |
| Median query embedding + scan + fusion/evidence | 7.28 ms | 10.36 ms | 3.99 ms | 7.61 ms |
| Median vector scan, including SQLite read/decode/validation | 1.18 ms | 4.70 ms | 1.08 ms | 4.23 ms |
| Median complete fresh cached process | 2.15 s | 2.22 s | 1.81 s | 1.85 s |
| Median peak RSS | 1,485,260 KiB | 1,485,168 KiB | 799,676 KiB | 799,780 KiB |

The comparison used the same development CPU, release configuration and two inference threads as the preceding pass. Four alternating fresh subprocesses per storage format/model were measured; the first was discarded. Query/scan medians cover fifteen unquoted samples per cell. OS caches were warm; full lexical retrieval, IPC/rendering, cold disk and reference devices were excluded. The timed SQLite copies were on RAM-backed `/tmp` (tmpfs on this host), so those timings do not establish disk-backed read latency; see the later disk/cache validation. Peak RSS remains effectively unchanged because model startup dominates it. SQLite removes whole-vector materialization and roughly halves this probe's serialized store size, at an observed **3–4 ms warm query cost** and approximately **40 ms additional startup synchronization**. These are small-snapshot observations, not scalable-library p95 guarantees. Initial SQLite synchronization/embedding took **334.75 s float32 / 108.93 s INT8**, one development run each with some overlapping checks/builds, not a controlled indexing-throughput comparison.

Every unchanged restart reported **0 embedded / 4,365 reused / 0 deleted**, and its index file remained byte-identical. The runnable lifecycle regression checks restart reuse, reordered document identities, page-only changes, selective source edits, deletion (including an empty corpus), scope, shared-reducer ranking, invalid vectors, model mismatch, unrelated files and rollback after inference failure/cancellation. Both explicitly supplied real models also passed streamed/in-memory ranking equality and restart reuse on the generic English/Arabic fixture. **Five ordinary experiment checks plus two explicit cached-model checks** passed; strict Clippy, formatting, review self-checks and diff checks passed.

Separate real-process probes cancelled an active INT8 update and killed another writer after its transaction journal appeared. In each case SQLite retained the preceding **20-passage committed snapshot**, no candidate report was created, and restart reported **180 embedded / 20 reused / 0 deleted** for the completed 200-passage snapshot. One observed marker cancellation exited in approximately **76 ms**; this is one probe, not a cancellation SLA. A real float32/INT8 model mismatch rejected the index without changing its rows or publishing a report. Private fixed inputs, indexes, measurements, recovery checks and fresh review materials were preserved outside the checkout with hash manifests.

Next: review the proposed fresh cases, then prepare lexical/semantic/hybrid candidate evidence for the user's independent grades. Use that evidence to address concise-answer and irrelevant-neighbor weaknesses before changing retrieval policy. Complete source loading that fetches only winning evidence, realistic cold/disk and reference-device measurements, and app runtime/lifecycle compatibility before any Stage 7 integration decision. **Deferred app smoke checks remain open**; remind the user at the next pass about quotes, explicit retry, longer excerpts, all three Explore passages, paired HTML/PDF source jumps and compound queries in both modes.

## Stage 6 — fresh pooled relevance packet

The user authorized running the preceding fresh cases to prepare candidate evidence. Their **14 questions / 12 invented documents / 14 authored sections** were kept unchanged, then seeded into a new external SQLite library through the real document store. This controlled synthetic set includes English/Arabic, concise answers, fictional name/date distractors, conceptual neighbors, cross-language retrieval, a required quotation and a no-answer control. It is neither a representative book library nor independent real-library quality evidence.

The evaluator now has a small test-only preparation command. The existing fixture evaluation shares that same store-seeding helper; production app code and import/search behavior are unchanged. External source documents may specify HTML/PDF format and source page locators. The destination must be new, and failed preparation removes only the newly reserved output. Existing libraries are not overwritten. No private sources, question text, model files or word-pair overrides were added to tracked files.

```sh
npm run test:search-eval -- --prepare-library /external/cases.json /external/new-library
npm run test:search-eval -- --library /external/new-library/search.sqlite3 /external/questions-all.json /external/native-all.json --embedding-input /external/new-input
npm run test:search-eval -- --library /external/new-library/search.sqlite3 /external/questions-broader.json /external/native-broader.json
```

Both lexical modes used the real query parser and read-only search pipeline on the same snapshot. Float32 and explicit INT8 then used the same source-linked export and constraints, each in its own SQLite vector index. The required quotation remained a native lexical control, excluded from both embedding channels. Hybrid retained available native evidence exactly. Both embedding formats produced 14 source-complete passages. This run prepares evidence; it does not change the evidence heuristic or tune any ranking policy.

`--review-comparison FLOAT32_JSON INT8_JSON NEW_EXTERNAL_DIRECTORY` now pools the two formats into one packet. It requires shared corpus/snapshot/lexical hashes, question pools, native controls, source counts and evidence policies, plus distinct explicitly identified float32/INT8 models. Mismatched constraints, duplicate questions and semantic candidates for mandatory quotations are rejected. Optional `broaderResponse` evidence can be pooled alongside the All words direct channel; this pass attached it only after checking the real Broader report's snapshot, question text and all non-mode constraints. The packet retains input-report/model provenance and the Broader report hash in its separate key. A comparison-only packet without Broader remains supported, without implying it benchmarked both lexical modes.

```sh
npm run test:search-eval -- --review-comparison /external/float32-pooled-input.json /external/int8-pooled-input.json /external/new-review
```

The resulting packet contains **133 distinct question/document candidates and 140 distinct evidence sets** across All words, Broader, explicit retry where available, and both semantic/hybrid formats. Each channel contributes at most ten results. Documents are judged once per question, while differing excerpts retain separate evidence grades. Identical displayed text with identical locators is deduplicated even when lexical highlights differ. Authored text renders inertly, and candidate ordering is deterministic with channel names/ranks hidden. This reduces presentation bias; it is not a double-blind study.

**Every document/evidence grade remains null. No relevance scores or model winner were inferred.** The local packet includes full synthetic source documents and a short grading guide. The user can assign 0 irrelevant, 1 useful or 2 directly relevant, leave uncertainty unjudged, and supply reasons. Partial replies can begin with concise-answer, exact-name/date and no-answer cases; incomplete judgments must not be scored as irrelevant or advertised as a complete held-out result. Pooling top-ten candidates does not establish exhaustive recall, and synthetic grades alone cannot close the real-library Stage 6 quality gate.

Validation: **31 ordinary native search checks** passed, plus the explicitly invoked new preparation and read-only library evaluations in both modes, and production parser checks. Existing synthetic direct Recall@10 remained **41/45 = 0.911**. The new generic regression checks a prepared PDF's source highlight, section and nonzero page locator through native search, including exported `page_index` spelling. Review self-checks cover cross-format pooling, separate source-context grades, highlight-only deduplication, initially null grades, source-hash/control/scope mismatch and quotation protection. ESLint, Rust formatting and diff checks passed. Separate preparation guards preserved an existing library's bytes and cleaned up a failed new duplicate-ID library. **260 distinct semantic source-span/question pairs** matched authored UTF-8 sections, both quotation controls stayed identical, and available native hybrid evidence was unchanged. The synthetic source library remained byte-identical during evaluation. Models, source artifacts, results, indexes, packet and hash manifests remain in owner-only storage outside the checkout; the installed library was not used or modified.

Next: obtain the user's independent document/evidence grades and inspect any concise-answer, name/date or irrelevant-neighbor failures before making another relevance-policy change. The next technical pass can address **winning-evidence-only source loading**, followed by cold/disk and reference-device measurements, while those grades remain pending. Independent real-library quality, runtime compatibility and deferred app smoke checks still precede a Stage 7 decision. Remind the user of the pending app smoke checks at the next pass.

## Stage 6 — load only winning source evidence

The isolated experiment now supports a **read-only query path** over a synchronized SQLite source/vector snapshot. It opens no installed-library files and makes no production app, dependency, model, ranking or evidence-policy change. The preceding pooled review remains ungraded; this pass does not infer a quality win or resolve the earlier INT8 excerpt weakness.

```sh
# First synchronize a separate external index copy through the existing full-corpus command.
FASTEMBED_MODEL_DIR=/external/cached-model-snapshot \
SEARCH_V2_VECTOR_SQLITE=/external/source-vectors.sqlite3 \
cargo run --release --offline --manifest-path experiments/search-v2-embeddings/Cargo.toml -- /external/corpus.json /external/lexical.json

# Then query without opening the source corpus JSON, tokenizing it or synchronizing vectors.
FASTEMBED_MODEL_DIR=/external/cached-model-snapshot \
SEARCH_V2_CANDIDATES_JSON=/external/new-candidates.json \
cargo run --release --offline --manifest-path experiments/search-v2-embeddings/Cargo.toml -- --query-index /external/source-vectors.sqlite3 /external/queries.json /external/lexical.json
```

The query-only input contains `queries`, `corpusSha256` and `snapshotSha256` copied from the frozen export; it must contain no source documents. The corpus hash is the SHA-256 of the complete exported corpus file, including its evaluation questions, rather than a new source-only production identity. The lexical report must bind that same export and match query text, phrases, scope and native result IDs. This remains a frozen experiment workflow, not app integration or a claim that arbitrary new native exports share the old corpus hash. Select the index through the argument; do not also set a vector-store environment variable. Explicit INT8 still requires its matching model directory and `SEARCH_V2_MODEL_QUANTIZATION=dynamic-int8`. Cancellation and new-output rules remain in force.

SQLite schema version 2 adds bounded authored passage text and a source snapshot containing document/section/page metadata, passage byte spans and body-token counts, plus corpus/model-cache provenance. Synchronizing a supported version 1 index adds these records in the existing vector transaction and reuses unchanged vectors; use a separate external copy to preserve earlier evidence. Query-only access rejects unprepared version 1 stores. Source edits, metadata refresh, deletion and version publication commit or roll back together. Unchanged restarts do not rewrite the database.

The query path retains lightweight metadata for **all documents/passages**, streams validated vectors through the existing ranking reducer, and fetches text only for the top-ten semantic results plus top-ten hybrid results lacking native evidence: at most **20 distinct document evidence passages per unquoted question**. Overlapping passage text is stored separately, trading additional disk space for bounded source reads without loading an entire long section. Each fetched passage must match its embedded-content hash, source byte length and document/section/page locators. Scan metadata must match the bound snapshot too. The existing source renderer keeps escaping and semantic provenance identical. Native hybrid evidence and mandatory quotation controls remain unchanged.

A read-only SQLite transaction pins metadata, vectors and excerpts to one committed snapshot. The generic regression also checks a concurrent WAL writer: the existing reader sees its original source, while a subsequent reader detects altered source bytes. This is not a production background-indexing implementation; readers can delay writer commits under rollback journaling. Full-corpus synchronization still loads and tokenizes authored text. Query metadata remains O(passages), exact scanning remains O(passages), and model startup remains substantial. Stream metadata or add an approximate index only if device measurements justify those changes.

Both formats reused every preceding vector on the fixed **25-document / 4,365-passage** pool and fresh **12-document / 14-passage** pool. **8,758 stored passage/vector records** retained their exact preceding values; their added text matched authored UTF-8 spans and embedded-content hashes. All **42 question/model comparisons** retained exact rankings, scores, displayed excerpts, native hybrid evidence and quote controls against the preserved reports. **360 distinct selected source-span/question pairs** were verified. No relevance regrading was needed for unchanged evidence; all fresh independent grades remain null.

| Warm development median | FP32 full-corpus restart | FP32 query-only | INT8 full-corpus restart | INT8 query-only |
| --- | --- | --- | --- | --- |
| Complete fresh process | 2.11 s | 1.09 s | 1.92 s | 0.83 s |
| Source-input/metadata loading | 3.21 ms | 7.36 ms | 3.36 ms | 10.44 ms |
| Passage reconstruction/tokenization | 946.94 ms | skipped | 962.41 ms | skipped |
| Index synchronization/identity check | 103.79 ms | 0.07 ms | 104.41 ms | 0.07 ms |
| Query embedding + scan + fusion/evidence | 11.54 ms | 11.71 ms | 9.13 ms | 9.74 ms |
| Peak RSS | 1,482,980 KiB | 1,481,824 KiB | 797,168 KiB | 796,432 KiB |

Four alternating fresh release subprocesses per path/model used the same development CPU and two inference threads; the first was discarded. Query medians cover fifteen unquoted samples per cell. Peak RSS was read from each individual child with `wait4`, rather than a cumulative child high-water mark. OS caches were warm; native lexical retrieval, IPC/rendering, cold disk and reference hardware were excluded. The timed indexes were on RAM-backed `/tmp`; the later disk/cache pass provides actual disk-backed measurements. This compares the two current paths, not timings from a different earlier build. Every measured restart/query left its index byte-identical.

On this fixed pool the query-only path reads a **13,570-byte question file** instead of the **2,920,187-byte source export**, retains **zero authored/embedded body-text bytes before queries**, and fetches **37,375 bytes FP32 / 42,395 bytes INT8** of selected evidence across five unquoted questions. It avoids retaining 2,623,195 authored bytes plus 3,192,526 embedded-text bytes. It still loads metadata for all 4,365 passages, so these counters are not total process-memory bounds. The source/vector index grows from **8.96 MiB to 13.88 MiB** per format. Fresh-process time roughly halves by skipping tokenization and synchronization; model startup still dominates peak RSS, and steady query latency is effectively unchanged on this small warm probe. These observations do not close scalable-library, cold-disk or device gates.

Validation: **six ordinary experiment checks and two explicit cached-model checks** passed, alongside strict Clippy, formatting, review self-checks and diff checks. The new generic regression covers UTF-8/Arabic and nonzero PDF page locators, shared-reducer equality, absence of retained source bodies, unselected broken source text, selected-source/hash and scan-metadata rejection, read-only writes, pinned concurrent snapshots, unchanged restarts and cancellation before source-snapshot publication. It also rejects missing indexing documents/text and source-bearing query-only inputs. **Twelve real-process rejection probes** preserved index bytes and produced no partial report for stale corpus/snapshot/lexical bindings, missing bindings, invalid scope/phrase controls, source-bearing inputs, cancellation, conflicting stores, unprepared schema, model mismatch and altered winning text. Prior baselines/models/review grades were preserved outside the checkout; the installed library was not accessed.

Next: measure realistic cold/disk behavior and a chosen reference device, then verify app native-runtime compatibility before any Stage 7 integration decision. Independent fresh and real-library relevance grades remain pending. **Deferred app smoke checks remain open**: quotes, explicit retry, longer excerpts, all three Explore passages, HTML/PDF jumps and compound queries in both modes. This isolated pass does not require rebuilding the app to repeat its checks; remind the user of the pending app checks at the next pass.

## Stage 6 — disk/cache and controlled scale validation

This pass measured the existing release experiment without changing code, dependencies, models, ranking or app behavior. The earlier timed index copies were under `/tmp`, which is **tmpfs on this host**. Their warm-process/source-loading findings remain valid, but they did not establish physical disk-backed SQLite read costs. The historical notes now make that distinction explicit.

New owner-only model/index copies ran on **NVMe-backed btrfs with zstd compression**, using the development **Intel Core Ultra 5 325, eight available logical CPUs, approximately 23 GiB RAM**, with the existing two inference threads. The user chose this machine for current testing and deferred a weaker laptop/phone. The additional two-CPU process-affinity profile is a stress probe on the same CPU, not reference-device evidence. No host-wide cache drop, power-setting change or installed-library access was used.

Three conditions were checked before every fresh child:

- **Warm:** all selected model/tokenizer, question, lexical-control and index file pages resident.
- **Index cold:** only the copied SQLite index pages evicted; other selected input pages resident.
- **Files cold:** all selected model/tokenizer, question, lexical-control and index pages evicted.

The standard-library harness used per-file `posix_fadvise(DONTNEED)` and Linux `mincore` through `ctypes`, requiring zero resident pages for each evicted file immediately before launch. Warm inputs required complete residency. Individual `wait4` resource reports showed zero input-block counts for warm runs and nonzero counts for the cold conditions. These are **verified targeted file-page-cache conditions**, not reboot/device-controller-cold starts: executable/shared-library and filesystem metadata caches, compressed storage and NVMe controller behavior were not reset. The first unquoted query is reported separately because later queries warm the index.

The fixed 4,365-passage pool retained all preceding results and evidence. Separate **50,000 / 100,000-passage** indexes contain **500 / 1,000 generated document identities**, each with 100 sections. They repeat already verified invented source passages and their **real, unchanged model vectors**; changing identities and section positions does not change the embedded title/heading/text. Both English and Arabic questions ran against empty lexical controls to isolate retrieval costs. These scale controls do not measure native lexical retrieval, quality, exhaustive recall or initial indexing throughput. Bodies are short: **11–63 tokens, about 214 UTF-8 bytes on average**, rather than representative 200–350-token book passages. Repetition and filesystem compression also limit generalization of disk sizes/read costs.

| Disk-backed probe | FP32 warm | FP32 files cold | INT8 warm | INT8 files cold |
| --- | --- | --- | --- | --- |
| 4,365: complete fresh process | 1.12 s | 1.21 s | 0.87 s | 0.89 s |
| 4,365: first query embedding + scan + evidence | 12.73 ms | 14.75 ms | 9.98 ms | 13.86 ms |
| 50,000: complete fresh process | 1.19 s | 1.30 s | 0.96 s | 1.00 s |
| 50,000: first query embedding + scan + evidence | 66.89 ms | 81.41 ms | 64.32 ms | 74.53 ms |
| 100,000: complete fresh process | 1.37 s | 1.50 s | 1.14 s | 1.21 s |
| 100,000: first query embedding + scan + evidence | 127.04 ms | 144.02 ms | 124.08 ms | 140.08 ms |
| 100,000: peak RSS | 1,467.1 MiB | 1,467.8 MiB | 799.9 MiB | 800.1 MiB |

Each path/model/cache/profile cell used four fresh subprocesses, discarding the first. Normal cache-condition order alternated; there were **88 runs / 66 retained** in total. Fixed cells retain fifteen unquoted samples; scale cells retain six, including three first-query observations. Table values are development medians, not p95/SLA estimates. All conditions retained exact candidates, scores, source excerpts, hybrid/native controls and quotation behavior against their fixed or scale warm reference. Query time includes embedding, scan and fusion/source evidence; complete-process time also includes model and metadata startup. Native lexical retrieval, IPC/rendering, background importing, memory-pressure/thermal/battery behavior and full app responsiveness are excluded.

Index-cold first scans on the fixed pool took **8.67 ms FP32 / 9.66 ms INT8**, versus approximately **5.9 ms warm**. Warm first-scan cost grew approximately linearly: **59–60 ms at 50,000**, **119 ms at 100,000**. Files-cold first scans at 100,000 were about **134 ms**, with observed isolated query maxima of **145.90 ms FP32 / 142.06 ms INT8** across the retained samples. Full metadata startup also grew: approximately **7 ms → 81–89 ms → 164–170 ms** for the fixed, 50,000 and 100,000 warm pools. Metadata remains O(passages); these costs must not be hidden behind a query-only timing. The 50,000 and 100,000 source/vector files are **126.36 / 252.80 MiB logical size per format**, not physical allocated-disk estimates or predictions for longer book passages. INT8 model inference still stores float32 vectors.

Restricting only the child to two logical CPUs left 100,000-passage files-cold first-query medians at **142.19 ms FP32 / 140.03 ms INT8** and fresh-process medians at **1.49 / 1.19 s**. This supplies no weak-device or mobile feasibility claim. Model startup continues to dominate peak RSS: the larger pool adds only about 19–20 MiB relative to the small pool, while the model process itself remains roughly **1.43 GiB FP32 / 0.78 GiB INT8**. The earlier INT8 evidence weakness remains unresolved, so lower memory does not select a model winner.

Validation checked all 88 page-residency states and result controls. **300,000 repeated source/vector records** across both scale sizes/formats exactly matched their verified donors; copied fixed/model input hashes remained unchanged. Sources, reports, cache-state/resource measurements, scripts and manifests were preserved outside the checkout. Fresh independent grades remain null; no relevance policy was tuned and no quality gain was inferred. This pass adds no tracked benchmark framework or new app tests because it uses the existing release binary and standard-library measurement helpers.

Decision: keep the current exact scan for the experiment. This development-host result does **not currently justify ANN or a metadata-streaming rewrite**, and does not close the weak-device, real-library quality or full-app latency gates. Next: the **desktop native-runtime compatibility probe**, then obtain the deferred reference-device and independent relevance evidence before any Stage 7 decision. Deferred app smoke checks remain open: quotes, explicit retry, longer excerpts, all three Explore passages, HTML/PDF jumps and compound queries in both modes. Remind the user again at the next pass; this measurement-only pass requires no new app build.

## Stage 6 — desktop native-runtime compatibility

The experimental directory remains the isolated decision prototype described in the original implementation plan. Its passage indexing, semantic retrieval and lexical/semantic fusion are implemented; its performance feasibility on this development machine does not establish a held-out quality win or authorize Stage 7 integration. Existing app lexical improvements are separate. The next app step remains optional hybrid retrieval only after quality, runtime and device gates pass.

The architecture comparison also needs precision: DocFinder's current default search encoder is **BGE-M3**; **Qwen3.5 powers its optional AI chat**. Papercut's prototype uses multilingual E5-small embeddings, preserved lexical controls and source excerpts. This pursues similar meaning-based retrieval, without adding generated answers or claiming equivalent retrieval quality. [DocFinder embedding source](https://github.com/filippostanghellini/DocFinder/blob/main/src/docfinder/embedding/encoder.py), [README](https://github.com/filippostanghellini/DocFinder). The original plan's historical reranker/chunking observations were not revalidated in this compatibility pass.

The probe traced the current desktop `native-tts-shared` feature: sherpa-onnx 1.13.4 plus Libtashkeel 1.5.0, with app `ort`/`ort-sys` pinned to `2.0.0-rc.1`. FastEmbed 7.1.0 requires `ort=2.0.0-rc.13`. No app or experiment dependency, build script, model or ranking change was made.

| Isolated check | Result |
| --- | --- |
| Add FastEmbed to current pins | Cargo rejects the incompatible `ort` versions; reproduced with a minimal crate and a copied full app manifest selecting `native-tts-shared`. |
| Inspect existing desktop shared runtime | ONNX Runtime **1.27.0**, supporting the old wrapper's API 17 and FastEmbed's API 24. This host does not need a native-library bump merely to expose those APIs. |
| Current Libtashkeel / `rc.1` baseline | Compiles and restores Arabic diacritics using that cached runtime. |
| Raise wrapper pins in an isolated probe | The old `ort-sys/load-dynamic` feature is removed. After correcting feature selection and suppressing old default download features in a temporary Libtashkeel manifest, its **unchanged Rust source fails with six API compiler errors**: relocated session/optimization types, tensor inputs/ndarray compatibility and the changed `inputs!` result. This is not an exhaustive migration-error list. |
| Dynamic FastEmbed + existing sherpa shared runtime | **Four same-process runs passed**: FP32/INT8, each speech-first and embedding-first. Each produced three finite normalized 384-dimensional vectors and finite non-silent 22,050 Hz Arabic VITS audio, with exactly **one** ONNX Runtime library image loaded. Vectors stayed exact before/after synthesis and across initialization order for each format. |

The successful coexistence processes **exclude Libtashkeel**, whose unchanged upgraded combination does not compile. Speech durations varied between runs; finite non-silent samples do not establish waveform equality or listening quality. This is a Linux shared-runtime probe with one VITS backend and sequential inference, not a full app, packaged launch, concurrency, static-runtime, other TTS backend, mobile or other-desktop-platform acceptance test.

Final builds and inference used offline Cargo and explicit cached runtime/model paths. A one-time authorized dependency fetch supplied missing Rust crates for dynamic loading; no model download succeeded or was required. The old baseline build initially attempted a runtime download despite Cargo's offline flag; setting its existing `ORT_LIB_LOCATION` to cached native libraries avoided that build-script path. Exact probe sources/locks, initial failures, final logs, model identities, verification and a 45-file hash manifest were preserved in owner-only storage outside the checkout. App/experiment manifests and lockfiles, plus `dev-desktop.js`, remained hash-identical. This evidence pass adds no tracked test framework and makes no new relevance judgments.

**Decision:** the demonstrated obstacle is the Rust wrapper/API migration, while the existing Linux shared native runtime passed this limited coexistence check. Next: an isolated **Libtashkeel/ORT migration check**, preferring a supported compatible dependency route before considering a small local adaptation. Prove Arabic preprocessing plus embeddings and speech together before changing app dependencies. Further ANN work is still unjustified. Independent fresh/real-library relevance, deferred reference hardware and app smoke checks remain open before Stage 7. Remind the user to check quotes, explicit retry, longer excerpts, three Explore passages, HTML/PDF jumps and compound queries in both modes; this probe requires no new app build.

## Stage 6 — isolated Libtashkeel / ORT migration

The wrapper conflict now has a **tested local migration candidate**. The published `libtashkeel_base` remains at 1.5.0; upstream's renamed core currently pins ORT `rc.9`, so neither inspected route supplies FastEmbed's required `rc.13` directly. [Published crate](https://docs.rs/libtashkeel_base/latest/libtashkeel_base/), [upstream core manifest](https://github.com/mush42/libtashkeel/blob/main/crates/core/Cargo.toml). This pass retains a small patch and reproduction instructions under `experiments/search-v2-embeddings/compat`; it does not vendor the complete crate/model or apply the patch to app dependencies.

The patch changes only the pinned crate's normalized manifest and ONNX engine. It selects ORT `rc.13` with explicit features and no default runtime downloads, updates session/optimization imports, owned tensor inputs, output extraction and generic recoverable errors, and removes the crate's now-unused ndarray dependency. The public inference trait still takes `&self`, while the new session API needs `&mut self`; a **mutex per engine** provides safe serialized access and converts poisoning into an inference error. No new inference pool, service or scheduler was added. The model, character maps, Arabic rules, threading/optimization settings and public interface remain unchanged. Standalone preprocessing selects API 17; combining with FastEmbed selects its higher API 24 requirement through Cargo feature unification.

Validation:

- **Six crate checks passed**: five existing checks plus one new regression covering malformed tensor shape, four callers sharing an engine with stable Arabic output, and a recoverable poisoned-lock failure. Invalid shape returns an error without poisoning the engine; valid calls subsequently succeed.
- **24 old/new preprocessing comparisons were byte-identical**, using six invented everyday Arabic inputs across both existing preservation settings and default/explicit taskeen thresholds. These include punctuation, numbers, mixed-language text and existing diacritics. This is a compatibility sample, not an Arabic accuracy benchmark.
- **Four same-process checks passed**, FP32/INT8 each with speech-first or embedding-first creation and preprocessing created between the other two engines. All three models remained alive together. Diacritized Arabic went into VITS synthesis; output was finite, non-silent 22,050 Hz audio. English/Arabic embedding vectors remained exact before/after synthesis and against the preceding native-runtime reports. Each process mapped exactly one existing ONNX Runtime 1.27 library.
- The retained patch applied without fuzz to a fresh crate copy after the documented CRLF-to-LF normalization of its Rust file; that copy passed the same six checks. Original crate data/model hashes and app/experiment manifests, locks and desktop development script remained unchanged. Final builds/inference were offline. Diff checks passed.

The new regression is included in the patch, so a later adopter can reproduce the boundary checks without adding another tracked benchmark framework. Exact sources, locks, baseline comparisons, coexistence reports and verification are preserved outside the checkout. Audio sample lengths varied; no waveform equality or listening-quality claim is made. Full app initialization still needs the new `init_from`/`commit` handling and feature selection. Packaged launches, static/mobile/other-platform builds, concurrent speech/embedding jobs and reference hardware remain untested here.

**Next:** if continuing the technical integration checks, apply the proven binding migration to the app's existing preprocessing path and validate a real desktop build, including Arabic TTS. Keep semantic retrieval experimental until independent fresh/real-library relevance and device evidence justify Stage 7. The patch demonstrates feasibility of the migration; it does not make the current app compatible with FastEmbed yet. No additional ANN or benchmark infrastructure is justified by this pass. Deferred search smoke checks remain open: quotes, explicit retry, longer excerpts, three Explore passages, HTML/PDF jumps and compound queries in both modes. This isolated pass requires no app rebuild or new manual smoke test.

## Stage 6 — app ORT adoption and desktop validation

This historical pass adopted the tested Libtashkeel migration through the app's Arabic preprocessing path. It selected **ORT `2.0.0-rc.13`** and a local Libtashkeel 1.5.0 adaptation through Cargo's patch mechanism. Its manifest and ONNX engine contained the compatibility changes; remaining original source, character maps and the **4,788,213-byte model** were verified byte-identical. That model was already embedded by the previous dependency. The later official-dependency restoration below supersedes this adoption; adaptation sources and probe evidence remain archived privately. See the current [compatibility status](search-embedding-provider.md).

The app initializer handles the new `init_from`/`commit` APIs. Dynamic-library load failures stay ordinary string errors: constructing an `ort::Error` before loading its native API could itself panic. Existing static/dynamic feature names, build/development scripts and Tauri configuration are unchanged. The Linux native runtime remains **1.27.0**. No semantic retrieval is wired into the app, no model selection or ranking policy changes, and no unrelated locked package upgrades are retained.

Validation:

- **49 ordinary native TTS checks validated:** 48 passed in the sandbox; the localhost mirror-download regression passed separately with local socket access after the sandbox denied its mock server. Two asset-dependent checks remain ignored in the ordinary run. The two preprocessing checks also passed after the final initializer change, verifying exact invented Arabic output, repeat stability and unchanged non-Arabic text.
- **Six vendored crate checks passed**, including the shared-session regression for malformed shape recovery, four callers and recoverable lock poisoning.
- **One explicit cached-Arabic app smoke check passed.** The actual app preprocessor restored the expected diacritics and fed its existing Arabic VITS backend; resulting audio had a positive sample rate, meaningful duration, finite samples and non-silent output. The new ignored check requires an explicit cached-model directory, performs no download and writes no audio. This does not establish listening quality.
- A copied full app manifest with FastEmbed 7.1.0 resolves offline with **one ORT/ort-sys version**, API requirements unified through API 24, and no implicit runtime-download feature. FastEmbed is not added to the real app manifest; this is dependency-resolution evidence, not app semantic integration.
- The unchanged desktop build command completed frontend checks and produced **`Papercut_1.9.2_amd64.deb`**. Extracted executable linkage has no missing libraries, and the package contains one ONNX Runtime file with the same hash as the existing cached native runtime. No package installation or GUI launch was performed.
- The static preprocessing branch and `native-tts-ios` feature graph pass an offline **Linux-host Cargo check** with cached native libraries. This is not an iOS target build, static executable link or device test. Other advertised platforms remain unvalidated for this migration. Formatting and diff checks passed.

Final logs, copied-manifest resolution, linkage verification and package evidence are retained in owner-only storage outside the checkout. Installed document data was not accessed. Prior search changes and experiment results remain separate; independent fresh relevance grades are still pending, the earlier INT8 evidence weakness remains unresolved, and development-host measurements do not close the reference-device gate.

**Next:** use a rebuilt app to listen to an Arabic voice preview with Libtashkeel enabled and disabled, check an English preview, and save/play a short audiobook. These checks matter now because this pass changes app runtime bindings. The deferred search checks also remain open: quotes, explicit retry, longer excerpts, three Explore passages, HTML/PDF jumps and compound queries in both modes. After smoke checks, complete the prepared independent relevance review and remaining device/platform evidence before deciding whether Stage 7 optional hybrid retrieval is justified. Do not add another benchmark framework or ANN implementation merely to extend the experiment.

## Stage 6 — provisional fresh relevance assessment

The user reports having completed testing of the rebuilt app, with no failures reported. This records the user smoke confirmation; individual checklist observations and a platform/backend matrix were not supplied. Historical deferred-smoke notes above describe their status at those earlier passes.

The user chose **provisional assistant grading of the full prepared packet**. All **133 pooled document candidates and 140 displayed evidence sets** now have separate assistant judgments, with reasons, outside the checkout. The original user/independent grades remain null and unchanged. Judgments were frozen and hashed before opening the rank/channel key. The assistant has prior knowledge of the experiment and authored sources; hiding ranks does not make these judgments independent. These cases are development diagnostics, not held-out acceptance evidence, and any subsequent tuning on them must retain that distinction.

| Same 12 unquoted answerable questions | Directly relevant document at rank 1 | Directly relevant evidence at rank 1 | Pool-relative document nDCG@10 |
| --- | --- | --- | --- |
| All words | 3/12 | 3/12 | 0.207 |
| Broader | 5/12 | 5/12 | 0.309 |
| FP32 semantic / hybrid | 12/12 | 10/12 | 0.913 |
| INT8 semantic / hybrid | 12/12 | 10/12 | 0.917 |

The required-quote control is reported separately: lexical evidence remains correct and semantic/hybrid retrieval remains excluded. The no-answer control is excluded from positive-ranking averages: All words and Broader return no candidates, while each semantic/hybrid variant returns **ten candidates graded irrelevant**. The pool is bounded, synthetic and small; these results neither estimate corpus-wide recall nor establish a real-library quality gain. Natural-language questions also expose the intentionally strict lexical contract, so empty All words results alone are not a lexical regression. Hybrid here fuses All words with semantic retrieval; Broader is a separate comparator, not an additional fusion input.

The two first-excerpt failures occur in both languages and both model formats. Full sources contain the requested concise facts, and maximum cosine correctly ranks those documents first. The **32-body-token evidence eligibility rule** then excludes their **11-/16-token answer sections**, selecting **61-/55-token non-answer context** instead. The existing regression proves that this length rule behaves as implemented; it cannot prove that selected text is relevant. Read-only stored metadata and the retained document/selected-passage scores confirm this mechanism. Broader already exposes useful native evidence for both cases, but the prototype cannot reuse it when its All words fusion input is empty.

Three cross-language name/place judgments are marked useful rather than directly relevant because the sources do not establish an exact place-name alias. A sensitivity check giving those ambiguous matches no credit leaves the rank-1 findings unchanged; semantic document nDCG@10 becomes **0.928 FP32 / 0.929 INT8**. The small ordering differences provide no convincing model winner and do not resolve the earlier INT8 weakness. The single no-answer control is insufficient to calibrate a universal cosine cutoff; no threshold, phrase/word override, new model or reranker was added.

Verification matched **84 question/channel mappings** to their frozen native and model reports, checked judgment coverage and channel-specific evidence assignment, and rechecked original review-file hashes. The review-preparation self-check and diff check passed. No inference rerun was needed to grade unchanged evidence. Annotated judgments, aggregate/per-question scores, sensitivity checks, the grading freeze and reproduction helpers are preserved in owner-only storage outside the checkout. App/experiment behavior, dependency pins, models, vectors and ranking policies were unchanged in this assessment pass.

Platform preflight confirms that this host currently has only the Linux Rust target and Android command-line tools, without an installed Android NDK/platform SDK. The cached Android native libraries alone do not establish an Android build. No SDK/target installation or platform acceptance claim was made. Reference hardware remains deferred by the user's earlier choice.

**Decision and next pass:** remain in Stage 6. The next justified corrective experiment is to **retain the highest-scoring passage alongside bounded source context**, rather than discard it solely for being short. Compare the concise-answer controls with the earlier citation/heading failures while preserving document order, native evidence, quotes and source locators. Then address irrelevant tails/no-answer behavior using a broader relevance-reviewed set rather than tune a cutoff on one example. Independent real-library quality and remaining device/platform evidence still precede any Stage 7 integration decision. This assessment pass requires no further app rebuild or smoke repetition.

## Stage 6 — retain score-winning source alongside context

The isolated experiment now returns **at most two distinct authored passages per semantic result**: its preceding context passage remains the lead, and the document's maximum-cosine passage is included when different. The existing native-style `passages` representation carries both sources; no new ranking heuristic, model, dependency or query-word rule was added. The 32-token rule now chooses lead context rather than excluding short evidence altogether. Reports explicitly record `minimumContextBodyTokens`, winner retention and the two-passage bound.

One shared assembler handles full-corpus and read-only query paths. Each selected source retains its own excerpt, score, section/page and UTF-8 span; the ranking score remains separate. When lead and winner coincide, the assembler reads that source once and adds no duplicate passage. Cancellation/source validation applies to each read, including the added winner. Available native lexical evidence is still copied unchanged into hybrid results.

Eight release runs covered both model formats, both saved corpora and both source-loading paths. **All 42 question/model comparisons** retained exact document membership/order, scores, native results and quote controls; the **36 unquoted comparisons** additionally verified the new evidence. Lead excerpts stayed byte-identical, including the context that previously displaced citation/heading noise. Full-corpus and query-only outputs matched. Every index stayed byte-identical and all **8,758 stored vectors** were reused without passage re-embedding. The retained quotes remain lexical-only.

Across the two corpora/models, **50 semantic evidence sets** gain a distinct score winner, including **42 sets** in the fresh packet. **410 distinct question/source-span pairs** were checked against authored bytes and page locators. The existing generic regressions now check the two-source assembly, score/locator pairing, single-source fallback without duplication, and added-winner failure propagation through the read-only store. Six ordinary experiment checks passed; two unchanged cached-model indexing/tokenization checks remain ignored in the ordinary run. Strict Clippy, formatting, review-preparation self-checks and diff checks passed. A real-process corruption probe rejected an altered added winning source, preserved the index and published no report.

The fresh development packet was provisionally reassessed using the preceding document judgments and source-grading policy. **Four evidence-set grades change from 0 to 2** because the concise answer is now included alongside the unchanged non-answer lead. Both models' semantic/hybrid top-result **expanded evidence** is answer-bearing on **12/12** unquoted answerable questions, versus **10/12** previously. This grades the two displayed passages together: **the lead snippet alone remains answer-bearing on 10/12**. Keeping the old lead preserves earlier citation/context behavior but does not solve universally choosing the best single card excerpt. Document nDCG and the no-answer control remain unchanged; each semantic/hybrid variant still returns ten irrelevant candidates for that control. Original independent grades remain unassigned and untouched. These assistant-graded cases are development evidence, not a held-out quality win or a model selection.

Read-only loading remains bounded to **40 selected sources per unquoted question** rather than the preceding maximum of 20; lightweight scan metadata remains O(passages). Across the fixed five unquoted questions, selected-source reads increase **50→55 FP32 / 50→53 INT8**, and fetched bytes **37,375→37,705 / 42,395→42,577**. Across thirteen fresh unquoted questions, reads increase **130→151** per format, with **1,056 additional bytes** per format. These counters are specific to short added sources in these pools, not a general latency or memory guarantee. No new performance conclusion, model initialization measurement or reference-device claim is made.

Sources, reports, revised provisional grades, comparison/corruption checks and exact reproduction helpers are preserved outside the checkout. App runtime bindings, app search behavior, build scripts and dependency/model identities are unchanged in this pass. The user's rebuilt-app smoke confirmation remains recorded; this isolated evidence change requires no new app rebuild or manual smoke repetition.

**Next:** address irrelevant semantic tails/no-answer behavior with a broader reviewed mix of answerable and unanswerable questions, including plausible topic neighbors and both languages. Use the current result scores as diagnostics, not calibrated probabilities; do not pick a universal cutoff from the lone no-answer case. Keep native precision, required phrases, evidence/locator invariants and independent real-library/device/platform gates explicit before any Stage 7 decision. No ANN rewrite or reranker is justified by this evidence-retention fix.

## Stage 6 — expanded answerability and irrelevant-tail assessment

This assessment expands the question controls without changing retrieval behavior. **40 expectations were frozen before retrieval**, equally split between English and Arabic: 12 answerable questions, 12 missing-fact questions with potentially useful background, 12 irrelevant conceptual neighbors, two present-phrase and two absent-phrase controls. This remains a small development diagnostic, not representative corpus coverage or independent acceptance evidence. Exact questions, sources and judgments remain outside the checkout.

All **366 pooled document candidates and 386 channel-specific evidence sets** have source-derived provisional assistant grades. Grade 2 requires the requested fact; grade 1 permits useful background or an uncertain entity alias; grade 0 means irrelevant. A missing fact does not make every topical background result irrelevant. Independent grades remain null. These controls can diagnose failures and guide subsequent experiments, but cannot become held-out evidence after tuning against them.

| Unquoted answerable questions | Answer-bearing first result | Answer within ten results |
| --- | --- | --- |
| All words | 0/12 | 0/12 |
| Broader | 7/12 | 7/12 |
| FP32 semantic / hybrid | 9/12 | 12/12 |
| INT8 semantic / hybrid | 9/12 | 12/12 |

Natural-language questions expose the intentionally strict All words contract; these empty results are not a lexical regression or a general quality estimate. Hybrid uses that All words input, so equals semantic for these unquoted controls; Broader is a separate comparator. Both models share **three wrong first results**, with supporting sources at ranks two, two and five. The bounded evidence includes the answer when its source is retrieved; remaining failures concern ordering. No model winner is established, and the earlier INT8 evidence weakness remains unresolved.

For missing-fact questions, semantic/hybrid retrieval places useful background first on **10/12**, without any source supplying the absent requested fact. Separately, every irrelevant-neighbor question returns ten irrelevant semantic/hybrid candidates: **120 per model/channel**. All words returns none; Broader returns one irrelevant candidate on one question. Similarity scores must not be presented as answerability probabilities.

Counterfactual cutoff/cap analysis rejects two tempting shortcuts. The highest cosine cutoff retaining at least one supporting source for every answerable question is **0.772985 FP32 / 0.775051 INT8**; both still admit **11/12 irrelevant-neighbor questions**. This uses actual supporting-source scores, including those below an incorrect lead, rather than only positive queries' top scores. Keeping only three results retains an answer for **11/12** questions and **13/20** frozen supporting-document/query pairs. Keeping five retains answers for all twelve questions but only **17/20 FP32 / 18/20 INT8** supporting pairs. These are bounded expected-pair coverage counts, not corpus-wide recall. No cutoff, candidate-cap or scoring change was adopted.

Two real cached-model runs reused all **14 unchanged passage vectors per format**; only the copied index's query snapshot metadata changed. Verification checked **1,638 semantic/hybrid source-span occurrences**, 20 native excerpts, all 386 grading bindings, unchanged source/vector rows and all 21 original review archive hashes. Present phrases retain one native result each, absent phrases retain none, and all quoted semantic/hybrid routes remain excluded. Production parser/native evaluation checks, the review-preparation self-check and diff check passed. No new benchmark framework, dependency, model, word override or app test was added. Full annotated assessment and reproduction evidence are preserved outside the checkout.

**Decision and next pass:** remain in Stage 6 and improve ordering before attempting a tail policy. Compare body-focused passage scoring with the existing title/heading-enriched input, using the frozen controls plus earlier citation/context checks. This tests a hypothesis about metadata versus body relevance; it does not establish metadata as the cause or justify a new reranker/model yet. Independent representative real-library relevance and deferred reference-device/platform evidence remain necessary before Stage 7. The user's rebuilt-app smoke confirmation still stands; this assessment changes no app behavior and requires no new rebuild or smoke repetition.

## Stage 6 — body-focused input comparison

**Decision: retain the current title/heading-enriched passage input.** A separate scratch probe embeds only the E5 passage prefix and authored body. It preserves the baseline chunk budget and source spans, query inputs, model files, quantization rules, reducer and two-passage evidence policy, isolating the effect of removing metadata. This does not test a larger body budget, different chunk boundaries or all possible input policies. No experimental default or app behavior change was adopted; exact sources/questions and probe code remain outside the checkout.

| Provisional diagnostic, both model formats | Existing input | Body-only input |
| --- | --- | --- |
| Expanded answerable controls: answer-bearing first result | 9/12 | 9/12 |
| Expanded answerable controls: answer within ten | 12/12 | 12/12 |
| Frozen supporting-document/query pairs within ten | 20/20 | 20/20 |
| Missing-fact controls: useful background first | 10/12 | 10/12 |
| Irrelevant-neighbor candidates per semantic/hybrid channel | 120 | 120 |
| Earlier fresh controls: answer-bearing expanded first result | 12/12 | 12/12 |
| Earlier fresh controls: answer-bearing lead alone | 10/12 | 10/12 |

The same three expanded-control first-result failures persist. One supporting source stays at rank five with FP32 and falls from five to six with INT8. Body-only scoring still admits **11/12 irrelevant-neighbor questions** at the highest cutoff preserving an answer for every positive question. No cutoff or candidate-cap policy was selected. Removing metadata is therefore insufficient to solve these ranking and tail failures; neither format establishes a quality winner.

On the larger preceding controls, both formats retain **2/5 directly relevant first documents** under the existing provisional judgments. One FP32 first-document grade improves from irrelevant to useful partial context, without becoming directly relevant. Newly surfaced tail documents remain unjudged; no whole-pool nDCG or corpus-wide recall is inferred by treating them as zero. The preserved token spans also retain existing boundary-readability limitations. The expanded packet provisionally grades **367 pooled documents and 386 channel-specific evidence sets** using the previously frozen expectations. Independent grades remain null; these comparisons are development evidence.

Twelve real inference processes covered three query packets, both formats and full/read-only source paths: **244 question/model/path comparisons**, including **28 quoted controls**. Verification checked **4,770 source-span occurrences** and **8,758 unchanged chunk-layout/source records** across formats. New input vectors were built separately; earlier fresh queries reuse fourteen body vectors per format. A distinct compatibility identity prevents mixing input profiles; both real-process incompatibility probes reject without publishing a report or rewriting the index. Native evidence, scopes and quote controls remain unchanged. Cached model artifacts, baseline sources and all 21 original independent-review archive files retain their hashes. Six ordinary scratch-crate checks, strict Clippy, formatting, review self-check and diff checks passed; two unchanged cached-model checks remain ignored in the ordinary run. Reproduction sources, results, provisional grading and integrity checks are preserved outside the checkout. No tracked framework, dependency, model, word rule or additional app test was added.

**Next:** remain in Stage 6 and assess **one small multilingual local reranker** for license/export/runtime and resource feasibility. If viable, compare bounded candidate reranking against the retained baseline, measuring quality and incremental latency/memory together. The installed FastEmbed dependency already exposes a reranking API; no second framework or generative model is needed for that probe. Candidate recall is present while ordering still fails, matching the original plan's condition for considering a cross-encoder; this is experimental evaluation, not Stage 8 shipping or Stage 7 acceptance. Preserve phrase/scope/evidence and earlier citation/context/name/date controls. Independent representative real-library quality, reference hardware and platform gates remain open. This pass changes no app behavior and requires no rebuild or repeated smoke checks.

## Stage 6 — bounded multilingual reranking probe

**Decision: retain an opt-in experimental probe; do not enable reranking in the app.** One published multilingual cross-encoder works through the existing FastEmbed reranking API and cached ONNX Runtime. Its quality is mixed and its resource cost is material. No new dependency, retrieval model, index format, app configuration or word override is introduced. This follows the original plan's conditional small-reranker investigation when candidates contain answers but their ordering fails; it does not satisfy Stage 7 acceptance.

The selected [mMARCO mMiniLMv2 model](https://huggingface.co/cross-encoder/mmarco-mMiniLMv2-L12-H384-v1), revision `1427fd652930e4ba29e8149678df786c240d8825`, declares Apache 2.0 and English/Arabic coverage. Its published `model_quint8_avx2.onnx` export is **118,620,016 bytes**; the five selected graph/tokenizer/config files total **135,704,241 bytes (129.4 MiB)**. Cached files and published large-file hashes were verified. This evaluates one export on Linux, without establishing other-platform or mobile compatibility. Language coverage is not a quality guarantee.

The isolated crate now accepts:

```text
search-v2-embeddings --rerank INDEX_SQLITE RERANK_MODEL_DIR CANDIDATES_JSON NEW_OUTPUT_JSON
```

All paths remain outside the checkout. Models must already be cached; this route performs no download. It reuses the retained E5 top ten semantic documents and at most two verified authored passages per document. Paired query/body input is counted with the reranker tokenizer, with **512 tokens maximum and rejection rather than silent truncation**. One pair per inference avoids dynamic-quantization batch-peer effects. Maximum raw pair logit ranks each document, with document ID breaking ties. The logit is not an answerability probability; passage order and displayed evidence remain unchanged. Output adds `rerankedSemanticResponse` separately, preserving the original native, semantic and hybrid channels. Mandatory quoted queries stay lexical-only, scopes and source bindings are checked, and every selected source is validated through the read-only index. This probe neither reindexes passages nor retrieves additional candidates.

| Provisional first-result diagnostic, both E5 formats | Retained baseline | Reranked semantic |
| --- | --- | --- |
| Expanded bilingual answerable controls | 9/12 | 9/12 |
| Expanded controls: answer within ten | 12/12 | 12/12 |
| Earlier fresh answerable controls | 12/12 | 12/12 |
| Larger controls: directly relevant first document | 2/5 | 4/5 |

The expanded controls contain **one corrected head and one new regression** involving an uncertain cross-language entity alias. Another supporting source moves closer to the top without becoming first. Earlier fresh expanded evidence remains answer-bearing on all twelve positives; a useful extra passage does not make every lead snippet sufficient. The larger-set gains concern **full-document relevance**: separate provisional reading of the displayed first evidence finds direct support on **3/5 FP32 and 2/5 INT8**, so the earlier INT8 evidence weakness remains. These are not independently judged wins or matched before/after excerpt metrics. Unjudged tail documents are not assigned zero.

Missing-fact controls still put useful background first on **10/12** without supplying the absent fact. All **120 irrelevant-neighbor candidates per format** remain: permutation alone adds no abstention. No universal cutoff, score calibration or model winner is selected. The frozen expanded packet contains 366 pooled documents and 386 evidence sets with source-derived assistant grades; independent grades remain null. Its semantic review channels represent the reranked output, while hybrid channels retain the baseline. This does **not** measure a hybrid search that consumes reranked candidates.

Three repeated scoring calls per unquoted query verify identical scores and order. After discarding each first call, short-control median reranking time is **117.2 ms FP32 / 116.3 ms INT8**; larger-control medians are **248.2 / 272.4 ms**, with a maximum observed call of **489.2 ms**. These used **10–12 pairs**, below the 20-pair cap. Timings cover scoring and document aggregation, excluding source validation, retrieval, model startup, IPC and rendering. Model initialization is approximately 0.66–0.72 seconds. They are development-host diagnostics, not whole-app p95, reference-device results or a worst-case guarantee. FP32/INT8 here identify the E5 candidate format; both use the same quantized reranker.

Process peak RSS is **669–684 MiB** for reranking alone. Same-process E5/reranker coexistence reaches **1,448 MiB FP32 / 1,034 MiB INT8**; encoder-only comparison peaks are **1,446 / 777 MiB**. Startup peaks prevent interpreting subtraction as incremental steady-state memory. Both engines run successfully together and the encoder's checked vectors remain identical before/after; this does not test full-app/TTS concurrency or weaker hardware.

Seven ordinary experiment checks, strict Clippy and formatting pass; two unchanged cached-model checks remain ignored in the ordinary run. The added generic regression checks document maxima, deterministic ties, evidence binding and invalid/omitted scores. Real-process rejection probes cover altered evidence, phrase bypass, scope violations and oversized pairs, publishing no output and preserving indexes. Verification covers **202 question/run comparisons**, including **22 quote exclusions**, **540 repeated scoring calls**, and **2,048 source-span occurrences**. Frozen inputs/indexes, retrieval model artifacts and all 21 original independent-review archive files retain their hashes. Results, grading, model provenance and reproduction helpers are preserved outside the checkout; private corpus details are absent from tracked changes.

**Next:** prepare a fresh, blinded, representative real-library comparison of retained retrieval versus bounded reranking, with independent relevance review before a go/no-go decision. Include bilingual entity ambiguity, names/dates, context, unsupported questions and irrelevant neighbors. Resolve whether document gains provide useful displayed evidence and justify the additional cost before trying further models or changing app behavior. Actual hybrid-result quality, deferred reference hardware and platform/lifecycle gates remain open. This isolated pass needs no app rebuild or repeated smoke test; the user's prior rebuilt-app confirmation still stands.

## Stage 6 — fresh blinded real-library review preparation

**Decision: the review packets are ready; no new quality win or integration decision is inferred.** Questions and source anchors were frozen before their inference runs, with retrieval/reranking settings unchanged. The available frozen real-library scope contains **25 English documents**, unlike the earlier invented bilingual controls. Four Arabic questions test cross-language retrieval against those English sources; this provides **no Arabic-document validation**. The sources come from one narrow topical library and the assistant authored the questions from source material, so this is a bounded convenience sample, not representative user-query evidence or independently authored information needs. Independent review must confirm intent and proposed answerability before assigning relevance grades.

The main packet has **22 questions**: twelve distinct positive information needs, four Arabic translations, two document-scope repeats, present/absent phrase controls and two proposed unsupported/irrelevant-neighbor probes. Translations and scope repeats are correlated. A **separate six-query short-keyword batch** was subsequently frozen before its own runs to cover concise research searches as well as natural-language questions. It rephrases six existing needs and adds no independent positive needs. Keeping the batches separate prevents reporting an unqualified quality average across different query styles. The original packet and freeze remain unchanged.

The existing `scripts/search-eval.mjs` review helper now accepts **reranked semantic channels explicitly**, including both E5 candidate formats, while retaining baseline semantic and hybrid channels. It records the reranker identity, rejects missing/mismatched comparison identities or policies, and enforces quoted-query exclusions on semantic/hybrid review routes. Candidate order is deterministic but shuffled, ranks and scores are hidden, and identical displayed evidence shares one judgment across channels. The private key preserves each channel's actual rank. The generic self-check verifies changed ordering, evidence deduplication, all-null grades, policy/identity mismatches and phrase exclusions. This adds no new evaluator framework, ranking policy or app behavior.

Four production-parser/native evaluations cover All words and Broader for the two batches. Eight actual offline model processes cover query-only retrieval and bounded reranking for both E5 formats. Each format reuses **4,365 unchanged passage vectors**; copied and archived indexes remain byte-identical, with no indexing or vector rewrite. New requests override old query metadata through the existing read-only route after the exported source documents and snapshot are checked for exact equality. Native evidence, candidate membership, scope restrictions, source locators and baseline hybrid output remain intact. Phrase controls produce one present native result and zero absent results; both semantic/reranked routes remain excluded.

The main blinded packet contains **220 query/document candidates and 341 evidence sets**; the keyword packet contains **77 candidates and 126 evidence sets**. Every grade remains null. The **six-question starting batch contains 44 candidates** and covers context, a named person, cross-language retrieval, document scope, a quote and a proposed missing fact. Full authored sources are linked for document judgments, while displayed evidence receives separate grades. Scope descriptions are visible; channel/rank keys and author expectations stay separate. Reviewers can leave uncertain grades unassigned and flag a faulty question instead of treating unknown as irrelevant. These packets are ready for judgment; their candidate counts are not quality metrics or corpus-wide recall.

The production parser checks and native evaluations, review self-check, targeted ESLint and diff checks pass. Integrity verification covers **56 question/model comparisons**, **four quote exclusions**, **156 repeated rerank API calls** and **1,393 semantic source-span occurrences**. Cached models, source indexes and all 21 original independent-review archive files retain their hashes. Private questions, source titles, expectations, raw results, blank grades, provenance and reproduction helpers remain outside the checkout. The two batches have no assistant-assigned grades and do not inherit earlier provisional grades.

**Next:** obtain independent grades, starting with the six-question batch and extending to enough of both packets to assess first-document relevance, first displayed evidence and ordering regressions. Check question validity before unblinding. Then compare the fixed baseline and reranker, separating query styles and correlated needs; incomplete pools must not turn missing grades into zero. Only after that evidence should we decide whether the reranker's resource cost justifies further work. Actual reranked hybrid evaluation, real Arabic-document coverage, broader usage, reference hardware and platform/lifecycle gates remain open before Stage 7. This preparation pass requires no app rebuild, import or repeated smoke test.

## Stage 6 — provisional assistant grading of the starting batch

At the user's request, the assistant completed the **six-question starting batch**, assigning **44 document grades and 65 displayed-evidence-set grades** after reading the excerpts and checking relevant full-source contexts. The original independent-review packets remain unassigned and unchanged. Grades and reasons are saved separately outside the checkout. Labels were fixed before opening the channel/rank key for this assessment, but prior query/model authorship prevents treating these as independent or fully blinded acceptance evidence.

| Four unquoted answerable requests | Directly relevant first document | Answer-bearing first displayed evidence |
| --- | --- | --- |
| All words / Broader | 2/4 | 2/4 |
| FP32 / INT8 semantic | 4/4 | 4/4 |
| Baseline hybrid, either format | 4/4 | 4/4 |
| Reranked semantic, either format | 4/4 | 4/4 |

The four positive requests cover **three distinct information needs**; one scoped request repeats a cross-language date question. The figures concern expanded displayed evidence, not a separate lead-only grade or corpus-wide recall. **Reranking adds no first-result improvement in this subset.** These results do not resolve failures in other packets or justify accepting/removing the reranker globally. Empty strict lexical results on natural-language and cross-language requests are consistent with its required-word contract.

The present-phrase control returns the source with the required clause; semantic/hybrid/reranked channels remain excluded. The scoped unsupported-fact question has useful event background but **no requested numerical answer in its complete source**. Its document and excerpts receive grade 1, not grade 2. Returning that background does not establish a correct answer or solve abstention.

There are five directly relevant, three useful-background and 36 irrelevant query/document judgments; evidence grades comprise seven direct, four background and 54 irrelevant sets. Broad vocabulary overlap alone is not useful context for a specific named fact. Verification checks all grade/code bindings, unchanged frozen judgments after unblinding, quote exclusions and **108 unchanged original artifact files**, including the still-null full/keyword grades. No new inference, app behavior, ranking rule or model change occurred.

**Next:** provisionally grade the remaining main and short-keyword packets before estimating an overall reranking benefit, keeping correlated needs and query styles separate. The user's delegation removes the need to wait for their manual grading for this development assessment; it does not convert assistant judgments into independent quality evidence. Representative independent quality and reference-device/platform/lifecycle gates remain open before Stage 7. No app rebuild or smoke repetition is required.

## Stage 6 — completed provisional assistant review

**Decision: keep reranking experimental; investigate displayed evidence selection next.** At the user's delegation, all **22 main questions and six keyword queries** now have provisional assistant judgments: **297 document candidates and 467 displayed evidence sets**, including the unchanged six-question starter. Judgments use targeted full-source searches/contexts and displayed bodies separately; they do not assume an excerpt's omitted continuation. Remaining labels were frozen before opening their rank keys. Prior development/query authorship and knowledge of the starter key prevent an independent or fully blinded acceptance claim. Original independent packets remain unassigned and unchanged; private judgments, reasons, source details and reproduction helpers stay outside the checkout.

| Twelve distinct English natural-language needs | Direct first document | Direct first displayed evidence |
| --- | --- | --- |
| All words | 8/12 | 4/12 |
| Broader | 10/12 | 5/12 |
| FP32 semantic | 9/12 | 8/12 |
| INT8 semantic | 9/12 | 7/12 |
| Baseline hybrid, either format | 11/12 | 7/12 |
| Reranked semantic, either format | 9/12 | 8/12 |

Both reranked formats correct one English document head and regress another, leaving document success unchanged. INT8 additionally switches between two directly relevant documents, improving displayed evidence. Two questions depend on unnamed source context; excluding them leaves ten more specific needs with the same conclusion: no document-head improvement, and one INT8 evidence-head improvement. Full-source review also identifies an additional directly supporting document beyond a question author's single source anchor.

Four Arabic translations against English sources remain separate and correlated: FP32 document/evidence heads improve **2/4 → 3/4**; INT8 document heads regress **3/4 → 2/4**, while evidence heads stay **2/4**. Two scoped repeats pass throughout. Across all eighteen unquoted main requests, both semantic pools contain a direct document within ten, but direct displayed evidence occurs within ten on only **15/18 FP32 and 14/18 INT8**. These are bounded pool counts, not library-wide recall. Permuting the same candidates/evidence cannot repair missing supporting passages. There is still no real Arabic-document validation.

| Six short-keyword queries | Direct first document | Direct first displayed evidence |
| --- | --- | --- |
| All words / Broader / baseline hybrid, either format | 6/6 | 6/6 |
| Semantic, either format | 6/6 | 4/6 |
| Reranked semantic, either format | 5/6 | 4/6 |

The keyword queries repeat six existing needs and use their visible topical/entity intent; they are not pooled with fact-question success rates. Reranking introduces the same document-head regression in both formats and supplies no first-evidence gain. **There is no consistent overall reranker win or justified model/default change.** The baseline hybrid's correct-document/weak-evidence gap is a stronger next target than another model comparison. Title, bibliography, date and imported-page boilerplate appearing in evidence also warrant tracing the existing passage-selection path, without source-specific exclusions or word overrides.

The present/absent mandatory-phrase controls pass and remain lexical-only. Both scoped unsupported-detail probes return useful event background, graded 1 rather than an answer. That is legitimate contextual retrieval, but does not validate answerability, global abstention or irrelevant-neighbor rejection. One probe has an artificial technical premise and should not be treated as representative usage.

Verification checks all **297 source/scope bindings**, **336 distinct displayed semantic byte-span occurrences**, grade completeness, deterministic metric reconstruction, unchanged reranker candidate/evidence membership and quote exclusions. All **108 original packet artifacts and eleven starter archive files** retain hashes; starter judgments and post-unblinding frozen labels are unchanged. No inference, index rewrite, app behavior or ranking-policy change occurred. No rebuild, import or repeated smoke test is needed for this grading pass.

**Next:** compare existing passage candidates and hybrid evidence choices on the recorded failures, then try the smallest bounded, source-verified evidence-selection correction. Preserve phrase/filter rules, literal counts/highlights, provenance and navigation. Replay these fixed labels as development regression checks; tuning on them does not yield fresh acceptance evidence. The original plan's Stage 6 quality gate still requires broader independent needs and actual Arabic-source coverage, with reference-device/platform/lifecycle evidence before Stage 7. Evaluate reranked hybrid only if its additional cost remains justified; do not promote this probe from mixed assistant grades.

## Stage 6 — retain conceptual context in experimental hybrid evidence

**Decision: retain this bounded correction in the experiment; app integration remains deferred.** The hybrid evidence selector previously discarded conceptual passages whenever a document also had native evidence. It now preserves native evidence exactly and attaches separately identified, source-verified conceptual context for the same document. Literal counts, marks, navigation targets, document order and semantic passage selection stay unchanged. The review helper labels conceptual context explicitly; it cannot imply literal word matches. No new model, reranker, dependency, synonym rules or production search change is introduced.

Source loading covers the union of the first ten semantic and first ten hybrid documents, using candidates already in the semantic top fifty. It loads at most two conceptual passages per selected document. Each hybrid review set retains up to three native passages plus two conceptual passages; generic checks exercise the full five-passage limit, provenance, invalid locators and unchanged native payloads. Comparison packets record and validate the evidence policy.

Four actual offline, query-only model processes replay the fixed main and keyword requests with both E5 formats. Provisional assistant grading evaluates the **expanded evidence together**, not a changed lead snippet or document ranking. Exact old evidence judgments are reused; fifteen newly displayed context occurrences and partial combinations are inspected separately. Original independent packets and frozen earlier judgments remain unchanged.

| Fixed development requests | Direct first hybrid evidence before | After, either format |
| --- | --- | --- |
| Twelve English natural-language needs | 7/12 | 9/12 |
| Four Arabic translations against English sources | 2/4 | 2/4 |
| Two scoped repeats | 2/2 | 2/2 |
| Six short-keyword queries | 6/6 | 6/6 |

English first-document relevance stays **11/12**; direct expanded evidence within ten improves **8/12 → 10/12**. Two English needs still lack the required passage anywhere in these evidence sets. Across all eighteen unquoted main requests, including correlated translations/repeats, first evidence improves **11/18 → 13/18**. No graded evidence regression occurs among **448 hybrid result occurrences**. These are tuned development checks, not independent acceptance, library-wide recall, lead-only improvement or a reranking win. The unsupported controls still provide background without the requested answers; real Arabic-source validation remains open.

The main replay loads eight additional source passages per format: **7,738 bytes FP32 / 8,793 bytes INT8** across twenty-two requests. Keyword source loading is unchanged. The largest observed joint set contains four passages, below the five-passage cap. Output grows because it now retains supporting context; brief phase timings from one process per batch/format do not establish whole-app or reference-device performance. Model/index identities and source-index hashes remain unchanged; all 4,365 passage vectors are reused without reindexing.

Seven ordinary experiment checks pass, with two unchanged cached-model checks ignored; the release build, strict Clippy, formatting, review self-check, targeted ESLint and diff checks pass. Replay verification covers **56 question/format comparisons**, **four mandatory-quote exclusions**, **587 semantic source-span occurrences**, scope/provenance/bounds, unchanged native controls and ranks, and complete provisional labels. Private requests, source details, reports and reproduction helpers remain outside tracked files.

**Next:** trace primary-passage selection on the remaining missing-excerpt cases, then test the smallest general correction with generic fixtures and these fixed regression labels. Measure lead evidence separately from expanded context. Keep the existing model and ranking settings until evidence supports a change. Independent representative quality, actual Arabic-source coverage and reference-device/platform/lifecycle gates still precede Stage 7. This isolated experiment requires no app rebuild or repeated smoke test; the prior rebuilt-app confirmation stands.

## Stage 6 — actual lead-excerpt review and selection check

**Decision: correct lead measurement; reject blanket conceptual-lead substitution.** Tracing native evidence finds that document-scope results can retain their original result-card excerpt while their expanded passages are selected separately by body-only BM25. Semantic results likewise distinguish the document score winner from their displayed body-context passage. The old review helper displayed expanded passages only, so neither their first passage nor their joint grade established the actual card lead's quality. Prior expanded-evidence figures remain valid for that stated scope.

The shared review helper now displays the **actual top-level result-card excerpt**, with its own source locator and unassigned `leadEvidenceGrade`, separately from the unchanged expanded-evidence grade. Evidence-set deduplication includes the lead body/locator, so identical expanded passages cannot hide different card leads across channels. The existing inert source renderer and locator validation apply to both. Generic checks cover differing card/expanded excerpts, distinct leads sharing expanded passages, lead-locator binding, null judgments and an invalid lead with otherwise valid expanded passages. No production or experiment selection policy changes.

The assistant provisionally grades **52 first-result question/format comparisons and 84 lead excerpt occurrences**, including available conceptual alternatives. All use previously recorded source-verified output, with no inference or extra source loading. A private counterfactual prefers the existing conceptual excerpt for the same first hybrid document when available, otherwise preserving its card lead. It neither reranks documents nor changes expanded evidence. These are first-result checks, not complete lead judgments for every candidate or independent acceptance.

| Fixed development requests | Direct current card lead | Direct lead if conceptual context preferred | Direct existing expanded evidence |
| --- | --- | --- | --- |
| Twelve English natural-language needs | 5/12 | 9/12 | 9/12 |
| Four Arabic translations against English sources | 2/4 | 2/4 | 2/4 |
| Two scoped repeats | 1/2 | 2/2 | 2/2 |
| Six short-keyword queries | 6/6 | 4/6 | 6/6 |

These counts match in both formats, though individual cross-language failures differ. Conceptual preference improves four English leads but also changes one useful-background lead to irrelevant; two keyword leads lose direct support. **Do not adopt that blanket rule or add a query-length/word-list workaround.** Expanded context already retains both sources. A missing source passage, a truncated snippet and an incorrect first document are different failure classes; switching an available lead cannot solve them all. Both unsupported controls remain background with no requested answer. Assistant authorship, known rankings and fixed-batch tuning prevent a fresh or blinded quality claim.

Fresh review packets retain **189 main candidates / 290 expanded sets** and **68 keyword candidates / 112 sets**, with every independent grade null. Separate first-lead judgments and reasons are preserved privately. Verification checks **943 channel-to-actual-lead bindings**, **84 authored-source lead occurrences**, **four quote exclusions**, scope constraints, unchanged archived reports/indexes/frozen judgments and separate lead/expanded grades. The review self-check, targeted ESLint and diff checks pass. Existing model, passage, ranking and runtime code is untouched; a repeated model build or inference run would not test this review-only correction.

**Next:** investigate a bounded source-body passage-selection probe with the existing model, using generic fixtures and the fixed failures to distinguish missing candidates from excerpt-window problems. Retain successful native research leads until a general alternative passes both question and keyword regressions. Measure lead and expanded evidence separately; do not tune a private-source-specific rule. Independent representative quality, actual Arabic documents and reference-device/platform/lifecycle gates still precede Stage 7. No app rebuild or smoke repetition is needed for this pass.

## Stage 6 — bounded body-only passage probe

**Decision: reject query-time body-only E5 lead selection; retain native leads and separately identified conceptual context.** A private probe reuses the existing encoder, tokenizer and read-only source/index validation. It scores authored bodies without the added document-title/heading prefixes, keeping the first hybrid document fixed. Candidates come from its card, up to three expanded passages and up to two conceptual passages: at most six distinct indexed spans, with five observed. This tests bounded evidence selection, not global body-only retrieval or a new document ranking.

The initial probe chooses the cached-score winner within each displayed native section. Long sections expose a flaw in that diagnostic: the selected chunk can omit the visible native snippet. A second probe binds chunks to the actual snippet through offline source-span preparation. Of **110 native snippet/span bindings**, **102** fragments fit wholly inside the chosen chunk; eight cross a boundary. One chunk per excerpt remains a coverage limit. Preparing these locators from exported sources does not establish a production query-time locator implementation.

| Fixed development requests, either format | Direct current lead | Direct snippet-aligned body-only lead |
| --- | --- | --- |
| Twelve English natural-language needs | 5/12 | 9/12 |
| Four Arabic translations against English sources | 2/4 | 2/4 |
| Two scoped repeats | 1/2 | 2/2 |
| Six short-keyword queries | 6/6 | 4/6 |

These gains are the same four English heads as the previously rejected conceptual preference. Snippet alignment prevents the FP32 useful-background regression, but INT8 still replaces that background with irrelevant context. Both formats retain the two keyword regressions. Reading their small candidate pools confirms that answering passages **are available but lose to background**, while two English needs lack their answering passages in those pools. Another English head has an unsuitable first document; passage selection cannot repair its document order. This separates selection, source-window and candidate-coverage failures without word lists, query-length exceptions or source-specific rules. Unsupported controls remain useful background without their requested answers.

Each aligned process creates **43 temporary body vectors**, about **64.5 KiB raw**, across 26 unquoted requests. These are additional embeddings, not reuse of the stored title-prefixed vectors; all **4,365 stored vectors** remain unchanged. Body embedding totals are **2.07 seconds FP32 / 1.04 seconds INT8**, with process peak RSS about **1,444 / 775 MiB**, including model startup. Requests reuse temporary vectors across this correlated batch. These first-document diagnostics neither establish whole-app p95 nor justify scaling query-time re-embedding to ten results or maintaining another persistent vector index. The quality tradeoff does not justify either cost.

Four successful offline model processes cover **104 first-document comparisons**, with complete provisional selected-lead grades and **26 targeted candidate-coverage judgments**. Verification covers **232 source-span occurrences**, **eight quote exclusions**, snippet alignment, scope/model identity, unchanged indexes/old reports/frozen labels and seven real rejection probes that publish no output. The private release build and seven inherited checks pass; two unchanged cached-model checks remain ignored. Reports, grades and reproduction code stay outside tracked files. The intended tracked change this pass is generic documentation; runtime, review code and search behavior remain unchanged. The fixed assistant-authored assessment is not independent acceptance or real Arabic-source validation.

**Next:** one bounded passage-level comparison with the already cached reranker can test whether it selects the available answering passages without the keyword losses, holding document order and this small source pool fixed. Check candidate coverage separately; no selector can recover an absent passage. Do not add another model or expand query-time embedding work. Independent representative quality, actual Arabic sources and reference-device/platform/lifecycle gates remain before Stage 7. No app rebuild or repeated smoke test is needed for this rejected probe.

## Stage 6 — bounded passage reranker probe

**Decision: promising passage selection; reject blanket lead replacement for now.** A private probe reuses the cached quantized mMARCO MiniLM reranker, tokenizer and read-only indexed-source validator. It ranks the previous snippet-aligned body pool within the same first hybrid document: at most six indexed spans, with five observed. Document order, candidate coverage, expanded context and native search stay fixed. No additional body embeddings, model downloads, dependencies or production policy are added. FP32/INT8 below describe the encoder-derived candidate pools; both use the same cached reranker.

| Fixed development requests, either candidate format | Direct native lead | Direct body-only E5 lead | Direct reranked lead |
| --- | --- | --- | --- |
| Twelve English natural-language needs | 5/12 | 9/12 | 9/12 |
| Four Arabic translations against English sources | 2/4 | 2/4 | 2/4 |
| Two scoped repeats | 1/2 | 2/2 | 2/2 |
| Six short-keyword queries | 6/6 | 4/6 | 6/6 |

The reranker recovers both available keyword answers that body-only E5 selection lost, retaining the four English question-lead improvements. English first-document relevance remains **11/12**. Two English answering passages are still absent from these small pools, and passage selection cannot repair the unsuitable first document on another need. Cross-language counts agree across formats, but their successful cases differ; these translated requests do not validate actual Arabic-source retrieval.

One unsupported control regresses from useful background to an irrelevant, bare heading for a different year in both formats. Neither unsupported control gains its requested answer. This is a lead-selection failure, not successful abstention or a reason to calibrate a universal raw-score cutoff. Preserve current native leads plus separately identified conceptual expansion. Do not patch this with private-source exclusions, word overrides or query-length rules.

Two successful offline processes cover **52 provisionally graded selected leads**, **116 candidate-span occurrences** and **four explicit mandatory-quote exclusions**. Every unquoted pool is scored three times with identical score/source bindings, using one pair per batch and two inference threads. All pairs fit the untruncated 512-token bound; the observed maximum is **331 tokens**. Earlier judgments are reused only for identical query/source locators and authored text; the two newly selected heading occurrences are explicitly judged irrelevant. These known, assistant-graded development requests remain unsuitable for independent acceptance. Existing reports, indexes, stored vectors and frozen earlier labels retain their hashes.

Warm one-document reranking medians are **48.3 ms / 51.5 ms**, with observed maxima **122.5 ms / 136.3 ms**, for the FP32/INT8 candidate pools. Model startup takes **0.73 / 0.71 seconds**; standalone process peak RSS is about **686 MiB** in either run. This does not measure combined encoder/reranker memory, full-result-list latency, whole-app p95 or reference hardware. The probe uses existing supplied spans and performs no reindexing or additional body embedding.

The private release build and seven inherited checks pass; two unchanged cached-model checks remain ignored. Ten real rejection processes cover quoted-clause bypass, wrong/malformed scope, wrong parent, duplicate/oversized pools, altered authored text, overlong pairs and snapshot/model mismatch. Each rejects without publishing output or modifying frozen inputs. Source bindings, fixed document identity, pool preservation and complete selected-lead grades pass verification. Private requests, source details, reports and reproduction code remain outside tracked files; the intended tracked change is this generic decision record.

**Next:** trace candidate preparation and source windows for the remaining missing answers and the heading-only regression, then test the smallest general correction with the existing model and fixed question/keyword controls. Short factual answers must remain eligible; an arbitrary minimum length or raw-score threshold is not established by this batch. Keep reranking experimental until a broader independent comparison and its added device cost justify it. Actual Arabic sources and reference-device/platform/lifecycle gates remain before Stage 7. No app rebuild or repeated smoke test is needed for this isolated probe; the prior rebuilt-app confirmation stands.

## Stage 6 — candidate preparation and source-window check

**Decision: reject bounded BM25 pool filling for primary-lead replacement; stop tuning selectors on this batch.** Both previously missing answer passages exist in the source index. They are absent from the displayed evidence pool: semantic document reduction retains a score winner and a context winner, while native evidence retains up to three sections under its lexical constraints. Source indexing did not omit either answer. The tiny heading-only candidate is also faithfully indexed text; the exported section/span metadata has no body-versus-heading role that would safely distinguish it from a brief factual answer. A length filter or private wording exception does not resolve that limitation.

A private diagnostic reuses SQLite FTS5 with the existing English tokenizer (`porter unicode61 remove_diacritics 1`) and body-only BM25. It preserves every prior candidate, then fills unused slots with the highest-ranked distinct body spans matching an OR query within the same first hybrid document. This ranks indexed spans, whereas native FTS evidence ranks sections; it is not a replay of native BM25 scores. The six-span bound, document order, quote exclusions and cached reranker remain unchanged. It adds no stopword list, synonym pairs, source exclusions, additional embeddings or production behavior.

| Fixed development requests, either candidate format | Previous bounded reranker | After bounded BM25 filling |
| --- | --- | --- |
| Twelve English needs: direct first lead | 9/12 | 9/12 |
| Four Arabic translations: direct first lead | 2/4 | 2/4 |
| Two scoped repeats: direct first lead | 2/2 | 2/2 |
| Six keyword queries: direct first lead | 6/6 | 6/6 |

One missing English answer is the first body-BM25 match and becomes a direct lead. Another previously direct explanation is displaced by topical background, cancelling that gain. The other missing answer ranks **22nd** under this body-BM25 query and remains outside the bounded pool; the two added candidates for it are explicitly graded background. Do not increase the bound until a known answer happens to enter it. The unsuitable first document on another need is unchanged. The unsupported query now selects a bare article title instead of the previous bare date: neither supplies its answer or restores the original contextual lead. Enlarging a source pool can improve coverage while worsening selection, and a verified source locator proves provenance rather than relevance.

Each format adds **77 candidate occurrences**, taking its pool total from **58 to 135** across 26 unquoted heads. Warm one-document reranking medians rise from **48.3 / 51.5 ms** to **135.8 / 129.7 ms**, with observed maxima **244.6 / 249.1 ms**. Standalone process peak RSS remains about **686–687 MiB**. This offline preparation reads all **4,365 indexed bodies**, about **2.79 MB** of source text, into an in-memory diagnostic FTS table. Its roughly 0.09-second preparation time is not an implementation or benchmark of the application's query path. No vector reindexing or body embedding occurs; the increased reranking cost is not justified by these unchanged direct-lead totals and the new regression.

Two successful processes provisionally grade all **52 selected leads**: 42 reuse identical query/text/locator judgments and ten new occurrences receive explicit grades and reasons. Four additional candidate-coverage judgments check the still-missing answer. Other newly supplied bodies are not silently graded irrelevant. Verification covers **270 candidate-span occurrences**, preserved original pools and first-document identities, four quote exclusions, three identical scoring repetitions per unquoted pool, the untruncated token bound and unchanged model/index/report/earlier-label hashes. Ten real malformed-input processes reject without output or frozen-file mutation. The scorer source and binary are identical to the previous checked build; no runtime change needs another model build. Private inputs, bodies, judgments and reproduction code stay outside tracked files. The intended tracked change is this generic decision record.

**Next:** freeze the retained approach—native primary leads with separately identified conceptual expansion—and prepare a fresh representative quality/go-no-go review. Keep the rejected body-only, blanket reranker and pool-fill lead rules off. Further improvements should follow failures in that fresh evaluation, rather than additional settings selected against these known labels. Assistant development grading is not independent acceptance; actual Arabic-source coverage and reference-device/platform/lifecycle evidence still precede Stage 7. No app rebuild or repeated smoke test is needed for this rejected private diagnostic.

## Stage 6 — frozen-policy fresh quality review

**Decision: hold Stage 7; correct silent lexical query truncation before further ranking work.** The retained experiment keeps native primary leads and separately identified, verified conceptual context. Rejected body-only, blanket reranker and BM25-fill lead policies remain off. Both cached encoder formats run the same frozen requests through the existing production query parser/native evaluator and isolated experimental indexes. No search policy, model, dependency, runtime integration or default changes this pass.

The private review contains **22 fresh requests** against 37 documents: the existing 25 English sources, six assistant-authored Arabic prose fixtures and six English near-neighbors. Requests, proposed answer anchors and answerability checks were frozen before inference. Six English needs and six controlled Arabic needs are the main positives; four translated repeats, two scoped repeats, present/absent quotes and two unsupported requests are separate controls. Translations and scopes are correlated repeats. Fresh questions against a familiar library and assistant-authored Arabic fixtures are development evidence, not independent held-out acceptance or representative Arabic-book validation.

| Direct support in the first result's displayed expanded evidence; either encoder format | All words | Broader | Semantic | Retained hybrid |
| --- | --- | --- | --- | --- |
| Six English needs | 1/6 | 2/6 | 5/6 | 4/6 |
| Six controlled Arabic-source needs | 1/6 | 5/6 | 6/6 | 6/6 |
| Four cross-language repeats | 0/4 | 0/4 | 0/4 | 0/4 |

Document, primary-lead and expanded-evidence grades remain separate. On the English needs, semantic first documents directly support **6/6**, but displayed evidence supports only **5/6**; retained hybrid achieves **5/6 documents and 4/6 displayed evidence**. One answering section is outside the displayed pool. Broader primary leads directly support only **1/6 English and 4/6 Arabic** needs; expansion improves those counts to the table's 2/6 and 5/6. Semantic/hybrid supply direct evidence for both scoped repeats. Both unsupported requests return background without the requested answer; this is not abstention. The four cross-language first-result failures include misleading nearby sources with different entities or dates. Same-language Arabic success does not establish cross-language precision.

**A production lexical correctness issue invalidates a fair full-query comparison.** `fts_fuzzy_terms` keeps only the first twelve nonempty whitespace tokens, before distinct-term handling. In this batch **12 of 20 unquoted requests exceed that bound**. The tail is silently ignored even though All words promises that all unquoted words are required. A distinguishing entity at the end of one longer request is dropped: native search admits an unsuitable first document, and document fusion ranks it above the directly supporting semantic candidate. Embedding retrieval sees the complete request, while native retrieval does not. These results diagnose that failure; they cannot establish an acceptance-level semantic improvement over an equivalent lexical baseline. Correct the shared query contract rather than tune fusion against this false positive or add private-word exceptions.

All **70 first-result review records**, covering **126 channel occurrences**, receive provisional document, lead and expanded-evidence grades with reasons and source hashes. Identical complete result records share a judgment; records with different result metadata can still display the same source text. These occurrences include unchanged native results repeated across formats; they are not independent questions. The blinded pooled packet contains **214 document candidates and 331 evidence sets**, with independent grade fields left empty. Other pooled candidates are not automatically graded irrelevant. Proposed answer anchors are incomplete judgments, so this pass makes no exhaustive Recall@10 or nDCG acceptance claim. Private questions, source text, judgments and reproduction helpers remain outside tracked files.

Four successful encoder processes cover synchronization and read-only querying for both formats. Each private index reuses all **4,365 original vectors exactly**, embeds only **24 new fixture passages**, deletes none and contains **4,389 passages**. Verification checks **8,778 stored source spans**, **732 semantic source occurrences**, native excerpt provenance, **26 preserved native hybrid hits**, scope boundaries, four quote exclusions and unchanged frozen input/policy hashes. Present quoted text retains native evidence; absent quoted text returns no result, and mandatory quotes exclude experimental semantic/hybrid retrieval. The existing pooled-review self-check and private end-to-end verification pass. No additional model build, app rebuild or smoke test is needed for this isolated review.

**Next:** a bounded corrective pass on silent long-query truncation, using generic regression cases for distinguishing tail terms, repeated words, quotes and scopes; then replay this frozen batch before further ranking changes. Preserve finite input handling without silently searching a different request. Independent representative grading, real Arabic books, reference hardware and platform/lifecycle evidence remain open before Stage 7. The current Arabic controls expose useful failures but do not close those gates.

## Corrective pass — complete bounded native queries

**Decision: ship the lexical correctness fix on this branch; keep Stage 7 on hold.** Native search now retains every supported unquoted term rather than silently keeping only twelve. Shared validation runs before database opening, migrations or search progress. A request supports at most **128 normalized whitespace terms and 4,096 Unicode characters across unquoted text and quoted phrases together**. Repetitions count toward the input bound. Oversized requests fail with the stable `searchQueryTooLong` error instead of returning matches for a shortened request. Quote candidate construction remains bounded and receives the complete validated phrase.

The existing search input renders that error as localized, accessible guidance to shorten the request. Successful retry clears the error. Broader-mode relaxation still applies only to **3–12 distinct term expressions**; longer requests require all terms. Previously the twelve-token truncation made longer inputs appear eligible for relaxation. Help text in every existing locale now explains the actual range. This pass adds no synonym pairs, stopword list, new ranking policy, dependencies, model or semantic app integration.

Two generic native regression checks exercise a distinguishing thirteenth term, same-section and cross-section matches, repeated words through the 128-term boundary, required quotes, document scope, and oversized unquoted/quoted/combined inputs. Rejection must happen before either the database or progress callback is invoked. Frontend checks cover the limit error, successful retry and the input's alert/ARIA association. The search suite passes **33 native checks**; its two private entry points are ignored in the ordinary run but both library replay invocations execute the relevant entry point successfully. **21 targeted frontend checks**, TypeScript checking, targeted ESLint, translation parity and diff checks pass.

The same **22 frozen requests** replay against the unchanged isolated library and both existing **4,389-passage indexes**, using the same experimental executable and cached encoders. There is no reindexing or new embedding. Every semantic response is exactly identical to its prior response. Short native requests and mandatory-quote responses remain identical; longer native results/evidence now reflect complete requests. Private questions, source identities, judgments and reproduction helpers remain outside tracked files.

| Provisional direct first-result support; either format | Before correction | After correction |
| --- | --- | --- |
| English hybrid document | 5/6 | 6/6 |
| English hybrid primary lead | 4/6 | 5/6 |
| English hybrid expanded evidence | 4/6 | 6/6 |
| English All-words expanded evidence | 1/6 | 2/6 |
| English Broader expanded evidence | 2/6 | 2/6 |
| Controlled Arabic hybrid expanded evidence | 6/6 | 6/6 |
| Controlled Arabic Broader expanded evidence | 5/6 | 3/6 |
| Cross-language hybrid expanded evidence | 0/4 | 0/4 |

The dropped tail term no longer admits the misleading native document; the retained hybrid promotes the directly supporting semantic candidate. Another complete query brings the previously missing answer section into native expansion while preserving its background primary lead. These are query-contract/evidence gains, not a selector or model change. Broader loses some long natural-language matches because it now requires the complete request outside its supported relaxation range; do not restore silently omitted words to recover those counts. Primary and expanded support remain separate, and the original assistant-authored/familiar-source limitations still apply.

All **56 first-result review records**, across **106 channel occurrences**, have provisional source-bound document, lead and expanded grades. The new pooled independent packet has **195 document candidates and 254 evidence sets**, with independent grades still empty. Verification checks unchanged frozen sources/indexes/earlier judgments, **8,778 stored source spans**, **732 semantic source occurrences**, native excerpt provenance, **14 native hybrid hits preserved exactly**, scope boundaries and four quote exclusions. No independent acceptance, whole-library recall or reference-device performance claim follows from these counts.

**Next:** diagnose the four cross-language failures with the frozen model/settings, separating document candidate coverage from ordering and evidence selection. Only two proposed supporting documents occur in the current ten-result semantic/hybrid lists in either format; reranking those lists cannot recover the other two. Check entity/date distractors and source-language competition before proposing a bounded general change. Independent representative grading, actual Arabic books, reference hardware and platform/lifecycle evidence still precede Stage 7. Rebuild and smoke-test long queries, the inline oversized-input message, quotes and document filters before relying on the changed app behavior; an offline diagnostic can proceed independently of that UI check.

## Stage 6 — cross-language candidate and evidence diagnosis

**Decision: indexing is intact; keep Stage 7 on hold and test ordering within existing bounds before adding policy.** A private metadata-only probe exports the exact scan's complete document maxima and query vectors. It preserves the existing model, chunking, scope, fusion, ten displayed results and native-primary/separate-context policy. All 22 frozen responses in each encoder format remain exactly unchanged. Every proposed supporting document is indexed and scored among the **37 eligible documents**; two fall below the displayed ten. The existing fifty-document fusion input already includes all 37 here. These are ranking/display-pool failures, rather than omitted source documents.

| Anonymous correlated case | Full target document rank, FP32 / INT8 | Best passage in proposed answer section, within target, FP32 / INT8 | Source-language counterpart document rank, FP32 / INT8 |
| --- | --- | --- | --- |
| A: English source, Arabic request | 13 / 18 | 7 / 12 | 1 / 1 |
| B: English source, Arabic request | 3 / 2 | 1 / 1 | 1 / 1 |
| C: controlled Arabic source, English request | 25 / 28 | 1 / 1 | 1 / 1 |
| D: controlled Arabic source, English request | 5 / 9 | 1 / 1 | 1 / 1 |

The four source-language counterparts return directly supporting first evidence in both formats. The cross-language requests still return **0/4 directly supporting first evidence**. In B and D, the proposed supporting document and its answering passage are already in the ten-result pool but rank below nearby sources. C selects an English counterexample over the Arabic explanation. D selects a different project's date even though its text explicitly distinguishes it from the requested project. A has two failures: the correct document is below ten, and its score/context winners discuss another event while the requested explanation ranks seventh/twelfth inside that document. Both winning spans have substantial body text; the failure does not justify a heading or minimum-length exclusion.

For three cases per format, the proposed target's selected evidence directly supplies its answer; A's selected target evidence does not. Proposed answering sections exist in every index and have verified source spans. Enlarging a displayed list alone would neither reorder the misleading first hits nor expose A's displaced explanation. A cosine threshold or private spelling/synonym pair would not establish the needed entity/event distinction. Paired request wording differs as well as language, so these pairs do not isolate a language-only effect. The familiar English sources, authored Arabic controls, nearby counterexamples and incomplete answer anchors remain development evidence; independent grading and representative Arabic books are still needed.

Four successful cached-encoder processes capture the original full ranking and confirm untruncated token counts; no passage embedding, reindexing, model download or production change occurs. The observed maximum is **34 query tokens** including prefix/special tokens, within the model's 512-token bound. Private exact float32 dot-product reproduction checks **70,224 passage scores across 16 query/format combinations**, agreeing with Rust document order and winners. Verification checks **40 unquoted metadata matrices**, **2,384 winner-to-index bindings**, **80 source-span occurrences**, four mandatory-quote exclusions and unchanged original source/index/report/judgment/implementation hashes. Eight paired case/format records have provisional document and source-evidence judgments with reasons; repeated sources and related requests are not independent samples. The private release build, seven inherited checks and end-to-end verification pass; two unchanged cached-model checks remain ignored. These diagnostics do not measure app or reference-device latency.

Private inputs, vectors, full rankings, source bodies, grades and reproduction code remain outside tracked files. The intended tracked change is this generic decision record. No app rebuild or additional app behavior change is part of this pass. The previous complete-query correction's manual smoke check remains **deferred by the user**, with a reminder requested when this pass finishes.

**Next:** one bounded document-order comparison using the already cached reranker and unchanged ten-document/two-passage pools, holding primary leads, expansion, quotes and scopes fixed. Check the available cross-language supports alongside English, same-language Arabic, entity/date and unsupported controls. Keep A/C's missing proposed targets explicit; reranking the supplied ten cannot recover them, and these incomplete anchors do not establish a whole-library recall ceiling. Do not enlarge a pool until known answers enter it or reintroduce the rejected primary-lead replacement rules. Retain the baseline unless the comparison shows a useful precision gain without control regressions and justifies its measured cost. Independent representative quality, actual Arabic books and reference-device/platform/lifecycle gates remain before Stage 7.

## Stage 6 — bounded document-reranker comparison

**Decision: reject adoption of this reranker configuration; retain the baseline and keep Stage 7 on hold.** The existing experimental runner and cached quantized multilingual reranker score the same 22 frozen requests against both encoder formats. This comparison changes document order only: at most ten supplied documents and two existing passages per document, maximum raw pair logit, one pair per batch, two threads and a checked 512-token pair limit with no truncation. Primary leads, expanded evidence, native results and the existing hybrid remain intact. No model download, reindexing, application change or new dependency is needed.

| Provisional direct support in first-result expanded evidence; either encoder format | Existing semantic | Reranked semantic |
| --- | --- | --- |
| Six English needs | 5/6 | 5/6 |
| Six controlled Arabic-source needs | 6/6 | 5/6 |
| Four cross-language repeats | 0/4 | 1/4 |
| Two scoped repeats | 2/2 | 2/2 |

The available cross-language support in case B moves to first with its answering passage unchanged. However, a previously correct same-language Arabic name/date result now ranks a different project's record first, even though that record explicitly excludes the requested entity. The correct answer remains second. Case D's different-project date remains first and its actual support moves to second. The two missing proposed targets in A/C remain absent; A still selects evidence about another event. English first documents remain directly supporting in 6/6 cases, but leads and semantic expansion remain 5/6 because one answering span is outside the supplied evidence. Document ordering cannot supply that span. This comparison does not apply the reranker to hybrid results.

The date regression also exposes a scoring-context limitation: the correct selected passage supplies the date, while the named entity appears in a separate introductory section outside that document's supplied scoring window. The distractor mentions the entity in an explicit exclusion. This observation does not establish whether bounded source context would help or whether the model would still fail the exclusion; do not infer a fix from one case or add entity-specific rules. Both unsupported requests still return background rather than the requested facts or abstention. Mandatory quotes remain lexical-only.

All **40 first-result query/format records** receive provisional source-bound document, lead and expanded-evidence judgments: 34 reuse equivalent earlier evidence and six newly ordered records are inspected separately. Formats and translated/scoped requests are correlated; these are assistant judgments on familiar sources and authored Arabic controls, not independent acceptance. The pooled independent packet retains 195 document candidates with its grades unfilled. Unjudged candidates are not presumed irrelevant; no exhaustive recall or nDCG claim follows.

Two standalone reranker processes complete successfully. Across **16 ten-document requests per format**, the observed median scoring time over three repetitions is approximately **231 ms / 303 ms**, with maxima **431 ms / 511 ms**. Model load takes about **0.67 seconds** and whole-process peak RSS is approximately **693 / 694 MiB**. Both processes use the same reranker; their encoder-derived passage pools differ. These measurements are development-machine observations, not an encoder-speed comparison, combined encoder/reranker memory measurement, app latency or reference-device result. Reranking is a per-query cost, separate from indexing.

Verification covers **328 reordered document hits**, **347 indexed source-span occurrences**, **1,041 pair evaluations across three deterministic repetitions**, eight restricted-scope hits and four quote exclusions. Original result fields and source text remain equivalent; JSON reserialization introduces 26 checked floating-point changes of at most one ULP in existing numeric metadata/scores. All frozen input, index, model, earlier judgment and implementation hashes remain unchanged. Eight malformed-input checks reject incompatible model/snapshot identities, excessive document/passage pools, out-of-scope candidates, changed source text, quote bypass and oversized token pairs, without creating output. The existing pooled-review self-check and private verification pass. Private source material, reports, judgments and reproduction helpers remain outside tracked files.

**Next:** a small controlled scoring-context/exclusion check using the cached model, to distinguish omitted entity context from failure to respect an explicit exclusion. Compare bounded source context on balanced generic controls before proposing a scoring change; keep retrieval bounds and displayed evidence fixed. Do not promote this configuration, enlarge pools to known answer ranks or replace primary leads. Independent representative quality, actual Arabic books and reference-device/platform/lifecycle evidence still precede Stage 7. The previous complete-query app smoke test remains deferred by the user, with its reminder carried into the **next pass**.

## Stage 6 — identity-context and exclusion controls

**Decision: stop tuning this cached reranker; reject context augmentation and retain the baseline.** A private scoring-only probe reuses the existing model-loading/tokenization path and cached quantized graph. Its inputs and expected relationships are frozen before inference. It tests 16 generic paired comparisons: two authored scenario families, both document roles reversed, and English/Arabic queries against English/Arabic sources in all four language directions. Two previously failing source-bound comparisons are separate diagnostic replays. No retrieval, index, candidate pool, displayed evidence or application behavior changes.

Each authored source separates reviewer identity from the dated answer and explicitly excludes the other source's identity. Role reversal gives both names/dates the same test. Three input policies compare the answer alone, the maximum of separately scored identity/answer passages, and their contiguous combined text. A fourth matched contrast changes an affirmative review statement to its explicit negation while preserving the date and other content. Higher score for the supporting source is the comparison criterion; ties do not count as success.

| Query → source language; four correlated comparisons per row | Answer alone | Separate maximum | Combined context | Matched affirmation over exclusion |
| --- | --- | --- | --- | --- |
| English → English | 0/4 | 0/4 | 0/4 | 2/4 |
| Arabic → Arabic | 0/4 | 0/4 | 2/4 | 1/4 |
| English → Arabic | 0/4 | 0/4 | 2/4 | 2/4 |
| Arabic → English | 0/4 | 0/4 | 2/4 | 3/4 |
| Total authored comparisons | 0/16 | 0/16 | 6/16 | 8/16 |

Combining the actual identity and dated answer reverses the earlier same-language Arabic failure in its diagnostic comparison. The corresponding English cross-language comparison still ranks the explicitly different project higher. Separately scoring identity and answer then taking their maximum fixes neither. Missing identity context therefore contributes to one observed failure, but adding it is insufficient to establish reliable source discrimination. Several combined/minimal contrasts have small margins. These are comparative relevance scores, not calibrated probabilities or a general proof about language understanding, negation or quantization. Do not derive a threshold, special word rule or a new source-window policy from these results.

The probe scores **140 frozen query/body pairs**, three times each with the middle pass reversing pair order: **420 evaluations** have exactly repeatable scores and verified query/body hashes. Its model identity matches the previous reranker, and the original answer-only scores reproduce the earlier source-bound comparisons in both encoder-derived reports. The observed maximum is **157 pair tokens**, below the checked 512-token limit with truncation disabled. Four boundary checks reject duplicate identities, oversized pairs, unknown input fields and checkout-local output without creating a report. The private offline release build and end-to-end checks pass; original sources, indexes, earlier judgments, cached assets and experimental implementation/executable hashes remain unchanged.

These authored pairs, mirrored roles and language variants are correlated development controls. Their expectation labels follow the constructed source relationships; they are not independent relevance grading, a full retrieval replay, representative Arabic-book evidence or quality acceptance. Adding an introductory section in this diagnostic does not make it an existing retrieved/displayed passage. All source bodies, control examples, scoring code and results remain outside tracked files. This pass adds no production code, dependencies or model and requires no app rebuild.

**Next:** a Stage 6 decision/cleanup pass: preserve the rejected reranker and diagnostics in private archives, remove its unused experimental entry point, and consolidate the remaining go/no-go evidence while retaining the embedding baseline. A whole-batch context-policy replay is not justified by this control result. Independent representative English/Arabic quality and reference-device/platform/lifecycle evidence still gate Stage 7. The complete-query app smoke test remains deferred, with its reminder carried into the **next pass** at the user's request.

## Stage 6 — rejected reranker cleanup and decision gate

**Decision: remove the rejected implementation and retain the isolated embedding baseline; Stage 7 remains on hold.** Verified private archive manifests preserve the document-reranker and identity-context diagnostics, including matching pre-cleanup source. A separate private snapshot preserves the old executable and complete runner source before removal. The maintained experiment deletes the 282-line reranker module and its four module/dispatch lines. The embedding dependency, model policies, storage, source evidence, fusion and application code are unchanged.

The review helper retains its validated archived-reranker report fields and comparison checks so previous private evidence can still be reviewed. It reads reports and does not provide a live scoring route. The removed rerank-only ordering check remains in the archived source; six maintained checks still cover fusion/deduplication, scoped source ranking and vector compatibility, evidence fallback, quantization policy, SQLite restart/update/rollback and UTF-8 source loading. The two unchanged cached-model indexing/chunking checks remain ignored in the ordinary run.

The offline release build, **six unit checks**, archived-review self-check and diff checks pass. The rebuilt executable rejects the removed CLI route with its existing usage error. Read-only replay of the **22 frozen requests in both encoder formats** yields **44 query records with every non-timing report field exactly identical**, including native/semantic/hybrid results, ordering, excerpts, locators, policies and source-loading counters. Four required-quote exclusions remain intact. Both 4,389-passage indexes retain their hashes; there is no new embedding, index synchronization or app rebuild. All unrelated pending changes remain intact.

The current decision table at the top separates retained branch behavior, experimental feasibility, rejected ranking work and remaining acceptance gates. Provisional familiar-source/authored-control grades cannot substitute for independent representative relevance; current-machine probes cannot substitute for reference hardware or mobile/product lifecycle tests. No further context policy, reranker model comparison or Stage 7 implementation is justified by this cleanup.

**Next:** validation/readiness against those gates, reusing the existing independent packets and obtaining actual Arabic-source and reference-device evidence. Keep lexical search available and the semantic prototype separate until there is a justified go decision. The complete-query app smoke remains deferred, with its reminder carried into the **next pass** as requested.

## Stage 6 — retained-baseline readiness and authored Arabic fixtures (2026-10-07)

**Decision: retain the baseline and keep Stage 7 on hold.** The user has no local Arabic books and requested authored test documents. A private set contains six fictional Arabic EPUBs, HTML companions with equivalent text, 48 sections and 3,951 whitespace-delimited words. Three paired scenario families provide related neighbors, entity/date distinctions, explicit exclusions and unavailable numerical facts. Twenty-four requests and proposed source relationships were frozen before retrieval. This is a development control set, not representative Arabic-book evidence, independent judgments or a scale benchmark. No document text, query examples or identities are added to tracked files.

Standard-library ZIP/XML checks verify the EPUB structure, RTL chapters, spine and authored section text. Offline evaluation supplies equivalent parsed text through the production document store and calls the existing frontend parser/native search orchestration. It does **not** exercise the production EPUB importer or reader; private import files and a manual-check guide are available for that pending check. Two authored expectation locators were corrected against unchanged source sections, with original controls and the errata preserved. Queries, document bodies and inference inputs were unchanged.

All words and Broader each run the 24 requests against an isolated library. Three present quoted phrases select their intended document; an absent phrase returns nothing. Marked Arabic names preserve native results, evidence and source locations while retaining the original query labels. Document scope holds across all checked channels. Strict All words correctly returns no result when a question contains required words absent from the document; Broader can recover related material but can rank an explicit excluding neighbor first. Literal word coverage does not establish semantic relevance or negation handling.

The unchanged FP32/INT8 executable independently indexes the 48 source passages in each format and compares native, semantic and document-fused results without a reranker or a new policy. Both formats have the same provisional first-result findings:

| Three distinct questions per group; scoped repeats are correlated | Intended first document | Direct answering lead/expanded conceptual evidence |
| --- | --- | --- |
| Arabic conceptual questions | 3/3 | 2/3 |
| Their English translations against Arabic sources | 2/3 | 1/3 |

One cross-language result ranks a different project's explicit exclusion first. Another reaches the right document but supplies background instead of the causal answer. Semantic-only retrieval also loses a marked name that native search and lexical fusion recover. Unsupported numerical questions return related background or a statement that the information is unavailable; this is not successful numeric answering or a tested abstention mechanism. The retained hybrid preserves native primary evidence and supplies conceptual context separately. These results support retaining lexical search and the experiment's separation, not a new normalization/model/context tuning loop or a production semantic-search promise.

Verification covers 96 indexed source spans across formats, 360 semantic and 336 native evidence occurrences, 30 scoped hit occurrences, eight required-quote exclusions and 28 unchanged lexical hybrid hits. Source bodies, encoder assets, inference policies, application code and unrelated pending changes retain their frozen hashes. All **111 available first-result channel records** receive assistant document/lead/expanded-evidence grades bound to the inspected source; repeated formats and scoped variants are correlated. Empty results do not receive invented relevance grades, and these provisional grades do not establish exhaustive recall or independent acceptance.

The retained model-free evaluation passes its **41 requests** on the generic tracked fixture (25 documents, 54 sections); labeled document Recall@10 remains **41/45**, a regression check rather than new quality evidence. **33 native search checks** pass; two private entry points remain ignored in the ordinary suite and are exercised explicitly for this pass. **31 targeted frontend checks**, the two evaluation-parser checks, TypeScript, eight-locale parity and targeted ESLint pass. The existing pooled-review self-check passes, and reconstructing the separate 22-question packet is byte-identical: its 195 document candidates and 254 evidence sets remain independently unjudged. There is no app rebuild, installed-library mutation, dependency change or reference-device performance claim.

**Next:** validate the authored EPUB import/reader path and pursue independent representative relevance and reference-device/platform/lifecycle evidence before reconsidering Stage 7. Actual Arabic-book coverage remains open even after these fixture checks. Keep the semantic baseline isolated; further ranking/model tuning is not justified by this pass. The user's complete-query app smoke remains deferred, with its reminder carried into the **next pass**.

## Stage 6 — actual EPUB parsing, persistence and reader targets (2026-10-07)

**Decision: automated fixture import/source checks pass; keep the semantic experiment isolated.** The six unchanged private EPUB files now run through the production EPUB parser and `write_and_index_document` in an empty external library. Their source-byte hashes supply the same document identities and HTML upload URLs used by app import. This bypasses the picker/AppHandle boundary, not the parsing, filesystem persistence or SQLite indexing logic; it does not claim interactive app smoke or mobile file-access coverage.

The real reader layout has **17 indexed sections per book, 102 total**: a generated book-title section plus eight chapter headings and eight body paragraphs. The previous readiness pass's 48-section library intentionally supplied equivalent paragraph text and did not exercise this layout. All 48 authored paragraph bodies remain exact. Every stored section matches the sanitized reader text and its generated ordinal target; the eight chapters per book retain Arabic language and RTL direction. Re-sanitizing changes permitted presentation/attributes, so source and reader HTML are not required to be byte-identical. Root/title alignment, visual navigation, font behavior and Find remain part of deferred interactive smoke.

A generic ordinary regression in `pipeline.rs` reuses the existing public EPUB diagnostics fixture to check persisted source, indexed text and sanitized reader section targets together. A second, ignored entry point accepts external EPUB paths and an empty library/new report destination, performs a bounded read, rejects duplicate source identities and writes source-derived evaluation metadata. Both are confined to `cfg(test)`; production import, reader and search implementation is unchanged. Private examples and paths are supplied externally rather than embedded in code/tests.

For a private source list shaped as `{"files":["/external/book.epub"]}`, run the entry point with `PAPERCUT_EPUB_EVAL_INPUT`, `PAPERCUT_EPUB_EVAL_LIBRARY` and `PAPERCUT_EPUB_EVAL_OUTPUT` set to external paths:

```sh
cargo test --offline --manifest-path src-tauri/Cargo.toml search_v2_epub_import_evaluation --lib -- --ignored
```

Reuse `scripts/search-eval.mjs --library` for queries against the resulting isolated `search.sqlite3`. Filters must use the source-byte-derived upload identities; equivalent-text fixture identities and section ordinals are not import identities/locators. Keep the original sources, queries and mapping provenance when comparing layouts.

The same frozen 24 query texts run in All words and Broader against the actual parsed library. Both unchanged encoder formats independently index its 102 passages. Native document orders/counts and all **111 available first-result leads** match the previous readiness results after mapping document identity and paragraph ordinals. Source-bound provisional document/lead/expanded judgments remain unchanged: Arabic conceptual first documents **3/3**, direct evidence **2/3**; their English translations **2/3** and **1/3**. These small authored groups and scoped/format repetitions remain correlated, not independent acceptance. Additional heading context does not supply the missing causal answer or repair the incorrect cross-language neighbor/marked-name results. No new policy, model, reranker or tuning is introduced.

Verification checks **102 generated reader targets**, **204 indexed source spans** across encoder formats, **336 native and 696 semantic evidence occurrences**, 30 scoped hit occurrences, eight quote exclusions and 28 unchanged lexical hybrid hits. Source-byte identities, format/source-kind metadata, original paragraph hashes, query text, frozen assets/policies and unrelated code are verified. **112 document-upload tests** pass, including the new reader/store regression; four opt-in tests remain ignored in that ordinary suite, with the new EPUB evaluator and existing private library evaluator exercised explicitly. **Eight frontend reader range/target tests**, Rust formatting and diff checks pass. Reports, generated reader HTML, isolated libraries and reproduction helpers remain private; the installed library is untouched.

**Next:** review release readiness of the model-free branch changes and the remaining rollout gates, rather than repeat model tuning or already-covered storage lifecycle probes. Interactive EPUB import/reader and complete-query smoke remain deferred, with their reminder carried into the **next pass**. Independent representative quality, actual Arabic books, reference hardware and product/platform lifecycle evidence still precede any Stage 7 go decision.

## Model-free release-readiness review (2026-10-07)

**Decision: keep the model-free improvements as the release candidate; Stage 7 remains on hold.** Reviewed the branch against its local `origin/main` merge base and the accumulated working-tree changes, then traced query parsing, native validation, candidate/evidence selection and result rendering. No further search implementation change is justified by this review. The original plan explicitly permits stopping after a documented experimental no-go; retaining useful lexical improvements does not require shipping embeddings.

Corrected stale current-behavior documentation in [user-document-search.md](user-document-search.md): removed the obsolete hard-coded related-form retry, described spelling help alongside starter hits, distinguished candidate and body-evidence scoring, and recorded complete-query bounds, nested quotes, optional adjacency and the three-to-twelve-word Broader window. Earlier chronological measurements remain historical records, not the current contract.

### Commit boundaries

This earlier separation proposal is superseded by the user's request for one combined relevant-change commit and the official-dependency restoration below. It records the scope considered before that decision.

| Group | Files and treatment |
| --- | --- |
| Model-free application corrections | Pending `document_uploads/search.rs` and `search/query.rs`; `useSearch.ts`, `phraseSearch.ts`, `SearchResults.tsx`; the corresponding search/parser/component regressions and eight locale files. Keep the existing committed field weights, Broader mode, normalization and explicit spelling behavior. No embedding dependency is needed by these corrections. |
| Search evaluation and import regressions | `scripts/search-eval.mjs`, its generic corpus, `search/evaluation.rs`, `pipeline.rs` test additions and `searchEvaluation.test.ts`. Retain runnable checks and source-locator contracts; private inputs stay external. The new EPUB import entry point is test-only. |
| Documentation | `docs/search-v2-progress.md` and `docs/user-document-search.md`. This pass changes only these tracked files. |
| Native runtime compatibility | Review/commit separately as one complete unit: `src-tauri/Cargo.toml`, `Cargo.lock`, both modified native TTS engine files and the complete `vendor/libtashkeel_base` source/model/maps/license/README. The manifest already references that currently untracked vendor directory; committing the manifest without it would break reproducibility. Prior runtime parity/speech evidence remains separate from today's default-feature checks. |
| Isolated experiment | `experiments/search-v2-embeddings` has no app integration. If retained in version control, select only its maintained source, manifest/lockfile and documentation, with its own review; never stage targets, downloaded assets or private reports. |
| Excluded from these commits | Editor settings, MemSearch, temporary/session files, root smoke documents, private libraries/relevance reports, generated build output and the locally supplied root planning handoff. Use explicit file lists rather than staging the entire worktree. |

These boundaries are a preparation record, not staged or committed changes. The checkout includes the pending runtime/vendor unit; today's successful checks alone do not prove a clean model-free-only checkout or every native feature configuration. Verify the actual selected commit from a clean checkout before packaging it.

### Verification and remaining gates

- Full frontend suite: **42 files, 142 tests pass**.
- Default-feature native library suite: **146 tests pass, four opt-in tests ignored**. The sandbox initially blocked two existing local-socket transfer tests; both pass when rerun with local socket access. No search regression fails.
- Generic native search evaluation and its two frontend-parser checks pass; document Recall@10 remains **41/45 = 0.911**. Current-machine warm p95 is **2.73 ms** on the small fixture, not a large-library or weak-device acceptance result.
- TypeScript, all eight locale resources, full ESLint, Rust formatting and diff checks pass. Production Vite build passes with a chunk-size advisory. No Pagefind implementation/index change or desktop/mobile installer is produced by this pass.

The automated checks protect complete required clauses and oversize rejection, scope/quote exclusions, stable ranking and adjacency tiers, authored source evidence/counts/targets, stale-request suppression and display of all retained cross-section passages. They are regression evidence; they do not establish independent semantic relevance or visual app acceptance.

**Next:** prepare explicitly scoped commits and verify their clean-checkout build, then complete release validation when the user resumes smoke testing. Carry the reminder into the **next pass**: complete long/mixed/nested-quote searches, explicit spelling retry, longer excerpts and all three Explore passages, HTML/PDF source jumps, compound queries in both modes, and actual EPUB import/reader navigation including RTL/title alignment. The user has deferred these checks again; they remain open and need not block this documentation pass. Large-library responsiveness and supported-platform checks still precede release. Independent representative relevance, actual Arabic books, weak reference hardware and optional-model lifecycle/platform evidence still precede any Stage 7 go decision; do not add another tuning pass merely to continue the roadmap.

## Official dependency restoration and combined commit (2026-10-07)

**Decision: retain upstream dependencies in the app and commit the accumulated relevant work together.** The user raised the ongoing maintenance burden of a local Libtashkeel adaptation and accepted restoring the official dependency before committing. The app manifest and lockfile return exactly to their pre-migration versions: published `libtashkeel_base` 1.5.0 and compatible ORT/ort-sys `2.0.0-rc.1`. The original dynamic/static initializers are restored. The local Libtashkeel override and vendor directory are removed; adaptation source is preserved privately, without another maintained copy in the checkout.

The stronger generic preprocessing regression and explicit cached-Arabic speech check remain. They test the official app path, with no new production TTS behavior. The separately built embedding experiment retains its own FastEmbed 7.1.0 manifest/lockfile and ORT `rc.13`; it neither requires nor changes the app's older wrapper. Same-process compatibility work is deferred until an upstream-supported route and Stage 7 acceptance justify it. Ordinary Cargo updates cannot refresh a vendored adaptation; returning to upstream avoids owning that patch while preserving the current search improvements and isolated experiment.

The combined commit selects the pending search/query/UI/localization corrections, their generic regressions, external-input evaluation/review tooling, EPUB source/store regression, retained preprocessing/speech tests, maintained experimental source/manifest/lockfile/compatibility note, and these documentation updates. It excludes editor settings, MemSearch, session files, root smoke documents, the locally supplied root plan, private corpus/queries/relevance reports, downloaded models and generated targets. No push, merge, app-data migration, model download or installer is part of this pass.

Verification uses an export of the explicitly staged tracked files, with cached Node dependencies and the app's existing shared native runtime supplied externally. The initial native build correctly required the ignored runtime resource directory; supplying that existing build prerequisite allows the clean source export to build without adding runtime binaries to the commit. This is a development-machine check, not a fresh dependency download, installer or platform acceptance claim.

- **142 frontend tests** pass; TypeScript, eight-locale parity, full ESLint and the production Vite build pass. The existing chunk-size advisory remains.
- **193 native library tests** pass with `native-tts-shared`, including official Libtashkeel preprocessing and existing local-socket tests. Six opt-in tests are ignored in that suite; the cached-Arabic speech test is additionally run explicitly and passes, with finite non-silent audio. No audio file is written and no model is downloaded; this does not establish listening quality.
- The default-feature generic search evaluator and its two frontend-parser checks pass; document Recall@10 remains **41/45 = 0.911**. Review-tool self-check and Rust formatting pass.
- The independently built experiment passes **six ordinary checks**, with two asset-dependent checks ignored. It retains its own dependency graph and introduces no app semantic integration.

**Next:** rebuild and resume the deferred app smoke checks when ready: complete/mixed/nested-quote queries, spelling retry, longer excerpts and all three Explore passages, scoped compound queries in both modes, HTML/PDF source jumps and actual EPUB import/reader targets/RTL title alignment. Listening to Arabic/English previews and saving/playing a short audiobook also confirms the restored official runtime path. Large-library responsiveness and supported-platform checks remain release gates. Independent representative relevance, actual Arabic books, reference hardware and optional-model lifecycle/platform evidence remain prerequisites for Stage 7. The combined commit is for review and user-managed push; no further tuning or integration is authorized by these regression results.

## Model-free desktop-scale native validation (2026-10-07)

**Decision: keep the existing native search implementation; no cache, extra worker or retrieval dependency is justified by this probe.** The user authorizes continued automated validation and again defers interactive app smoke. A new privately authored saturated-match workload runs the retained native path at **500 documents / 100,000 sections**, including three documents with 1,200 sections each. Source bodies contain English and Arabic, with sixteen repeated templates. This deliberately stresses common matching rows and supporting-section counts; its small vocabulary, repeated text and authored sections do not represent real-book diversity, spelling-vocabulary pressure, file extraction or relevance quality.

The existing external-input evaluator prepares the isolated library through the production store and calls the actual search orchestration in an optimized release test binary from `6f9fa7a`. Requests pass through the existing frontend parser. No new tracked fixture, test harness, application code, dependency or dev configuration is added. The installed library is untouched. Preparation takes **11.55 seconds**, including JSON decoding and all store/index work; it is not a measured interactive import or pure indexing-throughput figure. The resulting database is **210.55 MiB**. Indexing unchanged documents is reusable, but edits/replacements and future index-version changes still have work to perform.

Each case has one first measured submission and five warm repeats. A read-only `VACUUM INTO` snapshot precedes retrieval; the OS page cache is not cleared. First submissions are therefore **not cold-disk measurements**. Timings cover native database/query/evidence work, excluding frontend IPC, rendering, import and snapshot-copy time. Runs are interleaved by repetition rather than finishing one query family before beginning the next.

| Saturated-match case | First submission, ms | Warm median, ms | Warm maximum, ms |
| --- | ---: | ---: | ---: |
| All words, two terms | 682 | 670 | 686 |
| Required quoted phrase | 291 | 288 | 293 |
| Word plus required phrase | 400 | 395 | 402 |
| Broader, three terms | 874 | 853 | 868 |
| Arabic, two terms | 680 | 666 | 678 |
| All words, twelve-document scope | 575 | 559 | 564 |

All six families meet the existing numerical thresholds **on this development machine**: unquoted warm median at most one second, exact/mixed at most three seconds, and no warm run above five seconds. Candidate retrieval is the largest measured phase. The limited twelve-document scope still requires appreciable common-term FTS work; do not infer cost proportional only to the visible result count. These results do not close the chosen weak-reference-device, mobile, genuine cold/disk or full-app responsiveness gates.

Every unscoped case reports all **500 documents / 100,000 matching sections** while returning fifty cards. Scope reports exactly **12 documents / 5,346 sections** and returns those twelve. Quoted/mixed occurrence counts equal the supporting section count because the authored phrase appears once per section. Warm responses are identical, with verified deduplication, scope, source excerpts/targets, complete counts, three-passage/twelve-bin bounds and term counts. Verification covers **1,572 hit occurrences, 18,996 source-locator occurrences, 2,244 term-count records and 11,664 distribution bins**. Maximum compact UTF-8 response size is **179.38 KiB**; the twelve-document scope is **40.71 KiB**. These are payload-size checks, not measured IPC or DOM latency.

The native child process peaks at **50.88 MiB RSS**, including snapshot creation and all 36 searches; compiler/fixture preparation are excluded. This is the release evaluator process, not whole-app memory or per-query allocation. Source, input and database hashes remain unchanged. The two frontend-parser checks, explicit production-store preparation and read-only native evaluator pass. Raw sources, requests, reports, isolated libraries, verification and reproduction helpers remain outside the repository.

**Next:** resume the deferred rebuilt-app search/EPUB and listening/save-play smoke checks when the user is ready, then measure representative sources and reference hardware/platforms. No more search code or semantic tuning is required by this pass. Stage 7 remains on hold for independent representative relevance, actual Arabic books, reference hardware and product-model lifecycle/platform evidence. This documentation-only follow-up records a development-host scaling result, not production release acceptance.

## Official Libtashkeel interface / shared-runtime check — 2026-10-07

**Decision: the published interface provides a viable no-fork Linux route.** Added a small, separately built [same-process compatibility check (historical; now retired)](https://github.com/muhannadnouri/papercut.io/blob/cf161ca0f293718414a02aefda88913b2fa2acff/experiments/search-v2-embeddings/compat/README.md), using official `libtashkeel_base` 1.5.0 with default/ORT features off and a caller-supplied `InferenceEngine`. The adapter handles tensors and a serialized ORT session; upstream retains its language processing and maps. FastEmbed 7.1.0 and sherpa 1.13.4 share ORT `rc.13` in the check. There is no Libtashkeel patch, copied upstream implementation, new model download or production dependency change. The separate executable is a validation boundary, not the proposed production architecture.

Four offline combinations pass: FP32 and dynamic INT8, each with preprocessing/speech-first and embedding-first initialization. Six generic Arabic outputs match a fresh run through the official older ORT backend exactly and remain stable after embedding/speech calls. Four concurrent calls through one preprocessing engine return the expected result. Three 384-dimensional vectors per run are finite and normalized, exactly unchanged after synthesis and across initialization order within each format. Synthesized Arabic is finite, non-silent and 22,050 Hz; waveform equality and listening quality are unclaimed. `/proc/self/maps` confirms one native ORT image, at the explicit cached runtime path, in every combined run.

Input-shape mismatch and invalid ONNX assertions pass. Five additional processes reject invalid initialization order, unsupported quantization policy, missing/corrupt diacritization models and missing tokenizer assets with errors and no success report. The adapter preserves upstream's flattened class-logit layout; treating it as one logit per character would reject the actual model. Strict Clippy, formatting and the retained experiment's six ordinary tests pass; its two cached-model checks remain ignored in that ordinary suite.

Both the actual probe graph and a private copy of the full app's Linux shared-TTS/embedding manifest resolve with one ORT/ort-sys `rc.13` pair and registry Libtashkeel with no enabled features. The copied manifest removes the old direct `ort-sys` dependency/`load-dynamic` feature, which is unavailable in `rc.13`, and delegates loading to `ort`. **This is dependency resolution, not an app build or mobile validation.** App manifests, preprocessing/synthesis source, unrelated editor changes, upstream source/model and cached inference assets remain unchanged.

The model was compared byte-for-byte with the cached published crate archive, whose package checksum matches the new lockfile. It is 4,788,213 bytes; its digest is recorded in the compatibility README. No upstream source or model blob is added to the repo. Because the built-in backend privately embeds those bytes, production adoption still needs explicit integrity-checked provisioning and attribution of that unchanged asset. A caller-supplied engine avoids a fork but remains Papercut integration code that needs compatibility checks when updating dependencies or the model.

**Next:** validate a concrete model-provisioning path and app-level dynamic/static feature builds before adopting this adapter. Keep platform claims bounded to actual checks. Optional enhanced search remains feasible as Stage 7 opt-in functionality with lexical fallback; a UI toggle does not solve dependency conflicts, and choosing another encoder requires a compatible index rebuild. Begin with a validated model, then one stronger alternative only if its measured quality justifies its resource costs. Independent representative quality and reference hardware remain open; this pass changes neither ranking nor the model selection decision. Interactive search/EPUB and audio smoke stay deferred to the consolidated final pass requested by the user; remind then and carry the pending checks through subsequent passes.

## Verified bundled asset / no-fork app adapter — 2026-10-07

**Decision: adopt the tested public-interface route for native preprocessing.** The app now depends on official `libtashkeel_base` 1.5.0 with default/ORT features disabled and ORT `rc.13`, without a direct `ort-sys` dependency or Libtashkeel patch. The existing dynamic/linked initializers retain recoverable setup errors. The app's [small adapter](../src-tauri/src/native_tts/engine/tashkeel.rs) owns tensor conversion and a serialized session; upstream still owns linguistic processing and character maps. The standalone compatibility executable now imports that exact adapter instead of maintaining a second implementation. The TTS feature selects API 17; the prospective embedding dependency can enable its higher API requirement separately. FastEmbed is not yet an app dependency.

The unchanged model from the checksum-verified published package is [retained as an attributed artifact](../src-tauri/tts/libtashkeel-1.5.0/README.md), not a vendored package. Native preprocessing builds verify its size and SHA-256, then compile it into the binary/library. This replaces the upstream backend's existing embedding of the same 4,788,213-byte asset. It adds no model download, Cargo-cache lookup, separate process or model resource-path dependency. Builds without preprocessing do not embed the model. The original MIT notice is retained and configured as a bundle resource. Upstream Rust updates remain ordinary dependency changes with compatibility checks; changing model/maps additionally requires matched artifacts and a new preprocessing ID when synthesis behavior changes. Current model/maps and verified outputs retain the existing preprocessing ID/cache policy.

Validation completed offline using cached assets:

- **194 native shared-TTS library tests pass**, including the new malformed-input/corrupt-ONNX/shared-call regression. Six opt-in tests remain ignored in that ordinary suite. The cached-Arabic speech check is explicitly run separately and passes with finite non-silent audio.
- **146 default-feature library tests pass**, with four opt-in tests ignored. Sandbox-denied local-server tests are rerun with local socket access; no test failure remains.
- The linked initializer compiles and passes **three preprocessing tests on Linux** against the existing shared runtime. This exercises that Rust code path; it is not iOS compilation, a static-archive link or device execution.
- All **four FP32/INT8 startup-order combinations** pass through the actual app adapter and newly retained model. Six Arabic outputs match the original official-backend baseline, embeddings are stable and one native ORT image is loaded per process. Strict compatibility-check Clippy and Rust formatting pass.
- Build verification rejects **missing**, **wrong-size** and **same-size corrupted** model assets before compilation. The published model checksum is preserved.
- The ordinary production desktop command builds a **Linux Debian package** successfully, including frontend type checking, locale checks and Vite build. Archive inspection finds exactly **one unchanged model payload** in the executable, exactly **one native ORT runtime**, and the matching original license at `licenses/libtashkeel-1.5.0.txt`; all three packaged native runtime libraries match the prepared inputs. No installation or interactive app run is performed.

The locked shared-TTS graph contains one ORT/ort-sys `rc.13` pair and official Libtashkeel with no enabled features. Only ORT-related package versions change; no broad dependency refresh is retained. Existing build helpers/package scripts and search ranking/index/UI behavior are unchanged. Other desktop targets, Android and iOS still need their own native build/runtime evidence. Automated synthesis is not listening-quality acceptance. This pass establishes the Linux app prerequisite, not a held-out semantic retrieval win.

**Next:** prepare the smallest desktop opt-in provider/model lifecycle prototype, reusing verified downloads and job controls, with lexical search available throughout and explicit compatible-index rules. Keep it an experimental path until quality/resource gates justify enabling hybrid results; do not build a model catalogue or choose the smallest encoder globally. Carry all deferred search/EPUB/navigation and audio listening/save/play checks into the user's consolidated final smoke pass, including later stages, and remind then. The release gates for representative relevance, reference hardware and other supported platforms remain open.

## Shared default-off provider lifecycle prototype — 2026-10-07

**Decision: retain an explicitly compiled provider prototype; production hybrid
search remains on hold.** Desktop-first is validation sequencing, not a permanent
product exclusion of Android/iOS. The original Stage 7 plan explicitly calls for
validating desktop first and supported mobile targets separately. Opt-in remains
appropriate for model downloads, indexing and device resource costs; it does not
require imposing the smallest model on all users.

The app now has shared native capability/configuration commands and session-only
state. No startup model read/load, automatic download, UI preference, semantic
index or search ranking is added. Normal builds omit FastEmbed and report the
provider uncompiled. Explicit experimental dynamic/linked features use FastEmbed
7.1.0 without download, hub, image or Candle features, enabling ORT API 24 only
when selected. Ordinary TTS retains API 17. Both paths reuse one initializer;
official Libtashkeel has no features/override and no fork. The lockfile adds only
the optional provider graph; no existing package version is upgraded.

Activation accepts only the two pinned baseline formats in an app-owned cache.
Every model/tokenizer byte size and checksum is verified before native loading;
there is no arbitrary caller path or mutable model URL. A valid normalized
384-dimensional warm-up is required before publishing a provider. Only one load
can run. Same-format activation is idempotent, while a different format requires
explicit disable first and leaves the working provider intact when rejected.
Disable cancels pending activation and releases an active model/session; a native
setup already in progress must finish before its cancelled session is discarded.
The shared ORT environment remains available to TTS. Inputs cannot silently
truncate beyond 512 actual tokens, and encoder identity includes the exact ONNX
checksum. This is not yet a complete semantic-index compatibility key.

**Resource/stability limits:** an early concurrent feature-build/test attempt
exited with native `SIGSEGV` during the initial hot-switch check. Its cause was
not established. The hot-switch path is removed as unnecessary; five isolated
runs of that earlier path and one debugger run passed, and five runs of the
retained disable-first lifecycle passed, including cancellation after native
setup. These results do **not** establish the crash's root cause or release
stability. Keep native stability investigation open before exposing model controls
for release. Per-format direct native-test peaks are about **1.42 GiB FP32** and
**846 MiB INT8** on this host; the full retained cached check peaks near 1.42 GiB.
The tests read/hash/load assets and exercise inference/unload, and the INT8-only
check also constructs a second, cancelled session sequentially. These are
whole-test process peaks excluding compilation, not full-app budgets, cold-disk
measurements, per-query latency or mobile acceptance. The tested INT8 export is
specifically AVX512/VNNI-named; portability is unverified. Removing overlap does
not make FP32 loading cheap or demonstrate a substantial peak-memory reduction.

Detailed cache layout, commands, artifact provenance, bounds and reproducible
checks are in [the provider notes](search-embedding-provider.md). Downloads and
settings UX remain a bounded follow-up; do not expose hybrid ranking on the
strength of these loader checks. Independent representative relevance,
reference-device resources and all advertised-platform builds remain release
gates. Interactive search/EPUB/navigation/audio and later model controls remain
deferred to the user's consolidated final smoke pass; carry the reminder forward.

Validation on the final implementation:

- **147 default-feature tests pass**, four ignored; **195 normal shared-TTS tests
  pass**, six ignored; **197 provider-enabled shared-TTS tests pass**, seven
  ignored. Existing local-socket tests run with socket access.
- Three ordinary provider checks cover default-off capability/unknown format,
  missing/wrong-size/changed assets and cancellation, duplicate activation and
  error/panic cleanup. The ignored cached check is explicitly exercised for FP32
  and INT8: normalized English/Arabic vectors, repeat stability, no silent token
  truncation, rejected format switching, unload/restart and cancelled publication.
- Actual app cached embeddings, official Arabic preprocessing and finite
  non-silent speech pass together in one native process for both runtime
  initializer configurations. Each Linux provider check sees one ORT image.
  The linked configuration uses the cached Linux shared library through linked
  symbols, not an iOS static archive/device. Provider-only builds/checks also pass
  without enabling TTS. Deliberately removing runtime discovery returns a loader
  error, and restarting with the existing runtime succeeds.
- Rust formatting and diff checks pass. Clippy completes with existing warnings
  in untouched modules; none point to the new provider/initializer or modified
  preprocessing module. No unrelated lint cleanup is added.
- The normal desktop locale/TypeScript/Vite/build pipeline and Debian package pass;
  release compilation takes **41.17 seconds**. The **41,698,614-byte** package
  contains the unchanged preprocessor once, its exact MIT license and one matching
  ORT library alongside the sherpa libraries. Its SHA-256 is
  `1ae64bec6c32298b04cba9555fce364fde01bea74e03f6e39a2ef9b60b4702c9`.
  Normal-build dependency metadata excludes FastEmbed and retains the API 17
  wrapper; the experimental graph has one ORT/ort-sys pair with API 24. No
  application launch, package installation, model download or installed-library
  mutation occurs. User editor settings, root fixtures and memory remain excluded.

**Next:** investigate the remaining native stability/resource concern before
release-facing model controls, then add the smallest verified cancellable
staging/download and opt-in settings path. Keep semantic ranking/index integration
behind the original quality, compatibility, resource and platform gates. No
interactive smoke is required now; remind the user when the consolidated final
search/EPUB/audio/model-controls pass is ready.

## Native stability follow-up — atomic build copies and measured retention

The bounded stability/resource investigation found a concrete Unix build hazard:
the sherpa build script overwrote shared-library files in Cargo profile output
directories while a running development app could still map those files. It now
reuses atomic staged-copy/rename, with process-specific staging names and failed
copy cleanup. Local archive copies share this helper; Windows DLL copying remains
unchanged. A stdlib-only runnable regression preserves the old open inode, checks
new destination bytes and failed-copy preservation. Its old direct-copy negative
control fails. A separate Linux mapped-file truncation control receives `SIGBUS`,
while atomic replacement preserves the mapping.

**This does not establish the earlier `SIGSEGV` cause.** That test appears to have
loaded the original runtime directory, outside the overwritten Cargo profile
copies. No original crash backtrace exists. Five retained FP32/INT8 lifecycle
runs with actual Arabic preprocessing/speech together pass without recurrence,
with one ORT image, cancelled native setup and unload checks. These remain bounded
development-host checks, not release stability or mobile acceptance.

New measurements distinguish load peaks, active-provider RSS and post-disable
RSS. FP32 reaches about 1.42 GiB, stays near 997 MiB after warm-up and near 613 MiB
after disable; INT8 stays near 663 MiB after warm-up and 551 MiB after disable,
with a whole-check high water of 846 MiB including a second cancelled load.
Combined embedding/TTS processes peak near 1.52 GiB. A separate debugger-only
glibc trim control reduces post-disable FP32 RSS from 614 MiB to 50 MiB. This
demonstrates substantial allocator retention in that run. No allocator hook,
dependency fork, lower-quality model default or new runtime is introduced.
Device memory pressure, repeated-load behavior and responsiveness remain gates.
Measurement methods, limits and the regression command are in
[the provider notes](search-embedding-provider.md#native-stability-and-memory-follow-up).

Validation: the copy regression passes with warnings denied; **195 normal
shared-TTS tests** and **197 provider-enabled shared-TTS tests** pass. Cached
normal-build Arabic preprocessing/speech also passes. Formatting and diff checks
pass. The normal offline desktop build/package passes, including its existing
locale/TypeScript/Vite steps. The package contains one unchanged preprocessing
model, its exact MIT notice, one matching ORT image and matching sherpa libraries;
no embedding weights are included. No application launch, installation, model
download, installed-library mutation or push occurs.

**Next:** prepare verified cancellable model staging/download and cache-removal
as an experimental provider follow-up, then opt-in settings. The earlier crash,
device resources, representative independent relevance and advertised-platform
validation still gate release controls and production hybrid integration. All
interactive search/EPUB/navigation/audio/model-control checks remain deferred to
the user's consolidated final smoke pass; remind the user when that packet is
ready.

## Explicit embedding-cache download and removal prototype

The next bounded provider pass adds backend commands for cache presence, explicit
pinned downloads, cancellation and removal. The installer and native loader now
share one five-asset manifest and a bounded streaming size/SHA-256 checker; the
existing encoder identities, model snapshot and inference policy are retained.
Downloads never activate the provider or change search ranking, indexing or
preferences. Normal builds still omit FastEmbed. The existing optional reqwest
client is reused; no package versions, new dependencies or voice-catalogue entries
are introduced.

Before writing, the installer reuses the existing free-space check for the full
payload plus 64 MiB reserve. It writes and verifies a sibling staging tree, syncs
files and publishes the complete directory under the same state lock that
arbitrates cancellation. Failed/cancelled work removes staging; interrupted work
is restarted on the next explicit request. A valid existing cache is verified
without fetching again. A corrupt/incomplete final cache is preserved and needs
explicit removal before redownload. No resumable protocol or automatic retry is
added. Removal is idempotent and refuses pending operations or an active selected
model. Cache presence is size information, not a claim of fresh integrity,
relevance quality or platform readiness.

The download busy slot blocks activation and removal until cleanup returns.
Disabling also cancels a pending download; dedicated download cancellation leaves
an already active provider alone. HTTP errors, bad lengths/checksums, truncated
bodies and offline failure recover without publishing incomplete files. Network
cancellation is cooperative between reads, with connection/idle-read deadlines;
it is not instantaneous. Backend progress events are ready for later settings
consumption. See [the command contracts](search-embedding-provider.md#explicit-downloads-and-cache-removal).

Validation: **147 default tests**, **195 normal shared-TTS tests**, and **200
provider-enabled shared-TTS tests** pass. The six ordinary provider/cache checks
also pass without TTS. Three new cache regressions cover staged integrity,
stream/promotion cancellation, reuse/corrupt-cache preservation, interrupted work,
storage preflight, duplicate work, activation/removal exclusion, panic cleanup,
local HTTP status/length/checksum/truncation/idle timeout, HTTPS-only policy and
offline errors. The cached FP32/INT8 lifecycle and actual Arabic preprocessing/
speech checks pass together; active-model removal is refused before touching the
cache. The local HTTP check caught a missing timer context in the initial blocking
idle-timeout implementation; the shared fetch boundary now supplies it.
Production Clippy and standalone-provider test Clippy complete with existing
warnings and no findings in the changed modules. The combined test-lint command
still stops at the existing voice-installer fixture's unchecked read in
`native_tts/engine/model.rs`; that unrelated fixture is unchanged. Formatting and
diff checks pass. No live upstream weight download, application launch,
installation, installed-library mutation or push occurs.

The normal offline desktop locale/TypeScript/Vite/build and Debian package also
pass. Package inspection confirms the unchanged preprocessing model and MIT
notice, one matching ORT library and matching sherpa libraries, with no embedding
assets bundled. This does not exercise the new commands through app IPC or a
live Hugging Face/CDN download; those checks remain in the final smoke packet.

**Next:** opt-in model-setup settings for controlled prototype testing: consent,
size/capability information, progress, cancel/remove and recovery. Do not label
setup as working conceptual search while the provider has no ranking integration.
The earlier native crash is still an unexplained observation, not a proven fluke
or resolved cause. Stability, device resources, independent representative quality
and advertised-platform validation remain release gates. Interactive search,
EPUB/navigation, audio and future model controls remain in the user's consolidated
final smoke pass; carry the reminder forward.

## Opt-in prototype model-setup settings

The shared native cache controls are now available in **Settings → Developer
mode** in Tauri builds. Normal builds report model setup unavailable and still
omit FastEmbed. Explicit provider builds offer FP32 and the existing smaller INT8
export, with model-file size, disk reserve, measured native-test memory costs and
unverified INT8 CPU/mobile compatibility. This is preparation for controlled
prototype testing, not a conceptual-search switch or platform release claim.

Opening settings only subscribes and reads capability/cache status. Downloads or
cache verification require an unchecked consent box naming Hugging Face plus an
explicit action. Format changes and finished attempts clear consent. Presence by
size is labeled separately from verified integrity. Removal can clear an
incomplete cache or stale staging; active selected models and pending native work
block mutation. No new dependencies, model catalogue, native command, build/dev
script, activation preference, semantic index or search integration is added.
All eight locales have translated setup text.

The panel subscribes before its first status read, refreshes on download lifecycle
events and polls only while work is pending. Duplicate clicks cannot enqueue
another local job. Cancellation remains pending until native cleanup returns.
Closing settings or hiding Developer mode releases listeners/timers without
cancelling the native job; reopening restores native status. Failed subscriptions
can be retried, native errors remain visible and explicit removal/redownload
provides damaged-cache recovery. Observed terminal failure/cancellation events
also inform a reopened panel. There is no persistent job/error history, so
reopening after completion shows current cache presence rather than a claim about
an earlier attempt.

Validation: **147 frontend tests**, including five setup regressions, pass.
The new checks exercise unavailable normal builds, consent/format resets,
duplicate-click rejection, cancellation through cleanup, downloads starting while
settings are open, pending-work recovery after reopening, unrelated progress,
active-cache removal refusal, explicit verification, failure/retry and delayed
listener cleanup. TypeScript, ESLint and eight-locale parity pass. The offline
provider-enabled desktop release build and normal Debian package also pass.
Package inspection verifies the unchanged bundled preprocessing model, exact MIT
notice, one matching ORT and matching sherpa libraries, with no embedding assets.
These mocked hook/control checks and compilation do not establish actual WebView/app IPC, keyboard/RTL,
upstream downloads, native release stability or mobile/device acceptance. The
unchanged backend's previous native results are documented in the prior pass;
they are not presented as newly rerun tests here.

**Next:** validate the native command/cache lifecycle in disposable app data and
prepare the consolidated setup/search/EPUB/navigation/audio smoke packet. Actual
WebView and live-download smoke remain deferred at the user's request, alongside
independent representative quality, reference-device resources and supported-
platform release checks. Stage 7 ranking integration stays on hold until those
gates justify it; do not treat prototype setup availability as acceptance.


## Native command/cache validation and consolidated smoke packet

The six existing embedding commands now have Linux mock-runtime IPC checks using
Tauri's actual command macros and JSON serialization. The path/cache handlers
accept the standard Tauri runtime generic rather than being fixed to Wry; normal
app behavior is unchanged. The existing Tauri dependency's `test` feature is
enabled only for development tests. Production dependency trees omit it, and
there are no package-version, runtime, model-policy or search changes.

An ordinary check verifies build capability, camelCase status, rejected format
arguments, explicit disable/cancel and recoverable feature-off errors. An ignored
real-cache check requires a canonical disposable data root below the temporary
directory, matching `XDG_DATA_HOME`, a fresh test app identifier and explicitly
cached INT8 assets. MockRuntime creates no browser profile: its profile path
points at the existing temporary directory. The test copies source assets rather
than linking/mutating them and removes its own app-data tree.

Through the actual handlers it checks metadata without directory creation,
missing/corrupt activation, complete-by-size files versus integrity, failed-cache
preservation, progress field/phase serialization, verified cache reuse without
fetching, activation, active-cache removal refusal, removal of another format,
disable, fresh-state cache reuse without automatic activation, idempotent removal
and interrupted-staging cleanup. A held test reservation verifies both cancel
commands retain busy state; the previous real-worker regression separately
covers cleanup completion. This is not a live interrupted transfer or process
restart test, and the scoped handler registry does not exercise full application
startup/plugins or WebView delivery.

Validation: **148 default**, **196 normal shared-TTS** and **201 provider-enabled
shared-TTS** native tests pass, plus **147 frontend tests**. The explicit command
cache lifecycle and actual Arabic preprocessing/speech checks pass together in
one process; no native crash recurs. Production Clippy and standalone-provider
test Clippy complete with existing warnings and no findings in the changed
modules. Formatting and diff checks pass. No live download, GUI launch,
installation, library mutation or push occurs. The earlier unexplained native
crash, allocator retention, quality/device/platform gates remain open.

The offline provider-enabled Wry release build and normal desktop Debian build
also pass. Inspection of the normal package verifies the unchanged bundled
preprocessing model, exact MIT notice, one matching ORT and matching sherpa
libraries, with no embedding assets. Neither built application was launched;
build/package validation does not establish interactive or platform acceptance.

The [consolidated smoke packet](search-v2-smoke.md) is prepared, **not run**. It
covers isolated launch (including the single-instance redirect hazard), complete
lexical queries/quotes/filters/evidence/navigation, imports/replacement/deletion,
model setup/download/cancel/offline/restart/removal, RTL/keyboard, audio and
foreground responsiveness. It references generic examples rather than private
source/question details. All interactive checks remain deferred to the user's
one final pass; preparation does not count as acceptance.

**Next:** collect actual smoke and release-gate evidence rather than add another
speculative feature. The user may keep smoke deferred; the checklist is ready
when they resume. Stage 7 remains on hold until representative independent
quality, compatible indexing/fallback, reference-device resources and supported-
platform/native lifecycle evidence justify it; a documented no-go remains valid.

## Acceptance checkpoint — 2026-10-07

**Decision: retain the model-free release candidate and experimental baseline;
do not begin production hybrid ranking.** Rechecked the original handoff's
Stage 6 go/no-go and Stage 7 acceptance requirements against the retained code,
reports and build paths. This is a documentation/validation pass, not a new
retrieval policy, model comparison or release approval. The user again chose to
keep interactive smoke deferred.

Fresh validation on `47c2067` uses the unchanged version-5 generic fixture and
actual native search path: **41 requests**, **25 documents / 54 sections**,
document Recall@10 **41/45**, MRR@10 **0.917** and nDCG@10 **0.912**. All previously
retrieved baseline labels remain in the top ten. The six Arabic and two mixed
fixture requests retain their labeled documents; four passage-labeled requests
retain all five passages. These familiar, small, incompletely judged fixtures
are regression evidence, not representative or independently held-out quality.
Debug native warm p95 is **2.29 ms**; fixture preparation warms the filesystem,
and IPC/rendering are excluded. Do not compare that figure to the earlier
release-mode 100,000-section probe or claim a new performance improvement.

The two evaluation-parser checks, pooled-review self-check and **six retained
experimental checks** pass. The latter cover document fusion/deduplication,
scope/vector compatibility, evidence fallback, quantization policy, SQLite
restart/update/rollback and UTF-8 source loading; two cached-model checks remain
ignored in this run. For a strict-offline rebuild of the standalone experiment,
set `ORT_LIB_PATH` to its already-cached static ORT library directory as well as
`CARGO_NET_OFFLINE=true`: Cargo's offline flag alone does not configure native
linking. The app's shared-runtime path is separate and unchanged. No model is
loaded/downloaded or index rebuilt here. Previous native suites and package
checks remain previous-pass evidence; no app rebuild is needed for these docs.

| Acceptance item | Status and next evidence needed |
| --- | --- |
| Model-free behavior | Automated regressions and development-host scale checks pass. Complete the [consolidated smoke packet](search-v2-smoke.md), including quotes/long queries, source navigation, EPUB/RTL and listening/save/play, before product acceptance. It remains deferred, not passed. |
| Representative semantic quality | **Open.** The retained independent sheet has 22 requests and 195 document candidates; all document/evidence grade fields are still null. Assistant grades remain provisional development diagnostics. Judge a representative frozen set independently, then compare the same queries/pool/filters at document and displayed-passage scope; do not treat null grades as irrelevant. |
| Relevance failures | **Unresolved.** Prior controlled cross-language/entity/date and missing-answer-span failures remain. Do not tune another policy against these familiar examples; use independent results to justify a bounded change or one alternative model. |
| Product index/fallback | **Not implemented.** Incremental SQLite/source-span behavior passes in the isolated experiment; the app has no semantic index or semantic query path. Setup downloads do not close compatibility, deletion/reimport, partial coverage, quote/filter/evidence or fallback acceptance for future hybrid results. |
| Native/resource lifecycle | **Partial Linux evidence.** Cached command, speech and build checks pass. Actual WebView/download/interruption/restart and long-lived responsiveness remain untested. The original native crash is unexplained; allocator retention and reference-device memory/CPU/battery checks remain open. |
| Advertised platforms | **Open.** This host has only the Linux Rust target. Existing desktop/Android/iOS build scripts and CI use normal TTS features; their presence is not a completed build, device result or provider-feature test. Validate each shipped target and explicitly compiled provider configuration before claiming availability. |

**Next:** collect acceptance evidence rather than schedule another feature pass.
When the user resumes, start with the consolidated app smoke; obtain normal-build
platform results and representative responsiveness for the model-free candidate.
Reopen Stage 7 only when independent relevance and compatible product-lifecycle,
resource/platform evidence justify it. Continuing with model-free search and
leaving the provider experimental is a valid outcome of the original plan.

## Stage 6 — frozen multilingual fact controls (2026-10-08)

**Decision: retain the baseline as an experiment; Stage 7 remains on hold.**
At the user's request, the assistant completed provisional grading of the prior
small review batch: four questions, eight document cards and nine evidence
variants. English conceptual, Arabic causal and exact Arabic quotation controls
had direct first results; the cross-language question returned unrelated
entity/purpose results. That assistant-authored and familiar subset is neither
independent grading nor an untouched held-out evaluation. Its separate grades
do not fill the original independent sheet's null fields.

This pass freezes fresh private inputs and exact source-answer clauses before
retrieval: **24 short documents / 48 sections and 46 requests per mode**. The
requests comprise 32 positive fact controls, eight keyword controls, four
present/absent quotation controls and two unavailable-fact controls. Four facts
cover entity/date and causal distinctions in English and Arabic, with related
wrong-purpose and background neighbors. Each positive is paired across query
language and fixed source scopes. Single-language scopes contain 12 documents;
mixed-language scopes contain 23, excluding the target's correct translation
counterpart so that it cannot rescue a cross-language result. These authored,
translated and scoped pairs are correlated controls, not 32 independent facts.
No private source text, query examples or identities enter tracked files.

The existing production parser/native evaluator prepares a disposable library
and exports the same anchored corpus for All words and Broader. The unchanged
release experiment loads the retained cached FP32 and dynamic INT8 assets,
sequentially, with separate fresh SQLite vector indexes. Each initially embeds
48 passages; each Broader run reuses all 48 with zero new embeddings. No model,
token/prefix policy, passage selection, fusion rule or product behavior changes.

| Main positive controls (32 per row) | Correct document first | Correct document in top 10 | Full answer clause in first result's displayed evidence | Full answer clause in top 10 evidence |
| --- | ---: | ---: | ---: | ---: |
| Native All words | 0 | 0 | 0 | 0 |
| Native Broader | 16 | 16 | 16 | 16 |
| FP32 semantic, either mode | 24 | 31 | 18 | 25 |
| INT8 semantic, either mode | 22 | 32 | 18 | 28 |
| FP32 hybrid with All words | 24 | 31 | 18 | 25 |
| INT8 hybrid with All words | 22 | 32 | 18 | 28 |
| FP32 hybrid with Broader | 24 | 31 | 24 | 31 |
| INT8 hybrid with Broader | 22 | 32 | 22 | 32 |

Displayed support requires the target document and its full frozen answer clause
in a lead or expanded source span, including separately attached semantic
context. This strict clause-coverage diagnostic does not grade every partially
useful or related result as irrelevant. Inspection of the missing spans finds
introductory material rather than the decision/answer section; adding the
currently retained expanded semantic evidence does not recover those clauses.
Broader hybrid preserves native excerpts and recovers every returned target's
answer in these controls. It does not repair the cross-language ordering.

In target-language-only scopes, FP32 puts all eight cross-language targets first;
INT8 puts six first, with all eight in the top ten. With mixed-language
competition, **both formats put zero of eight cross-language targets first**:
nearby documents in the query's language displace the correct foreign-language
source, including explicit wrong-purpose/exclusion material. FP32 retains seven
of eight targets in the top ten; INT8 retains eight. Both put all 16 same-language
targets first. This controlled scope comparison supports a competition weakness;
it does not establish a universal language bias or an INT8 quality win.

Native keyword controls retain all eight targets in both modes, six first;
hybrid places all eight first with full answer evidence. Present quotations
return the correct source; absent quotations return nothing in both modes, and
all quoted requests remain excluded from semantic/hybrid retrieval. Both
unavailable-fact controls still receive semantic neighbors without an answer:
the baseline ranks sources and has no answerability/abstention contract. Native
All words returns none; Broader returns literal neighbors for one. These are not
generated answers or evidence of a broken lexical clause contract. Natural
questions containing absent required words also explain the empty All words
positive results; this diagnostic does not justify silently relaxing that mode.

Validation matches all **184 embedded native responses** to the native reports,
checks **3,360 semantic source spans** against their UTF-8 bytes, verifies scope
and document deduplication, and preserves **236 native hybrid evidence objects**
unchanged. Input, answer-key, corpus-export and source-library hashes remain
frozen. The standalone strict-offline release rebuild and both native evaluator
runs pass. This path does not exercise file extraction, app import, WebView,
mobile/shared runtime, weak hardware or independent product-quality acceptance.

**Next:** the original Stage 6 plan permits stopping or comparing one justified
alternative encoder when gains are weak. If continuing semantic work, compare
one compatible multilingual alternative against these unchanged controls and
the retained broader packet, with resource costs and separate document/evidence
results; do not tune word exceptions, language boosts or thresholds to these
answers. Treat these controls as development evidence, then obtain representative
independent quality before any Stage 7 go decision. Interactive app smoke remains
deferred to the user's consolidated final pass; the [checklist](search-v2-smoke.md)
is ready. Reference-device, supported-platform and native lifecycle gates remain
open alongside the original unexplained crash and allocator-retention concern.

## Stage 6 — one alternative encoder comparison (2026-10-08)

**Decision: keep both candidates in the isolated experiment; no production
promotion, and Stage 7 remains on hold.** Compare one alternative rather than
start a model leaderboard: retained E5 Small FP32 against E5 Base FP32. Base
recovers a known cross-language answer in the broader packet, but does not
establish a net quality gain against the frozen controls or justify its higher
resource cost. No app search, model manifest, download UI, dependency, ORT or
Libtashkeel change is included.

The alternative is the official MIT-licensed `intfloat/multilingual-e5-base`,
pinned to revision `d128750597153bb5987e10b1c3493a34e5a4502a`. Its [official
ONNX assets](https://huggingface.co/intfloat/multilingual-e5-base/tree/d128750597153bb5987e10b1c3493a34e5a4502a/onnx)
were verified against upstream object hashes and sizes. This is a 768-dimensional
FP32 encoder using the [model card's](https://huggingface.co/intfloat/multilingual-e5-base)
query/passage prefixes, mean pooling and L2 normalization. Comparing FP32 with
FP32 avoids combining an encoder change with a quantization change. The Base
ONNX file is 1,110,059,084 bytes, versus Small's 470,268,510 bytes. Assets and
private source/query records stay outside Git.

The minimum comparison adapter reads `hidden_size`, accepts only 384 or 768,
and validates finite unit vectors, JSON caches and SQLite model/dimension
compatibility. Existing Small identities and stored indexes remain compatible;
changing model requires a separate index. The app has no Base provider or
semantic index. The standalone experiment retains its existing two-thread,
320-token passage / 40-token overlap, source evidence and fusion policies.
No language boosts, score thresholds or word exceptions were tuned.

The same frozen **46 requests per native mode** and 24-document / 48-section
fact corpus are reused. Each encoder embeds 48 passages once, then reuses all 48
for Broader with zero new embeddings. The 32 positive controls remain four
paired facts across languages and scopes, not 32 independent questions.

| Main positive controls (32 per row) | Correct document first | Correct document in top 10 | Full answer in first result's displayed evidence | Full answer in top 10 evidence |
| --- | ---: | ---: | ---: | ---: |
| Small semantic, either mode / hybrid All words | 24 | 31 | 18 | 25 |
| Base semantic, either mode / hybrid All words | 22 | 32 | 14 | 22 |
| Small hybrid Broader | 24 | 31 | 24 | 31 |
| Base hybrid Broader | 22 | 32 | 20 | 28 |

Both put all 16 same-language targets first. In target-language-only scopes,
Small puts all eight cross-language targets first; Base puts six first. With
mixed-language competition, both still put **zero of eight** first; Base retains
eight in the top ten versus Small's seven. Base can select an introduction
instead of the answer section even when it retrieves the correct document.
The full frozen answer-clause diagnostic separates that evidence failure from
document recall; it does not classify every partial passage as irrelevant.
Quoted requests remain lexical-only; native responses, explicit scopes and
primary hybrid evidence remain unchanged. Unsupported questions still retrieve
neighbors without an answerability/abstention guarantee.

The retained broader packet has **37 documents / 3,442 source sections / 22
requests**. Small makes 4,389 passages and Base 4,379: each uses its own tokenizer
under the same token limits. Small reproduces its prior model identity and all
22 native/semantic/hybrid result and evidence records exactly. Of 20 unquoted
requests, 19 retain the same first document in each semantic/hybrid channel;
rank movement alone is not a relevance metric. The latest source-bound review
of four known cases retains the two direct first answers and the lexical-only
quotation. Both models still put an explicit wrong-purpose neighbor first for
the remaining cross-language question. **Base recovers its correct source and
explanatory passage at rank two; Small misses that source in the top ten.**
This is a useful bounded gain, alongside the fact-control regressions. These
assistant grades are familiar development diagnostics; the original independent
sheet remains unfilled, and the other requests are not silently graded zero.

| Larger-packet measurement | Small FP32 | Base FP32 |
| --- | ---: | ---: |
| Model load | 0.95 s | 2.42 s |
| Initial indexing | 323.85 s (5.4 min) | 1,060.49 s (17.7 min) |
| Query p50 / runner p95 | 13.80 / 17.26 ms | 35.51 / 40.90 ms |
| Peak native-process RSS | 1,448.11 MiB | 2,900.47 MiB |
| SQLite index bytes | 14,028,800 | 22,994,944 |
| Raw vector bytes | 6,741,504 | 13,452,288 |

These are sequential, single-run Linux native-child measurements on this
development host; OS caches were not cleared. They exclude WebView, app
responsiveness and shared-runtime/device acceptance. Base costs roughly 3.3×
initial indexing and 2× peak RSS/raw vector storage here. Unchanged passages
reuse vectors, but imports, edits, model changes and indexing-policy changes
still incur work; indexing is not a permanent one-time cost.

Validation: **six ordinary experimental checks and two actual cached Base
checks pass**, including both-dimension restart/update/delete/rollback, wrong
model/dimension rejection, bounded embedding order/resume, long sections and
UTF-8 source spans. Strict-offline release build, formatting and Clippy with
warnings denied pass. Comparison verification checks 4,064 semantic source
spans, 456 scoped/deduplicated result lists, 262 unchanged native primary hybrid
evidence objects and 20 quoted-request exclusions. Frozen input/key and official
model hashes match. No installed library was changed. This does not close the
original unexplained native crash or allocator-retention concern.

**Next:** while interactive smoke remains deferred, compare these same two
retained FP32 candidates on a small, fixed public human-labelled English/Arabic
retrieval subset. Freeze selection and labels before retrieval; keep public
benchmark sources and queries outside tracked files too. Do not add another
model or tune against the familiar controls. Public labels can supply an
independent relevance reference without requiring the user to know the subject
matter; they do not replace representative private-book, displayed-evidence,
reference-device, index/fallback or supported-platform acceptance. The
[consolidated app smoke checklist](search-v2-smoke.md) remains deferred to the
user's final pass and must be reminded then.

## Stage 6 — public human-label comparison (2026-10-08)

**Decision: retain Small as the practical experimental baseline and Base as
the already-tested alternative; Stage 7 remains on hold.** No app, encoder,
runtime, dependency, index policy or fusion change is included. This pass reuses
the existing native parser/store/evaluator and unchanged experiment; its tracked
change is this evidence record. No third model or word/threshold tuning.

The [official public development set](https://huggingface.co/datasets/miracl/miracl)
provides native-speaker binary passage judgments, including judged negatives.
Select **32 English and 32 Arabic queries** by fixed SHA-256 seed/query-ID order,
requiring positive and negative candidates. Selection, labels and source inputs
were frozen before retrieval; no subject or model-result selection. All official
judged passages for each selected query form its explicit candidate scope.
There are **648 distinct source passages and 653 query/passage judgments**.
Each benchmark passage is one experimental document with one body section;
these are passage rankings, not article/book judgments. Actual source/query
examples and benchmark files remain outside tracked files.

Topics/labels are pinned to revision
`5be20db9509754dadad47689368639fcec739c00`; corpus sources to
`d921ec7e349ce0d28daf30b2da9da5ee698bef0d`. The official English converted cache
was unavailable, so stdlib helpers verified required raw source files against
upstream SHA-256/byte counts, retaining only selected text and provenance receipts.
Temporary bulk downloads were removed. This dataset-access preparation scanned
5.11 GB of compressed sources; it is separate from measured embedding indexing,
not a production model-download cost. No parser dependency was installed.

One Arabic query contains an exact phrase and remains lexical-only. Comparisons
below cover the same **63 unquoted requests: 32 English and 31 Arabic**. Original
quoted native rows are retained separately, not counted as encoder misses.
Metrics use unchanged human labels: 1 relevant, 0 judged negative; per-query
macro nDCG uses binary gains. All candidates in each scope are judged for that
query; there is no unjudged-as-zero imputation or assistant replacement grading.

| Language / path | Small relevant first | Base relevant first | Small nDCG@10 | Base nDCG@10 |
| --- | ---: | ---: | ---: | ---: |
| English semantic-only (32) | 21 | 20 | 0.8206 | 0.7962 |
| Arabic semantic-only (31) | 21 | 24 | 0.8413 | 0.8724 |
| Combined semantic-only (63) | 42 | 44 | 0.8308 | 0.8337 |
| Combined hybrid All words (63) | 39 | 42 | 0.8132 | 0.8206 |
| Combined hybrid Broader (63) | 30 | 30 | 0.7202 | 0.7164 |

Combined semantic MRR@10 is **0.7946 / 0.8119** for Small/Base. Base's nDCG
changes are paired: **18 improvements, 15 regressions, 30 ties**, averaging
only **+0.0029**. Native Broader puts 27/63 relevant passages first, with MRR
0.5501 and nDCG 0.5667; native All words puts five first. Natural questions can
contain absent required words, so those All words misses do not justify relaxing
its lexical contract. Semantic ranking is useful within these controlled pools;
Base's mixed gain does not justify promotion or establish a universal model win.

**Fusion is the more actionable concern.** Broader hybrid changes Small's
positive semantic first results to judged negatives on 15 queries and makes
three reverse improvements; Base loses 19 and gains five. Every deterioration
promotes a judged negative appearing in both native and semantic lists. In
six/nine cases, the displaced Small/Base semantic winner is absent from native
retrieval. This is the current equal-weight RRF (`k=60`) policy behaving as
implemented, not an arithmetic bug. It needs retrieval-scale evidence before
changing the policy: median scope is ten, and fusion admits up to 50 candidates,
so nearly every scoped passage enters the semantic list. Agreement can then
reward literal neighbors even when human labels reject them.

Candidate pools range **10–11 English and 10–13 Arabic**, with **53/64 at most
ten**. Combined semantic Recall@10 is 0.9921 for both encoders, largely a ceiling
of these restricted pools, not full-corpus retrieval success. Whole-passage
human relevance also does not establish that a selected chunk/excerpt contains
the full answer. These public labels are independent of assistant judgments,
but the [E5 model card](https://huggingface.co/intfloat/multilingual-e5-base)
lists this dataset's training split in fine-tuning. The development comparison
is not a completely unseen-data, cross-language or private-book acceptance test.
The original private independent sheet remains unchanged and unfilled.

| Fresh initial index measurement | Small FP32 | Base FP32 |
| --- | ---: | ---: |
| Model-token chunks | 784 | 781 |
| Model load | 0.92 s | 1.82 s |
| Initial indexing | 60.92 s | 195.20 s |
| Query p50 / runner p95 | 6.04 / 7.43 ms | 18.94 / 25.26 ms |
| Peak native-process RSS | 1,446.34 MiB | 2,898.95 MiB |
| SQLite index bytes | 2,895,872 | 4,489,216 |

Both Broader runs reuse every vector with zero new embeddings. These are
sequential, single-run Linux native-child measurements with two inference
threads; inputs/indexes are in `/tmp`, and OS caches were not cleared. Base
costs about 3.2× indexing and 2× peak RSS here. This is not cold-disk, steady-state
memory, app/WebView, weak-reference-device or mobile acceptance.

Validation: production parser checks, disposable library preparation and both
64-query native evaluations pass. The scorer's ideal/reordered/empty/unjudged
self-checks pass, and its grades match official qrels and exact exported source
text. Verification covers **5,040 semantic UTF-8 spans, 504 fully judged result
lists, 988 unchanged native primary hybrid evidence objects and four quoted
encoder exclusions** across repeated runs. Input/export/model/binary identities
and cache reuse match. No installed library changed; existing experimental
unit/cached-model suites remain previous-pass evidence, not newly rerun claims.

**Next:** widen retrieval over this same frozen passage union using the same
queries, encoders and indexes, before changing fusion. Compare language-scoped
and mixed-language pools so the 50-candidate boundary has meaningful competition.
Report known-positive recall/ranks and judgment coverage; passages outside a
query's original judged pool are unjudged, not presumed irrelevant or scored
zero in nDCG. Use that result to decide whether one minimal fusion correction
needs a fresh labelled verification subset. No new model or tuning against this
familiar packet. The [consolidated app smoke](search-v2-smoke.md) remains deferred
to the user's final pass. Reference hardware/platform, original native crash,
allocator retention and product index/fallback gates remain open.

## Stage 6 — Frozen wider-pool check (2026-10-08)

Status: complete; **retain Stage 6 as an experiment and Stage 7 on hold**. Reuse
all 64 frozen public development questions, the exact 648 source passages,
original per-query human labels, Small/Base FP32 models and incremental indexes.
This pass changes retrieval scopes only: **324 passages in the query's language**
and **648 English/Arabic passages combined**, each with native All words and
Broader. The unchanged standalone runner queries both indexes read-only; no
new embeddings, fusion changes, model downloads or app integration are needed.
The only tracked change is this evidence record.

The original 653 query/passage judgments remain query-specific. Added passages
are **unjudged**, including passages judged for a different question; they are
never assigned a negative grade. One quoted Arabic request stays lexical-only,
leaving **63 common unquoted comparisons**. Known-positive Recall@10 measures
retrieval of the original labelled positives, and known-positive MRR@10 measures
their first rank. Neither estimates all relevant results in an incompletely
judged pool. Report positive/negative/unjudged/empty first results and top-ten
judgment coverage separately; do not calculate complete nDCG or precision here.

Both widened scopes produce identical semantic/hybrid top-ten identities and
source evidence for these requests. The table therefore applies to both:

| Path | Small positive first / 63 | Base positive first / 63 | Small known-positive Recall@10 | Base known-positive Recall@10 | Small known-positive MRR@10 | Base known-positive MRR@10 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Semantic-only | 42 | 44 | 0.9921 | 0.9921 | 0.7946 | 0.8119 |
| Hybrid All words | 39 | 42 | 0.9921 | 0.9921 | 0.7581 | 0.7841 |
| Hybrid Broader | 30 | 30 | 0.9540 | 0.9598 | 0.6463 | 0.6426 |

Every other encoder/hybrid first result is a human-judged negative; none is
unjudged. Semantic top-ten judgment coverage is **93.65% / 93.02%** for Small/Base,
with **40 / 44 unjudged positions out of 630**. Broader hybrid coverage falls to
**90.63% / 90.16%**, with **59 / 62 unjudged positions**. Unjudged results may be
useful; these counts are missing labels, not precision failures. Native Broader
has 26 positive, 35 negative, one unjudged and one empty first result; its known
Recall@10 is 0.7530 and known MRR@10 is 0.5393. Native All words has five positive,
seven negative and 51 empty first results, consistent with required-word matching
on these natural-language questions.

**The confirmed fusion losses persist.** Broader fusion changes 15/19 Small/Base
positive semantic first results to judged negatives, with three/five reverse
improvements. Every displaced winner is replaced by a negative present in both
native retrieval and the semantic top ten; six/nine positive semantic winners
are absent from native retrieval. Equal-weight RRF (`k=60`, up to 50 candidates
per channel) still rewards this agreement. This is a policy issue, not an
arithmetic defect, and it survives widening beyond the original ten-item pools.

Widening changes semantic top-ten lists on **17/20** Small/Base requests while
leaving each known-positive first rank and Recall@10 unchanged. Broader hybrid
lists change on **31/28** requests; known-positive first rank improves on two/one
and worsens on four/four, while Recall@10 falls on three/four. Thus added
candidates affect retrieval, and fusion can additionally displace known positives
below the first ten. The union still consists of selected human-judged passages,
including hard negatives; most top results remain from the original query pools.
This is a bounded scope-sensitivity check, not full upstream-corpus, private-book
or cross-language acceptance. Mixed-pool equality supplies no cross-language
relevance judgment. Whole-passage labels still do not grade displayed answer spans.

Validation: four 64-query production parser/native evaluations and eight
query-only encoder runs pass. Model/executable identities, original labels and
exact source text are verified; all source/index SQLite hashes remain unchanged.
Query-only inputs bind the original indexed corpus identity after verifying the
new exports have identical documents and source snapshot, while retaining the
new request/export hashes separately. Scorer self-checks preserve unjudged status,
empty lists and known-positive ranks. Verification covers **10,042 semantic UTF-8
spans, 1,008 partially judged result lists, 2,122 unchanged native primary evidence
objects, eight quoted exclusions and 252 language/mixed top-ten matches**.
Scorer/report replay is deterministic. No installed library was opened or changed;
previous unit/cached-model suites remain earlier evidence, not newly rerun claims.

Across four runs per model, query p95 is **8.17–9.44 ms Small / 25.47–26.89 ms Base**;
peak native-process RSS is **1,453.51–1,480.66 MiB / 2,906.39–2,932.74 MiB**.
These are sequential Linux query-only runs with two inference threads and uncleared
OS caches; no indexing cost is remeasured. They do not establish app responsiveness,
steady-state memory recovery, cold-disk, mobile or weak-reference-device readiness.
Private replay inputs, adapters, indexes, hashes, labels and reports stay outside
tracked files.

**Next:** compare one minimal semantic-first ranking control against unchanged
fusion on a freshly frozen human-labelled verification batch. Preserve native
source evidence, explicit scopes, quote exclusions and the model-free fallback;
this is an experimental ranking comparison, not a production contract change.
No weight sweep, third encoder or word exceptions. Decide whether that simpler
policy earns a quality gate before considering Stage 7. The user-approved
[consolidated rebuilt-app smoke](search-v2-smoke.md) remains deferred to the final
pass; remind the user then. Original native crash, allocator retention, reference
hardware, platform and product index/lifecycle/fallback gates remain open.

## Stage 6 — Fresh-query fusion verification (2026-10-08)

Status: complete; **retain Stage 6 experimental and Stage 7 on hold**. The
existing semantic-only output supplies the semantic-first ranking control;
there is no need to add another ranker to test that hypothesis. Compare only the
retained Small FP32 encoder with unchanged native retrieval and equal-weight RRF.
No model, fusion, application, runtime or dependency changes in this pass.

Freeze **16 new English and 16 new Arabic development questions** by SHA-256
seed/query-ID order before source preparation or retrieval. Exclude all 64
previous questions and require original official positive and negative judgments;
selection does not depend on subjects or search results. Reuse the pinned official
topics/labels and corpus revisions from the public-label comparison. The fresh
union has **326 passages and 328 query-specific human judgments**; four passages
overlap the earlier union with identical text, and 322 source texts are new.
Each request searches all **166 English or 160 Arabic passages** in its language.
One Arabic exact-phrase request stays lexical-only: common encoder comparisons
cover **31 requests (16 English, 15 Arabic)**.

Keep every original human label unchanged. Added candidates judged for another
question remain unjudged, not negatives. Known-positive recall/rank metrics only
measure the supplied positives; no complete nDCG or precision is imputed. These
are fresh queries for this experiment, not completely unseen model data or an
independent private-book/product acceptance packet.

| Path | Positive first / 31 | Known-positive Recall@10 | Known-positive MRR@10 | Judged top-ten coverage |
| --- | ---: | ---: | ---: | ---: |
| Native All words | 3 | 0.0565 | 0.0968 | 100.00% |
| Native Broader | 17 | 0.8833 | 0.7056 | 92.42% |
| Semantic-only | 27 | 1.0000 | 0.9274 | 96.13% |
| Hybrid All words | 26 | 1.0000 | 0.9032 | 96.13% |
| Hybrid Broader | 24 | 0.9758 | 0.8683 | 93.23% |

Semantic-only first results include four judged negatives; Broader hybrid includes
seven. Neither channel puts an unjudged result first here. There are **12 / 21
unjudged top-ten positions out of 310** for semantic-only/Broader hybrid; unknown
passages may still be useful. Native All words returns only seven top-ten positions
in total and is empty on 26 requests, so its 100% judgment coverage describes that
small returned set, not search quality. Required-word misses on natural questions
remain consistent with the literal contract.

**The Broader fusion concern reproduces on new questions.** It changes six
positive semantic first results to human negatives, with three reverse improvements.
Semantic-first known-positive MRR beats fusion on six requests and loses on four;
the mean difference is **+0.0591**. It also retains two positive top-ten passages
that fusion displaces, giving **+0.0242 macro known-positive Recall@10**.

| Language | Semantic positive first | Broader hybrid positive first | Semantic known-positive MRR@10 | Broader hybrid known-positive MRR@10 |
| --- | ---: | ---: | ---: | ---: |
| English (16) | 13 | 11 | 0.9063 | 0.8281 |
| Arabic (15) | 14 | 13 | 0.9500 | 0.9111 |

**Do not generalize this into a universal semantic-first policy.** English
All words hybrid improves positive first results to **14/16**, above semantic-only
13/16; Arabic All words hybrid falls to **12/15**, below semantic-only 14/15.
Native All words is the separate literal control; experimental semantic additions
do not redefine its required-word contract. Keep semantic-only ordering as the
conceptual reference and decline production promotion of automatic Broader RRF.
The selected passage union and high judgment coverage still limit generalization;
whole-passage relevance does not establish correct displayed answer spans, names,
dates, cross-language retrieval or full-library precision.

Validation: production-store disposable library preparation, both 32-query
parser/native evaluations and both Small encoder runs pass. Checks bind source
text, official judgments, exact scopes/requests, executable/model identities and
source SQLite hashes. The first run embeds **370 model-token passages**; Broader
reuses all 370 with zero new embeddings. Verification covers **1,240 semantic
UTF-8 spans, 124 partially judged result lists, 271 unchanged native primary
objects and two quoted exclusions**. Scorer checks preserve unjudged status and
empty-list behavior; grade/report replay produces identical hashes. Prior
unit/cached-model suites are earlier evidence, not newly rerun claims.

Initial indexing takes **29.46 s**, versus **0.010 s** for cache reuse; query p95
is **8.00 / 10.03 ms** and peak native-process RSS **1,447.38 / 1,453.04 MiB**
for All words/Broader runs. SQLite is **1,355,776 bytes**. These are sequential
Linux native-child runs with two inference threads and uncleared OS caches;
not app responsiveness, steady-state memory, weak-device or mobile acceptance.
Source preparation separately verifies 53 pinned raw shards totaling 4.04 GB
compressed, then deletes temporary bulk files. An initial connection reset was
recovered using per-shard checkpoints without changing selection; total retry
traffic was not measured. Benchmark preparation is not app indexing or model
installation cost. Private inputs, source checkpoints, labels, helpers, hashes,
indexes and reports remain outside tracked files; only this evidence record changes.

**Next:** verify evidence-preserving semantic-first result assembly inside the
isolated experiment, reusing the native evidence builder, with exact-name/date,
quoted-clause and scope regression checks. Preserve literal evidence/counts/source
navigation and the model-free control. A semantic-only ranking advantage does not
by itself prove that a differently ordered hybrid response preserves those fields.
No weight sweep or additional model. Complete that contract check before any
application integration decision. The consolidated rebuilt-app smoke remains
explicitly deferred to the final pass; remind the user then. Product index/fallback,
model lifecycle, weak reference hardware/platform, original native crash and
allocator-retention gates remain open.

## Stage 6 — Evidence-preserving semantic-first assembly (2026-10-08)

Status: implemented and contract-checked **inside the standalone experiment**;
Stage 7 remains on hold. Candidate reports now include `semanticFirstResponse`:
up to ten results in the existing semantic document order, assembled through the
same evidence builder as `hybridResponse`. The semantic and equal-weight RRF
controls remain unchanged. No new ranking heuristic, model, dependency, app
setting, native schema or production search path is introduced.

For a document present in native retrieval, its result object remains exact:
lead excerpt, marks, match fields, passages, section/page and navigation metadata
are preserved. Conceptual source context is attached separately as
`semanticEvidence`. Otherwise the result is the verified semantic source object,
with no invented literal counts. The shared bounded assembler rejects missing
source evidence. Quoted requests remain lexical-only and emit a null
`semanticFirstResponse`; existing scope validation and scoped vector ranking apply
before assembly. This changes experimental response order, not literal eligibility.

The existing external review helper pools this response as `semantic-first-hybrid`,
keeping ranks hidden and identical evidence deduplicated. Its comparison helper
also retains the optional INT8 response. Existing quote and source-context guards
apply to both new channels. Review grades remain null; packet creation is not
relevance grading. Use the existing command:

```sh
node scripts/search-eval.mjs --review /external/candidates.json /external/new-review
```

Validation replays both frozen 32-query Small reports against their existing
read-only index and adds twelve generic English/Arabic native contract cases:
names and differing years, mandatory quotes, single-document filters and rejected
quoted/name scopes. All **64 prior native/semantic/RRF query rows** retain exact
requests, retry objects, ranking and source evidence. Across those runs and the
contract cases, the new response passes **70 assembled queries, 648 UTF-8 source
spans, 260 native primary objects, 388 semantic-only objects and six quote
exclusions**. Source/library/vector SQLite hashes remain unchanged during the
final read-only replay. Review packets for all three reports generate successfully.
Seven ordinary Rust checks pass, including one new assembler check for ordering,
real native match-field shapes, navigation, the ten-result bound, empty lists and
missing-source failure. The two cached-model indexing/tail suites remain ignored;
actual cached-model query replay supplies this pass's inference check. Review
self-checks cover new-channel evidence parity, quote rejection, mismatched context
and INT8 comparison transport; no new INT8 inference result is claimed. Offline
release build, Rust formatting and diff checks pass.

Reused human passage labels give the new response **27/31 positive first results,
known-positive Recall@10 1.0000 and MRR@10 0.9274** in either native mode, matching
the existing semantic control. This is response/evidence preservation on the same
batch, not fresh held-out quality evidence or an answer-span relevance grade.

**Names/dates remain a product-contract boundary.** In two scoped controls, the
only allowed source discusses another year: native All words correctly returns
nothing, while semantic ordering returns that source as conceptual evidence.
The name/year fields and source text are not rewritten, and no literal match
metadata is fabricated. Passing assembly checks therefore does not establish
exact unquoted-name/date satisfaction. Keep All words strict; do not silently
substitute this response for it. No automatic name detector or word exceptions
were added. Quotes and document filters remain hard constraints.

A missing-model probe fails without publishing a semantic report; the independent
native evaluation still returns identical result objects afterwards. This checks
experimental failure isolation, **not an integrated application fallback**.
No installed library was opened, no model downloaded and no index format changed.
Private replay inputs, generic documents, source/index hashes, exact compiled
executable and reports remain outside tracked files.

**Next:** review the proposed conceptual-search contract against the existing
name/date and answer-evidence failures, and record a bounded go/no-go decision
before application integration. Treat semantic-first assembly as a candidate for
explicit conceptual retrieval, preserving model-free All words and Broader as
controls; successful serialization alone does not earn a Stage 7 gate. No further
weight or model sweep. Product index/fallback and optional-model lifecycle,
reference hardware/platform, original native crash and allocator retention remain
open. The consolidated rebuilt-app smoke is still explicitly deferred to the final
pass; remind the user then, and do not count it as passed.

## Stage 6 — Conceptual contract review (2026-10-08)

**Decision: retain the experimental candidate; no-go for production integration.**
Review the original Stage 6/7 plan, current literal app contracts, public passage
labels, and known entity/date, cross-language and answer-evidence failures.
Replay the existing semantic-first assembler on both frozen 46-request Small
FP32 fact-control reports, using the exact previously tested executable and
read-only index. No new policy, model, data selection, dependency or app code.

Document retrieval and displayed answer support remain separate measurements:

| Output / native evidence input | Correct first / 32 | Correct top ten / 32 | Full answer in first displayed evidence / 32 | Full answer in top-ten evidence / 32 |
| --- | ---: | ---: | ---: | ---: |
| Semantic-only, either native mode | 24 | 31 | 18 | 25 |
| Semantic-first assembly, All words | 24 | 31 | 18 | 25 |
| Semantic-first assembly, Broader | 24 | 31 | 24 | 31 |

Native Broader evidence recovers **six first-result and six top-ten full-answer
clauses** without changing semantic order. This retains the conceptual ranking
control's public passage-label advantage while providing the evidence benefit
seen in the fact controls. Those are separate datasets and metrics, not one
combined score or a new independent quality win. The original answer clauses
are frozen development diagnostics over four facts translated/scoped into 32
correlated positive requests; partial support is not automatically irrelevant.

The change does not repair every failure. Both native evidence inputs still
put **zero of eight mixed-language cross-language targets first**, retaining
seven in the top ten. All sixteen same-language targets remain first, and
Broader evidence contains their full answers. Both unavailable-answer controls
still receive conceptual neighbors with no answer. The earlier different-year
scope controls likewise show that semantic similarity does not enforce literal
unquoted names/dates. Source-span integrity and native field preservation do
not by themselves establish relevance or answerability.

The proposed contract for any later conceptual-search prototype is:

| Boundary | Contract and present status |
| --- | --- |
| Intent and scope | Explicit, optional conceptual **source retrieval in Your Library**. Model-free All words and Broader keep their current behavior. There is no app Conceptual mode or ranking integration today. |
| Ranking and evidence | Candidate: semantic document order with unchanged native Broader evidence when available; otherwise verified conceptual source spans. Keep literal primary fields and separately attached conceptual context honest. This is implemented only in the standalone report. |
| Names and dates | Conceptual similarity is not exact identity/date satisfaction. Preserve the existing strict literal path and source text; do not infer hard constraints through an automatic name detector, word list or exception pair. |
| Quotes and filters | Mandatory source-verified quotes and explicit document filters remain hard constraints. The current experiment excludes quoted requests from semantic retrieval and retains the original native response; do not advertise conceptual handling of quoted combinations. |
| Result claims | Show authored source passages and their locations. No generated answer, full-answer guarantee, calibrated answer confidence or guaranteed empty result for an unavailable fact. A related passage is not proof that the requested fact exists. |
| Availability and coverage | A future UI must distinguish model setup, compatible index readiness and partial coverage. Missing/disabled/corrupt/incompatible/pending model or index must leave model-free search available, with the actual search path reported. App index/query/fallback integration remains unimplemented. |
| Platforms and resources | Shared intent for desktop, Android and iOS, with availability gated by each target's actual validation. Model/resource choice stays optional; no weak-model mandate or permanent desktop restriction. Current native evidence is development-host Linux evidence. |

This is a candidate contract, **not a waiver of the Stage 7 gates**. Explicit
conceptual labeling cannot excuse misleading source evidence, broad relevance
regressions, incompatible indexes or unstable native lifecycle. The remaining
full-book/displayed-evidence, compatible indexing/fallback and supported-device
checks still determine whether to integrate. The original independent private
sheet stays unfilled; public passage labels do not populate its document or
answer-evidence grades.

Validation: both 46-request read-only replays pass; all **92 prior native,
semantic and RRF rows** are unchanged. Exact source clauses and scopes, **2,520
UTF-8 spans, 118 native primary objects and eight quote exclusions** are verified.
Source/vector inputs and cached model/executable hashes remain unchanged; no new
embeddings, installed-library access or model download. Grader self-checks cover
complete/partial clauses, wrong-document support and separately attached context;
replay produces identical grade hashes. This pass does not rerun or claim new
unit-suite, app, platform or resource acceptance.

**Next: stop automatic model/fusion iteration and collect acceptance evidence.**
The [consolidated rebuilt-app smoke](search-v2-smoke.md) is prepared and remains
user-deferred; run it when the user is ready, then record model-free acceptance
separately from optional-model preparation and semantic feasibility. It does not
require subject-matter relevance grading or a new semantic UI. Leave Stage 7 on
hold until the remaining quality, product index/fallback, lifecycle and device/
platform gates are met. The original native crash and allocator-retention caveats
remain open; do not claim they were resolved by this review.


## Native provider reload check — 2026-10-08

While interactive smoke remains explicitly deferred, this pass reuses the
existing ignored app-provider test to cover repeated native loads in one process.
Test-only `PAPERCUT_EMBEDDING_TEST_CYCLES` accepts 1–20 cycles, defaults to one,
and respects the existing single-format selector. Each format's English vector
must equal its first-load baseline. The existing inference/input, disable-first,
failed-loading, cancelled-publication and one-ORT checks repeat; phase logs
identify cycles for external resource sampling. Production behavior, dependencies,
model choices, UI and experimental ranking are unchanged.

Two fresh ten-cycle processes pass **40 successful activations and 20 cancelled
native loads** collectively. The unchanged default-cycle control also passes.
Seven ordinary provider/cache/IPC tests pass; the two asset-dependent tests are
ignored in that ordinary run, with the cached-provider test run explicitly.
Invalid zero/21/non-integer counts fail before requesting assets. Offline locked
compilation, Rust formatting and diff checks pass. No native crash recurs.

Both repeated processes peak near **1.80 GiB**, versus **1.43 GiB** for the
single-cycle control. Post-disable completion RSS rises from about 626 MiB to
665 MiB by cycle two and then roughly plateaus, ending at 655–657 MiB. This
updates resource sizing: first-load checks understate reload headroom. Full
method, timings and limitations are in the
[provider notes](search-embedding-provider.md#same-process-reload-check--2026-10-08).
It is a bounded debug Linux check with warm filesystem caches, without TTS
synthesis, real WebView or reference-device acceptance. It cannot explain the
original crash or guarantee long-term leak-free behavior. No allocator workaround
or weaker-model default is introduced.

**Decision:** retain the experimental provider; Stage 7 remains on hold. This
adds lifecycle regression evidence, not permission to enable semantic results.
**Next:** check actual command status/disable responsiveness during native
activation and cancellation, using the existing isolated command harness rather
than another model/fusion sweep. Actual WebView/download/restart, representative
quality, compatible product indexes/fallback, native stability, reference hardware
and advertised-platform gates remain. Keep the consolidated rebuilt-app
search/EPUB/RTL/audio/model-control smoke reminder for the final acceptance pass;
the user has explicitly deferred it and it is not recorded as passed.


## Native command loading check — 2026-10-08

This pass extends the existing ignored, disposable-cache command check without
adding production hooks or changing locks. Cached INT8 activation runs on a
worker while actual status handlers are polled. The check observes loading,
rejects a duplicate activation, disables a pending load, waits for its cancelled
result and verifies cleared state plus successful reactivation. A full native
load is monitored to completion, followed by the existing active-cache removal
protection, unload, fresh-state cache reuse and removal checks.

Three fresh isolated processes pass **1,018 status calls** collectively, with
maximum observed status response **0.674 ms** and pending-load disable responses
**0.094–0.187 ms**. Full activation takes **3.425–3.466 s** including verification
and warm-up. Early cancellation finishes near 21 ms. This does not prove an ONNX
call can be interrupted: the separate cached-provider test retains its
cancel-after-native-setup publication check. There is no new production behavior,
model/fusion comparison, dependency, index or UI change.

Seven ordinary provider/cache/IPC regressions pass, with two asset-dependent
checks ignored in that run. The extended command check is explicitly run in each
of the three fresh processes under a 120-second outer timeout; copied cache data
is cleaned up, and source-model/runtime/executable hashes remain unchanged.
Offline locked compilation, Rust formatting and diff checks pass. Socket access
is required only for the existing ordinary loopback HTTP regression. Timing is
debug Linux/MockRuntime/cached-assets evidence, not real WebView, release,
foreground reader, reference-device or mobile acceptance. The original crash and
allocator-retention caveats remain open.

**Next:** verify process-interruption and restart recovery at the existing model
staging/cache boundary, keeping it isolated from installed app data. Stage 7
remains held on quality, product index/fallback, lifecycle, stability and device/
platform gates. The user continues to defer the consolidated rebuilt-app
search/EPUB/RTL/audio/model-control smoke; carry its reminder to final acceptance
and do not record it as passed.


## Cache process-interruption check — 2026-10-08

The new ordinary Linux check reuses the existing installer, publication and
removal boundaries plus the tiny authored cache fixture. It starts its own test
children, waits for atomic checkpoint markers and confirms `SIGKILL` termination
without destructor cleanup. Separate recovery processes retry the same cache.
Partial-file and verified-before-publication interruptions leave no final cache;
retry discards staging and fetches both assets again. Interruption immediately
after the publish rename preserves a verified cache, reused with zero fetches.
Recovered state remains inactive and has no download reservation, staging is
absent, and removal works. Owned child processes are reaped even on assertion
failure, and checkpoint/restart waits have ten-second guards.

Three targeted parent runs pass **nine abrupt kills and nine fresh-process
recoveries**. The ordinary provider/cache/IPC suite also passes **eight tests**,
with two asset-dependent checks ignored. Offline locked compilation, Rust
formatting, diff checks and frozen source/runtime/executable bindings pass.
The check uses temporary test-owned data, no model weights, network download,
installed library or production hooks. The existing HTTP regression alone uses
loopback socket access. No installer algorithm, runtime, dependency, model choice,
index, ranking or UI change is required.

**Limit:** this is Linux process-recovery evidence using tiny assets. It is not
power-loss durability, full-sized transfer, real WebView startup or mobile
acceptance. Explicit retry restarts abandoned staging, not a resumable transfer.
See [the provider notes](search-embedding-provider.md#process-interruption-recovery--2026-10-08).

**Next:** verify recovery through the public cache/configuration command handlers
with real cached model assets and disposable app data. Keep Stage 7 held on
representative quality, product index/fallback, lifecycle, native stability,
resource/reference-device and advertised-platform gates. The user continues to
defer consolidated rebuilt-app search/EPUB/RTL/audio/model-control smoke; remind
at final acceptance and do not record it as passed.


## Real-cache command restart — 2026-10-08

The existing ignored, isolated command test now launches a child using the same
disposable app identifier and copied real INT8 cache. Public handlers verify the
cache and explicitly activate it; an atomic marker appears only after successful
native warm-up. The parent confirms `SIGKILL`, then starts a separate recovery
child. Fresh status is inactive with no loading/download reservation or encoder
identity. Cached files survive but do not enable embeddings automatically.
Public commands then verify/reuse the cache, activate with the expected encoder
key, protect an active model from removal, disable and remove. The parent confirms
cache absence. Only test-owned children are killed/reaped; 30-second waits and
the existing 120-second outer timeout bound failures.

Three isolated parent runs pass **three native kill/restart cases** and the
existing missing/corrupt cache, early cancellation, status, progress and removal
checks. Complete runs take **16.58–17.69 seconds**, including setup and all
processes, not restart-only or query latency. Eight ordinary provider/cache/IPC
regressions pass, two asset-dependent checks ignored there and the extended
command test explicitly exercised separately. Copied app-data trees are cleaned;
source-model/runtime/executable/test hashes, offline locked compilation, Rust
formatting and diff checks pass. No model download, installed-library mutation,
production hook/algorithm, runtime/dependency, model, index, ranking or UI change.

This is debug Linux/MockRuntime/cached-INT8 evidence, not real WebView startup,
interrupted upstream transfers, power-loss durability, release-profile or mobile
acceptance. It does not explain the original native crash or remove allocator
retention. Methods and limits are in
[the provider notes](search-embedding-provider.md#real-cache-command-restart--2026-10-08).

**Next:** run the existing native provider/command recovery checks under the
optimized release profile. Do not add another model/fusion sweep or production
semantic integration. Stage 7 remains held on representative quality, compatible
product indexing/fallback, remaining lifecycle/native stability and device/
platform acceptance. Consolidated rebuilt-app search/EPUB/RTL/audio/model-control
smoke remains explicitly user-deferred; carry the reminder to final acceptance
and do not record it as passed.

## Optimized native validation — 2026-10-08

The existing checks pass after locked, offline release compilation with
`native-tts-shared,experimental-search-embeddings`. No code, dependency, runtime,
model, ranking or UI change is needed. Eight ordinary provider/cache/IPC tests
pass, two cached-asset tests ignored there and explicitly exercised separately.

The default provider cycle and one ten-cycle FP32/INT8 process pass inference,
input limits, vector equality across reloads, disable-first selection, failed-load
cleanup and cancellation after native setup. The repeated run completes in
22.57 seconds, peaks at **1.791 GiB** whole-process high water and retains about
**653 MiB** after its final cycle. Release optimization does not remove the
memory/headroom concern. A separate combined provider/Arabic-audio process passes
both checks, including actual finite non-silent speech through the official
preprocessor adapter and shared runtime.

Three isolated public-command runs pass real cached INT8 activation,
`SIGKILL` and fresh-process recovery, default-off status, cache reuse, disable
and removal. Complete checks take **2.66–3.18 seconds**, including copied-cache
preparation and all child processes; these are not query or restart-only timings.
Disposable data is cleaned and source/runtime/executable bindings verified.
Rust formatting and diff checks pass. Methods and limits are in
[the provider notes](search-embedding-provider.md#optimized-native-validation--2026-10-08).

These optimized Linux development-host tests use cached assets and MockRuntime,
not a real WebView or reference/mobile hardware. They do not explain the earlier
crash, prove unrestricted lifecycle stability or establish foreground performance,
upstream-transfer recovery or power-loss durability. Quality acceptance remains
open and production search still does not use the provider.

**Next:** review the model-free tranche against the original plan and its release
acceptance checks, identifying what can ship separately from the retained
experiment. No further model/fusion sweep or production semantic integration is
justified by this result. Stage 7 remains held. Consolidated rebuilt-app
search/EPUB/RTL/audio/model-control smoke stays explicitly user-deferred; remind
at final acceptance and do not record it as passed.
