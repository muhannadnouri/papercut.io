# Search v2 branch acceptance and review

## Settings follow-up — 2026-10-14

The six approved settings changes add shared safe-area-aware modal padding/height limits, model-loading feedback in Manage Model, removal of the redundant toggle-card On line, **How It Works** explanations, concise model instructions and standard removal-button hover feedback. Main Settings readiness summaries, confirmations and job/data guards are retained.

Checks: **281 frontend passed / 1 optional skipped in 55 files**, lint/i18n/typecheck/Vite build/diff. Four new real component/controller regressions cover activation completion, error/retry, disable/late completion and native loading versus separate verification, with live-status and description-ID checks. Native IPC is mocked; no hardware performance/layout acceptance is claimed. Details are in [the progress record](search-v2-progress.md).

**Current-source CI remains pending; further smoke stays deferred.** The verified CI below covers `15b3710`, and the reported CachyOS/Android smoke predates these changes. Android safe-area propagation/clipping and iOS installed-app testing remain unverified; release gates below are unchanged. No push, merge or release is claimed.

## Result-view follow-up and reported smoke — 2026-10-14

The approved follow-up adds a tonal **Keyword / Conceptual** selector directly below the query, sharing a compact sticky row with Keyword-only All Words/Broader options. The selector/options group, helper text and keyword examples are centered in both opt-in states, with responsive stacking before the shared filter. Decorative magnifier/related-node icons sit physically left of the labels; automatic label direction is retained. Tab minimums are 36px on mouse-only screens and 44px with any coarse pointer. Keyword examples are hidden in Conceptual view. Ten-card batches per channel preserve the shared query/scope and independent retrieval. Session-selected view never changes on async completion. Switching retains list expansion/evidence and scroll position; a new submission resets list windows/positions. Accessible hidden/inert panels reuse installed React Aria tabs. All Words/Broader applies to the Keyword view, and fallback text points there without switching automatically. Counts identify returned documents, not exhaustive library matches or confidence.

Checks: **277 frontend passed / 1 optional skipped in 54 files**, lint/i18n/typecheck/Vite build/diff. Real DOM coverage includes icon-child selection and decorative focus/name attributes, grouped control order, live matching callbacks/help/input descriptions, retained open filter, keyword-only example fills and explicit submission, plus keyboard/selection, loading/failure versus zero counts, scoped batches, source targets, busy guards, retained evidence/positions and new-request reset. Reverting Conceptual example hiding fails the new regression; the earlier production-bound mutation was also rejected. Native tests were not repeated for this frontend-only change.

**The current-source CI gate is reopened for this follow-up.** The verified run below covers `15b3710`. The user's [reported smoke subset](search-v2-smoke.md) covers CachyOS Linux and Android 17 with Small FP32; Android explicitly downloaded/enabled/indexed/searched. It does not cover the new tabs, upgrades, recovery, other models or every checklist case. iOS testing was not conducted; Windows/macOS installed-app coverage and signed distribution remain unconfirmed. Further smoke stays deferred. Because merging is intended to release immediately, these remaining release gates must be resolved before release. No merge/release/push is authorized by this record.

## Context preview follow-up — 2026-10-13

Approved after the eleven review fixes: concise mode labels, nearest-three conceptual context previews, localized Show More/Show Less and PDF display reflow. Expansion is per card and source ordered, with the matched passage emphasized. Previews are Unicode/word bounded and visually clamped; More reveals further previews, while reader actions retain the full original text/page/section target. PDF display uses the existing conservative wrap rule and retains blank-line paragraph boundaries. No source offsets, index schema/policy, ranking or confidence labels change.

Checks: **262 frontend passed / 1 optional skipped**, lint/i18n/typecheck/Vite build/diff checks. Real DOM regressions cover focus, selection/order, More/Less, per-card/reset state and unchanged original reader navigation; mutation checks reject a first-three window and a shortened reader target. Native tests were not repeated locally for this frontend-only change; the full CI run below includes them. Visual layout and screen-reader/device smoke remain deferred.

The full CI record below now verifies this application change at **`15b3710`**. Its automated gate is closed; visual smoke and the remaining resource/distribution limits stay open.

## Current-source CI verified — 2026-10-13

