# Desktop TTS acceleration plan

**Status, November 21, 2026.** Owner approved the 20% threshold. The original ONNX export hit a compiler blocker. Intel’s prepared export runs on the Redmi GPU. It cleared the speed threshold on exact replays against Papercut CPU. Across six unseen chapters with a retained driver cache, the GPU took 596.2 seconds and the same model on OpenVINO CPU took 196.2 seconds. Keep CPU as the default on this Redmi. This is the working plan for `feature/onnx-gpu-spike`, reviewed at `d154bdd2` after merging `main` at `d642f241`. The Redmi feasibility trial is complete. The next proposed Linux MVP candidate is NVIDIA CUDA on the Ryzen 5 3600 dual-boot desktop. Review the existing CUDA integration and ordinary-app delivery design now. Apply the 20% threshold during release qualification.

## Decisions and starting evidence

- Keep the branch and selectively replace the August spike. Its `desktop:cuda` build, provider probe, and diagnostics are useful investigation tools. The intended release is the ordinary desktop app with CPU available offline and acceleration selectable for qualified models. A separate GPU AppImage is outside the release design. OpenWhispr provides a precedent for optional desktop GPU downloads and recovery from a worker crash. Knotwork shows the alternative of including a small accelerator in its Android runtime. [OpenWhispr GPU pack](https://github.com/OpenWhispr/openwhispr/blob/007826d936af15e89291c1106042f1b280a75933/src/helpers/whisperCudaManager.js) · [Knotwork release packaging](https://github.com/alexeyw/knotwork/blob/main/docs/release.md)
- Start with **English Kokoro v1.0, voice `af_heart`, Linux x64, Intel integrated GPU**. Prior inspection of the Redmi Book Pro 16 2026 found a Core Ultra 5 325, Panther Lake GPU `8086:b090` on `xe`, and a separate NPU `8086:b03e` on `intel_vpu`. CachyOS exposed the GPU through Mesa Rusticl and had an NPU device node. Subsequent trials below verified GPU offload. NPU inference remains untested. [Intel GPU configuration](https://docs.openvino.ai/2026/get-started/install-openvino/configurations/configurations-intel-gpu.html) · [Intel NPU driver](https://github.com/intel/linux-npu-driver)
- Keep native acceleration scoped to Sherpa synthesis. Search embeddings and text preprocessing share the app's ORT runtime. SILMA uses a separate PyTorch worker. Preserve the existing voice frontends, chunk validation, cancellation, resume, playback, highlighting, export, and saved-book identity. A device preference belongs to engine reuse and diagnostics. Existing audio stays usable when the device changes. [Runtime ownership](kokoro-tts.md) · [App ORT initializer](../src-tauri/src/onnx_runtime.rs)
- Treat compiled providers, requested device, session creation, actual device work, and chapter speed as separate observations. A prior CUDA smoke produced a valid WAV after a missing cuBLAS library triggered CPU fallback. Save diagnostics preserve the requested provider and leave accelerator placement unknown until verified. [Sherpa engine creation](../src-tauri/src/native_tts/engine/synth.rs) · [Provider discovery](../src-tauri/src/native_tts/engine/providers.rs)

## Original Intel GPU feasibility procedure

The following procedure guided the Redmi investigation. Keep the runner, alternative libraries, WAVs and traces in an isolated experiment directory outside the checkout and the user's audiobook cache. Record exact commands and versions with the results.

1. **Freeze the workload and comparison rule.** Choose one reproducible English chapter, preferably a public-domain text with a named edition. Export or prepare its ordered Papercut chunk manifest once. Record its SHA-256, chunk and character counts, model and voice-file hashes, voice, speed, preprocessing, and thread count. Use this manifest for every device. Apply the approved threshold of **at least 20% lower median warm chapter wall time**, repeatable across three paired runs with acceptable first-chunk latency and listening quality.
2. **Establish the runtime on the host.** Record OS, kernel, power source/profile, GPU PCI ID, compute driver, OpenVINO, Sherpa and ORT versions and loaded library paths. Enumerate OpenVINO devices. Stock Sherpa 2.0.1 expects an OpenVINO-enabled ORT and uses `openvino:<config-file>`. Set `device_type=GPU` explicitly because Sherpa defaults to NPU. The cached Papercut CPU ORT is 1.28.2. The standalone OpenVINO plugin needs device selection and registration on the model session; Sherpa owns that session. Establish a reproducible compatible combination before timing. Record any missing host driver and review host-wide installation separately. [Sherpa setup](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/OPENVINO.md) · [Kokoro session](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/sherpa-onnx/csrc/offline-tts-kokoro-model.cc) · [ORT provider lifecycle](https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html)
3. **Prove placement with one chunk.** Use stock Sherpa or a disposable runner around its published API. Feed identical inputs to explicit CPU and OpenVINO GPU sessions. The existing `cached_audiobook_smoke` shows how to replay a manifest with one warm engine, but the branch's build-time selector cannot request OpenVINO. Keep that limitation out of the product code during the experiment. In a dedicated profiling run, enable `ProfilingFilePrefix`, retain an ORT trace showing substantial compute assigned to OpenVINO, and corroborate the chosen physical GPU through execution-device metadata or a process-attributed GPU measurement. Record CPU partitions and fallback messages. Run a CPU control. A provider list, elapsed-time improvement, or valid WAV alone leaves offload **unverified**. If placement is ambiguous, report the blocker and withhold the GPU speedup claim. [Manifest replay](../src-tauri/src/native_tts/engine/synth.rs) · [Sherpa profiling and selection](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/sherpa-onnx/csrc/session.cc)
4. **Time the chapter fairly.** Use the same runner, Sherpa frontend, candidate runtime, and chunk boundaries on CPU and GPU. Also measure the shipping CPU path if the candidate CPU runtime differs, so a slow experimental baseline cannot inflate the product benefit. Capture first launch/compile and time to first valid WAV separately. For warm measurements, initialize the engine and apply the same warm-up procedure, then time from the first chapter chunk through the last validated WAV. Keep one engine for the chapter. Use fresh output directories, turn profiling off, alternate CPU/GPU order across three pairs, and record compilation-cache state, power profile and thermal anomalies. Label this measurement as chapter synthesis plus WAV writing; the installed app's full save includes additional work.
5. **Compare speed and output.** Record raw times per pair, medians and range, audio seconds per wall second, peak process memory, and GPU memory if observable. Compute wall-time reduction as `100 × (CPU median − GPU median) / CPU median`. Verify every expected chunk parses and contains finite audio at the expected sample rate. Compare durations and listen to matched short, long and punctuation-heavy passages for omissions, pronunciation, clipping and pace. Mark human listening pending until reviewed.
6. **Review the evidence before product work.** Append the date, branch revision, manifest hash, runtime/driver versions, device trace summary, raw timing table, quality findings, and durable evidence location to this document. For a faithful product comparison, identify any differences from Papercut's production frontend or save path. If GPU work is verified but chapter gains are small, record that result and stop product work. If gains and quality justify proceeding, review delivery options below. If evidence is incomplete, specify the one measurement needed to settle it.

## Separate Intel NPU trial

Deferred. If separately prioritized, the same manifest can support an independent NPU trial. The installed Kokoro graph accepts a variable token sequence and returns variable-length audio. It includes recurrent and control-flow operations. Intel's documented NPU dynamic-shape support is experimental, requires bounded shapes and specific compiler/driver support, and was validated on vision workloads. [OpenVINO NPU limitations](https://docs.openvino.ai/2026/openvino-workflow/running-inference/inference-devices-and-modes/npu-device.html#dynamic-shapes)

Select `device_type=NPU` explicitly. Verify NPU registration and real execution for short, medium and long chunks, then attempt the chapter only if these pass. Use the same quality and latency checks and, where measurable, compare energy per generated audio minute at the same power settings. Record compilation, fallback and device-specific limits independently. Re-exporting the model, padding inputs or changing voices requires a new decision. NPU success is optional for the Linux GPU MVP and applies only to the tested hardware/runtime combination.

## Archived Intel delivery options

Park both Intel delivery designs. Reopen if a new model export, runtime or workload demonstrates the approved gain on unseen chapters.

| Option | Acceptance criteria |
| --- | --- |
| Use OpenVINO in the existing process | Choose this simpler integration if a maintainable upstream runtime works with stock Sherpa, the measured installer increase is acceptable, CPU-only startup works, and search and preprocessing pass with the same ORT. Evaluate failure behavior in the reader process. |
| Offer an optional persistent Intel worker | Prefer this if runtime isolation, recoverable native crashes, or a substantially smaller base installer justify the extra process. Pin the platform, ABI, size and digest. Reuse verified staging, worker self-test, and promotion from the SILMA installer where applicable. Keep the model loaded during a save, reuse Sherpa synthesis logic, and leave chunk publication to the existing save pipeline. [Existing installer](../src-tauri/src/native_tts/engine/silma_sidecar.rs) |

Measure download and installed sizes, review dependency notices, and document a maintained update path. ORT deprecates the built-in OpenVINO provider used by Sherpa 2.0.1. An isolated worker still needs a compatible upstream API. Revisit upstream support if maintaining a local Sherpa fork becomes necessary.

Once a design is approved, qualify the ordinary Linux app on the Redmi, a clean supported Linux distribution, and a CPU-only machine. Exercise missing dependencies, insufficient memory, cancellation and a mid-chapter failure. Retry an unfinished chunk once on CPU after a recoverable acceleration failure, preserve committed chunks, and honor explicit cancellation. For a downloadable worker, also verify interrupted install, offline reuse, upgrade with an old pack and removal without deleting voices or books. Expose acceleration only for model/device combinations with evidence.

## Proposed desktop sequence

**Existing NVIDIA evidence.** The owner previously ran the branch on the Ryzen 5 3600 / NVIDIA desktop and observed substantially faster synthesis. The exact checkout, launch method, OS boot and timings are unrecorded. Branch commit `92f93ef3` added the opt-in Linux CUDA desktop build. The fallback observed in a separate smoke test explains why release qualification includes a placement check.

1. **Linux x64, NVIDIA first.** Review the existing CUDA build against current main and identify the smallest path to optional acceleration in the ordinary app, including CPU-only startup and recovery. When the Ryzen desktop is available, reproduce the working path there. On the Ryzen 5 3600 desktop, record the exact GPU, VRAM and driver. Check compatibility with the stock Sherpa CUDA build and its CUDA/cuDNN requirements. Use the branch’s existing diagnostic runner with Papercut’s shipping English Kokoro model, frontend and af_heart voice. Confirm substantial synthesis work on the GPU with an execution trace and device activity. A previous CUDA smoke silently fell back to CPU. [CUDA compatibility](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)
2. **Qualify the Linux MVP for release.** Compare against shipping Papercut CPU on that desktop. Warm both on Psalm 23, then run the six frozen chapters in three paired sessions with identical Papercut chunk boundaries, alternating order and retained caches. Read each chapter once per engine. Record cold start, time to first audio, warm chapter times and matched listening samples. Qualify the tested configuration for release with verified offload, at least 20% lower median warm chapter time, repeatable gains, acceptable first audio and comparable speech quality. If CUDA misses, keep CPU and record the result before investing in distribution.
3. **Windows x64, same NVIDIA card.** Repeat on the Windows boot with the native Windows runtime and its own CPU baseline. Check DLL loading and fallback. Make an independent support decision. Windows qualification need not delay a passing Linux MVP.
4. **Apple Silicon macOS, hardware-dependent.** Once a physical Mac is available, test the existing Kokoro export through Sherpa/ORT with Core ML, inspect graph placement and verify accelerator activity. Apply the same local CPU comparison and listening checks. Core ML can partition the graph and use multiple compute units. Record the observed device before making a GPU-specific claim. [Core ML execution provider](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)

CPU remains the supported path for the tested Redmi. Other Intel GPUs and Windows Intel graphics are unqualified. Intel NPU, AMD and mobile remain deferred. For any passing target, review optional acceleration in the ordinary desktop app, CPU-only startup and recovery before choosing packaging.

## Linux NVIDIA integration review

Reviewed the merged branch at `d154bdd2` against its main baseline `d642f241`. Keep the existing Sherpa synthesis and audiobook save paths. Use the CUDA build as a reference for the libraries that worked together. The owner’s Ryzen trial establishes a working prototype. Qualification of the merged build is still pending.

| Finding in the merged code | Consequence for one ordinary Linux app |
| --- | --- |
| The desktop build selects both the Sherpa library set and the default provider at build time. The bundler stages a complete runtime and adds the CUDA provider libraries for the CUDA variant. [Build selection](../scripts/desktop/build-desktop.js) · [Linux staging](../scripts/lib/linux/sherpa.js) | A preferences switch alone cannot enable CUDA in the CPU artifact. Start by testing a single CUDA-capable Sherpa/ORT core with CPU selected by default and optional CUDA libraries. Measure the base size increase. |
| Search and preprocessing use the shared ORT initializer. Provider discovery queries that runtime. [Initializer](../src-tauri/src/onnx_runtime.rs) · [Provider probe](../src-tauri/src/native_tts/engine/providers.rs) | Keep one compatible ORT in the reader process. Check CPU-only startup, search, preprocessing and Kokoro with CUDA libraries absent before considering an in-process pack. |
| At review time, successful Sherpa creation copied the requested provider into the effective field. The explicit CPU retry runs only if creation returns no engine. Save progress and results forward both fields. [Engine construction](../src-tauri/src/native_tts/engine/synth.rs) · [Save reporting](../src-tauri/src/native_tts/engine/save.rs) | Fixed below. Save diagnostics preserve an unknown effective provider for unverified accelerator requests. |

**Provider reporting.** Save progress and results preserve the original request. Explicit CPU selection reports CPU, successful accelerator creation reports a null effective provider, and successful CPU retry reports CPU. A native regression test exercises all three creation paths and checks the provider attempt order.

**Then test the packaging assumption.** Stage the pinned Linux x64 CUDA-capable core in a disposable directory. Inspect its loader dependencies and compare installed sizes with the CPU core. Exercise CPU startup and the shared ORT consumers with the optional libraries absent. On the Ryzen, add the matching CUDA provider and dependencies from a separate directory and verify discovery, actual GPU work and CPU recovery. Record how the loader locates the libraries and whether a restart is needed. If those checks pass, review a versioned, verified optional download for the ordinary app. If the core requires CUDA at startup, the separate directory cannot be loaded reliably, or the base-size cost is unacceptable, evaluate a persistent isolated worker. Keep the existing downloader, worker and save pipeline as reference points until this experiment settles the process boundary.

### Redmi Linux packaging experiment, November 21, 2026

**The CPU-only half passed.** The ordinary Papercut 1.10.2 Linux x64 release executable opened a visible Papercut window with a disposable profile. Its relative package library path resolved the CUDA-capable Sherpa and ONNX Runtime cores with no loader override. The process map confirmed both staged libraries. The staged directory contained the upstream CUDA archive’s ORT, Sherpa C and C++ libraries plus the ordinary package’s Piper core. Both ORT provider libraries, TensorRT and the NVIDIA toolkit were absent from the layout. The core ELF dependencies and the host loader cache showed no required NVIDIA runtime.

The download matched the branch’s pinned SHA-256 (`2aa541a65c1e7d921a053283419b70b30c1f6ae0a0f52f64b947ebe88c5f2a77`). The GUI used the previously packaged release binary. For functional checks, the merged checkout’s native test binary was built offline with `native-tts-shared,experimental-search-embeddings` and run against the same staged libraries using `LD_LIBRARY_PATH`. Loader traces confirmed those paths and showed no CUDA provider or NVIDIA library initialization.

| CPU workload | Observed result |
| --- | --- |
| English Kokoro, af_heart | The six-chunk Psalm 23 manifest produced six valid WAVs totaling 31.02 seconds. |
| Arabic preprocessing | Cached Libtashkeel produced the expected diacritized text. Piper then synthesized finite, non-silent audio. |
| Keyword search | All 29 native document search tests passed. |
| Semantic search | The cached dynamic INT8 embedding model activated, ran inference, passed index/query checks and unloaded. |

| Size comparison | CPU | CUDA-capable base | Increase |
| --- | ---: | ---: | ---: |
| Installed ORT + Sherpa C and C++ libraries | 32,185,897 B | 32,771,793 B | 585,896 B (0.56 MiB) |
| Matched Debian packages | 58,302,536 B | 59,961,134 B | 1,658,598 B (1.58 MiB, 2.8%) |

The Debian comparison used the same extracted release tree, regenerated checksums, normalized tar metadata and gzip level 6 for both variants. The optional CUDA and shared provider libraries alone occupy 621,477,968 bytes installed (592.69 MiB), before any required CUDA/cuDNN dependencies. The complete upstream CUDA archive download is 393,636,488 bytes versus 9,710,131 bytes for the CPU archive. A future optional download needs its own dependency inventory and compressed size.

**Decision.** Continue testing one ordinary app with a CPU default. The next experiment belongs on the Ryzen NVIDIA machine. Load the matching provider and its dependencies from a separate directory, verify real Kokoro GPU execution, and exercise CPU recovery with the pack absent or incomplete. These Redmi checks cover native functionality and window startup. Installed-package lifecycle, UI-driven synthesis/search and AppImage sizing remain outside this measurement.

## Progress

- [x] Merge the baseline, review the branch and agree on the 20% threshold.
- [x] Reproduce the original export blocker and verify actual GPU work with Intel’s prepared export.
- [x] Complete the six-chapter Redmi comparison and park Intel GPU delivery for this workload.
- [x] Record the owner’s successful NVIDIA branch trial on the Ryzen desktop.
- [x] Review the merged CUDA integration and ordinary-app delivery options.
- [x] Correct requested/effective provider reporting and cover all three outcomes.
- [x] Verify the CUDA-capable base without NVIDIA libraries on the Redmi and measure its size.
- [ ] Verify the optional provider directory and dependency loading on the Ryzen NVIDIA desktop.
- [ ] Qualify the current Linux build against shipping CPU on the Ryzen desktop using the approved 20% threshold.
- [ ] Qualify Windows NVIDIA independently. Schedule macOS feasibility when Apple Silicon hardware is available.

## Redmi experiment result, November 21, 2026

**Hold delivery work.** The Intel GPU executed a numerical control. Both OpenVINO routes failed to compile the unchanged Kokoro graph before inference. No Kokoro operations were verified on the GPU. The 20% warm-time criterion remains unevaluable.

| Native CPU run | Whole-process wall time | Generated audio | Valid chunks |
| --- | ---: | ---: | ---: |
| 1 | 8.687 s | 31.015 s | 6/6 |
| 2 | 8.708 s | 31.021 s | 6/6 |
| 3 | 8.679 s | 31.015 s | 6/6 |

The native baseline used the existing Papercut `cached_audiobook_smoke` binary, one fresh process and output directory per run, English Kokoro v1.0, `af_heart`, speed 1, four threads. Median **8.687 s** includes engine startup and WAV validation. The short workload was Psalm 23, KJV, six verse chunks and 118 words. These cold-engine numbers are context for the placement trial. A longer workload and paired warm measurements would be needed for a shipping decision. An auxiliary Python CPU runner skipped out-of-vocabulary words and was excluded from the comparison.

On CachyOS kernel 7.3.5-2, AC power and CPU energy preference `performance`, the distro Intel compute runtime 26.40.40196.8 and graphics compiler 2.42.6 were SHA-256 checked against pacman's repository metadata and extracted under the scratch directory. An experiment-local ICD exposed the Panther Lake GPU. The system driver installation was untouched.

1. Sherpa 2.0.1's Python native binary requires ELF symbol version `VERS_1.28.2`. Substitution of the OpenVINO-enabled ORT 1.24.1 library failed at load time.
2. Direct ORT 1.24.1 with its OpenVINO provider targeted `device_type=GPU`. Kokoro failed during session creation with `rank().is_static()`. Repeating with `disable_dynamic_shapes=True` produced the same error.
3. ORT 1.32.0 with standalone OpenVINO EP 1.8.1 explicitly selected the enumerated GPU device (Intel `0xb090`, PCI 00:02.0). The same Kokoro model again failed the static-rank check during session creation, before text or synthetic input could be submitted. The error identifies a compiler constraint; the offending operation remains unknown.
4. With that same plugin and device selection, a MatMul/ReLU control returned the expected numerical output within tolerance and its ORT kernel trace attributed execution to OpenVINO. This distinguishes model compilation failure from a missing GPU. Further investigation would start by isolating the failing subgraph for upstream. App integration, packaging and the independent NPU trial remain on hold.

**Reproduction evidence.** Local project evidence directory `.memsearch/evidence/2026-11-21-redmi-kokoro/` contains the chapter, manifest, probes, logs and ORT profiles. Scratch runtimes and WAVs are in `/tmp/redmi-kokoro-spike/`. Replay the native CPU test with the saved manifest, the installed Sherpa model root, and a fresh smoke output directory. The isolated graph probes select the GPU before session creation. No matched listening or GPU memory measurement was possible.

| Input | SHA-256 |
| --- | --- |
| Chapter text | `437ad0ba280642b7f2f0641dcec3a4aa31ec928043ac360d4fb74188b93f91d6` |
| Chunk manifest | `bc999cbc708aee732581396cce942737f46f7dd950845ac68f578431cead6e55` |
| Kokoro model | `b40f62b166ac8164b0627ef48a0b358eda0985e272fb03ef5252e7206305da11` |
| Voices | `1c5a5b983d3d50d8586d437a51f3faa2da7919ce76a013c081e65671a3447c29` |

### GPU compiler reduction, November 21

**One operator reproduces the error.** In the original opset 14 graph, node 2154 (zero-based), `/Squeeze`, takes `/Cast_3_output_0` and emits the duration vector `onnx::Shape_3411`. Extracting that node with an INT64 input of shape `[1,N]` gave a 140-byte ONNX reproducer without weights. The standalone OpenVINO EP failed during session creation. Adding constant axes `[0]` produced a 163-byte passing control. With input `[[0,1,2,3]]`, it returned the exact vector on the explicitly selected Intel GPU and recorded an OpenVINO kernel event. An isolated axes-free waveform squeeze also reproduced the error. Rewriting only the waveform squeeze or the two conditional squeezes left the original full-model assertion in place.

An implicit squeeze removes every dimension of size one. With dynamic N the output can be a scalar or vector. Axis zero gives a vector for every N. The native model's CPU graph showed duration inputs of shapes `[1,5]` and `[1,8]` on synthetic token sequences and exact agreement for their duration outputs. Equivalence at N=1 would require a separate argument. These checks establish a compiler reproducer and a local workaround for that node.

| Scratch full-model experiment | Result during GPU session creation |
| --- | --- |
| Explicit axis on duration squeeze only, ORT 1.32.0 + OpenVINO EP 1.8.1 | Passed the original assertion, then failed in the sine generator's Add combining FP16 Gather and FP32 ScatterND results. |
| Same model, GPU precision hint FP32 | Reached a convolution whose inferred time dimension was zero, too short for its kernel. |
| Same FP32 test with token length fixed to five | Graph conversion still failed on Conv and InstanceNormalization. |
| Duration edit, built-in ORT 1.24.1 OpenVINO provider, FP32 | Static-rank assertion persisted. |

**Driver assessment.** The failing and passing one-node models used the same Redmi GPU, kernel, temporary Intel OpenCL runtime 26.40.40196.8 and graphics compiler 2.42.6. Those packages matched the local distro repository metadata. System-wide installation would make the compute stack available to other launches; the experiment already loaded it. The observed failure tracks model rank handling during compilation. A different driver version was not A/B tested. Intel documents dynamic-rank limits for the GPU backend. [GPU dynamic shapes](https://docs.openvino.ai/2024/openvino-workflow/running-inference/inference-devices-and-modes/gpu-device.html)

**Replay.** With an accessible Intel GPU and its compute runtime, run `python docs/repros/kokoro-gpu-rank.py` using Python 3.12, ONNX 1.23.2, ORT 1.32.0, onnxruntime-ep-openvino 1.8.1 and NumPy. The script generates both graphs in memory and verifies placement and output for successful sessions. Local evidence in `.memsearch/evidence/2026-11-21-redmi-rank/` includes the original extracted graphs, transformation scripts, and compiler logs. The next upstream question is whether the GPU backend can handle the dynamic duration rank and subsequent decoder partition. No usable full-model GPU fix or chapter speedup was established.

### Intel-prepared Kokoro on the Redmi GPU, November 21

**Real GPU synthesis confirmed. The approved 20% improvement was achieved on same-chapter replays only.** For the first chapter in a sentence-warmed engine, the prepared Intel path was slower than the native Papercut CPU baseline. This result supersedes the earlier model-compilation blocker only for the separately converted Intel model.

Used Intel's [pinned native OpenVINO IR](https://huggingface.co/OpenVINO/Kokoro-82M-int8-ov/tree/9c035d0bfab136f0c1c525a5841d3411d7220c66), OpenVINO 2026.4.1, local Intel compute runtime 26.40.40196.8, Misaki US English 0.9.4 with eSpeak fallback, the export's vocabulary and af_heart style table. Verified the IR weights and voice against upstream SHA-256. The direct Python harness selected GPU explicitly, supplied style row (phoneme length minus one), speed 1 and the six original Psalm 23 chunks. CPU used Papercut's native manifest replay, existing model/voice, four threads and the same ordered texts. AC power and performance preference. Each warm-up sentence was excluded. Timings span the completion of the warm-up WAV through the final verse WAV, including text processing, synthesis and file writes. These are synthesis benchmarks, without Tauri UI/book indexing.

| Engine state | CPU seconds | Intel seconds | Median wall-time change |
| --- | --- | --- | --- |
| First chapter after one warm-up sentence in each fresh process | 6.653, 6.659, 6.637 | 25.513, 10.045, 10.239 | 6.653 → 10.239 s, **53.9% longer** |
| Three exact replays following a full chapter in one persistent process per engine | 6.681, 6.658, 6.641 | 2.238, 2.557, 2.269 | 6.658 → 2.269 s, **65.9% shorter** |

The initial alternating CPU/GPU, GPU/CPU, CPU/GPU series reused the GPU process and gave Intel 25.513, 2.329 and 2.237 s. Two additional fresh GPU processes and a dedicated replay series separated those different conditions. Fresh-process samples were sequential follow-ups rather than all randomized pairs. GPU model compilation alone took about 2.0–2.2 s, excluded from the table. The repeated text reuses identical input lengths. The replay result establishes potential for a persistent engine; performance on *new* chapter text in that engine remains open.

**Placement.** A separate profiled synthesis of the actual first verse selected GPU.0. The process's xe compute-engine counter on PCI 0000:00:02.0 increased from 4,517,914 to 27,160,040 cycles across the request. The profile recorded GPU matrix/convolution kernels and CPU operations, including recurrent work in the probe. This is mixed CPU/GPU execution. Profiling was disabled during timing.

**Speech comparison.** All six verses produced valid 24 kHz mono PCM in each run. On the third paired run, CPU yielded 31.011 s and Intel 37.450 s at nominal speed 1. The Intel recording was 3.0 dB lower in whole-file RMS; neither had samples at or above 99.9% of full scale. End-of-verse quiet time (10 ms frames, −50 dBFS RMS) totaled 0.51 s CPU versus 2.90 s Intel and accounts for part of the duration difference. Offline Whisper small.en decoded both recordings without a reference prompt. The differences from the 118-word source were written compound forms: “for ever”/“forever” on both, and “name’s sake”/“namesake” on CPU. No clear lexical omission was detected. This is an intelligibility check; pronunciation, expressiveness and preferred pacing await human listening.

**Decision.** Keep the prepared export as a viable Intel inference path. The next discriminating benchmark is new, varied text in a persistent engine after a representative warm-up, with matched listening and the same 20% wall-time criterion. Replaying the identical chapter is an optimistic estimate for normal audiobook creation. App integration remains on hold. Local evidence, scripts and the full matched listening pair are in `.memsearch/evidence/2026-11-21-redmi-prepared/` (`cpu3.wav`, `gpu3.wav`). The isolated scratch runtime is in `/tmp/redmi-prepared/`.

### First pass on Psalm 24 after warming on Psalm 23, November 21

**Intel missed the approved 20% threshold on unseen Psalm 24: median 37.499 s versus Papercut CPU 10.447 s (258.9% more wall time).**

| Pair | CPU wall s | Intel wall s | CPU audio s | Intel audio s | Intel GPU compute cycles during Psalm 24 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 10.499 | 38.434 | 47.37 | 56.90 | 207,251,263 |
| 2 | 10.440 | 37.499 | 47.37 | 56.90 | 228,220,192 |
| 3 | 10.447 | 37.264 | 47.36 | 56.90 | 227,007,388 |

Three sequential pairs on the same Redmi, alternating CPU/GPU, GPU/CPU, CPU/GPU. Each process loaded its engine once, synthesized all six Psalm 23 verses, then immediately synthesized the ten previously unseen Psalm 24 verses (KJV, 178 words). Each GPU process began with a fresh Intel compute runtime disk-cache directory to avoid cross-trial reuse of work from Psalm 24. Engine startup, compilation before synthesis, and the six warm-up verses were excluded. Compilation or specialization needed during the measured chapter counted toward the result. Both paths used the same af_heart voice name, speed 1 and chunk boundaries. Native CPU used four threads; Intel used the same pinned native IR, Misaki frontend and GPU.0 as the previous trial. AC power and performance preference.

Elapsed time was measured between final WAV modification times for the last warm-up verse and the last measured verse, as in the earlier trial. All 60 measured verse files contained non-silent mono 24 kHz PCM. The benchmark process's xe compute-engine counter increased across each measured GPU chapter. This corroborates GPU execution; prior profiling showed some operations also execute on CPU. This is a time-per-chapter comparison. Quality was not graded in this trial.

Reproduction inputs, commands and results: local project directory `.memsearch/evidence/2026-11-21-redmi-unseen/`. Psalm 24 text SHA-256 `7e88927ea2c7ded83a088358ecfc39764d50f04a0a40b74439265e604fee8b20`. One short chapter and one device limit generalization. App integration remains on hold.

### Same Intel Kokoro model on OpenVINO CPU and GPU, six longer unseen chapters

**Intel GPU missed the 20% threshold against the same model on OpenVINO CPU: 190.7% more time comparing the median two-chapter session totals.** Across all chapters GPU used 596.20 s versus CPU 196.15 s (203.9% more time). These are generation times. Total output durations were 1141.2 s on CPU and 1141.5 s on GPU.

| Psalm | Words | CPU wall s | GPU wall s | GPU relative to CPU |
| --- | ---: | ---: | ---: | --- |
| 22 | 586 | 31.52 | 115.81 | 267.4% more time |
| 31 | 507 | 27.15 | 83.86 | 208.9% more time |
| 35 | 573 | 31.99 | 98.94 | 209.3% more time |
| 69 | 672 | 36.71 | 105.84 | 188.3% more time |
| 104 | 604 | 32.89 | 85.69 | 160.5% more time |
| 105 | 637 | 35.89 | 106.06 | 195.5% more time |

Two-chapter totals by session — session 1: CPU 58.68 s, GPU 199.68 s (240.3% more time); session 2: CPU 68.70 s, GPU 204.78 s (198.1% more time); session 3: CPU 68.78 s, GPU 191.74 s (178.8% more time).

**Design.** Three independent engine lifetimes, two distinct chapters in sequence per lifetime. Each engine warmed on all six Psalm 23 verses, then voiced each measured chapter once per device across the entire experiment. The GPU driver cache was seeded from the earlier Psalm 23/24 experiment and retained across every chapter and restart. No pre-existing audio files were inputs. The device order alternated, with devices running serially. The two engines in each process shared one Misaki frontend and ran the identical pinned INT8 IR, vocabulary, af_heart style table and speed 1. All chapter input token stream hashes matched between devices. Each used OpenVINO 2026.4.1 latency mode, CPU with four inference threads, GPU explicitly targeted on the same Redmi with Intel compute runtime 26.40.40196.8. AC power and performance preference were observed. This compares configured execution paths, including their device-specific kernels and internal precision choices.

**Scope of timing.** Monotonic wall time from before the first verse through validation of the final WAV. Includes phonemization, synthesis, WAV writes, validation and first-use work during those verses. Model loading and Psalm 23 warm-up excluded. Each run validated finite, non-silent output and verified 24 kHz mono PCM. The xe compute-engine counter increased during every measured GPU chapter. CPU and GPU audio durations were recorded; listening quality was outside this timing trial. Results describe one device, one CPU thread budget, this corpus and verse chunking. The production Papercut CPU pipeline is a separate baseline.

Corpus: six whole KJV Psalms, 3,579 words, frozen in `chapters.json` (SHA-256 `f61afbff3513404fa3299b7cb73c08b6a6aafe21f0c3f77948d6caa2bcb19a18`). [Source text](https://github.com/thiagobodruk/bible/blob/master/json/en_kjv.json). Reproducible local evidence: `.memsearch/evidence/2026-11-21-redmi-long/`, including model and voice hashes, warm-up and measured texts, runner, raw timing/device events, validation and summary. No app changes.
