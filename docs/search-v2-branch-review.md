# Search v2 branch review — 2026-10-10

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
records the implementation and regressions. Finding 3 remains the shipping-package
gate. Smoke is still deferred; this is not merge or release approval.

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

The workflow adds 204 lines to the existing jobs, rather than another workflow or
model/job family. PR checks still use changed-surface detection. Ordinary checks
run deterministic frontend/default/provider regressions and runner self-tests;
model downloads, inference and emulator execution require manual
`provider_inference=true`. Keep that opt-in boundary.

- Default and provider Rust tests currently check different feature boundaries;
  they are not interchangeable. The Linux provider build also prepares native
  runtime files before copying and running tests. Do not delete it without
  replacing that preparation and preserving non-test compilation coverage.
- Normal and provider app builds intentionally check different configurations.
  Normal desktop/Android artifacts are uploaded before the provider build; no
  provider installer is uploaded. iOS normal/provider simulator archives and
  unsigned device links are build evidence, not signed distribution evidence.
- There is avoidable work **once the shipping provider configuration changes**:
  select one actual shipping package per target instead of retaining both full
  packaging passes for the same feature set. Preserve default-feature Rust
  checks and the explicit runtime path. The extra Windows/macOS unbundled build,
  Android provider APK and second iOS simulator/device build can then be folded
  into that shipping pass; do not expand the matrix further.
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

**Next:** integrate the provider into the existing shipping build paths (finding
3), validate actual feature packages, then fold obsolete duplicate packaging
passes into those jobs. Keep default-feature regressions and explicit native
inference checks. No new build-script family, model sweep or Stage 8 is justified.
Smoke stays deferred until the consolidated interactive pass; representative
same-language quality and physical/reference-device resources remain acceptance
gates. No release, push or additional CI dispatch is performed.

Lean: the compatibility crate's **2,028 lines** are removed, mostly lockfile data;
production Arabic-TTS integration and its unchanged upstream model/license remain.

Not checked: deferred rebuilt-app smoke, physical/mobile minimum-OS and weak-device
resources, signed provider installers, independent product-quality acceptance and
the root cause of the original Linux native crash.
