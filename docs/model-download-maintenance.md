# Model download maintenance

## Stage 1: pinned Kokoro repair and monitoring

The September 8, 2026 upstream replacement of `kokoro-multi-lang-v1_0.tar.bz2`
changed its SHA-256 from `c133d263…be7046` to `c5f7e2d2…ea3298`.
The old AppImage correctly refused the replacement. Failed Kokoro downloads
are deleted and each attempt starts fresh; clearing the cache cannot repair
an outdated checksum embedded in an app.

`src-tauri/tts/model-manifest.json` now owns archive URLs, exact sizes, and
SHA-256 values, with one entry per archive in its schema-version-2 `archives`
array. `src-tauri/build.rs` generates Rust constants from it; the build and
monitor reject duplicate archive names. Model IDs, languages, runtime
configuration, and voice mappings remain in `models.rs`.

Kokoro's primary URL targets the Papercut release
`tts-kokoro-v1.0-2026-09-08`. The upstream URL is a fallback for HTTP/connection
failure before streaming starts. Both locations must supply exactly the same
bytes: size and SHA-256 verification always run before extraction. Midstream
failures and integrity failures are reported and cleaned up, not silently
accepted or retried against a different checksum. Existing installed models
remain usable without downloads or upgrades.

Piper and Supertonic retain their upstream URLs and checksums. Supertonic's
old approximate download size is replaced with its exact 128,774,318 bytes
so size verification is meaningful. Their mirroring is outside this Kokoro
repair; in particular, retain the documented Piper provenance review before
redistributing it.

Local validation on CachyOS passed all 173 native unit tests and the explicit
archive smoke test for ten Sherpa entries (seven Kokoro languages, Supertonic
English/Arabic, and Piper Kareem). All three unique upstream archives were
freshly downloaded and verified. The monitor's regression check, workflow YAML,
and shell syntax checks passed. The unpublished mirror returned the expected
404; hosted workflow execution, notification delivery, and packaged-app checks
remain activation steps below.

## Activate after committing

The workflows below only become operational once pushed. No model release,
repository setting, user notification preference, or installed app is changed
by the source patch itself.

1. Merge the changes and enable GitHub Actions failure notifications for the
   account responsible for the schedule. Run **Model Downloads → metadata**
   before publishing the mirror: this is an expected failure. Confirm that the
   failure notification reaches you. Native GitHub notifications are used; no
   email/webhook integration or customer telemetry is added.
2. Run **Actions → Model Downloads → Run workflow →
   prepare-mirror**. It downloads Kokoro once, checks its published identity,
   verifies the actual bytes, extracts it, checks required files and speaker
   ranges, and generates non-silent audio for all seven language entries using
   the production Rust synthesis functions. It also runs native unit tests.
3. After validation, the workflow creates a **draft** release with the original
   archive, `SHA256SUMS.txt`, and the manifest. It never overwrites an existing
   release. If a draft already exists, inspect it rather than rerunning upload
   with `--clobber`.
4. Enable GitHub immutable releases in the repository settings **before
   publishing the draft**, then publish it without marking it latest. This
   setting applies to subsequent app releases too: immutable releases cannot
   be repaired by replacing assets; release a new version instead.
5. Run **Model Downloads → full**. Production checks deliberately require the
   primary mirror to exist and be immutable; they do not hide a failed mirror
   by switching to upstream. The app release workflow also checks production
   model metadata before any platform build starts.
6. Ship a new Papercut version and test its first-time install in a clean app
   profile on each supported platform. Current released AppImages still contain
   the old checksum and require that app update.

The hourly check runs at minute 23 UTC; the daily full check runs at 08:41 UTC.
The former compares GitHub asset availability, exact byte count, and digest;
the latter performs fresh downloads (never a model cache), Rust verification,
extraction, and native synthesis for all Sherpa catalog entries. The mirror
preparation mode intentionally selects only Kokoro. Rust build outputs may be
cached; downloaded model archives are not cached by the monitor.

These checks exercise the default-branch manifest and the release workflow's
selected app tag. They do not continuously test every historical app release,
SILMA's separate installer, the packaged GUI, or audio quality. A successful
synthesis check proves that the runtime can produce valid non-silent audio,
not that pronunciation is perfect. Supported-version monitoring belongs with
the next catalog stage. GitHub schedules can be delayed and can be disabled
after repository inactivity; check the Actions history periodically and use an
external heartbeat if strict monitoring delivery becomes a requirement.

## Local checks

```bash
node --test scripts/check-model-downloads-selftest.js
node scripts/check-model-downloads.js
node scripts/check-model-downloads.js --download-dir /tmp/papercut-models

ORT_LIB_LOCATION="$PWD/src-tauri/target/sherpa-onnx-prebuilt/sherpa-onnx-v1.13.4-linux-x64-shared-lib/lib" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --features native-tts-shared --lib

PAPERCUT_MODEL_ARCHIVE_DIR=/tmp/papercut-models \
ORT_LIB_LOCATION="$PWD/src-tauri/target/sherpa-onnx-prebuilt/sherpa-onnx-v1.13.4-linux-x64-shared-lib/lib" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --features native-tts-shared \
  --lib model_archive_smoke -- --ignored --nocapture
```

Before the mirror exists, add `--upstream` to the download command. For a
Kokoro-only check, also use `--family kokoro` and set
`PAPERCUT_SMOKE_KOKORO_ONLY=1` for the Rust smoke test. These options are for
artifact preparation, not substitutes for the production mirror check.
Native tests require the normal Linux build dependencies. The ignored smoke
test requires real archives, leaves no installed models in the app profile,
and removes its extracted working directory on success. On failure it retains
that directory beside the archives for inspection.

## Next stage

The next step is rollout: publish the mirror, verify notifications, run the full
hosted check, and release and test the repair as described above. Create the PR
for this stage before adding more download infrastructure.

A later signed remote catalog would allow compatible download repairs without
an app update. Defer that implementation until rollout is complete. It needs
app/runtime compatibility constraints,
verified cached and bundled fallbacks, and explicit artifact revisions. It
should authorize only reviewed compatible artifacts; it must not automatically
trust new upstream hashes or silently upgrade Kokoro v1.0 to v1.1. Include
checks for supported released-client contracts and retention of their artifacts.
Resumable Kokoro downloads remain a separate improvement.

References: [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases),
[Actions notifications](https://docs.github.com/en/actions/concepts/workflows-and-actions/notifications-for-workflow-runs),
[scheduled workflow behavior](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#schedule).