[CI run 38500026388](https://github.com/muhannadnouri/papercut.io/actions/runs/38500026388), attempt 1, completed successfully at **`15b3710086050391958d75f61a3b53f8fb22235c`** on `feature/search-v2`. The run SHA matches the context-preview/label/PDF-display commit built on the eleven completed review fixes. This supersedes the earlier verified `7dabec7` CI for current-source acceptance. All **10 jobs** succeeded; the full desktop/mobile/SILMA jobs and explicit provider-inference steps ran, rather than being skipped.

| Gate | Verified evidence |
| --- | --- |
| Checks and configured audits | Frontend 262 passed / 1 optional utility skipped; Rust 190 default / 248 provider-enabled passed (5 / 10 ignored in those suites). Lint, frontend/Pagefind build, npm/Rust audit, monitor/runner regressions and SILMA worker self-test steps passed. The existing `RUSTSEC-2026-0235` waiver remains; its all-features/all-targets inactive-`rkyv` guard passed. |
| Desktop packaging | Linux AppImage/Debian, Windows MSI/NSIS and macOS Intel/arm64 build and artifact steps passed, including platform bundle checks. |
| Mobile and sidecar builds | Android APK, unsigned iOS simulator/device builds and the Linux CPU SILMA runtime archive job passed. |
| Explicit cached-model runtime check | Logs show `cached_provider_activation_switch_failure_and_unload` executed and passed once on Linux, Windows, both macOS architectures, Android x86_64 emulator and iOS arm64 simulator. This closes the cached-provider runtime CI gate for this source, not every ignored test or model choice. |

**The automated CI gate was closed for `15b3710`.** The result-view follow-up above requires a fresh CI run. App/WebView and remaining platform/accessibility/resource coverage stay open beyond the user-reported subset; further smoke remains deferred. The signed-device/distribution gates and earlier relevance/resource caveats are unchanged. This evidence update changes documentation only and does not authorize a merge or release.

## Maintenance follow-up — 2026-10-13

Review items **8–11** are complete against baseline `a36e213`. Together with the two follow-ups below, this addresses all eleven ranked recommendations; it does not close merge/release acceptance.

| Rank | Finding | Resolution |
| --- | --- | --- |
| 8 | Index failures omit document/section identity | Source and checkpoint boundaries add document ID and one-based section context, without adding source/query text. Canonical cancellation stays unchanged. Production-store regressions cover malformed roles, tokenizer/inference/vector failures, document-wide errors and checkpoint reuse after retry. |
| 9 | Unused standalone model-manager mode duplicates state | Require the App controller; delete local format state and optional-controller branches. Cache-action tests supply that configuration and assert selection ownership, retained failure diagnostics and explicit consent/job guards. |
| 10 | Optional evaluation utility is counted as passing without executing | Skip explicitly when input is absent; verify a generic explicit input produces the correct native request, and duplicate IDs fail without replacing output. |
| 11 | Obsolete translation keys and absent-label assertions remain | Delete eight confirmed unused keys across eight locales and three assertions against dead labels; retain live coverage checks and dynamic status keys. |

Current checks: **253 frontend passed / 1 optional skipped**, **190 default / 248 provider-enabled Rust passed** (5 / 10 ignored), lint/i18n/typecheck/Vite build, Rust formatting and diff checks. These tests exercise Papercut's behavior with synthetic failure inputs; they do not accept physical-device performance, WebView painting or accessibility behavior. The next gate is existing full CI at the committed source. Smoke remains deferred, with no current CI run, push or merge claimed here.

## Test-quality follow-up — 2026-10-13

The approved review items **5–7** are complete against baseline `5488e66`.

| Rank | Finding | Resolution |
| --- | --- | --- |
| 5 | Mocked modal effects/state do not exercise lifecycle behavior | Real React/DOM tests drive Escape, live busy updates, Tab wrapping, focus restoration, form events and listener cleanup. Transfer tests drive actual component state through mocked IPC preparation/waiting/cancellation/completion, without hook-slot overrides. |
| 6 | Fake PDF controller misses integration regressions | A generic committed Unicode-wrap PDF is parsed by the real worker and searched through Papercut's production wrapper/adapter with the installed Find controller. Tests assert counts, source-offset mapping, preserved extraction and page navigation. |
| 7 | Parallel test-only phrase verification can diverge from shipping behavior | Removed `retain_exact_phrase_hits`; six regressions run the complete production `search_uploads_with_db` path. |

The busy-guard and Unicode-wrap tests fail when their production fixes are
removed, then pass after restoration. Current checks: 253 frontend tests,
187 default / 245 provider-enabled Rust tests, lint/i18n/typecheck/Vite build.
`jsdom@29.1.1` is test-only and fits the existing pinned Node engines. Tests use
synthetic visibility geometry and do not validate canvas painting, browser layout
or screen readers. Remaining review items **8–11** are the next cleanup stage;
current-source CI and deferred app/device smoke are still required for acceptance.

## Corrective review follow-up — 2026-10-13

Reviewed application baseline: `d53a6aee73c613a3c28105f06ed4fcc47241706a`.
The user approved fixes **1–4** from the ranked branch review. They are implemented
with behavioral regressions; the earlier acceptance consolidation below remains
historical and does not describe the current application HEAD.

| Rank | Finding | Resolution |
| --- | --- | --- |
| 1 | Inline notes flatten into preferred body evidence | Role-aware UTF-8 ranges preserve literal source/locators; schema-13 verified saved-source repair invalidates affected vectors and resumes after restart. Missing/mismatched sources retain known ranges. |
| 2 | Download failures disappear after Settings closes | App-owned failure/model diagnostic and Review notice survive navigation/status retry; new job IDs guard stale terminal events. Explicit retry still requires download consent. |
| 3 | Runner cleanup hides native inference failures | All owned cleanup is attempted; secondary failures are reported; original error identity is retained. Cleanup failure alone still fails the runner. |
| 4 | Native query limits leave starter work running | Frontend budget rejects before provider calls, clears conceptual input, and stops superseded starter term loops; native trust-boundary checks remain. |

Regression checks: 257 frontend tests, 187 default / 245 provider-enabled Rust
tests, 7 runner self-tests; lint/i18n/typecheck/Vite build pass. Real modal DOM
interaction and production PDF.js integration coverage remain open (review items
5–7); these are the next recommended fixes. Lower-ranked maintainability/test
cleanup remains unaddressed. App/device/screen-reader smoke stays deferred; no
current acceptance checkbox, CI run, push or merge is implied.

## Current Acceptance Consolidation — 2026-10-12

**Development and the bounded scorer experiment are complete for this pass;
merge/release acceptance remains open.** Reviewed local source
`e6fe8cc716d4ea6724cdec7ca654fc9ec3c6ff5e`. The last application change is
`5b29ab4` (wrapped PDF role correction); subsequent commits contain evaluation
and documentation only. This is an acceptance-status reconciliation against
the original plan and connected current flows, not another whole-branch audit.
The dated reviews below are historical snapshots.

| Boundary | Current decision / remaining evidence |
| --- | --- |
| Model-free search | Keep field ranking, All Words/Broader, bounded explicit spelling retries, Arabic normalization and source evidence. Hard-coded word pairs remain removed; Stage 5's maintained alias list is intentionally deferred. Unified keyword cards and body-preferred previews are implemented but need current app smoke. |
| Semantic integration | Keep the user-approved default-off opt-in, explicit cache/index actions and separate conceptual section. This supersedes the original plan's trial fusion proposal. Small FP32/INT8 and Base/Large FP32 are embedding choices; the tested BGE scorer is not an app model option. |
| Source and upgrade behavior | Keep natural passage boundaries, body/auxiliary roles, schema-11 PDF refresh and bounded original nearby text. Old semantic policies require explicit rebuild; affected PDF coverage requires update. Preserved source text/locators and required quotes have native regressions; exact current-reader landing and upgrade/restart UX remain deferred. |
| Same-language quality | The fresh assistant-graded batch finds useful first evidence for 6/6 supported questions, complete primary evidence for 2/6 and complete expanded evidence for 5/6. All four unsupported questions still return nearest documents without useful answers. These are small provisional development results, not independent or representative acceptance. No calibrated answerability cutoff is validated. |
| Stage 8 selection | Close the tested scorer route under its frozen policy: intact inference works, but one warm pair takes 10.39 seconds against a five-second whole-query target. Broader relevance grading stopped at that gate. Do not integrate it, shrink evidence or start another automatic model/cap/thread sweep. Candidate grouping and compact-window policies also remain unshipped. No ANN scan bottleneck is established by these selection failures. |
| CI / packages | The last recorded verified full/inference run is `38110242011` on `1f020d3`, before the later UI, model and source-policy changes. It is historical evidence, not current-source acceptance. Run the existing full workflow on the pushed current branch, with provider inference enabled once for the native/model changes; ordinary inference stays optional. No new jobs or matrix are needed. |
| Device/model coverage | Small FP32/INT8 desktop/emulator/simulator runtime controls are recorded; their success does not validate Base/Large on every device. Physical Android arm64/minimum-OS behavior, weak reference hardware, foreground responsiveness, battery/thermal effects and signed Apple distribution remain unaccepted. |
| Smoke / native limits | All current Stage 7 interactive cases remain user-deferred in the [consolidated checklist](search-v2-smoke.md). The earlier reported five-group pass does not cover this revision. The original Linux native crash remains unexplained; disable does not promise immediate RSS recovery. |

**Next:** push the current branch and use the existing manual workflow with
`validation_scope=full`, `provider_inference=true`; bind the resulting run to its
actual source SHA before closing the CI gap. When the user resumes smoke, build
with `npm run desktop` (or the existing native Android/iOS commands) and run the
single off/on checklist in an isolated library, including package launch and
upgrade cases. Record failures and fix reproducible defects; do not add another
speculative feature pass merely to advance stages. Relevance/device/distribution
limits still need evidence or an explicit, scoped product decision before claiming
acceptance. This review performs no push, dispatch, merge, release or interactive
smoke and marks no deferred case passed.

Keep the existing lexical evaluator and mobile runner/self-tests. The old
retrieval experiment remains frozen outside app/npm/CI builds; the compatibility
crate stays retired. New private diagnosis/scorer helpers, assets and reports
remain outside the checkout and introduce no maintained shipping/CI scripts.

## Historical Branch Review — 2026-10-10

What this change does: The branch improves local lexical ranking, Broader mode,
source evidence, spelling suggestions and Arabic search forms. It also adds a
default-off semantic provider, incremental index and separate conceptual results,
with explicit native runtime checks across desktop and mobile.

Reviewed `main` **`8be9952fe02a10921ea4955df204b115268bc686`** against
`feature/search-v2` **`878fbbf2a21d19d5c84f106d40b123834d025c17`**, before this
documentation commit. Local `main` matches the GitHub branch at review time and
is the merge base. The committed comparison is 95 files, 24,347 added and 318
removed text lines; the bundled upstream model is additional binary data.
Uncommitted editor settings, memories and root fixtures/handoff are excluded.

The review follows the original root handoff's search/source, lifecycle, platform
and solo-maintainer contracts. The later approved default-off toggle and separate
conceptual section supersede its trial fusion suggestion; a combined ranking is
not required. Expected load is one offline desktop/mobile user with hundreds of
documents, including long books, rather than a multi-user search service.

## Corrective status — 2026-10-10

**Findings 1 and 2 are fixed; finding 4 is retired.** The findings below describe
the reviewed source above; the [corrective pass](search-v2-progress.md#stage-7--branch-review-corrections--2026-10-10)
records the implementation and regressions. Finding 3 now has [shipping build integration](search-v2-progress.md#stage-7--provider-in-shipping-builds--2026-10-10).
The [shipping CI acceptance and consolidation](search-v2-progress.md#stage-7--shipping-ci-acceptance-and-consolidation--2026-10-10)
verifies full run `38107442282` on `e686960`: shipping builds and optional native
inference pass across supported platform runners. Redundant build steps are now
removed; [final acceptance below](#final-acceptance-review--2026-10-10) verifies the
simplified workflow in full run `38110242011` on `1f020d3`. Signed distribution
and smoke acceptance remain open; this is not merge or release approval.

## Final acceptance review — 2026-10-10

What this change does: The corrected branch keeps lexical search as the default
and adds an explicit optional conceptual section. Existing shipping builds now
contain the provider, so the duplicate build variants are removed.

Reviewed source **`1f020d31a34f3a7943772f9fa806391f060d49b6`** against unchanged
local/remote `main` and merge base **`8be9952fe02a10921ea4955df204b115268bc686`**.
Current branch totals: **95 files, 23,079 added / 323 removed text lines**, plus
the upstream model binary. This follow-up builds on the initial review below,
reading the corrective delta and its connected flows/tests; it is not an
independent re-audit of all 95 files. Expected load remains one offline user with
hundreds of documents, including long books.

**No additional reproducible corrective finding in the re-reviewed flows.**
Findings 1 and 2 have regressions that reproduced their failures; finding 3's
shipping integration and simplified CI now pass, and finding 4's separate
compatibility crate is removed. The original findings remain below as history.

| Acceptance boundary | Evidence and remaining limit |
| --- | --- |
| Lexical and opt-in separation | App owns semantic state separately; fresh/invalid preferences start off. Cache download/verification does not enable use. Conceptual results/fallback are hidden while off; All words/Broader remain independent. Saved valid opt-in reloads cache, not indexing/downloads. Hook/card/settings regressions pass. |
| Replacement requests and rollback | One running conceptual IPC drains only the latest eligible replacement; stale results and invalidated pending work cannot publish. Pre-7/pre-8 migrations invalidate unsupported derived projections transactionally, preserving authored/original data. Twelve hook cases and schema-6/7 round-trip coverage pass in the verified suites. |
| Source/query/storage contract | Native retrieval constrains scope and authored quotes before top-k selection, keeps one result per document and validates source hashes/UTF-8 spans. Querying is read-only; missing/partial/incompatible indexes fall back. Incremental checkpoints and generation checks reject stale writers/results. These controls pass native tests; exact reader landing still needs app smoke. |
| Shipping and CI | [Run 38110242011](https://github.com/muhannadnouri/papercut.io/actions/runs/38110242011) has ten successful jobs without redundant builds. It passes 184 frontend tests, 162 default Rust tests (five ignored), 217 provider Rust tests (ten ignored), audits, package uploads and runtime/media checks. Both formats' explicit index/query/lifecycle controls pass on all six runtime targets. No artifact download/install is done in this review; iOS builds are unsigned. |
| Native dependency ownership | Official registry Libtashkeel, its unchanged attributed model and the shared initializer remain. Android's process-lifetime environment guard and atomic Unix runtime-copy/Windows loader fixes remain scoped compatibility code. Successful bounded inference does not explain the original Linux crash or promise immediate RSS recovery on disable. |
| Quality and resources | Existing judged comparisons support bounded same-language conceptual retrieval, not a universal synonym, answer or ranking guarantee. Private-book/displayed-evidence relevance, physical Android arm64/minimum-OS behavior, weak hardware, foreground responsiveness and battery/thermal effects remain unaccepted. Opt-in limits exposure but does not replace this evidence. |
| Retained experiments | The frozen retrieval crate is outside app builds, npm and CI. Keep the actual-path lexical evaluator and mobile runner/self-tests; do not reintroduce a separate compatibility graph or an obligatory model benchmark job. |

**Verdict: automated branch acceptance complete; hold merge/release acceptance.**
The remaining work is evidence, not another speculative code pass. When the user
resumes testing, run the consolidated installed-app Stage 7 smoke and record any
actual failure; resolve resource/relevance and signed-distribution limits before
advertising them as accepted. Smoke is explicitly deferred, so this review neither
requires it now nor marks it passed. Stage 8 remains skipped without a measured
bottleneck. No production edits, push, dispatch, merge or release are performed.

Not checked: deferred installed-app/reader/audio/RTL smoke, physical/minimum-OS and
weak-device resources, signed/notarized Apple distribution, independent product
quality and the root cause of the original Linux native crash.

## Must fix at review time

1. **A replacement conceptual search gets stuck in fallback.**
   Sources: [`useConceptualSearch.ts:69`](../src/search-semantic/useConceptualSearch.ts#L69),
   [`semantic.rs:61`](../src-tauri/src/document_uploads/semantic.rs#L61),
   [`useConceptualSearch.test.ts:110`](../src/search-semantic/useConceptualSearch.test.ts#L110).
   - **What this is:** React rejects outdated results while Rust allows one
     conceptual scan at a time.
   - **Problem:** Submit a second query or change scope before the first query
     finishes: the second IPC call is rejected as busy, and its fallback persists
     after the first finishes. Effect cleanup only ignores the old result; the
     existing frontend test allows overlapping mock queries that Rust rejects.
   - **Fix:** Keep one in-flight conceptual call and coalesce replacements into
     the latest eligible request, starting it when that call settles. Add one
     hook regression with Rust's single-slot behavior, preserving stale-result,
     scope, disable and unmount guards without an unbounded queue or polling.
   - **If we skip it:** Ordinary repeated searches lose conceptual results until
     another submission, especially during slow native work. Lexical search
     remains available, but the opted-in feature behaves incorrectly.

2. **Returning from an older app can mark stale lexical projections ready.**
   Sources: [`store.rs:221`](../src-tauri/src/document_uploads/store.rs#L221),
   [`store.rs:280`](../src-tauri/src/document_uploads/store.rs#L280),
   [`store.rs:345`](../src-tauri/src/document_uploads/store.rs#L345),
   [`store.rs:1040`](../src-tauri/src/document_uploads/store.rs#L1040).
   - **What this is:** Older `main` writes schema version 6 and updates only the
     original FTS index. The branch adds normalized FTS and spelling checkpoints.
   - **Problem:** After using the branch, rename a document in `main` with the
     same app data, then reopen the branch. Migration resets readiness, but both
     backfills skip existing rows/checkpoints and declare the stale data ready;
     a new Arabic title misses while its old title still returns the renamed
     document, and the title vocabulary stays old.
   - **Fix:** On a pre-7/pre-8 migration, invalidate the corresponding derived
     FTS rows and vocabulary/checkpoints transactionally before backfill. Keep
     authored documents/original FTS intact and add a round-trip regression that
     starts with populated derived tables rather than empty legacy tables.
   - **If we skip it:** Switching between this branch and the current app, or
     returning after a rollback, can silently give incorrect search results.
     Separate test profiles avoid exposure but do not repair the migration.

## Should fix at review time

3. **Release packages do not yet contain the user-opt-in feature.**
   Sources: [`Cargo.toml:23`](../src-tauri/Cargo.toml#L23),
   [`build-desktop.js:22`](../scripts/build-desktop.js#L22),
   [`build-android.js:41`](../scripts/build-android.js#L41),
   [`build-ios.js:50`](../scripts/build-ios.js#L50),
   [release workflow](../.github/workflows/release.yml).
   - **What this is:** The current separation deliberately keeps ordinary
     packages on the existing native-TTS feature set.
   - **Problem:** Release invokes those ordinary scripts, while CI's additional
     provider builds are unbundled or not uploaded. A release from this branch
     would improve lexical search but leave semantic controls unavailable.
   - **Fix:** After feature acceptance, enable the appropriate dynamic or linked
     provider in the existing shipping build paths and validate those actual
     packages. Keep saved user consent off by default, weights unbundled and
     lexical-only Rust regression coverage; do not globally enable a dynamic
     Cargo default or add another Tauri config/npm build family.
   - **If we skip it:** Users cannot opt in to the advertised semantic feature.
     This is an explicit pre-release integration gate, not evidence that the
     existing lexical-only packages are broken or that publishing is authorized.

4. **Retire the separate compatibility experiment before merge.**
   Sources: [`compat/Cargo.toml:10`](https://github.com/muhannadnouri/papercut.io/blob/cf161ca0f293718414a02aefda88913b2fa2acff/experiments/search-v2-embeddings/compat/Cargo.toml#L10),
   [`same_process.rs:98`](https://github.com/muhannadnouri/papercut.io/blob/cf161ca0f293718414a02aefda88913b2fa2acff/experiments/search-v2-embeddings/compat/same_process.rs#L98),
   [`onnx_runtime.rs:6`](../src-tauri/src/onnx_runtime.rs#L6).
   - **What this is:** A second Cargo project tested the earlier official
     Libtashkeel adapter/embedding/speech compatibility decision.
   - **Problem:** It keeps its own lockfile, API-24 configuration and runtime
     initialization instead of the app's API-17 graph and shared initializer.
     Updating this check cannot establish compatibility of the shipping graph;
     the current app already owns native provider and Arabic-audio regressions.
   - **Fix:** Remove the four `compat/` files and replace current run guidance
     with the app checks. Preserve the historical decision/results in Git;
     migrate a uniquely needed initialization-order assertion into the existing
     app check rather than keeping a second runtime project alive.
   - **If we skip it:** A solo maintainer has another native dependency graph to
     update, with a passing check that can diverge from the app. This experiment
     is not currently bundled or invoked by CI.

## Boundaries worth keeping

| Boundary | Assessment and removal implications |
| --- | --- |
| Lexical search | The production SQLite path owns all-terms/Broader semantics, bounded authored-word suggestions, normalized search forms and real source evidence. No hard-coded synonym pairs or new Pagefind work are needed. Keep this independently of semantic search. |
| Rust semantic path | `search_embeddings.rs` and its cache own the model; `document_uploads/semantic` owns derived storage/jobs/retrieval. Integration is state/command registration plus shared storage access, not a second importer or reader. Remove these optional modules/features/registrations together if the product is retired. |
| React semantic path | `src/search-semantic` owns opt-in and conceptual state/cards; App connects it through an additional-results slot. `SearchModelSetup` is the settings boundary. Remove those integrations, preferences and semantic locale keys together; keep lexical mode/evidence/spelling UI. Finding 1 is a boundary bug, not a reason to add a scheduler framework. |
| Shared native runtime | Official Libtashkeel 1.5.0 remains a registry dependency with its public engine interface. The small app adapter, unchanged model/license and shared ORT initializer also serve Arabic TTS; removing semantics does not justify deleting them. The existing sherpa vendor patch predates this branch; the branch's atomic Unix replacement and Windows DLL placement fixes are narrow build changes. |
| Evidence and private data | Conceptual cards render authored text without synthetic word marks/counts; byte metadata stays native and reader opening uses existing source targets. Model manifests are pinned and verified; explicit download/index/remove is separate from query execution. Private evaluation inputs/reports and personal fixture files stay outside commits. |

## Experimental scripts: keep, freeze or remove

| Files | Decision |
| --- | --- |
| `scripts/search-eval.mjs`, generic `scripts/fixtures/search-v2/corpus.json`, `search/evaluation.rs`, parser evaluation tests and `docs/search-v2-baseline.json` | **Keep.** They exercise the actual lexical parser/store/search and preserve the initial comparison. The Rust evaluator is test-only; private snapshot/review modes are explicit developer actions. Do not replace these with manual checks. |
| `scripts/check-mobile-provider-inference.mjs` and its self-tests | **Keep.** One runner selects the Cargo-built executable, requires both formats' index/query/lifecycle completion and successful native exit, and cleans owned emulator/simulator fixtures. It is CI/developer tooling, not app code or a new search implementation. |
| `experiments/search-v2-embeddings/compat/` | **Removed in the corrective pass**, as finding 4 specifies. The four archived files total **2,028 lines**, including 1,753 lockfile lines and 174 Rust lines. Do not delete the app's official Libtashkeel adapter or its model. |
| Remaining `experiments/search-v2-embeddings/` manifest/lockfile and `src/main.rs`/`storage.rs` | **Keep frozen for reproducible historical quality comparisons.** Its Small/Base, FP32/INT8, semantic/fusion and standalone-index controls differ from the app. It is outside the app Cargo graph, npm build and release packaging; do not synchronize it with product migrations, add models/options or make it mandatory CI. Product acceptance must use the app path; prune the frozen experiment in a later dedicated archival pass if historical replay is no longer needed. |
| Root smoke HTML, root handoff, `.memsearch/`, editor/session files | **Exclude from this commit.** They are not shipping tooling or reviewed branch implementation. |

The original review changed documentation only. The corrective pass removes the
four compatibility-project files and updates their references; the historical
four initialization-order controls remain in Git. Current run guidance uses the
app's provider and Arabic-TTS checks, whose coverage is recorded separately.
Keeping frozen retrieval research is not a commitment to maintain two product
search implementations.

## CI duplication and packaging policy

At the original review, the workflow added 204 lines to the existing jobs, rather
than another workflow or model/job family. This consolidation removes 20 lines.
PR checks still use changed-surface detection. Ordinary checks
run deterministic frontend/default/provider regressions and runner self-tests;
model downloads, inference and emulator execution require manual
`provider_inference=true`. Keep that opt-in boundary.

- Default and provider Rust tests currently check different feature boundaries;
  they are not interchangeable. The Linux provider build also prepares native
  runtime files before copying and running tests. Do not delete it without
  replacing that preparation and preserving non-test compilation coverage.
- Ordinary native shipping builds now contain the provider, with user consent
  still default-off and weights unbundled. Run `38107442282` passes their builds,
  uploads, runtime checks and optional inference. Artifact uploads do not establish
  installed-app acceptance; iOS simulator archives and unsigned device links do
  not establish signed distribution acceptance.
- Keep one shipping build per target. The redundant Windows/macOS unbundled
  build, Android provider APK and second iOS simulator/device builds are removed:
  four workflow definitions, six executions per full run. Their shipping scripts
  prepare the same runtime prerequisites; preserve default-feature Rust checks
  and the explicit inference path. Full run `38110242011` confirms the remaining
  shipping steps provide the prerequisites; do not expand the matrix further.
- Tauri repeats its frontend build hook for app variants. Collapse obsolete
  variants first, rather than introducing a build-cache/config framework merely
  to suppress those repeated hooks. Existing platform staging/link/bundle checks
  still serve native packaging and should remain.
- Keep scale, repeated native reload, interruption/recovery and frozen quality
  experiments explicit developer/release checks. Their assertions should stay
  automated; they need not all become every-PR jobs. Physical resources,
  accessibility, reader/audio/RTL and real-app lifecycle still need manual smoke.

Retain the Android environment-lifetime guard until an upstream fix is validated.
The original Linux crash remains unexplained, and native allocator retention
means disable does not promise immediate process-memory recovery. These are
documented limits; this review does not turn them into new speculative findings
or claim simulator success proves weak/physical-device safety.

## Original review evidence

Fresh focused frontend validation passes **33 tests in six files** covering the
semantic hooks/cards/settings, cache setup and lexical request concurrency.
An isolated temporary hook test using the native single-slot contract fails the
desired latest-query recovery assertion: expected ready, received fallback even
after the old request finishes. A separate disposable SQLite replay executes the
branch schema and reproduces finding 2's older-title write, unchanged derived
rows/checkpoints and incorrect ready/search outcome. This is SQL-boundary evidence,
not an executed older/newer Tauri app round trip. No installed app data is used.

The previously verified [mobile run 38094061234](https://github.com/muhannadnouri/papercut.io/actions/runs/38094061234)
on `f42af57` remains the native index/query evidence; it is not rerun or relabeled
as interactive acceptance here. The default/provider migration test starts by
deleting derived tables, and the frontend concurrency mock permits simultaneous
calls, explaining why that passing run does not cover these two findings.

## Corrective verification and next pass

The new hook regression reproduced native single-slot rejection before the fix;
now its twelve cases pass, including coalescing, scoped clauses, stale completion,
failed-call recovery, queued-request invalidation and StrictMode replay. The Rust
round-trip regression failed with a stale row before the fix and now passes for
schema 6 and 7 returns. Existing interrupted-batch coverage also passes; authored
text, original FTS and library placement remain intact.

Fresh checks pass: **175 frontend tests in 47 files**, **162 default Rust tests
(5 ignored)** and **217 provider-enabled Rust tests (10 ignored)**, lint,
TypeScript and Rust formatting. No installed app data is used. Ignored asset/runtime
checks and platform CI are not rerun by this corrective pass.

**Next:** consolidated smoke and remaining release evidence, when the user resumes
testing. The final review above accepts the simplified workflow in full CI. The
[shipping pass](search-v2-progress.md#stage-7--provider-in-shipping-builds--2026-10-10)
records local package evidence; the subsequent full run passes platform shipping
builds and native inference. Keep default-feature regressions and explicit native
inference checks. No new build-script family, model sweep or Stage 8 is justified.
Smoke stays deferred until the consolidated interactive pass; representative
same-language quality and physical/reference-device resources remain acceptance
gates. No release, push or additional CI dispatch is performed.

Lean: the compatibility crate's **2,028 lines** are removed, mostly lockfile data;
production Arabic-TTS integration and its unchanged upstream model/license remain.

Not checked: deferred rebuilt-app smoke, physical/mobile minimum-OS and weak-device
resources, signed provider installers, independent product-quality acceptance and
the root cause of the original Linux native crash.
