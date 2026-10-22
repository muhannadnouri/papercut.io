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
Existing translations, source documents, audio and highlighting are not rewritten.
The additive translation schema v2 stores review evidence for new assisted results;
older results without that evidence remain readable and read-only for formatting.

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

New OPUS results also retain immutable, generation-owned source/English evidence
and resumable B/I drafts. Stage 3 supplies native draft commands and a pure preview.
Stage 4 adds guarded native publication; a user-facing review editor is still pending.
Saving a draft does not change
the current bilingual fallback, Library status, search, narration or saved audio.
Explicit Finish replaces the fallback only after the current draft passes its
approval, structural and consumer checks. The review UI is the next stage.

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
IDs, notes, links, media and literals. This led to the separate
[7B Q4 screen](#hy-mt2-7b-q4-quality-screen--2026-10-21); size alone was not
evidence that it would pass. No upstream fork or local runtime patch is proposed.

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
The later Q4 screen below records actual memory with an 8192-token context;
allow additional headroom for the app and other workloads rather than inferring
capacity from weight size. Q8 allocations remain unmeasured. Use conventional official
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
or present mixed-language output as complete. This initial feasibility assessment
did not load weights; the later quality screen below records the actual Q4 run.
No throughput benchmark or acceleration result is claimed.

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
emphasis and output completeness remain separate acceptance problems. A native
HTML comparison was considered, but the maintainer has now declined further
provider/runtime evaluations; see the [conclusion](#conclusion-and-development-direction--2026-10-21).
A native runtime or older/different pack may produce different output, but source
inspection alone cannot establish an improvement. Do not revive
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
| [Phi-4-mini-instruct 3.8B](https://huggingface.co/microsoft/Phi-4-mini-instruct) | MIT-licensed general-purpose model whose documented languages include French, Spanish and Arabic; a smaller generative comparison candidate. | Its multilingual/reasoning and instruction-following evidence does not establish translation-specific HTML fidelity. Apply the same protected transport and full acceptance set; qualify its exact quantization, template and runtime before integration. No formatting advantage has been measured here. |
| [TranslatePsy-EuroNano](https://huggingface.co/qvac/TranslatePsy-EuroNano) | Apache-2.0 Marian/Bergamot translation models. One `xx-en` pack covers French/Spanish → English; quantized Tiny/BaseMemory/Base weights are about 17/31/42 MB, plus vocabulary/shortlist files. | No Arabic coverage. These are Bergamot assets, not GGUF or a drop-in CTranslate2 pack. [Current-runtime evaluation](#euronano-through-the-existing-bergamot-runtime--2026-10-21) requires omitting the supplied shortlist for useful text, but still fails meaning/markup acceptance; keep it unavailable for production. |

Do not add several providers before establishing useful outputs. The
[revised 7B profile](#revised-7b-profile--2026-10-21) also failed acceptance.
EuroNano was selected for a feasibility check through the existing unmodified
Bergamot runtime. The [evaluation below](#euronano-through-the-existing-bergamot-runtime--2026-10-21)
is complete and does not qualify it. This is not a measured quality ranking over
Phi or other models. The [official alternative toolkit](https://github.com/tether-ai-research/qvac-translatepsy-euro-nano)
provides text inference and explicitly lacks an HTML translator; adopting it or
the QVAC SDK would not itself solve placement. The current EuroNano screen fails
the meaning/markup gate; keep Bergamot gated. Prior completeness counterexamples
must also pass before any future admission.
Phi remains unevaluated; no further candidate comparison is scheduled. None of
these model cards certifies bold/italic correspondence. Retain the existing source-owned DOM, assets,
cancellation, cache and writer owners; no new generic aligner or model-training
project is proposed. Further TranslateLocally evaluation was declined; its
different model/runtime pins are not evidence of solved mapping.

If “Jev-like” refers to a narrow-task model philosophy, translation specialists
fit that analogy. [Jev itself](https://typesafe.ai/blog/introducing-system-one-models-and-jev)
returns typed decisions rather than generated translations; it is not the proposed
translation engine. TranslateGemma, MADLAD and Phi weights have not been downloaded
or run in these investigations; EuroNano's measured results are recorded below.

### HY-MT2 7B Q4 quality screen — 2026-10-21

**Evaluation only; provider admission is unchanged.** The official
[Q4_K_M GGUF](https://huggingface.co/tencent/Hy-MT2-7B-GGUF/tree/ab8472660ac61fac25f1af43fac2599d52a8a775)
at revision `ab8472660ac61fac25f1af43fac2599d52a8a775` was downloaded to an
owner-only disk-backed cache outside the repo. Its 4,624,648,896 bytes and SHA-256
`9f96256500f3fc1ab4d64336b58f52a949a95ad7516b0c229476eef782f9f77b`
were verified before loading. Private inputs, harnesses, raw replies, receipts and
inert previews remain outside the repo; no document text was sent to a remote service.

The exact twelve French/Spanish synthetic cases and two private paragraphs were
recovered after temporary files were cleared. Both input-file hashes match the
original 1.8B receipts byte-for-byte. Each case ran once with each of the same two
predeclared prompts: official structured-data and concise translation-only HTML.
Installed `llama-server` commit `f174553e1` ran offline over a private Unix socket,
CPU-only, six threads, one slot, 8192-token context and 4096-token output cap.
Sampler settings match the corrected 1.8B screen: temperature 0.7, top-p 0.6,
top-k 20, repetition penalty 1.05 over the context, min-p disabled and seed 42.
The 7B-specific embedded chat template was verified against both pinned GGUF
metadata and the official base model, rather than forcing the 1.8B template.

All 28 requests returned output with API finish reason `stop`, with no request
errors or reported length cutoffs. The server's observed peak RSS was about
8.94 GiB for this run, including mapped model memory and runtime allocations.
This establishes that the candidate runs on this machine; it is not whole-app
memory, a throughput benchmark, mobile evidence or GPU qualification.

| Prompt | Useful observed results | Remaining counterexamples |
| --- | --- | --- |
| Official structured-data | Correct reordered noun emphasis, nested noun-phrase styling, first-occurrence styling for a repeated word, whole-sentence styling, quantities and the French quotation/note/link/image case. Both private paragraphs retained their main propositions and apparent emphasis regions, without instruction leakage or the earlier negation reversal. | A protected code element's `translate="no"` attribute became visible code text. Quoted Spanish negation gained a bold interjection while the grammatical negation remained unbold. Drop caps expanded to whole-word bold. One private technical expression acquired awkward additional wording. |
| Concise translation-only | Preserved the code literal and the private paragraphs' main formatting/propositions. | Added bold to an unstyled adjective, expanded a nested italic scope, retained an untranslated French greeting and repeated the quoted-negation problem. One private metaphor was softened. |

Parser-valid HTML, unchanged attributes and equal tag ancestry did not establish
correct semantic emphasis. Literal auditing must compare the entire immutable
element payload and attributes, not merely find its original text somewhere in
the target. Whole-word drop-cap styling is a typography change; split negation
also needs a stated ownership policy, not an arbitrary exact-tag-count score.
Do not combine the best outputs from different prompts on individual cases.
These non-blinded, single-seed screens support concrete counterexamples, not a
general quality percentage. Q4 versus historical Q8 and different model variants
prevent attributing the observed improvements solely to parameter count. Recovered
historical printed outputs are not byte-identical original raw-response files.

**Packaging caveat:** the pinned GGUF declares primary EOS token 3 (`$`), while the
[official base configuration](https://huggingface.co/tencent/Hy-MT2-7B/blob/9b0eb4e8f001def3e5ff6469a0ac96fdb39ec223/config.json)
uses 127960 (`<|eos|>`), and its
[generation configuration](https://huggingface.co/tencent/Hy-MT2-7B/blob/9b0eb4e8f001def3e5ff6469a0ac96fdb39ec223/generation_config.json)
also permits 127967
(`<|extra_5|>`). Separate stock-chat pilots completed correctly, including a dollar
amount, but API `stop` reports do not independently identify the terminal token.
No weight edits, runtime patches or metadata overrides were applied. Resolve
termination qualification before admission; CLI chat behavior must not be assumed
equivalent to Papercut's current raw-token Rust adapter.

The recommended source-owned transport trial is recorded below. Keep the
candidate unavailable for production jobs; no model fork, generic aligner revival
or production code change is justified by this direct-HTML screen.

### HY-MT2 source-owned HTML trial — 2026-10-21

**Evaluation only; structural success, quality admission still blocked.** The
same fourteen source cases ran once with the official structured-data prompt
fixed, using the same pinned 7B Q4 weights, stock runtime, template and sampler
described above. Only transport representation changed. All private inputs,
disposable code, replies, receipts and previews remain outside the repository.

A disposable source copy exercised the existing sanitizer → `DomPlan` →
`ProtectedSource` → `ProtectedPlan::restore` path. Source `em`, `strong`, `b` and
`i` use real tags with opaque identities; source code literals use a `code`
placeholder. Other identities use spans. Original attributes, link destinations,
media and literal payloads remain in the Rust plan and never enter the prompt.
Strict XML admission checks the expected tag/identity and attributes before
converting the transport back to canonical spans for unchanged reconstruction.
No forgiving parser repair, prose rewrite or guessed emphasis placement occurs.

All fourteen replies reported `stop` and passed reconstruction. Exact restored
literal payloads, reference attributes, note payloads and media matched the
source-owned baseline. In particular, the French code/attribute corruption from
the direct-HTML screen was avoided. The public fixtures' unannotated superscript
links remain editable link text under the existing contract; their unchanged
numerals were observed, not made immutable by this trial. Explicit note-role
payloads retain the existing opaque protection and placement checks.

Inspection found correct adjective reordering, nested/repeated-word emphasis,
sentence styling and quantities. Both private paragraphs remained apparently
complete with the expected emphasis regions; one technical expression improved.
Spanish initial-letter styling stayed on the initial, while the French initial
still expanded to a whole bold word. Two important counterexamples remain:

- Spanish `No vendrá mañana` became **“No one will come tomorrow”**, changing an
  implied subject's absence into a claim that nobody will come. Valid identities
  and markup cannot detect this semantic change.
- Split French `ne`/`pas` emphasis landed on an added “it” in “I don't know it.”
  Preserving two source identities did not establish correct target ownership.

Validation: fourteen source roundtrips, fourteen malformed/mutated reply rejection
checks and all twenty existing focused protected-DOM tests passed in the
disposable copy. Rejections cover missing/duplicate/reparented identities,
changed literal placeholders, tag/attribute injection, namespaces, comments and
oversized output. Raw replies, canonical transport and restored HTML are retained
separately; the preview excludes active resources. These checks establish the
mechanical boundary, not bilingual acceptance, semantic completeness or a quality
percentage. This remains a small, single-seed, non-blinded inspection, including
an independent code-agent review. The stock GGUF termination caveat above remains
unresolved; no runtime or model patch was applied.

The subsequent review inspected all fourteen reconstructed source/English pairs,
including a static browser render of the private preview. The nine other synthetic
passages showed no obvious material meaning or emphasis error; the two private
paragraphs still require the maintainer's terminology/register judgment. The
Spanish subject change is a meaning failure, French split-negation styling is
unresolved, and French initial-letter expansion is a typography deviation. These
are separate outcomes, not an aggregate accuracy score or bilingual sign-off.
Image fidelity is outside this semantic acceptance decision at the maintainer's
request; no additional image investigation is required. Existing asset safety
checks remain mandatory. No new generation or packaged-app smoke occurred.

**Decision:** retain this source-ownership design as a viable integration seam,
but keep the 7B candidate gated. The [semantic acceptance gate](#semantic-acceptance-gate--2026-10-21)
below is now defined; the current outputs do not pass it. More DOM repair cannot
solve an incorrect translation. Any further quality screen must use these
criteria before integration, rather than redefine success around the outputs.
Broader benchmarks, packaged smoke and the version bump remain deferred.

### Revised 7B profile — 2026-10-21

**`fidelity-first-v1` failed the frozen acceptance gate; keep 7B gated.** After
committing the criteria as `5662c70`, one global prompt revision was frozen before
generation. It adds the trusted source-language label and prioritizes full-sentence
meaning, referents, negation, quantities and faithful grammar; formatting must not
invent prose merely to carry an identity or widen an initial-letter style. The
initial-letter rule is an evaluation constraint, not an approved general inference
about source typography. No per-case translation hints or alternative prompts
were used. The template SHA-256 is
`163c78e04e3bb9ac6eea5c52a98d174160e358f59e60d4d6e18e16f791ac8caa`.

All twelve synthetic cases and both private paragraphs ran once. Source bytes,
protected transport/plans, 7B Q4 weights, stock runtime/template, seed and sampler
match the preceding source-owned trial. Expectations and rendered prompts were
hashed before generation. All fourteen requests reported `stop`; thirteen passed
the unchanged strict renderer. Raw replies were not repaired or selected against
another profile. Results, receipts, rejected output and an inert preview remain
private and outside the repo.

| Case group | Revised result |
| --- | --- |
| Spanish quoted negation | The original subject error was not resolved faithfully: the reply isolates a quoted “No” and then asserts “he will come tomorrow.” This breaks polarity and quote/note association despite valid identities |
| French split negation | Both protected identities are missing, so reconstruction rejects with `Missing semantic identity`. Bold “anything” still has no defensible correspondence to the source negative particle and broadens the statement |
| Initial-letter styling | French still widens to the whole word; Spanish regresses from initial-only styling to the whole greeting and comma. These remain typography deviations, not preserved target scopes |
| Other eight synthetic passages | No obvious new material meaning or emphasis failure found in this inspection; nested/repeated scopes, quantities and literals retain the previous useful results |
| Two private paragraphs | Apparently complete with corresponding emphasis; terminology/register still need maintainer judgment. One rhetorical expression was softened relative to the source |

Fresh checks passed: fourteen source roundtrips and fourteen trust-boundary
mutation rejections, plus strict admission/audit of all fourteen new replies.
The single rejected reply remained rejected; thirteen structural admissions are
not thirteen quality passes. Independent code-agent review covered all cases,
and static preview inspection confirmed the visible failure. API `stop` still
does not close the native termination caveat. This is a regression screen on
known cases, not held-out evidence, a performance benchmark or human sign-off.
Image fidelity remains excluded from this decision.

**Decision:** do not integrate this profile or start another prompt iteration
automatically. The revised instruction did not meet the criteria and introduced
regressions. A different candidate needs its own unchanged full-set evaluation;
do not substitute whichever profile looks better on individual passages. No
production source, model admission, app data or version was changed. Benchmarks
and broader smoke remain deferred.

### EuroNano through the existing Bergamot runtime — 2026-10-21

**Evaluation complete; do not integrate this pack yet.** The official
[TranslatePsy-EuroNano Base `xx-en` INTGEMM pack](https://huggingface.co/qvac/TranslatePsy-EuroNano/tree/3a5e1e4e2f7001ff5cfd79785bef03cf19281b73/xx-en/Base/intgemm)
ran through the unchanged checked-in Bergamot loader and pinned Mozilla WASM
`v0.6.0+eea6e5a8`. One shared vocabulary handles French/Spanish → English without
a language prefix. The weights, shortlist and vocabulary were verified against
revision `3a5e1e4e2f7001ff5cfd79785bef03cf19281b73` before loading:

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `model.intgemm.alphas.bin` | 42,993,131 | `50f86c092f4c55055eab7c9052e3c952d38ef601d906d375e6245c0ba129fc2b` |
| `lex.50.50.xxen.s2t.bin` | 840,864 | `28d640429f5aa836ab0a527959476e0333dbe02dcac4f8dfe883c277bdeb7e65` |
| `vocab.spm` | 797,560 | `772276ad54e95628b7ded9a835b6a9eaf08e76b8e754bc57b99bf1cd3d99c138` |

The complete published pack is 44,631,555 bytes. This is download size, not RAM.
The model card declares Apache-2.0; no pack was redistributed or installed into
Papercut. The current production asset reader's 32 MiB per-file limit would also
reject these weights. Do not raise it or replace worker pins based on this screen.

**Method:** all twelve frozen synthetic cases and both private paragraphs ran
in each of two separately declared transports: current opaque protected spans,
and the preceding HY trial's semantic emphasis/code tags with opaque identities.
Both use the existing Rust sanitizer/DOM/source-owned plan and unchanged strict
restoration. These are identical whole-block inputs for comparison, not execution
of Papercut's 320-scalar Bergamot document planner or a packaged job. The stock
beam-1 config, `int8shiftAlphaAll`, soft alignment, one shared vocabulary, aligned
buffers and HTML response options match the worker. An external Node VM calls the
same module/service API because the production worker intentionally rejects
unrecognized asset pins. Its fallback GEMM is not Firefox's optimized kernel.
No library/loader patch, model conversion, new dependency, output repair or
per-case selection was used. Images remain outside semantic acceptance.

**The supplied shortlist is incompatible with useful translation in this setup.**
With all three published assets, even plain greetings became incoherent text or
just `.`. All fourteen cases per transport lost meaning; only one reconstructed
mechanically. As a harness control, Papercut's verified Mozilla French and Spanish
packs both returned “Hello, how are you?” through the same runtime/config. A
separate supported no-shortlist diagnostic then recovered sensible EuroNano
greetings. The [official model example](https://huggingface.co/qvac/TranslatePsy-EuroNano#quick-start-qvac-sdk)
loads model/vocabulary without a shortlist; [Bergamot supports that path](https://github.com/browsermt/bergamot-translator/blob/main/src/translator/translation_model.cpp#L26-L44).
A read-only vocabulary/shortlist audit found that all 24,042 word-boundary-prefixed
SentencePiece rows have no row-specific shortlist entries, and none of the
146,202 target edges names such a piece. Shared/global candidates can still be
available to the decoder. Mozilla's French shortlist covers 24,686 of its 24,713
corresponding rows. This and the recovery isolate the supplied shortlist as a
practical blocker for this setup; the cause of its coverage pattern and its
behavior in other runtimes are not proven.
Do not label the garbled run a measurement of the model's intrinsic quality.

The **complete fourteen-case set was rerun without the shortlist**, with the same
weights, vocabulary, runtime/config and two transports. Original results remain
separate. All requests returned text; this API does not establish semantic
completeness or expose a native decoder finish reason. Each arm admitted six
replies structurally and rejected eight for empty target mappings:

| Frozen case group | Shortlist-free result in both transports |
| --- | --- |
| Split French negation, Spanish adjective and repeated noun | Basic English meaning survives, but empty emphasis identities are correctly rejected |
| Nested French emphasis | Meaning survives and reconstruction passes, but “little” loses its original bold scope |
| French/Spanish initial-letter styling | Split/untranslated greetings, including “H wave, friends.”; empty mappings are rejected |
| French/Spanish note/link cases | Omitted chapter instruction or changed subject, displaced/changed note text and empty mappings; rejected |
| French/Spanish literals and whole-sentence styling | Useful meaning and corresponding scopes; the semantic-tag French literal adds a grammatical agreement error |
| Spanish directive/quantities | Prohibition becomes a statement about “He”; bold covers only “books;” and italics only “two”, despite structural admission |
| Private paragraph 1 | Consequential mistranslation and misplaced/narrowed emphasis despite structural admission |
| Private paragraph 2 | Material omissions/mistranslation and an empty emphasis mapping; rejected |

Independent code-agent inspection covered all replies. Seven per arm retained
basic meaning, with the French literal fluency caveat; only the literal and
whole-sentence cases combined useful meaning with corresponding scopes. These
are known-case observations, not an accuracy percentage or bilingual sign-off.
The transports produce the same visible text in thirteen cases; disclosing real
emphasis tags does not resolve the alignment failures. Valid restored HTML still
does not certify scope or meaning. Do not weaken empty-mapping guards or silently
repair placement to admit this candidate.

Private receipts, original/follow-up replies, source-owned reconstructions,
profiles, asset hashes, shortlist audit and inert preview are retained under the
external `euronano-bergamot-20261021` evaluation cache. Every arm passed fourteen
source roundtrips and fourteen malformed/mutated reply rejection checks through
the disposable source copy. The checked-in evaluation JSON and production code
remain unchanged; no private text, fixtures or harness was added to the repo.

**Decision:** stop integration of this pack in the current worker. The maintainer
declined further model/runtime evaluations, including TranslateLocally. Do not
regenerate the shortlist, fork the engine or reopen generic alignment to conceal
these failures. Bergamot and 7B remain gated; benchmarks, packaged/device smoke
and the app version bump stay deferred.

### Conclusion and development direction — 2026-10-21

**The observed remaining failures do not establish a parser-only fix.** The
source-owned identity roundtrips work, and the failing scopes or translations
already appear in raw provider replies before Rust reconstruction. EuroNano
already leaves the translated adjective outside bold; HY already changes the
subject/polarity. Empty identities are real output omissions. Retaining nonempty
identities or accepting valid HTML cannot establish correct linguistic scope.
This does not prove our code has no bugs or that every model/runtime would fail.
It does establish that relaxing this parser would conceal the demonstrated
failures rather than repair them.

There are three separate responsibilities:

- **Import and structural preservation:** source hierarchy, semantic tags,
  attributes, literals and attachments belong to Papercut. A defect here can and
  should be fixed locally. Historical inspection also found CSS-only emphasis
  removed by the reader's existing sanitization policy before translation.
  Safely normalizing a supported subset into semantic markup is a distinct
  possible improvement; preserving arbitrary CSS is not authorized. This would
  not fix the demonstrated `em`/`strong` mapping failures.
- **Translated-word ownership:** deciding which reordered, fused or omitted
  target words inherit a source span requires linguistic correspondence. The
  tested alignment/model outputs do not supply it reliably enough. Independent
  fragment translation previously preserved wrappers while corrupting meaning;
  it is not a new solution to retry. No production heuristic has been qualified.
- **Product policy:** current OPUS code deliberately preserves whole-sentence
  context and retains the formatted original beside English for unresolved
  scopes. That duplication is our chosen fallback, not a parsing accident or
  evidence that formatting was transferred to English. The maintainer found
  that experience unsatisfactory; do not present it as meeting the requirement.

**Recommended development direction, with no implementation authorized here:**
stop provider expansion and consolidate the existing OPUS sentence path, shared
DOM/security, installer, cancellation, cache and writer owners. Automatically
restore only supported whole-sentence/block or copied-literal scopes under the
existing checks. The maintainer has authorized the assisted-workflow plan below;
mixed inline formatting needs explicit review/correction, with the original
available on demand rather than repeated throughout the reading output. English
remains unchanged while formatting is edited. Stage 1's pure contracts and Stage
2a's inline import normalization are implemented; the assisted editor is still
pending. This is reduced
automation, not the original fully automatic requirement.

If automatic preservation of arbitrary inline bold/italic remains required
without manual correction, keep public translation release deferred. None of
the evaluated paths currently meets that bar, and further abstraction or test
passing cannot close the linguistic gap. Do not implement a new fallback,
silently flatten formatting, remove failed guards, or delete the reusable
foundation merely to declare the feature complete. Smoke, benchmarks and the
version bump remain deferred; no further candidate evaluation is scheduled.

## OPUS assisted formatting plan — 2026-10-21

**Stages 1–4 implemented; review UI pending.** Use existing OPUS-MT French/Spanish → English sentence
translation, then let the user correct bold/italic scopes without changing the
translated characters. Source formatting normalization and target formatting
review are separate responsibilities. CSS normalization prevents additional
import loss; it cannot fix incorrectly aligned target words or recover CSS
already discarded. Other providers remain gated, strict alignment stays retired,
and no aligner, phrase-matching fallback, engine fork or new candidate evaluation
is part of this plan. Benchmarks, packaged/device smoke and version work remain
deferred. Passing the implementation gates below will not establish bilingual
translation quality or release acceptance.

### Reading and review behavior

- New assisted results show English in the reading document. **Review formatting**
  opens a dedicated translation panel on an unresolved paragraph; **Show original**
  expands its saved sanitized source snapshot on demand. Namespace preview IDs
  and internal fragment links in a display copy so source comparison cannot
  collide with English anchors; never mutate the saved snapshot. Do not repeatedly insert
  Spanish/French blocks in the reading stream. Keep source comparison outside
  searchable/narrated English and outside read-along coordinates.
- V1 edits only bold and italic. Select English text in a read-only text field,
  apply/remove either mark, and inspect a semantic preview. Support Undo, Reset
  paragraph, a keyboard-accessible list of applied ranges, and explicit paragraph
  review. No `contenteditable`, arbitrary HTML, text correction, automatic model
  reruns or general-purpose document editor is needed.
- **Save review progress** persists a draft without modifying published HTML.
  **Finish review** publishes validated formatting. Leaving with unsaved edits
  must offer save/discard; errors and stale revisions preserve the draft. Review
  remains available offline without an installed translation model.
- A deliberate **Paragraph reviewed** action can accept an unstyled result;
  do not infer approval because the paragraph was opened or all marks removed.
  Editing or resetting approved marks returns that paragraph to pending review.
  “Formatting reviewed” records human formatting review, not verified meaning.
- Record separate obligations for emphasis, typography, links and notes/other
  attachments. B/I edits cannot resolve an unknown link or footnote location.
  V1 completion is limited to emphasis-only eligible blocks plus already checked
  structures. Unresolved non-emphasis obligations remain visible and block full
  completion; do not relocate or drop their source-owned payloads. A source
  snapshot on demand must retain access to those references. Non-emphasis editing
  is outside V1; importing an unsupported document remains an explicit limitation.

### Ownership and durable review contract

Keep the [existing owners](#architecture-and-ownership). Add one translation-owned
review module for contracts, pure B/I reconstruction and commands; keep UI state
in a dedicated review component. The MT runner produces immutable English and
review evidence once. Editing neither calls inference nor writes MT resume cache.
Use the existing derived-document transaction, reference lock, sanitizer,
search/TTS extraction and highlight invalidation; do not add another writer,
inference engine, cache owner or global reader state manager.

A versioned review record needs the variant ID, unique translation-generation
token, monotonic review revision and baseline published-document identity. Review
records are generation-owned and cascade with variant deletion; conditional draft
updates must never recreate a deleted variant through an upsert.
Retain one bounded immutable sanitized source-document snapshot per generation,
so cross-block footnotes/backlinks remain available after original deletion.
Per block retain the source-owned identity/ordinal and reference into that snapshot,
exact target text/hash, structural identity, validated existing marks,
editable B/I ranges, obligation types and review state. Retain owned source
assets under the existing document ownership rules. The current derived writer
filters assets against published HTML only; extend its existing validation to
the union of English and source-snapshot references, including source-only assets.
Deleting the original upload must not break the review snapshot. Source URLs alone are insufficient:
originals can change/disappear and deterministic variant IDs can be reused.

Commands accept IDs, expected generation/revision and ranges, never caller-supplied
HTML, English replacement text, source paths or asset URLs. Save scalar offsets,
explicitly convert frontend UTF-16 selections, reject split surrogate/grapheme
boundaries, and never normalize target characters. Validate range count/length,
block membership, protected literal/attachment boundaries and source hierarchy.
Canonicalize overlapping B/I marks into deterministic runs and escape prose
before emitting app-owned tags. V1 rejects edits that cannot preserve protected
structures; it must not flatten them to make a patch fit.

Keep pending obligations separate from draft saving and published status. The
additive version-aware database migrations (currently schema v3) live in `translation/storage.rs`;
the review wire/evidence contract remains version 1, a separate version domain.
Legacy variants/audio stay readable and untouched. Legacy results without the
new evidence are initially read-only for formatting edits; do not reconstruct
edit identities from a mutable original or guess a migration from old HTML.
Retranslation of a variant with review data creates a new saved variant identity,
retaining the old variant, annotations and audio; never silently reuse old ranges
against a new generation. Scope this identity change to preventing review-data
overwrite, without adding general variant grouping or backup policy.

Publishing must recheck generation/revision and baseline hashes under the existing
reference lock immediately before promotion. Extend the existing guarded writer
seam so HTML, metadata/index updates and review completion commit or roll back
together. Keep the prior variant and draft on any failure. Formatting-only edits
must leave exact readable English, block/chunk order, anchors and text coordinates
unchanged. Prove identical search/TTS chunk signatures and source spans before
reusing the saved URL and its existing audio. Do not overwrite audio or timings.

### Safe CSS normalization contract

Normalize source presentation at **import**, before the unchanged Ammonia
sanitizer. Fresh HTML preparation and full EPUB chapters share the inline/embedded and
EPUB-local stylesheet normalizer before sanitization/body extraction. Stage 2a
initially rejected unread sheets; Stage 2b now implements the bounded subset below. Keep normalization out
of MT adapters and the sanitizer used for generated/saved documents.

1. Start with inline `font-weight` and `font-style`. Resolve effective properties
   per text run, including inheritance, local resets, duplicate declarations and
   property-specific `!important`. Inherited importance never outranks a child's
   own declaration. Preserve exact text/whitespace and every non-formatting
   container, identifier, link, note, literal and attachment.
   During the inline-only substep, embedded or linked stylesheets whose effects
   are not yet interpreted make coverage unknown. Do not claim inline styles
   preserved when an unread stylesheet could override them with `!important`.
2. Freeze the initial subset: `normal`/`bold` and numeric weights 100–900;
   weights at least 600 map to visual bold and lower weights to no visual bold.
   `normal`/`italic`/`oblique` map to off/on visual italic. This intentionally
   reduces font-weight/angle distinctions; it is not exact typography retention.
   Support `inherit`, `initial` and `unset` for these two properties. Relative
   weights, angled oblique and other unsupported values are reported unresolved.
3. Generate `<b>/<i>` for CSS-derived visual formatting; retain authored
   `<strong>/<em>` semantics and identities. Resolve nested resets on ordinary
   styled containers through text-run emission rather than wrapping the whole
   parent. A reset conflicting with authored semantic emphasis is unsupported in
   V1: preserve the authored structure, flag the affected scope and do not claim
   visual equivalence. Do not clone identity-bearing containers or note anchors.
4. Add bounded embedded stylesheets and EPUB-local referenced CSS as a separate
   substep. Initially accept selector lists containing a type, class, ID or their
   compound forms; exclude combinators, pseudo-selectors and conditional/layered
   rules. Reuse Kuchikiki selector compilation/matching/specificity and a real
   declaration parser. `cssparser` is already transitive; make the chosen version
   an explicit dependency rather than write a regex CSS parser. Resolve stylesheet
   order, selector specificity, declaration order and inline/important precedence.
   V1 evaluates only ordinary unconditional active sheets. Conditional `media`
   attributes, disabled/alternate sheets and uncertain activation are unsupported
   coverage; never indiscriminately apply print or alternate styles.
   Keep source matching context explicit: standards HTML and qualifying XHTML
   may use the shared matcher; unsupported compatibility modes or XML-to-HTML
   repairs must warn instead of silently changing selector/inheritance behavior.
5. Unsupported `font`/`all` shorthands, variables, resets/layers or conditional rules
   can override otherwise supported font declarations. Do not silently ignore
   them and claim a computed style. Conservatively mark the affected property/
   subtree unsupported; if its reach cannot be determined, skip CSS-derived
   normalization for that chapter and report the limitation. Do not claim the
   absence of such styling when a required sheet is missing or over budget.
6. Read only EPUB archive-local manifest CSS through existing path validation and
   bounded ZIP reads. No HTML-adjacent filesystem reads, remote stylesheets,
   network fetch, `url()` resource loads, fonts, recursive `@import` or browser
   rendering. CSS is inert input only. Freeze explicit stylesheet byte/count,
   rule/declaration and total node-matching work caps before implementation;
   exceeding them reports incomplete normalization without weakening archive or
   sanitizer limits. Preserve readable document import with an explicit warning.
7. Run existing Ammonia afterward; no publisher stylesheets, style attributes,
   CSS URLs or executable content may survive. Store normalized safe markup and
   a normalization-policy version/coverage warning in import metadata. Bind
   translation/review/cache provenance to the resulting source snapshot/policy.

Old uploads retain only sanitized reader HTML, not the original raw file/archive.
Ordinary identical-file reimport returns the existing upload before parsing.
Consequently neither translation-time normalization nor ordinary reimport restores
lost CSS. Leave existing sources/results/audio unchanged. A later explicit
refresh-as-new-version from a user-supplied original is separate scope; do not
silently bypass deduplication or overwrite the old source in this work.

### Implementation stages and completion gates

Complete and review each bounded stage before the next. Tests use synthetic HTML,
EPUBs and Unicode cases; private articles, model outputs and sample-specific
fixtures/references remain outside tracked files.

| Stage | Deliverable and required regression gates |
| --- | --- |
| **1. Pure review and normalization contracts — implemented** | Freeze eligibility/obligation states, schema/command bounds, CSS grammar/work caps and policy identities. Test repeated identical paragraphs, mixed obligation types, invalid/oversized ranges, stale generation/revision, Unicode scalar↔UTF-16 conversion, combining marks/emoji/Arabic diacritics and B/I overlap. A reviewed emphasis scope must not clear a note/link obligation; edit/reset must invalidate approval. No UI, inference or published-document writes |
| **2a. Inline source normalization — implemented** | Pure normalizer plus HTML/full-EPUB-chapter import integration, followed by unchanged sanitization. Test bold/italic nesting, inheritance/local normal resets, semantic-tag conflicts, comments/escapes/invalid values, shorthands and limits. An unread embedded/linked `.x { font-weight: normal !important }` must make conflicting inline bold unresolved. Prove exact text, whitespace, IDs, links, note payloads, literals and chapter order; normalization and sanitization idempotence; scripts, CSS and unsafe URLs remain removed |
| **2b. Bounded local stylesheet normalization — implemented with coverage corrections** | Add the declared embedded/EPUB-local selector subset and cascade. Test specificity/source order/important precedence, child overrides of inherited importance, multiple sheets/chapter isolation, source matching modes, SVG/XML stylesheet evidence, missing/unsupported rules, media/disabled/alternate-sheet activation, EPUB-relative paths/traversal, recursive imports and budget exhaustion. Zero network or unrelated filesystem reads. Verify duplicate imports preserve old sources and changed normalized identity invalidates incompatible checkpoints |
| **3. Durable snapshots, drafts and pure reconstruction — implemented** | Version-aware schema v2, immutable evidence, model-independent native draft commands and pure preview. Regression gates cover file-backed save/reload/offline resume, source/model independence, repeated blocks, stale windows, new-generation conflicts, notes/backlinks and source-only assets, preview namespaces, exact target text/protected structure, hostile edits, legacy compatibility and atomic initial-save/draft failure preservation. Publication/UI remain separate |
| **4. Atomic publication — implemented** | Shared derived-document writer with revision/file checks before promotion, durable publication receipts and on-access interruption recovery. Tests cover concurrent save/delete/regeneration/Finish, partial file/SQLite/index failures, rollback retaining prior variant and draft, repeated completion and recovery before serving HTML. Deleted-variant stale saves create no orphan or revived variant; unresolved obligations or edited/reset approvals block Finish |
| **5. Accessible review UI and consumer integration** | Dedicated paragraph panel: source collapsed by default, read-only selection, B/I controls, mark list, undo/reset, explicit paragraph approval, progress save and completion. Test keyboard/touch selection, toolbar focus retention, selected/mixed states, navigation with dirty drafts, conflict/error/retry and non-emphasis blockers. Reuse `translationReview.test.ts` and current highlight invalidation to prove identical English extraction, search sections, TTS chunk IDs/text hashes/spans and read-along behavior across added text-node boundaries; existing audio/timings remain unchanged |

The Stage 2b review findings are addressed in the correction receipts below.
Stages 3–4 are implemented as documented below; review publication before Stage 5 UI.
CSS import work and assisted review
remain separate leaves even though they share normalized source identity. Do not
revive retired alignment setup or split unrelated UI merely to shorten files.
Implementation completion requires the above automated gates and a focused code
review. Later human FR/ES meaning/formatting usability and rebuilt HTML/EPUB
integration acceptance remain release gates; broader smoke, device benchmarks
and version bump are still explicitly deferred, not recorded as passed.

### Stage 1 pure contracts — 2026-10-21

The [review contract](../src-tauri/src/translation/review/mod.rs) compiles in base
and native-translation builds. Stage 1 originally added no IPC/persistence; Stage 3
connects those contracts to storage and native draft commands, without a UI caller.
It freezes `opus-assisted-emphasis-v1` and review schema version 1, separate from
the database schema version. Native-owned snapshots/normalization policy,
generation, source ordinal/identity and exact English are retained separately
from client edits. Edits cannot provide English, HTML, paths, obligation states
or an approval override. Explicit approval resolves emphasis only; editing/reset
invalidates it. Absent records and stale/exhausted revisions reject without
mutation. Source-validated fixed marks remain separate from editable marks.

UTF-16 selection conversion rejects surrogate/grapheme cuts without normalization.
Canonical B/I geometry preserves protected cut points: adjacent ranges cannot
merge across a zero-width note anchor. These are geometry/evidence checks, not
HTML reconstruction or proof that supplied native blocks correspond to source
hierarchy. Stage 3 must verify that correspondence; Stage 4 must enforce the
same optimistic preconditions under the existing writer lock. Pure eligibility
does not establish database atomicity, deletion cascades or durable publication.

| Review bound | Frozen limit |
| --- | --- |
| Raw edit request | 16 KiB before typed decoding; future IPC must use equivalent admission |
| Editable marks, fixed marks and protected ranges | 128 each per block; input counts checked before coalescing |
| Exact target text | 32 KiB per block; 25 MiB combined; at most 16,384 blocks |
| Native source and published snapshots | 120 MiB each; nonempty; byte hashes and normalization policy checked |
| Identity/revision | Native variant ID at most 128 hex characters; 32-hex generation token; bounded opaque block identities; revision at most JavaScript's exact integer maximum, `2^53 - 1` |

The raw decoder caps its own typed allocation/work, not Tauri's input-buffer
allocation. Do not bypass it by directly deserializing an unrestricted future
command DTO. These new bounds restrict assisted-review admission only; they do
not change import or ordinary translation budgets.

The [source-emphasis contract](../src-tauri/src/document_uploads/html/emphasis_policy.rs)
freezes `source-emphasis-v1`: typed font values/selectors, explicit sheet
activation/coverage and authored semantic-reset rejection. It parses no CSS and
performs no DOM/cascade/import work. Missing, unread, conditional or unsupported
sheets cannot certify coverage, and later success cannot erase a blocker.
Atomic work charges reject over-budget/overflow without spending a partial charge.

| CSS bound | Frozen limit for one entire HTML/EPUB import |
| --- | --- |
| One stylesheet / all CSS bytes | 64 KiB / 256 KiB, including inline declarations |
| Stylesheets / rule-and-selector work / declarations | 32 / 2,048 / 8,192 |
| Node work | 1,000,000 combined traversal visits and selector-node attempts across all chapters |

Shared cached sheets need charging only once, but matching work always counts.
Import normalization must precharge before parsing/matching and preserve readable import with
an explicit incomplete-normalization warning on exhaustion. These are conservative
work limits, not measured throughput or representative-book coverage claims.

Validation: 35 new synthetic regressions cover Unicode, repeated blocks, caller
injection, atomic rejection, revisions, independent obligations, fixed/locked
marks, canonical cut points, grammar/coverage and budget boundaries. Full base
Rust library tests passed (552 passed, four existing ignores); the final
native-translation library suite passed (554 passed, four existing ignores).
Existing transfer unit tests needed local loopback/temp D-Bus socket permission;
the restricted-sandbox failures were permission denials, not changed transfer
behavior. Formatting/diff checks passed and the historical evaluation JSON is
byte-identical. No model inference, frontend/packaged smoke, benchmark, source
rewriting, migration or version bump occurred. Other providers remain gated.

### Stage 2a inline import normalization — 2026-10-21

This is the Stage 2a handoff record; stylesheet admission is superseded by the
Stage 2b receipt below. The source-preservation guards continue to apply.

Fresh HTML imports use [import preparation](../src-tauri/src/document_uploads/html/prepare.rs)
and full EPUB chapters use the same [inline normalizer](../src-tauri/src/document_uploads/html/emphasis.rs)
before their existing sanitizers. Shared reader/generated/transfer parsing does
not normalize CSS. EPUB metadata is read from original XHTML before HTML
serialization; non-void XHTML empty tags are expanded before the HTML DOM parse.
This preserves chapter language, direction, `translate`, source order and empty
note anchors while root/body styles are still available for inheritance.

`cssparser` 0.29.6, already locked transitively through Kuchikiki, is now explicit.
It tokenizes inline declarations, comments/escapes, duplicate declarations and
property-specific `!important`. Text runs receive only needed visual `<b>/<i>`
wrappers; source containers/IDs are not cloned and characters are not normalized.
Local normal resets work on ordinary styled containers. Authored emphasis stays
intact; conflicting resets report incomplete coverage rather than flatten it.
The same guard covers retained default emphasis on headings/table headers and
`cite`/`dfn`/`var`, which would otherwise reappear after removing CSS. Removed
containers such as `address` do not count as surviving emphasis: their supported
explicit/inherited CSS italics become `<i>` wrappers before sanitization.
Unrelated properties are not rendered or executed. No publisher CSS or active
attributes bypass the unchanged sanitizers.

Any embedded/linked stylesheet, unsupported affecting declaration, conflicting
semantic reset or exhausted budget skips CSS-derived normalization for that
chapter. The original input still follows normal sanitization and remains
readable. This intentionally conservative fallback does not guess which rules
might apply. There are no stylesheet, network, adjacent-file, font or `url()`
reads; even local EPUB stylesheets await Stage 2b. One budget spans the book,
including declarations in earlier chapters. Planning precedes DOM mutation;
failure cannot leave half a chapter converted. A new 256-level DOM-depth bound
also avoids extra recursive serialization by the normalizer; generated wrappers
count toward depth and node work. Existing import/sanitizer limits remain intact.
If an extra serialization could consume a leading newline in `pre`, `textarea`
or `listing`, or invent closing-tag text inside `plaintext`, the chapter instead
keeps its existing import path and reports `literal-roundtrip`; unstyled input
needing no wrappers avoids that extra pass. This preserves the existing import
behavior rather than attempting to repair legacy raw-text parsing.

HTML whitespace directly within table structures remains unwrapped, so the next
parser does not move it before the table. Caption/cell prose still receives its
supported emphasis; non-ASCII text is not mistaken for structural whitespace.
Requested wrappers in `select`/`option`/`optgroup` would be discarded by the HTML
parser, so they trigger the atomic chapter fallback with `unsupported-context`.
Plain or reset controls needing no wrapper do not trigger this warning.

The existing upload transaction stores optional `source_formatting` JSON with
`source-emphasis-v1` and bounded reason codes. Its nullable additive column leaves
legacy documents unclassified and preserves their stored content/index/audio.
Import results and listing return `sourceFormatting`; incomplete imports show a
distinct localized warning that stays visible until dismissed (at most ten file
names, with an omitted count). This reports supported source B/I coverage, not
exact typography, correct English word placement or human formatting approval.
Restored/generated HTML receives no new source-coverage certification; transfer
formats are unchanged and this new import report is local metadata.

New normalized sources bind exact sanitized HTML plus the policy/report into
resume compatibility and retain the report in translated settings. Markup-only,
policy or warning changes invalidate that resume state. Legacy caches without a
normalization report remain compatible; request ownership keys and saved variant
IDs stay unchanged. Existing uploads are not reparsed, and identical-file
deduplication still returns the old source; recovering already-discarded CSS
requires the separately planned explicit source refresh.

Validation: 34 new Rust regressions plus four frontend regressions cover the
parser subset, reset/importance/inheritance, limits and atomic fallback, Unicode,
literal newlines, XHTML metadata/empty anchors, notes/order, unchanged sanitizer,
nullable migration, persisted reports and normalized-source save/resume. Full
base library tests passed (586 passed, four existing ignores); native-translation
library tests passed (588 passed, four existing ignores). Seven focused frontend
tests, TypeScript, eight-locale validation, targeted ESLint, Rust formatting,
documentation links and diff checks passed. The historical evaluation JSON is
byte-identical. Independent review found the XHTML metadata, retained-default
reset and literal-newline defects above; regression-backed corrections leave no
known blocking finding at that implementation handoff. The subsequent review
found the raw-text, removed-container and content-model defects corrected below.
Other providers and strict alignment stay
gated/disabled. No model inference, benchmark, packaged
or device smoke, assisted editor, existing-source rewrite or version bump is part
of Stage 2a. Inline import preservation does not solve mixed target-word placement.

#### Stage 2a source-preservation corrections — 2026-10-21

The shared normalizer now addresses all three follow-up review findings without
changing either sanitizer, the import entry points, provider admission, schema or
dependencies. `plaintext` uses the existing literal-roundtrip fallback only when
serialization would be required. Removed `address` containers no longer suppress
CSS-derived italics. Wrapper admission respects table whitespace and unsupported
select contexts before any mutation or generated-node work is committed.

Ten new synthetic regressions cover final HTML/EPUB output rather than only the
first parsed DOM: exact fallback source/section ranges, Unicode, no-wrapper
controls and resets, surviving IDs and cross-chapter notes, exact table whitespace
and hierarchy, formatted caption/cell text, and non-ASCII table text. Full base
Rust library tests passed (596 passed, four existing ignores); native-translation
library tests passed (598 passed, four existing ignores). Rust formatting,
documentation links/anchors, synthetic-input privacy and diff checks passed;
the historical evaluation JSON is byte-identical. Independent review found no
remaining issue in these fixes. Existing uploads/results/audio remain untouched;
benchmarks, smoke, full CI and the version bump remain deferred. Frontend code and
dependency pins are unchanged; no model inference was run.

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

### Stage 2b bounded stylesheet import normalization — 2026-10-21

Fresh HTML imports now interpret embedded XHTML `<style>` elements. EPUB chapters
also interpret linked archive members which are explicitly declared `text/css`
in the OPF manifest. The [shared normalizer](../src-tauri/src/document_uploads/html/emphasis.rs)
owns source traversal, import-wide budgets, per-element cascade and atomic wrapper
planning. The [stylesheet parser](../src-tauri/src/document_uploads/html/emphasis_sheets.rs)
shares one declaration tokenizer with inline styles; Kuchikiki compiles/matches
admitted selectors and supplies specificity. The [EPUB CSS reader](../src-tauri/src/document_uploads/epub/stylesheets.rs)
owns URL/manifest admission and bounded ZIP reads. No new dependency, rendering
engine, engine fork or provider behavior was introduced.

The selector subset is a type, class, ID, their simple compounds, and comma lists.
CSS comments/escapes are tokenized, not split with regexes. Only matched members
of a selector list contribute specificity. Each property's winner resolves
importance, inline precedence, specificity and source/declaration order; inherited
importance never outranks a child's own declaration. Repeated local sheet links
retain their occurrence order around embedded sheets. Cached rules are rematched
against each chapter; neither rules nor matched nodes leak into another chapter.
Styles after prose still apply to the whole chapter. Supported ordinary defaults,
resets and `<b>/<i>` emission retain all Stage 2a literal, semantic-reset, depth,
table/select and hierarchy protections.

Only unconditional ordinary CSS is admitted: missing/empty/`all` media, CSS type,
no disabled/alternate/named/scoped sheet. Conditional media, unsupported types or
non-UTF-8 link charset labels report `unsupported-activation`. All at-rules
(including `@import`, `@charset`, font-face, conditional/layered rules) and selectors
outside the subset reject the chapter's CSS-derived conversion. Unsupported
font/all shorthands or font values remain explicit failures. Truly inert
script/template/noscript contents do not introduce global rules. Active foreign
stylesheets instead warn: [SVG styles can affect surrounding HTML](https://www.w3.org/TR/SVG/styling.html#HTMLStyling).
The EPUB SVG extractor retains that evidence, and XML stylesheet-instruction
evidence, before extracting an SVG asset. This prevents false complete coverage;
SVG rendering itself is unchanged. CDATA-wrapped XHTML style text currently fails
safely as unsupported rather than being interpreted as ordinary HTML raw text.

Local CSS links resolve against each chapter's archive directory through existing
EPUB path resolution. Decoded and raw URL boundaries reject schemes, authority or
absolute paths, root escapes, backslashes, queries/fragments, invalid UTF-8 and
control bytes. Only canonical manifest CSS members are read. HTML base and XML
base semantics are unsupported; uncertain chapter bases warn, and OPF members
under XML bases are not admitted as CSS. Standalone HTML never reads adjacent
files. There is no network, recursive import, font, URL-resource or unrelated
filesystem loading. CSS ZIP input must be UTF-8; a leading external UTF-8 BOM is
removed as an encoding signature after its raw bytes are charged. Other encodings
remain unread with explicit coverage warning.

The frozen numeric caps remain: 64 KiB per sheet, 256 KiB total CSS, 32 sheets,
2,048 rule/selector work units, 8,192 declarations, 1,000,000 node/match work units
and 256 DOM levels. Rule work conservatively counts each rule plus each selector
component, bounding list and compound compilation as well as rule count. Traversal
counts both discovery and planning. Selector matching also charges compound
complexity and element-name/class/ID bytes in 64-byte units before matching, bounding repeated
attribute scans. Unique linked sheet attempts, including missing/invalid resources,
spend a slot before IO. The ZIP reader checks declared size against the lesser of
per-sheet and remaining import allowance before allocation/inflation, caps actual
bytes too, and charges bytes before decoding/parsing. Successful and failed local
loads are cached for this import only; repeated includes spend matching/traversal
work but never repeat rejected decompression. Embedded sheets precharge their
complete text size before copying/parsing.

Any blocker leaves the chapter's CSS-derived markup unmodified, then runs its
existing sanitizer and readable import path. The same bounded reason-code report
and `source-emphasis-v1` provenance are persisted; no schema, source ID, provider
admission or transfer format changes. Changed normalized markup or coverage
invalidates incompatible translation checkpoints. Existing source files, saved
translations, audiobook/highlighting data and identical-file deduplication are
unchanged. Ordinary reimport still returns the old source before normalization;
this cannot recover CSS already discarded from older imports.

Validation: 38 new synthetic regressions; the final base Rust library suite passed
(634 passed, four existing ignores), and the native-translation library suite
passed (636 passed, four existing ignores). Existing transfer tests required local
loopback/temporary D-Bus socket permission; sandbox-only permission failures were
rerun successfully. Rust formatting, diff checks and documentation links passed.
Tests cover full HTML/EPUB import plus final reader sanitization,
importance/specificity/order, matched-list specificity, inheritance, chapter
isolation, repeated cached includes, notes/assets/literals/Unicode, unsupported
activation/rules/paths/bases/encodings, all work caps, failed-read caching, BOMs,
foreign-sheet coverage and markup/coverage cache invalidation after reload. The
historical evaluation JSON is unchanged. No private source or repro files are
tracked. Other providers remain gated, strict alignment stays disabled, and no
inference, benchmark, packaged/device smoke, target formatting editor or version
bump is part of this stage. Source CSS normalization does not solve English
word-placement approval; Stage 3's durable assisted-review workflow remains next.


### Stage 2b review — 2026-10-21

Reviewed implementation commit `29999f467a19300fbc460411c575ec01abaebc2a` through
fresh HTML preparation, full EPUB import/SVG extraction, final sanitization,
metadata persistence and normalized-source cache/resume. Assumed the existing
single-user offline workload and bounded HTML/EPUB imports. This historical
review found three **P2 coverage blockers**, addressed by the correction receipt
below; do not start Stage 3 until the corrections pass their focused review.

1. **Source document modes are ignored by selector matching.**
   [The matcher](../src-tauri/src/document_uploads/html/emphasis_sheets.rs)
   calls Kuchikiki's HTML standards-mode API for every source. A no-doctype HTML
   `.X{font-weight:bold}` rule does not match `class=x`, although the source
   parser records quirks mode, whose class/ID matching is ASCII-insensitive.
   Conversely, EPUB XHTML `P{font-weight:bold}` incorrectly matches lowercase
   `<p>`, because HTML type-name matching is used for XML. Both return no warning
   with incorrect emphasis. Quirks table font inheritance also differs from the
   current computation. Follow the [selector case rules](https://www.w3.org/TR/selectors-3/#casesens),
   [quirks class/ID rules](https://drafts.csswg.org/selectors-4/#class-html) and
   [HTML table rules](https://html.spec.whatwg.org/multipage/rendering.html#tables).
   Smallest safe next step: make HTML compatibility mode versus EPUB XHTML
   context explicit, and reject/warn where the admitted matcher cannot preserve
   that context. Do not fix legacy HTML by case-folding all EPUB selectors.
   Regressions must compare doctype/no-doctype HTML, mixed-case class/ID/type
   selectors, escapes, inherited table emphasis and standards-mode EPUB controls.
2. **SVG extraction loses linked stylesheet evidence.**
   [The SVG extractor](../src-tauri/src/document_uploads/epub/inline_svg.rs)
   retains `<style>` evidence but does not retain active stylesheet `<link>`
   elements which disappear inside an extracted SVG. A synthetic XHTML SVG
   containing an HTML-namespace link to manifest CSS
   `p{font-weight:normal!important}`, followed by inline-bold `<p>Words</p>`,
   imports bold with an empty warning list. The shared normalizer warns when
   given that original chapter directly; the full EPUB pipeline loses the link
   first. [SVG's HTML-link rules](https://www.w3.org/TR/SVG/styling.html)
   establish that this sheet is not inert. Extend pre-extraction evidence to
   stylesheet links, retaining namespace/activation/inert distinctions and all
   existing resource limits. Test direct and foreignObject-nested links,
   negative inert/non-stylesheet controls, assets, text and final sanitizer output.
3. **XML parse fallbacks erase stylesheet-instruction warnings.**
   The same extractor returns `stylesheet_warning: None` when entity declarations
   are present or XML parsing fails. A chapter with an `xml-stylesheet` instruction
   and an unused internal entity declaration therefore imports inline-bold text
   with no warning. Removing the declaration produces the expected warning and
   no CSS-derived bold. This is a residual incomplete-coverage gap, as is item 2,
   rather than a newly introduced active-content leak. Preserve conservative
   stylesheet evidence even when XML conversion is skipped; do not enable entity
   expansion or broaden resource access. Test valid unused declarations,
   malformed XML, ordinary no-instruction controls and unchanged readable text.

The first issue introduces wrong sheet-derived output; the other two remain
uncovered paths in the new evidence handoff. All three can record complete source
coverage while adding or losing emphasis, so passing tests do not clear them.
No finding was confirmed in cascade priority, import-wide work limits, failed-read
caching, archive path/MIME admission, BOM accounting, persisted report delivery,
source identity hashing, or cache rejection after source changes.

Fresh focused parser/import/persistence/source/cache checks passed, including
23 stylesheet-filter tests, 11 sheet-parser tests, 23 normalizer tests, 11 HTML
preparation tests and 13 cache tests (filters overlap; these are not an aggregate
unique-test count). Synthetic repro probes compiled the actual source modules
against cached dependencies; full EPUB probes exercised ZIP input and final
reader generation. Repros remained in `/tmp`, with no private source inputs or
tracked fixtures. Rust formatting/diff checks passed, and the historical
evaluation JSON remained byte-identical. No new full/native CI claim, model
inference, performance benchmark, packaged/device smoke or version bump is made.
Other providers stay gated and strict alignment stays disabled.

### Stage 2b source-context and evidence corrections — 2026-10-21

The three findings above are fixed at the shared import boundaries. HTML versus
XHTML is now explicit, including EPUB chapters whose manifest says `text/html`
despite an `.xhtml` filename. The shared matcher remains unchanged. Standards-mode
HTML retains case-insensitive type matching and case-sensitive class/ID matching.
Quirks/limited-quirks HTML with font-affecting sheets instead reports
`unsupported-document-mode`; inline-only legacy imports also warn when tables
could reset inherited fonts. These conservative guards preserve baseline readable
import and authored semantic markup rather than invent a browser rendering engine.

XHTML is qualified against the original non-expanding XML parse before trusting
the reader's HTML tree. Ordered elements/text, depth and attribute values must
agree: implicit HTML containers, reparenting, folded names, unsupported namespaces,
malformed XML and entity-bearing sources cannot certify font CSS. Decoded uppercase
type selectors also warn, while uppercase/escaped class and ID identifiers retain
their XML case semantics. The comparison shares the import-wide node/depth budget
and uses depth-bounded traversal, not a second unbounded node collection. A source
context mismatch warns when parsed font CSS or active original-XML stylesheet
evidence is present; unstyled documents retain the existing readable import path.
Some harmless XML whitespace repairs can also trigger this conservative warning;
V1 does not attempt to certify arbitrary XML through an HTML-only matcher.

SVG extraction now retains active stylesheet-link evidence as well as style text.
XHTML keeps exact namespaces and genuine inert HTML ancestor handling. HTML EPUB
chapters conservatively recognize mixed-case style/link/rel and HTML SVG integration
points before XML extraction can discard them. No lost sheet is loaded or applied:
the chapter keeps its baseline source with an explicit coverage warning and its
existing SVG asset handling. The XML entity guard and parse-failure path also
retain `xml-stylesheet` evidence without fetching a DTD or expanding an entity.
When XML parsing is unavailable, bounded literal instruction detection may warn
for instruction-like text in comments/CDATA; this deliberately favors incomplete
coverage over a false completeness claim.

Synthetic regressions cover matching modes, repaired XHTML trees, final HTML/EPUB
sanitization, source order/semantic markup/assets, SVG-linked styles, XML fallback
instructions and unaffected controls. Cache roundtrip tests reject both directions
of a coverage-only mode change even when the sanitized reader HTML is identical.
Existing positive stylesheet fixtures now explicitly declare standards mode rather
than accidentally relying on no-doctype HTML, and XHTML positives include explicit
document containers. Existing literal/source-preservation assertions remain;
XML literal mismatches now report the earlier source-context guard rather than
the later HTML serialization guard.

Validation: 21 new synthetic regression functions. The complete base Rust library
suite passed (655 passed, four existing ignores), as did the native-translation
suite (657 passed, four existing ignores). Existing transfer regressions used
local loopback/temporary D-Bus socket permission. Rust formatting, diff checks and
documentation file links passed. Historical evaluation JSON is byte-identical;
private articles and external repros remain untracked. No new full CI or native
engine-inference result is claimed.

Other providers remain gated and strict alignment remains disabled. Existing
uploads, saved translations, identical-file deduplication, audiobooks and highlighting
are untouched. These fixes improve source evidence; assisted English emphasis
placement still awaits Stage 3 onward. Benchmarks, inference, packaged/device smoke
and the version bump remain deferred.

### Stage 2b literal and original-XML coverage follow-up — 2026-10-21

Review of `4a8244af` confirmed three residual import gaps. Numeric CR character
references bypassed the LF-only literal guard, prefixed XML stylesheets disappeared
before HTML-side detection, and explicit HTML EPUB chapters still passed through
XHTML empty-tag expansion. These are now corrected in their shared owners.

The existing atomic literal guard now covers leading decoded CR as well as LF in
`pre`, `textarea` and `listing`. It falls back only when generated emphasis would
introduce an unsafe serialization pass. Fresh HTML and full EPUB regressions prove
exact baseline reader HTML, text and section ranges for decimal/hex references;
unstyled and safe interior-CR controls still normalize normally.

The existing non-expanding XML parse retains compatibility and active-sheet
evidence in one depth-bounded traversal. Ordinary tree/name/attribute mismatches
stop HTML comparison, but the XML walk continues under the same import-wide work
cap. Expanded namespace/local names identify XHTML `style`/stylesheet `link` and
SVG `style`, including prefixed sheets and SVG styles outside an SVG container.
Only genuine XHTML inert ancestors suppress their descendants. A mismatched
context with original sheet evidence warns immediately, before stylesheet reads
or matching; unknown source sheets cannot become a complete-coverage claim simply
because the HTML parser forgot them. This is conservative: an uninterpretable
original sheet can warn even when its particular declarations might be harmless.
Ordinary prefixed prose, unrelated namespace elements and non-stylesheet links do
not invent stylesheet warnings. Depth/work exhaustion remains an immediate failure.
XML parse errors and rejected entity-bearing input also return an immediate
unchanged-source warning, including unstyled files: no original XML tree exists
to prove that HTML-side stylesheet detection is complete. Entities remain disabled.

EPUB source mode now reaches both pre-normalization compatibility processing and
final fragment sanitization, including the stylesheet-warning bypass. Nonvoid
self-closing tags are expanded only for XHTML; `text/html` retains HTML parsing
semantics even with an `.xhtml` filename. Paired final-import tests preserve the
different correct emphasis scopes while retaining metadata, notes/backlinks,
literals, text/section order and identical image assets.

Validation: 13 new synthetic regression functions. The final complete base Rust
library suite passed (668 passed, four existing ignores); the native-translation
suite passed (670 passed, four existing ignores). Existing transfer tests used
local loopback/temporary D-Bus permission. The CR tests reproduced lost newlines
before the guard fix; positive controls continued to pass. Independent code review
found no remaining issue in these fixes. Rust formatting, diff checks and
documentation file links passed; historical evaluation JSON is byte-identical.
Private sources/repros remain outside tracked files. No new full CI, inference
or app/device acceptance result is claimed.

No new parser, dependency, engine behavior, schema or provider admission was
introduced. Existing uploads, saved translations, deduplication, audio and
highlighting are untouched. Providers stay gated and strict alignment disabled;
benchmarks, engine inference, packaged/device smoke and the version bump remain
deferred. Their focused review found no remaining blocker to the Stage 3 work below.

### Stage 3 durable snapshots, drafts and pure reconstruction — 2026-10-21

Implemented through the existing translation/upload owners, without a model fork,
new dependency, editor UI or second document writer:

- [Storage](../src-tauri/src/translation/storage.rs) migrates translation schema
  v1 to v2 in one SQLite transaction, preserving legacy rows and rejecting unknown
  future versions. `translation_review` belongs to its variant through a cascading
  foreign key. Review schema/policy version 1 is independent of database version 2.
- Each new OPUS assisted result saves a full immutable sanitized source snapshot,
  exact indexed English baseline, normalization-policy identity, hashes, native
  node paths and source ordinals. Repeated identical paragraphs remain distinct.
  Existing results without this evidence are read-only; no evidence is invented
  from an original that may have changed or disappeared.
- [Snapshot capture and reconstruction](../src-tauri/src/translation/review/snapshot.rs)
  retain fixed source-validated marks and protected literal/link/note/attachment
  boundaries. Edits add native B/I wrappers by splitting existing English text
  nodes; characters, source-owned hierarchy, anchors and payloads never move.
  Reconstruction returns a separate draft preview, not published HTML. Fixed
  validated marks are not removed by editable-mark Reset.
- The [draft store](../src-tauri/src/translation/review_store.rs) keeps immutable
  snapshot JSON separate from mutable document JSON. Conditional generation/
  revision updates serialize competing saves; stale windows, unknown/deleted
  variants and invalid evidence reject without upsert or resurrection. Editing
  and Reset invalidate approval; emphasis approval cannot clear independent
  link/note/attachment/typography or source-normalization obligations.
- Initial evidence and the English variant commit in the existing derived-document
  transaction, with its filesystem rollback and reference lock. The same writer
  now validates/deduplicates the union of English and source-only asset references.
  Deleting the original leaves the variant's source evidence, cross-block notes,
  backlinks and owned images available. Display-only source previews namespace
  IDs, named anchors and local fragment links against the English baseline;
  the saved source snapshot remains unchanged.
- Assisted saves append a unique random generation token to the existing settings
  identity. A completed rerun can reuse compatible inference checkpoints while
  saving a distinct variant; it cannot overwrite older review ranges, annotations
  or audio. Legacy provider identities remain unchanged. This deliberately adds
  saved copies; no grouping, automatic pruning or backup/export policy is added.

Native commands are registered for the main WebView in base/native builds and
do not require a translation model, engine or network:

| Command | Input / result |
| --- | --- |
| `translation_review_load` | `{ id }` returns the current draft document, namespaced full-source preview, reconstructed draft HTML and variant-owned asset paths. `null` means no editable evidence, including legacy results; published reading remains available |
| `translation_review_save` | `{ request: JSON.stringify({ expected, block, edit }) }`, using the returned generation/revision and block key. Edits are `replaceMarks`, `reset` or `approve`; success returns the next draft revision. No input text, HTML, path, asset URL or caller-defined obligation is accepted |

The edge bounds JSON before typed edit decoding (16 KiB; the Tauri transport
still allocates its incoming buffer), validates identities and then checks the
native published file under the existing reference lock. Response asset reads
complete before revision commit so a failed response preparation permits the
same edit to retry. Loading/rebuilding uses only saved native evidence. There
was no Finish command in Stage 3: draft save cannot change Library status, published
HTML/index rows, MT resume cache, source data, audio or highlight timings.

Bounds are explicit failures, not permission to omit evidence or save partially:
16,384 blocks; 32 KiB exact text per block; 25 MiB aggregate target text; 120 MiB
per source/baseline HTML; 128 marks/locks per block; 262,144 DOM nodes and depth
128. Serialized snapshot and draft records each have a separate 256 MiB cap,
checked before copying stored JSON out of SQLite. JSON escaping can make an
otherwise admissible document exceed that storage cap. Unicode scalar offsets
must land on grapheme boundaries; the shared UTF-16 conversion rejects surrogate
splits. These are admission limits, not measured mobile memory/latency guarantees.

Regression coverage uses synthetic HTML/Unicode and temporary SQLite/filesystem
fixtures: restart/offline draft reload, source/model independence, repeated
paragraphs, revision/generation conflicts, deletion/cascade/no revival, failed
draft and initial-save transactions, changed published evidence, hostile edits,
legacy/future schemas, namespace collisions, notes/backlinks, source-only images,
hierarchy/literals, exact text and existing audio/highlighting preservation.
Private sources/repros are not tracked. Validation: 28 new synthetic regressions;
the complete base Rust library suite passed (696 passed, four existing ignores),
as did the complete `native-translation` suite (698 passed, four existing ignores).
Three existing cache/resume regressions now prove fresh saved generations with
unchanged prior evidence rather than deterministic result replacement. The
non-test library check passed with only the existing gated HY-MT2 dead-code
warnings. Rust formatting, diff checks and local documentation links passed;
historical evaluation JSON remains byte-identical. No model inference, full CI
or packaged acceptance is implied.

This Stage 3 receipt predates the guarded publication implemented below.
Stage 5 still adds accessible review UI and consumer acceptance. Until a review
is explicitly finished, the reading stream keeps its retained-original fallback;
no stage claims automatic semantic word placement. Other providers
remain gated; strict alignment disabled; benchmarks, smoke and version bump
remain deferred.

### Stage 3 whitespace and traversal corrections — 2026-10-21

The focused review found two implementation defects, corrected without changing
provider admission, English output or the review schema:

- Snapshot coverage previously compared normalized reader text with exact decoder
  text. Double spaces, line breaks, tabs or nonbreaking spaces could pass decoding,
  rendering and output checks, then fail at saving. Coverage now uses the existing
  plain-text whitespace normalizer on the comparison only. Saved English, literal
  payloads, hashes, scalar offsets and protected boundaries remain exact; decoder
  text is never parsed as HTML or rewritten to make the check pass.
- Resolving each paragraph's saved path previously rescanned earlier siblings,
  making large flat books quadratic. One depth-first walk now gathers all native
  block paths, retaining the existing all-child index representation. Validation
  checks those paths; reconstruction uses the already ordered block nodes rather
  than resolving every path again. Both previews share one checked pair of DOMs
  for that operation; no serialized admission flag or cross-command DOM cache is
  introduced. Immutable evidence and current drafts still receive their native
  checks on reload, and bounded edits retain their mutation-boundary validation.
- A draft save now loads its evidence once inside the SQLite write transaction
  and prepares its complete response before the conditional update and commit.
  Baseline-file checks, asset reads and preview reconstruction remain under the
  existing reference lock. A preparation failure preserves the prior revision
  and allows the same request to retry; no fallible response work follows commit.

New synthetic regressions cover exact whitespace/text/hash preservation through
native persistence, draft save/reload and preview; protected code, notes, image
bytes and locked ranges; changed-word/markup rejection; and response-failure retry
and immutable-record tampering. An 8,000-paragraph repeated-text fixture counts
actual path-walk node visits through capture, validation and reconstruction, with
no timing threshold. Nested/comment/whitespace child paths remain compatible with
saved records, and tampered paths cannot redirect an edit to identical prose.
This is deterministic traversal-work coverage, not a hardware speed/RAM benchmark.

Validation: eight new synthetic regressions. Complete offline Rust library suites
passed: 704 base tests and 706 with `native-translation`, with four existing ignores
in each. The whitespace test reproduced the final-save failure before the fix.
Rust formatting, diff checks and all 18 local documentation links passed;
historical evaluation JSON remains byte-identical. No inference, full CI or
packaged acceptance result is claimed.

These corrections passed focused review before Stage 4 below. The correction
itself leaves published fallback, sources, variants, audio and highlighting unchanged. Providers
remain gated, strict disabled; benchmarks, smoke and version bump remain deferred.

### Stage 4 guarded atomic publication — 2026-10-21

[Publication](../src-tauri/src/translation/review_publication.rs) now completes
an eligible native formatting draft without inference or a second writer.
`translation_review_finish` is registered for the main WebView in base/native
builds. Input is `{ request: JSON.stringify(expected) }`, where `expected` is
the exact schema/policy/variant/generation/revision token returned by Load or Save.
The 16 KiB decoder rejects unknown fields, replacement HTML/text, caller approval,
invalid identities, future versions and out-of-range revisions. Success returns
the existing `TranslatedDocumentInfo` shape with the same document URL and
published status `ready`. There is still no review editor or Finish UI button.

The pure [snapshot publisher](../src-tauri/src/translation/review/snapshot.rs)
requires every paragraph's current explicit approval and zero remaining
obligations. Editing or resetting a paragraph invalidates its approval. It applies
native B/I wrappers to existing English text nodes, removes retained-original and
explanatory-label nodes from the published reading stream, and transfers outer
source IDs/named anchors to the corresponding English element. Container/list
ownership and ordered English blocks do not move. The full immutable sanitized
source snapshot stays available in the separate comparison preview.

Publication compares exact consumer text, structural boundaries and non-emphasis
attributes, all identifier counts, and shared reader/search sections (heading,
text, auxiliary/body ranges and page order). Only native emphasis wrapper splits
and the intended outer-anchor transfer may differ. It then rechecks the current
sanitizer and HTML/node/depth bounds, including the extra B/I nodes. A mismatch is
a failure, not permission to flatten or change English. This guards saved URL,
search and narration coordinate reuse; frontend regressions additionally compare
actual chunk IDs/text/spans, reader locators and word highlights. No audio,
timings, MT resume cache or original document is overwritten.

The [existing derived writer](../src-tauri/src/document_uploads/derived.rs)
now accepts a preflight under the reference lock and a SQLite Immediate
transaction, **before** creating replacement files. Publication rereads the
generation/revision, draft and canonical file receipt in that guard. HTML/owned
assets are staged through the same pipeline; upload sections/FTS rows, translation
status and the publication receipt commit together. Index, metadata or receipt
failures restore the previous directory and roll back SQLite while preserving the
exact draft. Stale/deleted generations cannot become new uploads through the
writer's upsert. A fresh assisted translation still receives a different variant
identity, so concurrent regeneration cannot overwrite this reviewed generation.
When document/search projections are identical, the shared upsert preserves
existing section IDs, FTS/spelling rows and semantic-index readiness/vectors;
formatting-only publication does not require another conceptual-search build.
Changed text, title or auxiliary/body ranges still invalidate derived coverage.

Database schema **v3** adds nullable `published_revision`/`published_hash` to
`translation_review` in the existing version-aware migration. v1/v2 data and
immutable/draft JSON remain unchanged; review-contract and MT-cache versions are
still 1. The receipt binds the last live file to its review revision without
rewriting the initial source/baseline evidence. Repeated Finish of that revision,
including competing callers, returns the same result without republishing. Later
draft edits retain the last approved published file/status and require a new
approval before republishing. Old revision tokens reject after any intervening
edit. Both draft access and repeated Finish verify the actual canonical file
against the native receipt; a changed file is never silently accepted.

The existing `.generation`/`.replacing` recovery follows the committed upload row.
Publication timestamps strictly advance even for equal-size updates in one clock
tick. A precommit interrupted promotion restores the matching previous files; a
postcommit backup is cleaned after verifying the new generation. Recovery runs
under the reference lock on the next writer, review access or reader source
request after restart, before HTML is served; there is no eager startup sweep.
Missing/deleted or ambiguous generations fail without resurrecting backup files.
Ordinary source reads now share this lock through HTML/asset snapshotting, so they
cannot observe a promoted file while its SQLite transaction may still roll back.
The operation is recoverable across process interruption, not a new guarantee of
power-loss durability or cross-process filesystem locking.

Completion remains deliberately narrow. Unresolved link/note/attachment,
typography or source-normalization obligations block Finish. Source-only literals,
protected subtrees, authored line breaks and descendant anchors without an accepted English mapping
also block removal of their original; explicit emphasis approval cannot erase
them. Stage 5 must expose these publication preflight reasons alongside the saved
obligations, including older drafts whose stored obligation list predates this
structural guard. No text correction, non-emphasis editing, new aligner, model
fork or provider admission change is included.

Regression coverage uses only synthetic DOM/Unicode, file-backed storage and
mock native IPC: publication/approval/Reset, repeated and competing Finish,
postpublication file tampering, stale save/delete/generation interleavings,
index/metadata/receipt rollback and retry, old/new interruption recovery, locked
reader isolation, exact English/search/TTS coordinates and unchanged audio/assets.
Schema tests cover v2 evidence preservation and rollback of partially applied
column migrations; receipt corruption and hostile Finish input reject.

Validation: 26 new Rust regressions and three new frontend consumer cases.
Complete offline Rust suites passed: **730 base** and **732 native-translation**
tests, with four existing ignores in each. The focused consumer file passed all
seven tests; TypeScript build checking, the non-test Rust library check, Rust
formatting, diff checks and all 21 local documentation links passed. The Rust
library check reports only the existing gated HY-MT2 dead-code warnings. The
existing semantic reimport test now expects unchanged coverage/section identities;
changed locator/content controls still prove invalidation and vector reuse.
Historical evaluation JSON remains byte-identical. No new full CI, native-model
inference, packaged or language-quality acceptance result is claimed.

Next: focused Stage 4 review, then Stage 5's accessible editor and consumer integration.
Providers remain gated, strict disabled; benchmarks, packaged/device smoke and
the app version bump remain deferred. This does not qualify translation meaning
or complete the user-facing assisted-formatting feature.

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
| Assisted review | `review/`: immutable native evidence, bounded edit contract and pure B/I preview/publication; `review_store.rs`: revision-bound drafts and publication receipts; `review_publication.rs`: guarded completion through the existing derived writer, no inference |

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
| Durable variant/review | Existing upload database/files, shared sections/assets/FTS, `translated_documents` provenance and generation-owned `translation_review` evidence/drafts/receipts; translation schema version `3` in `upload_schema_metadata`. Review-contract version remains `1` |
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

The stable variant settings prefix binds source document ID, languages, model,
quality, repair, glossary and sentence/review policy, excluding job ID, timestamps
and hardware toggles. New assisted results append a unique generation token so
identical reruns retain older saved variants/drafts/audio. Legacy provider saves
retain their deterministic replacement behavior. Cache identity is unchanged;
source changes invalidate incompatible checkpoints independently of saved identity.

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

### Semantic acceptance gate — 2026-10-21

This is the maintainer-scoped gate for evaluating a new provider before
integration, not a production semantic detector or a claim that OPUS has completed
release acceptance. Evaluate meaning, emphasis and mechanical validity separately;
a fluent translation with valid identities can still fail. Image fidelity is
excluded from this decision; HTML/EPUB source and asset security stay unchanged.

| Judgment | Acceptable result | Failure or unresolved result |
| --- | --- | --- |
| Meaning and completeness | Preserve who does what, polarity, referents, material tense/modality, quantities/units, names, conditions and qualifications; cover every translatable proposition once | A material omission, invention, changed subject/negation/quantity, duplicated proposition or untranslated translatable clause fails. Deliberately protected literals/names are not untranslated-prose errors. Uncertain terminology/referents remain open |
| Unambiguous bold/italic scope | Apply each style to the corresponding intended meaning, preserving nested scopes and the intended occurrence of repeated words; allow faithful English reordering | Styling an unrelated concept, extending emphasis to unrelated words or inventing prose to carry an identity fails. Tag counts/IDs alone cannot qualify placement |
| Ambiguous scope and typography | Declare non-one-to-one scopes unresolved; annotate decorative initial-letter intent before judging the target | Split negation/fused forms do not require an arbitrary two-word English allocation. Widening an initial to a whole word is not exact preservation. Do not automatically merge styles or infer that every styled initial is decorative |
| Mechanical integrity | Retain existing source-owned hierarchy, references, immutable payloads, escaping, bounds and natural-completion requirements | Invalid/missing mapping, cutoff, malformed output, cancellation or save failure remains failure, even when the prose looks plausible |

Faithful paraphrases, grammatical articles/contractions and punctuation changes
can pass when the propositions, referents, register and emphasis remain equivalent.
Do not require one exact English string, ban every grammatical addition or count
every awkward phrase as a material error. Technical terminology and genuinely
ambiguous meaning need the maintainer's judgment; code-agent inspection alone is
not final bilingual acceptance.

An explicit formatting `needs-review` outcome may satisfy the existing fallback
contract only when its retained-original/labelled-translation presentation is
accepted as usable. It never counts as preserved target-word styling and cannot
excuse a meaning failure. The current code cannot reliably detect arbitrary
semantic mistakes or convert structurally valid but wrong 7B prose into an honest
review outcome. No such new adapter fallback is implemented here.

Before any integration decision:

1. Freeze model/quantization hash, language pair, runtime/template, prompt,
   sampling and transport policy. Keep all existing counterexamples. No per-case
   prompt repair, best-of-output selection, phrase stitching or hidden prose edits.
2. Annotate intended propositions, emphasis ownership and permitted ambiguity
   before generating fresh held-out FR/ES passages. Cover negation/referents,
   quantities/modality, nested/repeated scopes, full-sentence styles, literal
   islands and fused/intra-word styles. Keep private inputs and receipts outside
   the repo; do not revive the retired alignment suite or add benchmark tooling.
3. Record meaning as `acceptable / failed / undecided`, target styling as
   `preserved / wrong / unresolved`, and mechanics as `valid / invalid / unverified`,
   with a reason and source/target location. Structural validity does not close
   unverified decoder-termination requirements. Review all short cases, then one manageable
   complete representative document per language for terminology, omissions,
   emphasis and reading usability. Unread content remains untested; an opening
   or isolated paragraph does not qualify the rest of the document.
4. Keep a pair/profile unqualified while an observed material meaning failure,
   wrong unambiguous emphasis or unresolved reviewer decision remains in its
   acceptance set. Ambiguous/typographic cases need an explicit accepted policy,
   not reclassification as exact preservation. Passing one pair does not admit
   another. Finite reviewed evidence never promises zero errors on future text;
   do not average a hard failure away or present a quality percentage from this
   small screen. A changed profile must face the whole regression set again.
5. Only after a positive quality decision, qualify the actual Rust/native path
   and termination behavior, then existing lifecycle/cache/save and packaged
   acceptance. The stock chat endpoint's `stop` is insufficient. These checks,
   benchmarks and smoke are not executed by defining this gate.

The current Spanish subject error blocks 7B admission independently of formatting;
the French fused-negation and initial-letter cases have no accepted preservation
policy yet. Keep 7B gated. The next decision is whether further held-out quality
evaluation of this fixed profile is worthwhile as a diagnostic, or to evaluate a
changed profile/another maintained local model or defer it. More passing examples
cannot clear the known failure in an unchanged profile. Any replacement profile
must face the whole frozen set again. Do not begin production integration merely
because the DOM transport works.

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
| Save/delete | Disk/write/transaction failure and recovery; assisted rerun reuses eligible cache but creates a separate generation without modifying earlier variants/drafts/audio; legacy replacement unchanged; delete variant leaves original intact; source deletion leaves saved variant readable but new source jobs reject |
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
   completed assisted rerun saves a distinct generation while preserving the old
   variant, review and audio, even when inference is entirely cached. Delete a
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
rejected, and neither the [Firefox/HY screen](#firefox-dom-and-hy-mt2-18b-evaluation--2026-10-21)
nor the [7B Q4 screen](#hy-mt2-7b-q4-quality-screen--2026-10-21) qualifies a new
provider. The [source-owned trial](#hy-mt2-source-owned-html-trial--2026-10-21)
passed mechanical reconstruction but retained semantic counterexamples. Its
private preview has been reviewed and the [semantic gate](#semantic-acceptance-gate--2026-10-21)
is defined. The [revised 7B profile](#revised-7b-profile--2026-10-21) also fails,
including a structural rejection and a polarity regression. Keep 7B gated; stop
this profile's integration. The [EuroNano evaluation](#euronano-through-the-existing-bergamot-runtime--2026-10-21)
is also complete: omitting its supplied shortlist recovers useful text, but
meaning/scope failures still prevent admission. Further model/runtime comparisons
were declined. The [conclusion and development direction](#conclusion-and-development-direction--2026-10-21)
separate fixable source/structure defects from unqualified linguistic placement.
The maintainer has chosen planning for an explicitly assisted mixed-formatting
workflow and safe import normalization. Follow the [staged implementation plan](#opus-assisted-formatting-plan--2026-10-21).
Stage 1's pure contracts and Stage 2a's inline normalization are complete.
Stage 2b's coverage and literal-preservation blockers passed focused review.
Stage 3's durable snapshots, resumable drafts and pure reconstruction, including
the reviewed whitespace/traversal corrections, are complete. Stage 4 now implements
guarded atomic publication and recovery; review that change before Stage 5's
accessible editor/consumer integration remains pending. This is
not another provider experiment or a claim of automatic target formatting.
Additional passing examples cannot erase a known failure. Use Firefox's
DOM ownership as an architectural reference, not evidence of solved completeness
or semantic styling. Representative
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
