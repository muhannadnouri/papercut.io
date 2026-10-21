# Offline Translation

Current implementation and next work, reconciled against code on 2026-10-21.
This is the maintained guide; historical experiments and per-stage validation
receipts live in [Git history](#history-and-evidence). Passing structural tests
is not representative language-quality or packaged-device acceptance.

## Supported scope and admission

Translation operates on imported **HTML and EPUB** reader documents. PDF and
other formats are rejected by both UI admission and the native source loader.
Models are installed explicitly; installed translation runs offline without a
cloud service. Original documents remain available alongside generated variants.

| Provider | New public jobs and downloads | Current role |
| --- | --- | --- |
| OPUS-MT French → English / Spanish → English, CTranslate2 | Admitted when the native engine is compiled; jobs also require an installed pack | Desktop whole-sentence translation with explicit formatting-review results |
| Bergamot French → English / Spanish → English, WebView worker | Gated with `acceptance-pending` | Internal experiments, installed-file status/removal, existing saved results |
| HY-MT2 1.8B Q8, llama.cpp | Gated with `acceptance-pending` | Internal experiments, installed-file status/removal, existing saved results |
| Retired auxiliary alignment pack | No new install/repair or document inference | Legacy inventory, verified notices and removal only |

The OPUS model IDs are `opus-mt-fr-en-ctranslate2` and
`opus-mt-es-en-ctranslate2`. Source and target are fixed per model. OPUS accepts
only default quality, no glossary and no repair; unsupported direct requests
fail rather than pretending those controls work. It exposes no GPU selector.
Runtime availability, catalog platform recommendations and installed bytes do
not independently grant product admission or certify translation quality.

Catalog-owned admission also guards native starts, worker starts/asset reads and
new downloads. Stale UI or crafted commands cannot bypass the acceptance gate.
Saved-result access and explicit model removal remain available without a source
selection or runnable provider. Partial and zero-byte installations are removable.

The strict semantic-alignment experiment is **retired**, not an opt-in feature.
Its runtime, token contracts, projection, receipts and Cargo features were removed.
Legacy `preserveFormatting: true` requests reject; the internal `Strict` policy is
only a rejection/cache-namespace sentinel. Normal OPUS jobs need no aligner.
Existing translations, source documents, audio and highlighting are not migrated.

## Formatting: what is preserved and what needs review

Sentence meaning takes priority over guessing which translated words inherit
partial bold, italics or link scopes. Translating each styled fragment separately
was rejected because it can change negation and word order despite valid HTML.

1. The shared DOM plan reads the sanitized reader snapshot in source order,
   retaining source-owned tags, attributes, IDs, links and owned assets.
2. OPUS translates whole natural sentences across inline emphasis/link boundaries.
   Code and `translate=no` literals, note labels and media payloads stay outside
   inference. Over-budget sentences may need bounded clause/whitespace cuts.
3. Complete natural-sentence scopes or exact copied content can retain validated
   source wrappers. A token-limit cut or nearby whitespace is not semantic
   evidence that a partial style belongs on particular target words.
4. If a style or attachment lies inside a translated sentence and cannot be
   restored safely, retain the entire affected formatted original block once,
   followed by a clearly labelled, escaped translation. Mark the document
   **Saved — Formatting Needs Review** (`needs-review`), rather than ordinary
   completion (`ready` in saved metadata, `stored` in the job result).
   Ordered-list containers and numbering remain source-owned.
5. Decoder cutoff/empty output, malformed markup, invalid mapping, cancellation
   and save errors remain failures. Review is a planned formatting outcome,
   never a catch-all recovery or permission to save partial output.

For example, `Je <em>ne</em> sais <strong>pas</strong>.` enters MT once as
`Je ne sais pas.` Its formatted original is retained for review beside the
translation. `<strong><em>Bonjour le monde.</em></strong>` can keep both wrappers
around the translated whole sentence. Neither case proves linguistic accuracy.

Review originals and generated labels remain visible with usable source links.
The validated `data-papercut-translation-review="original|label"` markers exclude
those nodes from backend search/section text, reader Find, narration, locators
and read-along highlighting. Those consumers use translated content, including
when a highlight chunk spans blocks. Saved English documents use English review
labels; app setup/status explanations use the eight UI locales.

Preservation concerns **sanitized reader HTML**, not arbitrary original CSS or
scripts. Import sanitization can remove style-based bold before translation;
do not weaken that security boundary to recover appearance. Exact semantic
emphasis on target words remains unsolved for general partial-sentence markup.
Bergamot's own HTML restoration does not remove its output-completeness blockers.

### Best-effort emphasis evaluation — 2026-10-21

**Evaluation only; not admitted to the sentence/review path.** Reusing phrase
hints can restore some target bold/italic without changing English, but unique
matches do not prove semantic ownership. Do not present this as solved formatting
or reuse the full legacy renderer, which also guesses attachment placement.

The isolated Linux evaluation used unchanged current Rust DOM/matching helpers
and the native CTranslate2 adapter with manifest-verified installed FR/ES packs.
Seventeen synthetic cases used fixed, manually authored English targets, first
with controlled hints and then with real native phrase translations. Separately,
75 cached blocks from a private saved result supplied frozen English; all source
text hashes and protected DOM identities were verified before accepting the
pairing. Private inputs, evaluation code and outputs remain outside the repo.
This is a placement diagnostic, not new app smoke or a performance benchmark.

Two block-local matchers were compared: the existing exact/folded phrase matcher,
and a disposable tighter version requiring unique source/target matches under
case/accent folding, exact source-hint identity, no unchanged-source fallback
after a missing translated hint, and rejection of every overlapping candidate.
Only bold/italic text runs were eligible for output; wrappers were stripped and
compared with the frozen target UTF-8 bytes in every result. No target prose,
links, notes, IDs, assets, saved documents or app cache were rewritten.

| Probe screen on private saved blocks | Existing matcher: placed runs | Tighter matcher: placed runs |
| --- | --- | --- |
| Legacy eight-probe segment cap applied to each cached block | 282 / 921 | 236 / 921 |
| All eligible phrases, diagnostic comparison only | 491 / 921 | 473 / 921 |

These are **placement coverage counts**, not correct-emphasis percentages. The
921 units are coalesced source text runs with bold/italic stacks, not author tag
counts; there is no annotated target gold for this private document. This cache
has one segment per block; normal sentence/review jobs still skip legacy probes.
Probe collection retains the legacy eligibility rules (at least four alphanumeric
characters, at most 120 scalars, including non-bold/italic legacy scopes before
the eight-probe cap). The uncapped screen is not a proposed production budget or
a mathematical upper bound: extra hints can introduce conflicts too. Short
negation, differently phrased translations and some longer/nested scopes remain gaps.

The adversarial cases expose a correctness limit, even after tightening:

- With controlled hints, wrong placements fell from six to two; with actual
  native phrase hints they fell from three to one. The tighter native matcher
  also lost two correct placements. Required abstentions are separate from
  successfully preserved formatting; no representative precision score follows.
- A real Spanish `banco` probe returned `bank`. In a fixed sentence translating
  the styled `banco` as `bench` and unstyled `orilla` as `bank`, both matchers
  styled the unrelated `bank`. It is a unique match inside the same sentence,
  so sentence-local matching alone cannot fix this sense error.
- Existing exact-first matching can choose the wrong capitalized occurrence;
  unchanged-source fallback can style English `pain` for French `pain` (bread).
  Removing these paths helps, but does not solve the remaining meaning problem.

Recommendation: **do not wire either matcher into normal jobs from this result**,
raise probe caps, add word-specific exception lists, or revive the retired strict
stack. If approximate formatting is accepted as optional review assistance, a
separate bounded stage must retain immutable English, source-occurrence-bound
and sentence-local evidence, unresolved scopes, explicit review status and
original comparison. It also needs cancellation/work budgets, distinct cache
provenance, hierarchy/literal/attachment checks and save/resume regressions before
integration. A unique match must never automatically clear review status.
Manual correction or a separately qualified maintained HTML-aware provider remain
the paths to address unresolved scopes; this screen establishes neither. Current
provider admission and the retained-original fallback remain unchanged.

### Firefox DOM and HY-MT2 1.8B evaluation — 2026-10-21

The private placement preview was rejected. Commercial translation is excluded;
device resources are not restricted to low-power hardware. The next isolated
screen therefore compared a Firefox-derived HTML path and tag-aware HY-MT2,
without changing production code or provider admission. Inputs, harnesses and
previews stay outside the repo. This is a targeted quality investigation, not
an app smoke test, performance benchmark or release qualification.

Both paths received the same twelve frozen French/Spanish synthetic cases and
two private paragraphs. Cases cover reordered adjectives, nested/repeated
emphasis, split negation, drop caps, notes, links, images, literal islands and
whole-sentence styling. Inspect meaning and the intended English emphasis scope;
preserving tag counts or ancestry alone does not establish either.

- **Firefox-derived path:** stock installed Firefox 158.0.1 worker/loader and
  verbatim source-owned DOM reconciliation functions, with hash-verified Remote
  Settings FR/ES model 2.0 and WASM revision
  `1de4a085d3a7afb625c51a60aabb5ad298e4059f`. The harness uses jsdom and simplified
  static block/inline grouping. It does not reproduce Gecko layout, scheduling,
  viewport/shadow/mutation handling, browser sanitization or attribute translation;
  no actual Firefox UI acceptance was performed. Firefox's
  [translation implementation](https://firefox-source-docs.mozilla.org/toolkit/components/translations/index.html)
  adds DOM extraction/reconciliation to the engine; this is not a separate
  engine-independent semantic-formatting library.
- **HY-MT2:** official
  [1.8B Q8 GGUF](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/tree/1cd5208700acedef4ef93019b6cfc148b8522d45),
  1,908,528,192 bytes, SHA-256
  `5c3fe0b1408a5ceb0143184ef247b11b579c525f4b02b060e6c851bb76fef1a4`.
  Installed `llama-server` commit `f174553e1` ran offline on a private Unix socket,
  using the embedded Hunyuan chat template, one user message, no system prompt,
  8192-token context and 4096-token output cap. The
  [published structured-data instruction and sampler](https://github.com/Tencent-Hunyuan/Hy-MT2#hy-mt2-translation-task-instruction-examples-chinese-english-comparison)
  used temperature 0.7, top-p 0.6, top-k 20, repetition penalty 1.05 and seed 42;
  extra min-p filtering was disabled and repetition covered the context. All
  fourteen final replies ended naturally. This does not validate Papercut's
  compiled llama.cpp binding or its existing plain-text HY adapter.

Firefox-derived output correctly preserved several nested, repeated-word and
reordered scopes. It still duplicated split-negation text, broke drop-cap words,
duplicated a note numeral and omitted part of a bold quantity phrase. A separate
whole-private-document run completed 76 units but exposed 92 empty target
elements and 65 duplicate style identities. Its permissive reconstruction kept
source-language clauses inside English. Forty-nine retained-source span matches
include nested overlaps and possible proper names, so they are not an independent
error count; inspection confirmed definite untranslated clauses. Attributes and
counts survived for 37 links, three media elements and 31 author IDs; correct
note association and semantic completeness do not follow from those checks.

HY's structured-data outputs sometimes preserved HTML while extending bold over
unrelated words or mistranslating negation. Other outputs introduced Markdown,
malformed hierarchy or instruction-preamble leakage. A second, predeclared concise
translation-only HTML prompt removed that preamble leakage but still lost emphasis
and changed meaning on the private paragraphs. Prompt adjustment alone did not
qualify the 1.8B candidate. These small non-blinded screens establish counterexamples,
not a general model ranking or acceptable-error percentage.

**Decision:** neither result meets the requested translation/formatting quality.
Do not enable either provider, splice their outputs automatically, accept generated
attributes, or copy Firefox's empty-node retention/duplicate-node cloning wholesale.
Keep its useful pattern—whole inline context, genuine formatting tags and
source-owned reconstruction—within the existing DOM/asset/job pipeline. Any future
style splitting must distinguish attribute-free emphasis from exact-once author
IDs, notes, links, media and literals. A larger tag-aware HY model is the next
quality candidate to evaluate before another production mapping change; size is
not evidence that it will pass. No upstream fork or local runtime patch is proposed.

### Current-machine 7B feasibility — 2026-10-21

Read-only inspection of the current Xiaomi Redmi Book Pro 16 2026 found an
Intel Core Ultra 5 325 (eight cores, AVX2), 23.0 GiB usable RAM and an Intel
Panther Lake integrated GPU using the Linux `xe` driver. Outside the execution
sandbox, Mesa 26.2.4 exposes the real Intel Vulkan device. The installed inference
runtime also enumerates `Vulkan0: Intel(R) Graphics (PTL)` when its existing Vulkan
backend is loaded. The earlier sandbox's missing `/dev/dri` and CPU-only probe
were not evidence of absent GPU hardware. Device enumeration does not establish
working model offload or faster inference; integrated GPU memory shares system RAM.

At the earlier follow-up inspection, approximately 4–5 GiB of RAM was available
and zram held about 12–13 GiB of logical swapped data (about 7–8 GiB compressed).
This was memory pressure, not an additional physical-memory allowance. The
disk-backed filesystem had about 806 GiB free. `/tmp` was a 12 GiB RAM-backed
filesystem with only about 2.4 GiB free. Use an isolated disk-backed cache
outside the repo for the next evaluation, preserving private inputs separately.

A later read-only recheck on the same date found approximately 14 GiB of RAM
available, negligible swap use and an almost empty `/tmp`. Hardware identity and
the real Intel Vulkan device were confirmed again outside the sandbox. The earlier
memory-pressure observation is not a permanent hardware limit: a bounded Q4 or
Q8 evaluation is now capacity-feasible, subject to actual allocations. Continue
using disk-backed storage rather than consuming RAM with model downloads in tmpfs.

The [official 7B GGUF files](https://huggingface.co/tencent/Hy-MT2-7B-GGUF/tree/main)
are Q4_K_M 4.62 GB (about 4.30 GiB), Q6_K 6.16 GB (5.74 GiB), and Q8_0 7.98 GB
(7.43 GiB). Weights are not total process memory. From the
[model configuration](https://huggingface.co/tencent/Hy-MT2-7B/blob/main/config.json),
a conventional FP16 KV cache for 32 layers, eight KV heads and head dimension
128 uses approximately 512 MiB at 4096 tokens or 1 GiB at 8192, before runtime
and computation buffers. These are estimates, not measured 7B allocations.

**Assessment:** this machine is capacity-feasible for a bounded 7B evaluation;
hardware replacement is not justified by the available evidence. Check available
memory before each run and start with one job, short whole-inline-context blocks
and a 4096-token context.
Aim for roughly 8 GiB available for Q4, or 10–12 GiB for a Q8 comparison that
reduces quantization as a quality confound. These are conservative planning
targets, not admission thresholds or memory guarantees. Use conventional official
GGUFs and a maintained runtime; no custom quantization kernels or library patches.
CPU inference is the baseline; evaluate Vulkan offload separately if needed.
Do not terminate unrelated applications or clear user data automatically.

If 7B fits but is slow, consider it an optional background desktop mode using the
existing progress/cancellation/resume owners, subject to actual quality and
usability acceptance. If it cannot fit after freeing memory, first try conventional
Q4 and a smaller context while preserving whole sentence/inline units. If no
maintained local candidate meets both resource and formatting requirements,
evaluate another model or use a stronger computer owned by the user; commercial
services are excluded. An optional private-LAN inference service would be a
separate security/product decision, not the current offline implementation.
If none meets quality requirements, retain explicit review/manual correction or
defer release rather than resume phrase-placement guesses, silently drop emphasis
or present mixed-language output as complete. No 7B weights were downloaded or
loaded, and no throughput or acceleration result is claimed in this assessment.

### TranslateLocally comparison — 2026-10-21

Read-only source inspection pinned
[TranslateLocally](https://github.com/XapaJIaMnu/translateLocally/tree/1d0d382cd6c9085cb4b8144c4a8962d74ed609c8)
at `1d0d382cd6c9085cb4b8144c4a8962d74ed609c8`. Its Bergamot submodule is
`9271618ebbdc5d21ac4dc4df9e72beb7ce644774`, distinct from the Firefox runtime
screened above. No TranslateLocally binary was installed or tested on our cases.

- **Resource strategy:** compact specialized Marian/Bergamot models, distilled
  students, 8-bit inference and lexical shortlists. The
  [project's model guidance](https://github.com/XapaJIaMnu/translateLocally/blob/1d0d382cd6c9085cb4b8144c4a8962d74ed609c8/README.md#quantising-the-model)
  explains these optimizations. Its default
  [catalogue](https://translatelocally.com/models.json) currently lists French→English
  and Spanish→English tiny v1 packs; it also supports external model repositories.
  These records are not proof of better quality than Mozilla's v2 models or OPUS.
- **Desktop scope:** the
  [main window](https://github.com/XapaJIaMnu/translateLocally/blob/1d0d382cd6c9085cb4b8144c4a8962d74ed609c8/src/mainwindow.cpp#L444)
  reads and displays plain text. A successful desktop translation therefore does
  not demonstrate preservation of an imported HTML/EPUB document's rich markup.
- **HTML scope:** the
  [native-messaging handler](https://github.com/XapaJIaMnu/translateLocally/blob/1d0d382cd6c9085cb4b8144c4a8962d74ed609c8/src/cli/NativeMsgIface.cpp#L169)
  passes the request's HTML flag to Bergamot and returns its translated target.
  Its alignment worker queries model alignments for interactive highlighting;
  it is not an independent offline aligner or a model-agnostic rich-text mapper.

**Implication:** efficient local translation does not require a 7B LLM. Our
compact OPUS/Bergamot paths already follow that resource strategy, while semantic
emphasis and output completeness remain separate acceptance problems. If 7B is
impractical, a bounded comparison with an unmodified TranslateLocally release in
HTML mode and its exact model pack is reasonable before changing another engine
or wrapper. A native runtime or older/different pack may produce different output,
but source inspection alone cannot establish an improvement. Keep any such
comparison outside production and private inputs outside the repo. Do not revive
the retired generic alignment stack, promise perfect formatting, or build a local
fork merely because another app uses the same engine family.

### Smaller specialist alternatives — 2026-10-21

Compact translation specialists already exist: the Marian/OPUS/Bergamot models
above serve that niche. Efficient text translation and reliable placement of
semantic emphasis remain separate requirements. A model change must be tested
on both; a smaller parameter count or valid HTML cannot qualify formatting.

| Candidate | Reason to evaluate | Limits before integration |
| --- | --- | --- |
| [TranslateGemma 4B](https://huggingface.co/google/translategemma-4b-it) | Translation-specialized generative model; its [report](https://arxiv.org/abs/2601.09012) includes French, Spanish and Modern Standard Arabic into English. Closest smaller alternative to the proposed 7B screen. | Prescribed language-code chat template, 2K-token input context and no documented HTML-preservation guarantee. Open weights under Gemma terms; official HF downloads require accepting those terms. Quantized community packages are separate from Google's original weights and need provenance/runtime checks. Published results do not establish superiority in our directions. |
| [MADLAD-400 3B MT](https://huggingface.co/google/madlad400-3b-mt) | Multilingual translation-specialized T5 model with Apache-2.0 weights. The hosted conversion documents a 1.65 GB Q4K pack for Candle. | Encoder-decoder integration/tokenization differs from the existing Marian and HY adapters. The Candle GGUF must not be assumed compatible with llama.cpp. No rich-markup guarantee; prefer it as a later text-quality/resource baseline. |

Do not add several providers before establishing useful outputs. The next bounded
screen remains HY-MT2 7B with whole-context tagged input on this machine. If its
resource use or output is unsuitable, evaluate TranslateGemma 4B before production
changes. An unmodified TranslateLocally native-HTML comparison can check whether
its different model/runtime pins help, but it is not an independent formatting
solution. Retain the existing source-owned DOM, assets, cancellation, cache and
writer owners; no new generic alignment stack or model-training project is proposed.

If “Jev-like” refers to a narrow-task model philosophy, translation specialists
fit that analogy. [Jev itself](https://typesafe.ai/blog/introducing-system-one-models-and-jev)
returns typed decisions rather than generated translations; it is not the proposed
translation engine. No alternative weights were downloaded or run in this assessment.

### Bounds and explicit failures

- Default planning uses 900 Unicode scalars per segment and 16 segments per batch.
  OPUS retains whole reader blocks up to 32 KiB UTF-8 for DOM identity, then uses
  sentence/literal planning and the installed tokenizer's actual input budget.
- CTranslate2's Marian adapter reserves an EOS-inclusive safe source budget of
  448 tokens below the 512-position limit. Decoder admission distinguishes empty
  output, token-limit cutoff, missing natural end and overflow. Character caps
  alone are insufficient; oversized indivisible text fails explicitly.
- Exact-piece work is bounded separately: 128 translatable prose pieces and 641
  total metadata units. Protected transport HTML has a 32 KiB bound and at most
  128 source codes, plus hierarchy, identity, literal and depth checks. Restored
  reader HTML has a separate expansion budget that accounts for source-owned
  attributes/literal payloads; it may legitimately exceed the transport bound.
- Internal Bergamot planning caps inputs at 320 scalars with sentence/clause cuts.
  This is not a completeness guarantee, a product-admission bypass or a fix for
  its alignment failures. Worker replies remain bounded and validated.

These limits guard CPU, memory and reconstruction. Do not increase one budget
without checking the connected decoder, batch, cache and renderer limits.

## Architecture and ownership

Keep shared source/data safety separate from provider inference. Formatting
policy is selected once from the admitted request and carried through planning,
cache admission and saving; never infer policy from cached markup's presence.
No engine fork, generic plugin framework or second lifecycle owner is needed.

All Rust paths below are under [src-tauri/src/translation](../src-tauri/src/translation).

| Boundary | Owner and responsibility |
| --- | --- |
| IPC and admission | `commands.rs`, `models.rs`, `capabilities.rs`, `formatting.rs`: window/request/build/pair checks, provider gate and policy |
| Source and shared DOM | `source.rs`, `dom.rs`, `html.rs`: provenance from uploaded-document URL, safe reader snapshot, ordered blocks and owned assets |
| Bounded input | `segment.rs`, `job.rs`, `boundary.rs`, `inline_markup/pieces.rs`: source-owned whole blocks, sentences/literals and exact decoder budgets |
| Protected reconstruction | `inline_markup/protected.rs` and children: source hierarchy, opaque identities, literals, attachments and restoration validation |
| Native engines | `ctranslate2.rs`, `hy_mt2.rs`, `engine.rs`: inference adapters and output contract |
| Worker transport | `bergamot.rs`: one window/owner, nonce, sequence, bounded mailbox, deadlines, stop/cancel and lifetime |
| Worker mapping adapter | `bergamot/adapter.rs`: input admission, opaque HTML, reply binding/restoration and pre-save live-owner confirmation |
| Installation | `model_store.rs`, `model_store/verified.rs`, `model_install.rs`, `model_install/transaction.rs`, `bergamot/assets.rs`: trusted pins, legal payloads, verified delivery and recovery |
| Retired-pack management | `model_store/legacy_alignment.rs`: inventory/notices/removal, never inference |
| Job lifecycle | `runner.rs`, `runner/provider.rs`, `runner/lifecycle.rs`, `state.rs`: job/model claims, validated sequential batches, cancellation and save ownership |
| Resume | `cache.rs`: compatible, source/DOM-bound checkpoints; separate from durable results |
| Rendering | `render.rs`: coverage/order, policy dispatch, protected/review rendering, escaping and serialization |
| Legacy projection | `render/legacy.rs`: private best-effort text/probe fallback for the old provider path; cannot recover invalid sentence/protected claims |
| Durable result | `quality.rs`, `storage.rs`: output sanity checks, upload-backed atomic variant write/list/delete |

The frontend lives under [src/translation](../src/translation):

- `App` mounts `hooks/useTranslationManager.ts` above the conditional tab. This
  remains the single owner of native commands, progress listeners, terminal
  outcomes, worker abort controller, download retry and Library refresh.
  Its `enabled` flag controls listener subscription; a presentation extraction
  must not move the hook into a tab child.
- `components/TranslationPanel.tsx` coordinates selection, capability/busy state,
  model management and job presentation. `components/TranslatedDocumentsPanel.tsx`
  owns the saved list, its disclosure/delete-confirmation/dismissal state and the
  existing callbacks. Confirmation resolves its ID against the latest rows.
  Both views share language/status labels in `utils/presentation.ts`; saved
  `ready` and job `stored` use the existing localized completion label. Dialogs
  retain the house component, accessible labels and focus handling.
- `api/nativeTranslation.ts` and `utils/errors.ts` normalize capabilities,
  WebView support and stable command errors.
- `bergamot/assets.ts` delivers verified binary assets; `bergamot/nativeBridge.ts`
  owns one worker per document attempt, sequence/abort/visibility handling and
  the authoritative Rust save outcome. Stock runtime/glue live in
  [public/translation/bergamot](../public/translation/bergamot).

### Installation, status and removal

Models live in `<app-data>/translation/models/{model-id}`. Trusted manifests bind
paths, byte sizes and SHA-256 pins. Install streams and verifies every file,
checks destination free space, stages beside the destination and promotes with
rollback/recovery. Legal notice payloads are verified with the pack. Failed
retry must re-admit the resulting files; progress alone cannot certify install.

Ordinary native status uses cheap required-file/type/size checks, not a complete
hash scan every render. Auxiliary legacy inventory verifies its pins/notices;
worker delivery verifies immutable byte snapshots and WebCrypto hashes before
inference. These are different trust boundaries, not interchangeable status flags.

`hasStoredFiles` is independent of `installed`: empty/partial/corrupt packs can
still be removed. A known stored size of zero displays zero rather than a full
download estimate. Removal uses trusted model paths and is serialized against
install and active jobs. It does not delete saved translations or segment caches.
Terminated operations cannot be relocked by late progress events.

### Job, cancellation and atomic-save contract

- One attempt owns its job/cache identity and model lease throughout loading,
  inference, validation and save admission. Concurrent identical starts reject.
- Batches run sequentially. Validate the **whole batch** before checkpointing;
  an accepted first result cannot leak into cache if a later result is invalid.
- Cancellation is checked around planning, loads, native calls, cached outcomes,
  checkpoints and save. A native load/inference call may be uninterruptible;
  discard its late result rather than claiming immediate native interruption.
- Worker ownership must still be live at persistence, including fully cached
  retries. Wrong/stale window, nonce, sequence or oversized replies reject;
  each exchange has one absolute deadline. Background/visibility interruption
  cannot masquerade as a completed foreground worker job.
- An atomic save claim serializes cancellation with persistence: cancellation
  before the claim prevents writes; once saving owns the claim, cancellation
  is refused and Rust's result is authoritative. An unknown frontend outcome
  refreshes Library rather than asserting that saving failed or was cancelled.
- Shared upload replacement/delete transactions and recovery protect existing
  variants, indexes and assets on write failure. Model removal is independent
  of document deletion. No existing audio or timing files are rewritten.

Translation deletion validates the exact raw upload ID before storage access and
requires a matching provenance row before touching files. Ownership deletion is
held in an immediate SQLite transaction through staged-file deletion and commit.
Unknown valid IDs are harmless, even if unowned live/staged files exist. The
existing audiobook-reference lock covers the pending/completed reference check
and deletion, so source-reference publication cannot race past it. Failed DB
deletion restores staged files and rolls back provenance/index changes.
Derived replacement uses the same short lock through file recovery, promotion,
DB commit/rollback and backup cleanup. Parsing and inference stay outside it.
Library deletion validates the requested source kind against stored metadata and
returns the stored canonical URL. Audiobook checks compare upload IDs, so query/
fragment aliases in either requests or existing manifests cannot bypass them;
bundled document references retain their exact-URL behavior.
Deletion requires exact stored ID/directory case and rejects case-only aliases
or collisions before staging. Existing mixed-case legacy IDs remain valid;
no URLs, directories or metadata are lowercased or migrated. Audiobook reference
checks conservatively ignore ASCII ID case across supported filesystems.

## Storage, cache and compatibility

There are two distinct forms of storage:

| Data | Current implementation |
| --- | --- |
| Durable variant | Existing upload database/files, shared sections/assets/FTS and a `translated_documents` provenance row; translation schema version `1` in `upload_schema_metadata` |
| Retry/resume | JSON at `<app-data>/translation/segment-cache/{cache-key}/segments.json`, cache version `1`, inline strategy version `3` |

There is **no production `translation_segments` SQLite table**. Metadata records
source URL/nullable source ID, languages, model/engine, quality, settings/glossary,
status, path and timestamps. Deleting the source sets the provenance FK to null;
it does not delete the derived upload. A new job still needs a readable source.
Saved translations open without loading the model that created them.

Both translation and Library single/batch/folder deletion remove the variant's
provenance, upload, section and FTS rows in one transaction. Shared deletion checks
for the optional translation table, so existing version-1 databases need no schema
migration or prior visit to the translation screen. Deleting an original retains
its independent variants and their owned assets. Missing variant files do not
prevent removal of owned metadata; repeated deletion returns not found safely.
Without provenance, translation deletion never cleans up unowned files, including
staging left after a committed deletion whose filesystem cleanup failed. Such
cleanup failures remain explicit errors; this stage adds no orphan-file collector.

Successful saved rows use `ready`; successful job responses/progress use
`stored`. Both use `needs-review` for retained-original results. Listing returns
the saved row's status unchanged, so presentation/tests must distinguish these
contracts rather than supplying a fabricated `stored` row for every success.

Cache keys bind document URL, languages, model, quality, repair, formatting policy
and ordered trimmed glossary entries. Resume additionally checks segment text
hashes, DOM/location/context identity, planning settings, provider identity and
inline strategy. Plain phrase memory cannot supply protected mappings or review
outcomes. Old `aligned` records are ignored. The sentence/review policy tag is
`ctranslate2-sentences-review-v1`, distinct from retired fragment policies.
Native cache identity does not automatically fingerprint every engine binary;
review compatibility deliberately when changing runtime/model semantics.

Saved variant IDs bind source document ID, languages, model, quality, repair,
glossary and the sentence/review policy. They exclude job ID, timestamps and
hardware toggles. Identical settings atomically replace the same variant;
changed supported settings create a separate variant. Source changes invalidate
incompatible segments but do not themselves create a new variant ID.

Settings retain provenance, provider identity, source format, job information,
policy and review counts. Existing saved HTML/metadata/audio are not retrofitted
when cache policy changes. Do not confuse retry receipts with reader documents.

## Builds and CI

Use the existing entrypoints in [package.json](../package.json). Native C++
translation requires CMake and the platform toolchain; the Linux llama/Vulkan
build also needs Clang/libclang, Vulkan headers/loader, SPIR-V headers and `glslc`.
A missing `cmake` executable is a build prerequisite failure, not a model failure.

```sh
npm run desktop
npm run desktop:no-translation
npm run desktop:static
```

`desktop` builds release bundles with native TTS, conceptual search and both
native translation engines. Provider admission still allows only the OPUS pairs.
`desktop:no-translation` retains native TTS and conceptual search. Static builds
use static TTS and omit the shared-build conceptual-search feature; translation
is still compiled unless explicitly disabled. Launch the rebuilt artifact
separately (AppImage on Linux); compiling is not an installed-app smoke test.
Browser/Vite fallback is not native inference acceptance.

### Worker-Only Mobile Build And CI Opt-In (2026-10-19)

This heading preserves the link from [Apple distribution](apple-distribution.md).
Android/iOS default builds do not add the translation host. Explicit opt-in adds
only `native-translation`, without native CTranslate2/llama.cpp, model downloads
or deployment-target changes. **With today's provider gate, a worker-only build
has no runnable public document translator.** It retains management/saved results
and internal host tests; do not install a gated provider as an acceptance shortcut.

```sh
# Android SDK/NDK: existing native TTS/search build plus worker host.
npm run android:apk:native-tts -- --translation-worker
# Custom worker-only APK build without preparing native TTS libraries.
npm run android:apk -- --translation-worker android build --apk --debug --target aarch64
# macOS/Xcode: unsigned simulator and generic iPhoneOS link checks.
npm run ios:ci -- --native-tts --translation-worker
npm run ios:ci:device -- --translation-worker
```

Wrapper flags precede `--`; tokens after that separator belong to the runner.
Worker opt-in during project initialization rejects before SDK work. Existing
custom targets/features/runner arguments remain supported. For Apple setup and
distribution requirements, use the linked distribution guide.

In **CI → Run workflow**, choose the current branch, `validation_scope: mobile`
or `full`, `mobile_translation: true` and `provider_inference: false` for host
compilation. Mobile opt-in defaults off and rejects non-mobile scopes. The APK
artifact is `papercut-android-native-tts-translation-worker-apk`; ordinary release
and default artifact behavior stay unchanged. `provider_inference` runs separate
**search-provider** inference checks, not translation quality validation.

A compiled worker host still requires a secure context, Workers, WebCrypto and
WASM SIMD; verified modules and protected output checks remain mandatory. Build
success does not qualify phone RAM, latency, background execution or translation.

## Validation and acceptance

Use synthetic regression fixtures in tracked tests. Private articles/repros stay
outside tracked code, tests and documentation; do not add named-corpus references
or benchmark helpers to keep them alive. The immutable evidence JSON below is
retained because production asset-pin tests consume it.

Useful existing checks (Cargo `--offline` requires cached dependencies):

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib translation::
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --no-default-features --features native-translation --lib translation::
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --features native-translation-ctranslate2,native-translation-llama --lib translation::
npm test -- src/translation
npm test -- scripts/shipping-builds.test.js scripts/mobile-translation-ci.test.js
npm run build:typecheck
npm run check:i18n
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git diff --check
```

Native-feature checks require the native toolchains. Ignored real-inference tests
are not model-quality passes. A source refactor should run the affected suites;
a documentation-only change need not rebuild every runtime. Historical counts
are bound to their commits in the archive, not fresh CI for the current head.

Focused review of consolidation commit `8213dd9` on 2026-10-21 traced admission,
source/render/cache/save ownership, model verification and build/CI commands.
Assumed one foreground HTML/EPUB job with sequential bounded batches.
Checks passed: 221 base Rust translation tests, 121 frontend translation tests,
36 mocked shipping/CI tests, local/inbound/archive links, unchanged evidence
bytes, command/test-path checks and diff whitespace. Ten CI-script cases first
hit sandbox subprocess `EPERM`; all 11 cases passed outside that sandbox.
No production code changed. This does not add inference, full CI, language-quality
or packaged-device acceptance. The saved-status presentation gap identified
here is addressed by the saved-document extraction below.

### Acceptance cases to retain

This matrix consolidates distinct regression and manual checks. It records
required behavior, **not completed smoke**. Evaluate complete translation quality
separately from HTML fidelity and lifecycle success.

| Area | Required checks |
| --- | --- |
| Scope/admission | HTML and EPUB; PDF/other-format rejection; fixed FR/ES → EN pairs; unsupported quality/glossary/repair rejected; gated downloads/commands reject even with installed files; no OPUS hardware control |
| Offline setup | Explicit model install, disconnect network, translate; no cloud dependency; missing/partial/corrupt pack, hash/size/path/symlink rejection, low space, failed install/retry/rollback and reinstall |
| Management | Confirm/cancel removal, failed removal/retry, zero-byte/partial pack removal and honest size; active-job/install contention; saved documents/cache/audio survive removal; no runnable engine still allows saved access |
| Meaning | FR/ES paragraphs, chapters and long books; negation, adjective/word order, repeated terms/names and terminology across chapters; human review of omissions, hallucinations and awkward prose |
| Whole-sentence styles | Nested bold/italic covering whole sentences restored; partial styles retain the formatted original once with labelled translation and durable `needs-review`; never silently flatten or guess target emphasis |
| Source structure | Headings, paragraphs, tables, nested scopes, original IDs/internal links, ordered lists/numbering, literal islands and copied punctuation/Unicode; source remains independently openable |
| Attachments | Leading/trailing notes, backlinks, images inside styled parents, quotes in either order and inline identities; correct source hierarchy/order, no duplicated labels or lost targets |
| Owned assets | HTML images and EPUB-rewritten chapter assets stay usable after save/reopen; missing/unsafe paths fail safely; translation cannot introduce arbitrary asset URLs or executable markup |
| Bounds/errors | Dense markup, 900-scalar segmentation, real tokenizer limits, long indivisible words, grapheme cuts, empty/cutoff/missing-end output and reconstruction limits; explicit failure, no partial saved variant |
| Progress | Model-loading versus inference states, batch/segment/cache counts; cancellation/failure remain terminal rather than false 100% success; late events cannot erase terminal/review state or relock retry |
| Cancel/restart | During loading, inference, between batches, cached completion and immediately before save; retry after cancel/restart reuses only valid complete checkpoints; duplicate starts reject |
| Resume integrity | Same-URL changed source/DOM, settings/provider/policy mismatch, invalid cache and retired alignment receipts; mixed cache/fresh source order; invalid later batch row cannot checkpoint earlier rows |
| Save/delete | Disk/write/transaction failure and recovery; identical rerun replaces one variant, supported changed settings separate; delete variant leaves original intact; source deletion leaves saved variant readable but new source jobs reject |
| Reader/search/audio | Open original/variant independently; search/Find/narration/read-along include translated text only, exclude review originals/labels across chunks; links remain usable; existing audiobooks/timings unchanged; new TTS uses variant text |
| Presentation | Eight locales, disabled/build/acceptance reasons, narrow layout, light/dark theme, keyboard/focus/dialog labels; saved-result access independent of selection/runtime; job and delete status dismissal independent |
| Resource/platform qualification | Sustained long-book memory/thermals and cancellation latency; packaged desktop and physical-device checks, foreground interruption/background limits; compile success alone is insufficient |

Deferred qualification retains these distinct cases when their providers become
eligible: Arabic undiacritized prose/names/classical terms with RTL source and
LTR target; Chinese academic names, Russian terminology, German compounds;
glossary consistency and chapter repair for engines that actually support them;
HY-MT2 physical Vulkan detection/on/off, software-only Vulkan rejection and
CPU/GPU-compatible cache/variant identity. These are **not current supported
language or mobile quality claims**. Benchmarks, broader smoke and version bump
remain deferred.

### Packaged acceptance handoff — prepared 2026-10-21

**Full matrix not run.** Narrow inspection of a user-provided desktop Spanish
HTML result confirmed the retained-original fallback, not preserved English
emphasis or language-quality acceptance. This is the next manual gate when smoke
is explicitly resumed;
the matrix above remains the full acceptance inventory. Use the rebuilt desktop
app with admitted OPUS models. Today's worker-only mobile build cannot run these
public translation jobs; mobile host compilation is a separate check.

1. Record the tested commit, OS/architecture, artifact path/build time, model ID,
   app locale and whether the result used a fresh job or resume. The unchanged
   app version alone cannot identify this branch's build. Build with
   `npm run desktop`, close the old app and launch the new artifact separately.
   Install/verify both OPUS packs first, then disconnect the network for the jobs.
   Use a disposable library or a backup before deletion checks; keep private
   documents, screenshots and reports outside tracked files.
2. Cover all four language/format combinations: French HTML, French EPUB,
   Spanish HTML and Spanish EPUB. For each, use one plain/whole-sentence-style
   case and one rich case with partial emphasis: eight runs minimum. Include
   negation, word order, names, punctuation and Unicode; spread headings, nested
   bold/italic, links, code/`translate=no`, notes/backlinks, lists and images
   across the rich cases. Include EPUB chapter boundaries and rewritten assets.
   Inspect the imported original first: compare translation with that sanitized
   reader snapshot, and record appearance already lost during import separately.
3. Review meaning against the source, not an exact expected English string.
   Check every short-case passage for omissions, additions, reversed negation,
   broken names and unnatural fragment joins. Then inspect one representative
   longer document per language from beginning through middle to end, including
   chapter terminology and transitions. A fluent opening or successful save
   does not qualify the remaining document. Record unread portions as untested.
4. Check formatting independently of meaning. Validated whole-sentence nested
   styles should remain on the translation. Unresolved partial styles should
   retain each affected formatted original block once beside a labelled English
   translation, with **Saved — Formatting Needs Review** after save and reopen.
   Plain/fully mapped cases should not acquire spurious review originals. Check
   source order, numbering, note targets/backlinks and image placement; neither
   guessed target emphasis nor silently flattened source markup is acceptable.
5. Reopen after restart. Search and reader Find must include translated text,
   excluding retained originals/review labels; use distinct source/target words
   so shared names do not make this check ambiguous. With an installed TTS voice,
   narrate across reviewed and ordinary blocks: no repeated source/labels, and
   read-along stays on translated text. Open an existing audiobook too; its audio
   and timings must remain usable. Keep original and variant independently openable.
6. On disposable copies, cancel before saving and retry with unchanged settings;
   valid cached work may resume, but no partial variant may appear. An identical
   completed rerun replaces one variant rather than duplicating it. Delete a
   variant and check the source survives; separately delete a source and reopen
   its saved variant/images. Pending/completed audiobook references must block
   variant deletion through saved-translation and Library controls, and original
   source deletion through Library. Do not remove real audio to make a check pass.

Keep a local receipt per case: `commit/artifact | language/format | model |
fresh/resume | meaning pass/fail/untested | formatting pass/fail/untested |
reader/search/audio pass/fail/untested | lifecycle pass/fail/untested | notes`.
Record the passage/location and expected versus observed behavior for failures;
redact document contents/paths before sharing. Report actual review outcomes,
not just terminal job status. A correct `needs-review` result passes the fallback
contract only if its usability is acceptable; it is not exact target-word styling.
Any material meaning loss, missing content/assets, false completion, duplicated
narration or damaged saved work blocks qualification. Leave failed/untested gates
open and fix concrete failures before expanding providers or release claims.
This handoff adds no benchmark, model inference, fixture, script or version bump.

## Saved-document presentation — implemented 2026-10-21

`TranslatedDocumentsPanel` now owns presentation only. The parent passes rows,
model names and existing open/delete callbacks; `App` and `useTranslationManager`
still own native operations, worker lifetime, job results and Library refresh.
No new store, listener, IPC, dependency, CSS or localization key was introduced.
Disclosure/dismissal state survives prop refreshes while the view is mounted;
local presentation state resets when the view unmounts. No inference, provider
admission, cache, metadata, original document, audio or timing behavior changed.

Panel integration regressions cover actual saved `ready` versus job `stored`,
`needs-review` and existing `stored` handling; access without a source/runtime or
model; model-name fallback; delete confirmation/cancel/focus restoration; target
removal or metadata refresh while confirming; failed deletion/retry; collapse
across updates and empty refresh; independent job/delete status dismissal and
new outcomes. Removing `ready` localization makes the new real-row test fail.

Validation: 132 frontend translation tests passed, including 39 Panel cases;
TypeScript, changed-file ESLint, all eight locale checks and diff checks passed.
Evidence JSON is unchanged. These are synthetic interaction/lifecycle checks,
not browser-layout, native-inference or packaged-device acceptance.

Focused review of `f4cbc27e` traced both views, shared label callers, the parent
manager/App wiring, deletion API/storage boundary and house dialog/disclosure.
Assumed one foreground job and a local saved library. No actionable correctness,
security, data-loss, performance or unnecessary-abstraction finding was identified
in this extraction. The saved list/dialog JSX is retained, label logic differs
only by localized `ready`, and no second async owner or per-row command was added.
Fresh checks passed: 137 translation/dialog tests, TypeScript, changed-file lint,
eight-locale validation, local/inbound/history links, evidence identity and diff
checks. Disclosure/dismissal resets on view unmount are presentation-only;
manager-owned jobs and results survive conditional tab presentation as before.

## Branch integration/readiness review — 2026-10-21

Reviewed `4570dbf6` against local `main` (`8ebc9d0f`), using merge base `871d7026`:
118 branch-changed files, including tests, stock worker glue and retained evidence.
Main has one additional README-only commit; no merge or remote fetch was performed.
Assumed one foreground HTML/EPUB translation job with a local library;
crafted IPC requests still need
native validation. Traced import/sanitization, shared DOM/assets, sentence/review
planning, batch/cache admission, cancellation/save ownership, derived upload
storage, reader/search/TTS consumers, model management and build/provider gates.
No new inference, private corpus, provider re-admission or production changes.

The current separation is useful: OPUS keeps sentence context with explicit
review originals; shared source/cache/writer guards remain connected; worker
transport and mapping are separate; gated providers cannot become runnable from
installed files. Search/Find/narration exclude review originals/labels. Desktop
builds compile native engines; mobile opt-in compiles only the worker host and
still has no admitted public provider. Passing these paths' existing tests is
not a language-quality or packaged-platform acceptance result.

### Deletion-safety corrections — 2026-10-21

The integration review's three confirmed findings are addressed in native storage:

1. **P1 — Unchecked identity/ownership:** validate raw hexadecimal IDs before any
   storage bootstrap. The derived deleter requires provenance deletion to affect
   a row inside its immediate write transaction before staging files. Original
   HTML/EPUB/PDF IDs and unknown IDs cannot remove source files or index rows.
2. **P1 — Audiobook dependency bypass:** derived deletion uses the existing
   process-wide source-reference lock and checks the canonical variant URL.
   Pending and completed references reject deletion without changing reader,
   asset, index, provenance, audio or highlighting data.
3. **P2 — Ghost saved translations after Library deletion:** shared row deletion
   removes optional translation provenance atomically with upload/search rows.
   Single, batch and folder commands reuse it; source deletion still nulls only
   the source FK and preserves independent variants. No schema migration is needed.

Permanent synthetic IPC regressions live in
`src-tauri/src/translation/deletion_tests.rs`, registered through the existing
admission-test fixture. They use the real SQLite/reader/asset writer and actual
commands, without loading a model. Coverage includes malformed/unknown/original
IDs, both deletion screens plus batch/folder, zero/missing files, conflicting
staging, transaction failure/rollback, interrupted pre-commit staging/retry,
repeat deletion, pending/completed/unrelated audio references, serialization
with pending-reference publication, and original-source deletion semantics.
The original six regression tests were run before the production fix: five
failed, reproducing all three findings; the source-retention case already passed.

Validation after the fix: 509 base Rust library tests passed (4 ignored), 230
native-feature translation tests passed (2 ignored), 231 worker-only translation
tests passed, and 535 frontend/script tests passed (1 skipped). TypeScript,
eight-locale validation, Rustfmt, links/evidence and diff checks passed. Existing
immutable evaluation evidence is byte-identical. This is storage/lifecycle
validation, not language-quality, packaged-platform or new inference acceptance.
Cross-platform CI/linking, installed-app behavior, current dependency advisories
and real-device resources were not revalidated here.

### Save/delete serialization and URL identity — 2026-10-21

The [focused deletion review](https://github.com/muhannadnouri/papercut.io/blob/30d2344e0cef78f83b500acd1e520a92c2a60d49/docs/offline-translation.md#deletion-safety-review--2026-10-21)
reproduced two remaining P1 gaps: a save/delete overlap could leave a saved row
without live files or backup, and Library URL aliases/wrong extensions bypassed
canonical audiobook references. Both are addressed in shared native storage:

- `document_uploads/derived.rs` reuses the existing source-reference lock for
  the complete file/SQLite replacement, including recovery and rollback. Both
  deletion routes already hold this lock. Connections are opened before saving,
  without retaining a write transaction while waiting for the lock; mutation
  takes the reference lock before its SQLite transaction. No new job manager,
  lock registry, provider gate, schema, cache identity or dependency was added.
- `document_uploads/pipeline.rs` resolves stored upload metadata before deletion,
  rejects HTML/PDF kind mismatches and returns the canonical stored URL. When
  metadata is already gone, canonical identity still permits staged-file retry.
  `native_tts/references.rs` compares upload IDs on both sides of the reference,
  protecting pending/completed sources even with legacy query/fragment aliases.
  Existing bundled references still use exact matching; audio/timings are untouched.

Four new regression tests extend the existing synthetic native fixture. Controlled
metadata callbacks coordinate both operation orders and save/delete failures;
assertions cover HTML/assets, source preservation, provenance/index rows and
staging/backup cleanup. IPC cases cover 48 HTML/PDF × reference/request alias ×
single/batch combinations, wrong-kind rejection after removing audio, successful
canonical responses and protected staged retries without metadata. Existing
reference tests also cover alias manifests and unchanged bundled matching.
The overlap and aliased-request assertions failed against the prior production
code before the fix. No private samples, model weights or production test hooks
were added; concurrency checks live in `translation/mutation_tests.rs`.

Validation: 513 base Rust library tests passed (4 existing ignores), 234
native-feature translation tests passed (2 existing ignores), 235 worker-only
translation tests passed, and 535 frontend/script tests passed (1 existing skip).
TypeScript, eight locales, Rustfmt, documentation links/evidence and diff checks
passed. Evaluation evidence is byte-identical. The explicit post-commit cleanup
limit above is unchanged; this does not add inference, language-quality or
packaged-platform acceptance.

### Exact-case deletion identity — 2026-10-21

The [case-alias review](https://github.com/muhannadnouri/papercut.io/blob/03c12ccc7058ac9c59f307ee6124a21f4aaeae2a/docs/offline-translation.md#mutation-and-identity-follow-up-review--2026-10-21)
identified uppercase IDs bypassing exact DB/reference matching while a filesystem
could resolve them to lowercase storage. The shared destructive-boundary rule is
now **unique, exact stored case**, rather than globally lowercasing existing IDs:

- `store::validate_upload_deletion_id` rejects another upload ID differing only
  by ASCII case before either translation or Library deletion proceeds. This
  includes ambiguous case-only metadata collisions, even when an exact row exists.
  Syntax validation/read parsing and transfer restoration retain legacy ID case.
- Shared staged deletion checks actual live/staged directory names before any
  recovery/rename. Missing metadata cannot turn a case alias into a cleanup retry;
  exact-case retries still work. Errors roll back owned provenance and leave
  reader/assets/index/audio data intact. Batch/folder routes reuse the same gate.
- Audiobook reference matching ignores ASCII case for upload IDs on both sides,
  conservatively protecting potentially shared storage; bundled exact matching
  remains unchanged. No source URL, ID, directory, audio or timing migration occurs.

Four permanent synthetic IPC regressions in `translation/case_identity_tests.rs`
cover upper/mixed-case HTML/PDF requests with/without audio, exact mixed-case
legacy variants through translation/single/batch deletion, referenced staged
retries without metadata, live/staged filename aliases and conflicting metadata.
All four failed before the production fix and pass afterward. These assertions
exercise rejection on case-sensitive Linux too, without pretending to emulate
Windows filesystem aliasing. A real case-insensitive/device run remains deferred.

Validation: 517 base Rust library tests passed (4 existing ignores), 238
native-feature translation tests passed (2 existing ignores), 239 worker-only
translation tests passed, and 535 frontend/script tests passed (1 existing skip).
TypeScript, eight locales, Rustfmt, links/evidence and diff checks passed. No new
schema, cache identity, provider admission, model, dependency or app version.
Benchmarks, new inference, smoke and cross-platform CI remain deferred; structural
storage validation does not establish translation quality or release readiness.

### Exact-case deletion review — 2026-10-21

Reviewed `3cca1b6b` and its connected translation/Library single, batch and folder
deletion paths, shared save/delete/reference lock, SQLite ownership/rollback,
live/staged filesystem identity, transfer ID parsing and audiobook references.
Assumed one running app with one foreground translation job and concurrent
local deletion commands. No new actionable correctness, safety or test findings;
the scoped deletion fix is ready. Exact mixed-case legacy identities stay usable,
case aliases and ambiguous metadata fail closed, and rejected operations preserve
reader/assets/index/provenance/audio data. The documented post-commit cleanup
limit remains unchanged.

Fresh validation: 517 base Rust library tests passed (4 existing ignores), 238
native-provider translation tests passed (2 existing ignores), and 239 worker-only
translation tests passed. These include all four case-identity regressions and
both coordinated save/delete order tests. No production changes were needed.
Actual case-insensitive filesystems, packaged devices, inference and fresh
cross-platform CI were not checked. This review does not qualify language quality.

## Next bounded work

Deletion safety has no remaining finding from its review. The
[best-effort emphasis screen](#best-effort-emphasis-evaluation--2026-10-21) was
rejected, and the [Firefox/HY screen](#firefox-dom-and-hy-mt2-18b-evaluation--2026-10-21)
does not qualify either provider. Evaluate a larger tag-aware HY model before
another production mapping change; use Firefox's DOM ownership as an architectural
reference, not evidence of solved completeness or semantic styling. Representative
FR/ES meaning, review usability and rebuilt-app integration remain required under
the [prepared handoff](#packaged-acceptance-handoff--prepared-2026-10-21).
Further packaged acceptance execution stays pending until smoke is explicitly resumed.
Keep the existing source/DOM, cache, writer and reference-lock owners. Provider
admission is unchanged; strict stays retired. Benchmarks, smoke and version bump
stay deferred.

No further UI split is justified by this extraction review alone. Model/language
selection, provider admission and shared busy conditions remain coordinated in
the parent; model actions already reuse one button helper. Retain those owners
unless a concrete independent responsibility needs extraction. A shorter file
does not establish better translations or release readiness.

Release acceptance remains a separate gate: representative FR/ES meaning and
review-original usability, HTML/EPUB search/TTS/read-along and rebuilt packaged
behavior. No architectural cleanup certifies it. Bergamot completeness, arbitrary
semantic formatting projection, Arabic/new providers, mobile translation quality,
glossary/chapter repair, export/backup policy and variant grouping remain future
work; do not reopen an aligner integration or library fork without new evidence.

## History and evidence

The [complete pre-consolidation guide](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md)
retains research inputs/methods, decisions, stage-specific tests and review
receipts. Recover it offline with
`git show bdf65cd6:docs/offline-translation.md`. Its historical commands and planned
stages are **not current build instructions**. Useful entry points:

- [Bergamot versus OPUS evaluation and limitations](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#bergamot-versus-opus-mt-evaluation--2026-10-18)
  and [protected mapping integration](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#protected-rich-markup-integration-2026-10-19).
- [Alignment feasibility, stages and rejection evidence](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#engine-independent-offline-alignment-plan--2026-10-19),
  including auxiliary/fused ownership, OmniAlign FP32/four-bit and official CT2
  attention checks. [Last strict-code snapshot](https://github.com/muhannadnouri/papercut.io/tree/b0ecf08e8d7d4d8a0d029b26cf7e2743ebcf0987/src-tauri/src/translation).
- [Sentence/review decision and regressions](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#whole-sentence-translation-with-review-originals--2026-10-20),
  [provider admission](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#provider-admission--2026-10-21)
  and [strict retirement](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#strict-alignment-pipeline-retirement--2026-10-21).
- [Worker/renderer separation and review](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#worker-and-rendering-boundary-evaluation--2026-10-21)
  and [UI/documentation ownership evaluation](https://github.com/muhannadnouri/papercut.io/blob/bdf65cd6f992953086067eb85c1fa490c7793392/docs/offline-translation.md#ui-and-documentation-consolidation-evaluation--2026-10-21).

[offline-translation-evaluation.json](offline-translation-evaluation.json) remains
byte-identical. It records the 2026-10-18 isolated Linux evaluation and exact
Mozilla runtime/glue/model pins; Rust asset tests and the JS worker test load it.
Its process-level RAM, non-equivalent runtime/kernel/beam configurations, small
authored cases and non-blinded inspection cannot establish app memory, a model
ranking, phone performance or release readiness. Further benchmarks stay deferred.

For upstream API context, consult [CTranslate2](https://opennmt.net/CTranslate2/),
[ct2rs](https://docs.rs/ct2rs),
[Firefox's Bergamot documentation](https://firefox-source-docs.mozilla.org/toolkit/components/translations/resources/03_bergamot.html)
and [W3C ITS within-text flow](https://www.w3.org/TR/its20/#elements-within-text).
These references explain engine/markup boundaries, not new local verification.
