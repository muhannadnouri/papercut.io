# Library Transfer

## Purpose

Library Transfer copies Papercut-owned user data from one device to another
without an account, cloud service, or continuous synchronization. The source
device is unchanged and the receiving device merges new content into its
existing library.

The product deliberately says **transfer**, not **sync**. Continuous two-way
sync would require persistent device identity, deletion propagation, conflict
resolution, and background networking that this feature does not need.

The revised LAN UX target is **no terminal setup for ordinary users**: select
content and a nearby device, enter its pairing code, and approve an OS network
or administrator prompt when necessary. Do not require IP/port knowledge or a
firewall exception for each phone. Transfers remain local, including pairing and
discovery: no account, Internet rendezvous, relay, NAT-traversal service or router
port forwarding. This is a planned acceptance requirement, not a capability the
current manual transport already delivers. An isolated or administrator-blocked
network can still prevent LAN transfer; explain that limit and offer file export.

## Current Validation and PR Readiness

[Full CI run 39068099023](https://github.com/muhannadnouri/papercut.io/actions/runs/39068099023)
completed successfully after the Apple Silicon job rerun on October 17, 2026,
for `refactor/library-transfer` at `48a2fb6acaddcdfa24c4e5f64e55775764f0d9f2`.
All nine jobs passed across the full validation scope. Provider-inference steps
were skipped as intended (`provider_inference: false`).

- Checks passed: npm/Rust dependency audits, lint, frontend and Rust tests,
  download/provider-runner regressions, frontend/Pagefind build, SILMA self-test
  and opt-in search-provider compilation/regressions.
- Linux AppImage/deb, Windows MSI/NSIS, both macOS architectures and Android APK
  builds passed, including the applicable bundle checks and Windows/macOS native
  discovery tests. Android discovery callback tests passed.
- Unsigned iOS simulator and device builds with native TTS passed, confirming
  the Apple Bonjour linking repair against both SDK targets.
- The Linux CPU SILMA runtime packaging job passed.

The checks job passed 372 frontend tests (one existing skip), 267 ordinary Rust
library tests (four existing ignored tests) and 325 tests with native TTS and the
opt-in search capability compiled (nine ignored tests). These compilation/test
checks are separate from the deliberately skipped model-inference acceptance.

The [rerun Apple Silicon job](https://github.com/muhannadnouri/papercut.io/actions/runs/39068099023/job/117515950022)
created and uploaded the DMG, passed 21 native transfer-discovery tests and
verified the bundled runtime libraries. The first attempt had failed during DMG
creation after successful compilation and `.app` bundling. The unchanged commit
passed on rerun without packaging, signing or application changes; the original
disk-image failure's exact cause remains unconfirmed.

This run clears the automated build/test gate for the task-first flow, mobile
presentation, wider desktop dialog, explicit Send selection and Ammonia repair.
Their local reviews found no further actionable code issues. The earlier
[full CI run 39003088519](https://github.com/muhannadnouri/papercut.io/actions/runs/39003088519)
validated the ZIP/Apple linking fixes at `d0409fe`; the latest run also covers
the subsequent changes. Carry the latest validation and explicit device-acceptance
deferrals into the PR description. No additional implementation blocker is known.

The earlier [CI run 39065709932](https://github.com/muhannadnouri/papercut.io/actions/runs/39065709932/job/117492826914)
at `de1a566` failed its Rust audit on newly published Ammonia advisory
`RUSTSEC-2026-0373`. The 4.2.4 repair below is now validated by the successful
full run at `48a2fb6`, with the existing warning policy and sole waiver unchanged.

Packaged-device validation remains incomplete for discovery, permissions/firewall
behavior, reverse transfers, resume/cancellation and lifecycle handling. Keep the
remaining smoke tests and the app version bump deferred. Passing CI does not
establish the no-terminal LAN UX or authorize an automatic UFW exception.

The user subsequently reported a successful Linux → Android transfer on October
17, 2026. This is one successful device case; the broader permission, reverse-
direction, resume/cancellation and lifecycle matrix remains unverified.

## Architecture

One versioned `.papercut-library` package is the boundary between storage and
transport:

```text
Papercut storage -> package export -> user-selected transport -> package import -> Papercut storage
```

Stage 1 uses the operating system file picker. A user can move the package by
USB, shared storage, AirDrop, Quick Share, LocalSend, or any other mechanism.
Stage 3 sends that exact package directly to another Papercut device on the
same network; it does not introduce a second persistence format.

Rust ownership lives under `src-tauri/src/library_transfer/`. React ownership
lives under `src/library-transfer/`; shared dialogs and controls remain in their
existing component modules.

## Canonical And Derived Data

The package carries selected canonical user data:

- sanitized, normalized `source.html` for each generic HTML, EPUB, TXT, or Markdown upload;
- content-hashed reader-image assets referenced by transferred EPUB reading HTML;
- stable document ids, display titles, optional original filenames, and import metadata;
- uploaded-library folders and document placement metadata;
- optional completed-audiobook manifests, canonical chunk WAVs, and imported
  audiobook source documents;
- a manifest with package and payload checksums.

The package does not carry derived or platform-specific data:

- `search.sqlite3`, FTS rows, or Pagefind output;
- TTS models, SILMA runtimes, generated playback caches, or diagnostics;
- incomplete audiobook jobs or application build artifacts.

Completed audiobook metadata is discovered from the native manifest stored
beside each audiobook's canonical chunk WAVs. The saved-audiobook UI no longer
depends on a duplicate WebView `localStorage` registry, so restored native files
appear automatically. Audiobook export is optional, individually selectable,
and off by default because these payloads can make a transfer package several
gigabytes larger.

The receiver parses and sanitizes every transferred reflowable document again, then
rebuilds SQLite metadata, sections, and FTS rows with its installed app version.
Sanitizer regressions cover active content, encoded script URLs, and known
SVG/MathML mutation-XSS structures. CI audits both committed Rust and npm
dependency lockfiles on pull requests and weekly so newly published vulnerability
advisories fail closed even when application code has not changed.

The Tauri WebView uses a default-deny Content Security Policy. It permits
only bundled application resources, Tauri IPC, the narrowly scoped local asset
protocol, raster image data, blob-backed audio and workers, and the WebAssembly
execution required by the bundled PDF/OCR engines. Remote scripts, frames,
objects, forms, and network connections remain blocked. Tauri continues to
inject hashes and nonces for bundled scripts rather than allowing inline script.

The sole RustSec waiver is `RUSTSEC-2026-0235`: `rkyv` 0.7 remains in the
lockfile through `tauri-plugin-log` -> `byte-unit` -> `rust_decimal`, but
`rust_decimal`'s optional `rkyv` feature is inactive. CI checks Papercut's
complete feature/target dependency graph and fails if that edge becomes active.
Remove the waiver once the lockfile no longer contains `rkyv` 0.7.

The October 12, 2026 audit repair upgrades Ammonia to 4.2.3 and the locked
`quinn-proto` to 0.11.18 without new waivers. Compatible `anyhow`,
`event-listener` and rand 0.8 patches also remove their reported unsoundness
warnings. The updated lockfile has no blocking findings against that day's
RustSec database, but retains eight maintenance notices and upstream GLib 0.18
and rand 0.7 unsoundness notices. GLib is required by the Linux GTK/WebKit stack;
rand 0.7 is parser build tooling without the affected logging feature. These
remaining warnings are not treated as fixed or suppressed by this repair.

The October 16, 2026 manual mobile CI run
[38969619278](https://github.com/muhannadnouri/papercut.io/actions/runs/38969619278)
on `750fbd1` completed its Rust audit with zero blocking vulnerabilities and
11 warnings (eight maintenance, two unsoundness and one yanked `zip` 0.6.6
notice). The job then failed when `rustsec/audit-check` updated a separate GitHub
check, despite having `checks: write`; Rust tests and mobile packaging were
skipped. This is not Android acceptance or a successful CI run.
CI now runs pinned `cargo-audit` 0.22.2 directly, using its exit status and log
output without a separate Check/issue API call or write-token permission.
Blocking vulnerabilities and audit errors still fail the job; the existing
warning policy, sole waiver and inactive-dependency guard remain unchanged.
Local verification with that CLI and a fresh official advisory database reproduced
zero blocking findings and the same 11 warnings. A temporary known-vulnerable
lockfile and an unavailable database both returned failure. Workflow YAML,
checks-job shell syntax and whitespace validation passed; the replacement still
needs a new GitHub CI run before mobile packaging can be claimed successful.
The owner subsequently reported a successful Android APK build of `7525feb`.
No subsequent run URL/full CI result was independently verified here, and the
Linux-to-Android transfer below still failed; APK packaging is not LAN acceptance.

The October 17, 2026 audit follow-up raises the Ammonia minimum and locked version
from 4.2.3 to **4.2.4** for
[RUSTSEC-2026-0373](https://github.com/RustSec/advisory-db/blob/main/crates/ammonia/RUSTSEC-2026-0373.md).
The upstream flaw bypasses URL filtering for quoted, mixed-case CSS `URL(...)`
when an application enables inline CSS. Papercut's reader and EPUB chapter
sanitizers strip styles and do not enable that affected configuration. The patch
still removes the vulnerable dependency; both application boundaries now test
lowercase, uppercase and mixed-case CSS URLs while retaining readable text and
EPUB anchors. These tests protect the existing no-style policy rather than
claiming an application exploit was reproduced. No sanitizer policy, stored
document format or audiobook data changes.

Only Ammonia's version/checksum changes in the lockfile. Using CI's pinned
`cargo-audit` 0.22.2 and the same freshly fetched 1,338-advisory database, the
committed 4.2.3 lockfile fails solely on this advisory and the repaired lockfile
passes with **zero blocking vulnerabilities**. All 13 allowed warnings remain
visible (11 maintenance and two unsoundness notices), with no new waiver or CI
policy change. `cargo test --manifest-path src-tauri/Cargo.toml --lib
--no-default-features --locked --quiet` passes 267 tests with four existing
ignored tests; Rust formatting and documentation checks pass. The subsequent
full CI run `39068099023` passes at `48a2fb6` after the Apple Silicon DMG rerun,
with `provider_inference` off. Smoke and the app version bump remain deferred.

The later [CI run 38996924208](https://github.com/muhannadnouri/papercut.io/actions/runs/38996924208/job/117291439491)
on `dc92ed5` failed correctly: RustSec reported `RUSTSEC-2026-0367`,
`RUSTSEC-2026-0368` and `RUSTSEC-2026-0369` against the direct `zip` 0.6.6
dependency. These affect automatic extraction of permissions and symlinks;
Papercut instead reads declared entries through its own bounded, checksummed
import path and does not use those extraction APIs. The repair upgrades to
`zip` 9.0.4 with only the portable Rust DEFLATE backend enabled, without adding
waivers or changing the audit policy, package schema, LAN protocol or app version.
The dependency's Apple filesystem support requires `nix` 0.31.3 and a compatible
`libc` patch, so the lockfile also moves `libc` from 0.2.182 to 0.2.191.

ZIP 9 indexes entries by filename and collapses duplicates. The existing
standard/ZIP64 preflight now returns the original directory entry count;
opening a library package rejects a mismatch with the reader's unique entry
count before manifest validation or storage changes. Filename decoding errors
also fail closed. Fixed ZIP 0.6.6 stored/deflated Unicode-content vectors and
a duplicate-name vector cover compatibility and that rejection. A temporary
two-version harness also checked new DEFLATE output against the old reader.
Existing EPUB, image, audiobook, package/path/checksum and ZIP64 regressions
remain active. Full native tests pass (258 passed, four existing ignored), as
does the native TTS/search-feature suite (316 passed, nine existing ignored).
The same pinned audit CLI reports zero blocking vulnerabilities against RustSec
database `64f92cdc64401fab47baa10e2e11fe382aa57155`, with eight maintenance
and two upstream unsoundness warnings still visible. Full platform CI must run
again on the repaired revision; packaged smoke and the version bump stay deferred.

The subsequent [full CI run 38998060565](https://github.com/muhannadnouri/papercut.io/actions/runs/38998060565)
on `5efed41` passed the checks job (including the Rust audit and both Rust test
configurations), Linux packaging, Windows packaging/discovery tests and Android
APK/JVM tests. iOS simulator and both macOS architectures failed at linking with
`ld: library 'dns_sd' not found`; this was an Apple discovery binding error,
not a ZIP or TTS failure. The binding no longer requests that standalone library:
Apple includes `system_dnssd` in [libSystem's required libraries](https://github.com/apple-oss-distributions/Libsystem/blob/main/requiredlibs)
and [reexports those libraries](https://github.com/apple-oss-distributions/Libsystem/blob/main/xcodescripts/linker_arguments.sh).
Rust already links libSystem on these targets. No SDK search paths, copied
libraries, third-party Bonjour installation or additional build script is needed.

The existing 22 discovery regressions pass after this correction. Isolated
static-library compilation of the actual adapter for Intel macOS and ARM64 iOS
retains all seven Bonjour imports; Rust's native-link requirements retain
`-lSystem` and no longer contain `-ldns_sd`. These checks do not resolve symbols
against an Apple SDK or build a packaged app. That remaining build gate was
subsequently cleared by the full run in [Current Validation and PR Readiness](#current-validation-and-pr-readiness).
The newer compiler's `fetch_update` deprecation is nonblocking; it is separate
from this linker failure and the existing Rust 1.88 minimum stays unchanged.
Smoke and the app version bump remain deferred.

The follow-up ZIP review found a defensive import-boundary bypass: ZIP 9 can
recover an earlier end record when the final one is invalid, then collapse
duplicate names. An attacker-controlled comment could therefore make the old
preflight check two entries while the reader parsed a different, larger
directory. This affects arbitrary files selected as libraries and authenticated
LAN packages; it does not demonstrate filesystem writes or code execution.
The library-package opener now rejects ambiguous end records before constructing
the ZIP reader, uses explicit archive offsets, and checks the physical directory
headers against the advertised count and byte boundary. Fixed ZIP64 end records
must meet their locator exactly, preventing a different ZIP64 candidate or an
unbounded extensible-data allocation. The patched ZIP dependency and audit policy
stay unchanged.

Regression fixtures reproduce the standard-ZIP duplicate-name bypass and a
125,001-entry ZIP64 directory hidden by a forged footer comment; both failed the
new rejection assertions before the fix. Further cases cover the reader's wider
footer-recovery window, inconsistent directory counts/lengths, ZIP64 locator and
record-size manipulation, trailing data and inferred offsets. Ordinary comments,
standard/ZIP64 payload-and-checksum round trips and the fixed ZIP 0.6.6 compatibility
vectors remain accepted. These are synthetic local archive tests, not device smoke.
The ZIP64 bypass regression also gives the opener only a 128 KiB read budget:
rejection must finish in the tail scan without parsing the hidden large directory.
Verification: all 21 package tests pass, the full native suite passes (265 passed,
four existing ignored), and the native TTS/search-feature suite passes (323 passed,
nine existing ignored). Formatting, whitespace and local documentation-link checks
pass. The pinned audit CLI/database above still reports zero blocking findings,
eight maintenance warnings and two upstream unsoundness warnings. The subsequent
full CI success, including the Apple linking jobs, is recorded in
[Current Validation and PR Readiness](#current-validation-and-pr-readiness).
Smoke and the app version bump remain deferred.

Transferred display-title overrides and available original filenames are
restored after parsing so metadata corrections survive a device move.
Document ids come from the manifest rather than a hash of normalized HTML;
otherwise transferred EPUBs would get new URLs because their original archive
bytes are not retained by Papercut.

## Package Versions

Version 1 remains the document-only format:

```text
manifest.json
documents/<document-id>/source.html
```

`manifest.json` identifies `papercut-library`, schema version `1`, creation
time, every document payload and SHA-256 checksum, and folder organization.
Version 2 adds optional completed audiobooks:

```text
audiobooks/<storage-key>/manifest.json
audiobooks/<storage-key>/chunks/<chunk>.wav
audiobooks/<storage-key>/source/source.html     # imported bundles only
audiobooks/<storage-key>/source/metadata.json   # imported bundles only
```

Version 3 adds canonical PDF document payloads while retaining the same
manifest boundary and rebuilding PDF-derived text, thumbnails, and FTS rows on
the receiving device.

Version 4 adds checksummed EPUB reader-image payloads:

```text
documents/<document-id>/assets/<generated-image-name>
```

The receiver validates each generated filename, canonical path, size, and
checksum before restoring it. Versions 1 through 3 remain import-compatible.

All versions are limited to 500 documents. Versions 2 through 4 are also limited to 500
completed audiobooks and 100,000 audiobook files, with an 8 GiB expanded package
limit. EPUB reader images retain the import limits of 5 MB per image and 100 MB
per document. Before the library-package ZIP reader allocates entry metadata,
Papercut rejects more than 125,000 entries or a central directory larger than
64 MiB. A bounded 128 KiB tail scan covers ZIP 9's footer-recovery window and
requires one unambiguous end record whose comment ends at EOF. Preflight checks
fixed standard/ZIP64 end metadata and walks the 46-byte directory headers without
allocating entry names or extra fields, requiring the count and variable-field
lengths to fit the exact advertised directory. The reader uses known offsets
instead of searching for an alternative directory. The ZIP crate still decodes
and validates entries; Papercut validates the manifest, paths and checksums.
This deliberately rejects ambiguous footer-like comment/payload data, trailing
data, archives needing offset recovery and extended ZIP64 end records that
Papercut does not write. Ordinary comments and existing Papercut exports remain
supported. This preflight is specific to library-package imports, not the
separate EPUB document parser. Reading preferences are not represented.

Import rules:

- merge into the target; never replace the target library;
- skip an existing document id and never move that existing document;
- re-sanitize, parse, and re-index every new document;
- merge folders by case-insensitive sibling name and preserve hierarchy;
- place only newly imported documents into the mapped source folders;
- reject unsupported schemas, malformed ids, duplicate archive paths, checksum
  mismatches, unexpected entries, oversized manifests, and oversized payloads;
- reject oversized central-directory metadata before constructing the ZIP entry
  index;
- never extract archive paths directly onto the filesystem.
- stage and checksum every audiobook file, then require the native registry to
  validate its manifest, chunk set, cache identity, WAV headers, and measured
  playback timing/byte totals before installation;
- accept audiobook files only at canonical manifest, source, and single-level
  `chunks/<filename>.wav` paths on every operating system;
- allow restored audiobooks to reference only bundled `/documents/` routes,
  restored `/uploads/` documents, or included `/user-uploads/` source HTML;
- skip an already installed valid audiobook and never overwrite its files;
- re-sanitize imported audiobook source HTML before restoring it.

The importer retains successful documents if a later document fails and reports
partial results. This matches Papercut's existing batch-import behavior and
avoids discarding useful work because one payload is damaged.

Before writing, Papercut checks the target filesystem with a 64 MiB reserve.
Package creation checks the expanded payload estimate, LAN/file staging checks
the archive byte size when the provider reports it, and restoration checks only
missing manifest payloads in app data. Document estimates also reserve space for
normalized HTML, SQLite section rows, and the FTS index. These checks prevent
predictable failures but cannot reserve disk space against another process, so
normal write errors remain authoritative.

## User Experience

### Mobile Layout Repair — 2026-10-17

Android screenshots showed the Send/Receive control disappearing in taller
connection forms. It was still rendered: automatic grid-row sizing inside the
shared scrolling dialog body could collapse its clipped control group to 2 px.
Navigable dialogs now keep rows at their content height and scroll excess content
instead. The header remains outside the body scroll area. Discovery search,
refresh and peer buttons now share the existing transfer button treatment,
including disabled and keyboard-focus styling.

`LibraryTransferDialog.layout.test.tsx` renders the real `AppDialog` and
`NearbySessions` components with representative listener/connector content and
the production CSS in headless Chrome/Chromium. Eight cases check compact,
expanded and collapsed content at phone/desktop widths, RTL, and a shorter
viewport with 150% base text size. They verify full control-group height,
reachable lower actions, header visibility, no horizontal body overflow, and
consistent enabled/disabled discovery-button styling. Existing DOM tests retain
ownership, cancellation, mode-switch and discovery-consent coverage.

The browser cases run through `npm test` without a new npm dependency or custom
build script. They detect Chrome/Chromium on PATH or use `CHROME_BIN`. A local
machine without a browser explicitly skips them; CI fails rather than silently
skipping. The current [Ubuntu CI runner image includes Chrome and Chromium](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md#browsers-and-drivers).
Run `npx vitest run src/library-transfer/LibraryTransferDialog.layout.test.tsx`
for the focused browser check. These local HTML fixtures are not Android WebView,
virtual-keyboard or physical LAN acceptance.

All eight final browser cases fail against the saved pre-fix CSS and pass with
the repair. The focused transfer/dialog suite passes 60 tests; the full frontend
suite passes 352 tests with one existing skip. Lint, typecheck, the production
frontend/Pagefind build, i18n validation and documentation checks pass. This is
local validation after the linked full CI run, not a new platform CI result.

The subsequent task-first flow and stable mobile presentation below complete the
interaction/layout pass. Device smoke and the app version bump remain deferred.

### Task-first Transfer Flow — 2026-10-17

An idle dialog begins with Send/Receive choices and no connection form. Send opens
content selection; Continue opens connection setup only after content has loaded
and at least one item is selected. An empty library explains what is missing while
Receive remains available. Change Selection returns without losing the chosen
items. Receive goes directly to pairing setup without requiring local content.
The header identifies the chosen task; keyboard/screen-reader focus follows the
content/connection step heading. Back still returns to Settings and Close exits
Settings; changing the selected task uses the existing native-dismissal guard.

Nearby sessions are the primary connector path. Explicit peer selection displays
its unverified name as escaped, bidirectionally isolated text; the pairing code
then appears, and starting the transfer remains a separate explicit action.
Enter Address Manually reveals the address fallback. Network Settings contains
network selection/refresh and the listener port override. A unique eligible
automatic interface gets a compact name summary; ambiguous or unavailable
networks keep those settings open and block starting until resolved. Saving a
transfer file remains available on the content step without a network.

Other Connection Method replaces the second segmented control. Its explicit
action describes the next pairing behavior: Show a Pairing Code Instead or Use
the Other Device's Code. Both devices' instructions use these labels. The default
receiver-listens mode, selected-network consent, authenticated v4 role/mode
binding and guarded cancellation/restore behavior are unchanged. A method switch,
network change/refresh or a different peer clears old connection credentials;
there is no automatic direction fallback, TCP probe or firewall change.

Existing native foreground sessions still reattach before new work is enabled;
they bypass the new idle task choice so cancellation and retained results remain
reachable. Terminal dismissal failures keep the old result and its error visible.
Discovery and transfer begin only on explicit actions, not on step changes.
Diagnostics stay available during connection setup, active/recovered sessions
and file export/import activity or results. All eight locales include the new
labels and matching instructions.

Local validation: the full frontend suite passes 366 tests with one existing
skip, including all eight browser layout cases. Regressions cover task/step
navigation, retained content selections, empty libraries, explicit peer choice,
untrusted names, credential clearing, restored native ownership, file-error
diagnostics and matching instructions in every locale. Lint, typecheck, i18n,
the production frontend/Pagefind build and documentation checks pass. This does
not replace fresh platform CI or physical-device acceptance; smoke and the app
version bump remain deferred.

### Stable Mobile Presentation and Combined Review — 2026-10-17

At the app's mobile breakpoint (767 CSS pixels or narrower), the transfer dialog
fills the height available inside the existing dialog gutters and safe-area
insets. It stays in the same place when the task description disappears, content
expands, pairing is shown or results replace progress. The existing header keeps
navigation and the chosen task visible; the body owns scrolling and lower actions
remain reachable. The role choices remain in that body rather than adding another
fixed toolbar that would cover content on short screens. Desktop dialogs keep
their natural bounded height. Other dialogs do not acquire this presentation.

The implementation uses the existing `AppDialog.className` hook and transfer CSS;
there is no new dialog API, router, viewport listener or dependency. The browser
regression now starts with the actual `LibraryTransferDialog` shell, then inserts
representative step content to measure growing/shrinking layouts. Ten cases cover
portrait, narrow RTL, reduced height with 150% base text, landscape and desktop,
with explicit top/bottom inset fixtures. They verify stable mobile dialog and
navigation bounds, ordinary/desktop auto sizing, body scrolling, reachable lower
actions and discovery-button styling. Six of the final cases fail with the
committed pre-fix CSS; all ten pass with this change. The constrained-height and
desktop cases also preserve behavior that already worked.

Local validation: 368 frontend tests pass with one existing skip. Lint, typecheck,
i18n validation, the production frontend/Pagefind build and documentation checks
pass. Ponytail Review traced both connection modes, task/selection transitions,
native ownership restoration, cancellation/dismissal guards, file fallback and
diagnostics, assuming the existing single foreground session. It found no further
actionable code issues in this UI pass. The browser fixtures measure layout, not
hydrated native transfers, Android/iOS WebView keyboard behavior or screen-reader
acceptance. Those smoke checks and the version bump remain deferred. The passing
platform CI is recorded above; the next step is PR readiness review.

Desktop follow-up: at 768 CSS pixels and above, the transfer dialog uses a
viewport-bounded 32rem width instead of the shared 24rem default, giving diagnostic
events more room before wrapping. Mobile sizing and other dialogs retain their
existing widths. Browser coverage includes desktop width, enlarged-text RTL at
the viewport limit, and the unchanged ordinary-dialog width.

Fresh Send dialogs now leave documents and audiobooks unselected. A document-load
retry also leaves documents unselected; choosing an audiobook still includes its
required uploaded source. Explicit Select All remains available, and moving back
from pairing preserves the choices. Three selection regressions fail before the
change and pass afterward. The four desktop width cases likewise fail against the
24rem styling and pass at 32rem. The follow-up frontend suite passes 372 tests
with one existing skip, including all twelve browser cases; lint, typecheck,
i18n and the production frontend/Pagefind build pass. This is local validation,
not a new CI or complete smoke result.

### Current Flow

App Settings owns the entry point because transfer is device-level data
management rather than another document format. A **Data** section opens a
dedicated **Transfer Library** dialog organized by the user's role:

- **Send** presents content selection, then nearby-device selection and pairing,
  and keeps **Save Transfer File** under an expandable fallback on selection;
- **Receive** presents pairing details for the sender to use, with an explicit
  alternate connector method and **Import Transfer File** as a secondary fallback.

The role control remains available in every task step, and the fixed header
identifies the chosen task while the body scrolls. Local-network transfer is the
default path, while the transfer-file disclosures explain that files can
be copied through USB, shared storage, or another user-chosen method. The save
action also states that the ZIP-based package is not encrypted, contains readable
copies of the selection, and should be kept private and deleted when no longer
needed. This notice remains visible beside the action without adding a blocking
confirmation. The dialog consistently describes transfer as copying selected
content; it does not imply that the source device loses its library.

The send action offers separate collapsed checklists for uploaded books and
documents and for completed audiobooks. Both lists start with nothing selected
on a fresh dialog, including after a document-loading retry. Continue and file
export require an explicit selection; moving between selection and pairing
preserves the user's choices. Both lists support select-all and
deselect-all, while document filtering appears only for larger libraries.
When a document filter is active, its bulk actions apply only to matching
results and preserve selections hidden by the filter. Larger libraries also
show the visible and total document counts plus a **Show selected only** control.
That view intersects with the text filter, and **Deselect Shown** changes only
the documents currently visible so users can verify a large selection without
silently changing hidden items. If the sendable document list cannot be loaded,
the error offers **Retry** in place and returns to the same loading status while
Papercut tries again; users do not need to close and reopen the dialog.

Selecting an audiobook automatically includes its uploaded source document.
The UI marks that source as required, and Rust enforces the dependency again
before reading or hashing payloads. Bundled documents need no transferred
source, while standalone imported audiobook bundles retain their existing
embedded source. Subset packages include only selected document placements and
their ancestor folders. The same selection applies to local-network transfer
and the transfer-file fallback.

An omitted document selection retains compatibility with older callers by
including every uploaded document. An explicit empty selection includes no
ordinary documents unless selected audiobooks require one. This changes only
package construction; the version 4 manifest already supports document subsets.
The dialog reports document and audiobook counts plus failures. The transfer-file
summary repeats both selected counts, and an empty selection gets one visible,
shared explanation for the disabled local-network and file actions. The dialog
also exposes explicit source and target roles for same-network transfer. The
listening device displays its local address and pairing code; the connecting
device enters both. By default the receiver listens and the sender connects.
**Show a Pairing Code Instead** on Send pairs with **Use the Other Device's Code**
on Receive when the receiver needs an outgoing connection. Neither choice is
automatic; both are under **Other Connection Method**.

While Papercut checks and packages the selected content, the initiating action
stays disabled and an adjacent live status explains that large items can take
longer. Preparation remains indeterminate because hashing and ZIP construction
do not expose a trustworthy total; pairing details appear only after the package
is ready.

After sending starts, the selection lists and transfer-file fallback are
replaced in place by a compact guided state so current work remains above the
fold. Preparation leads to two numbered pairing instructions, the address and
one-use code, existing byte and receiver-import progress, and the final result.
**Cancel Transfer** stops active network work; after native cleanup,
**Change Selection** restores the retained choices. This uses the existing
dialog and session state rather than introducing routes or a multi-page wizard.

The header has icon-only **Back** and **Close** controls on a separate row above
the title. Back returns to Settings; Close dismisses the entire Settings flow.
During export, import, preparation, receiving or waiting/sending, both controls
are disabled and Escape/backdrop dismissal is blocked. Session cancellation
remains an explicit **Cancel Transfer** action in the body. **Finishing Import**
keeps navigation blocked and cannot be cancelled; network teardown still closes
sockets without aborting admitted local restore.

Completion also replaces the active controls in place and keeps the result open
until the user chooses **Back** or **Close**. The source reports the document and audiobook
counts sent because the protocol returns only a final receiver acknowledgement;
the target shows imported, already-present, and failed counts plus expandable
failure details. Partial completion uses a distinct warning state instead of
presenting every completed operation as an unqualified success.

## Same-Network Transport

The production path now uses **`PCLAN004` only**, through one native session
controller for both modes. There is no v3 fallback, automatic direction switch,
Internet relay or firewall mutation. Optional nearby discovery is implemented
through Linux Avahi, Android 14/API 34+ NSD, Apple Bonjour (macOS and iOS),
and Windows DNS-SD. Manual address/code entry remains available on every
platform; Android API 26–33 reports discovery as unsupported. Both
devices need v4-capable builds: the unchanged **1.9.2** app label alone does not
prove compatibility with an older released 1.9.2 build. Transfer-file formats
and existing libraries/audiobooks are unchanged.

1. The sender prepares an app-owned temporary package, closes its writable
   handle and computes its SHA-256 using cancellable bounded reads before
   connecting or showing its own pairing details.
2. In the normal flow, choose **Receive → Show Pairing Details** on the receiver.
   On the sender choose **Send → select content → Continue**,
   use **Find Nearby** and choose the name shown on the receiver, or enter its
   address manually, then enter the code and choose **Start Sending**. Discovery
   never chooses a peer or begins a transfer automatically. To receive
   through an outgoing socket instead, use **Other Connection Method → Show a
   Pairing Code Instead** on the sender and **Other Connection Method → Use the
   Other Device's Code** on the receiver. The selected listener binds
   TCP **53318**, or an explicit advanced port 1024–65535, on every eligible
   address of the selected interface. It alone displays a 12-symbol, 60-bit code.
   Partial bind failure closes the whole set; ports never change silently.
3. The connector opens selected-interface TCP, then both devices complete fresh
   TLS 1.3 and channel-bound SPAKE2 confirmation before content metadata or bytes.
   The listener checks the connector's proof before releasing its own. Protocol,
   mode, ordered hellos, listener ID and TLS exporter are bound to confirmation.
   TLS tickets, resumption and 0-RTT are disabled. An incompatible marker fails
   closed; incomplete authentication gives connection/update/file guidance
   without pretending that EOF proves a version mismatch.
4. The sender offers the immutable archive size and digest. The receiver checks
   private staging capacity (including existing storage headroom), then announces
   its retained length. Raw bytes follow, then a full staged SHA-256 check, even
   for a complete-length partial. Resume is owned by the same native foreground
   generation, mode, authenticated listener ID/endpoint and Offer. The private
   file is never recovered merely by matching an address/code or a filename.
5. The receiver atomically admits the verified file to the existing importer,
   preserving ZIP/path limits, checksums, sanitization, merge and index rebuild.
   Successful imported items survive cancellation of the network or a lost
   acknowledgement. All admitted import outcomes are terminal for this session,
   including storage failure; there is no automatic re-import.
6. The receiver forwards bounded verification/import status, stopping forwarding
   after its first failure. The sender reports success only after Complete. If
   delivery may be complete but the result is lost, malformed or cancelled, it
   reports **Outcome Unknown** and asks the user to check the receiver before
   sending again. It does not offer Retry for that terminal outcome.

The waiting/pairing eligibility window is ten absolute monotonic minutes and
never renews on noise or Retry. Each TLS-plus-PAKE attempt has one five-second
absolute budget; TCP connect has one cancellable 30-second budget. Successful
streaming/import can outlive the pairing window, but cannot reconnect after it.
The detailed inactivity, frame assembly and receiver-result budgets below apply;
there is no new total archive-transfer deadline.

Incomplete unauthenticated traffic closes only that socket and preserves the
listener's code/deadline. A complete incorrect proof consumes its session. For
eligible I/O interruption or a pre-Ready storage rejection, the listener waits
in the same generation; the connector must explicitly choose **Retry**. There
is no automatic reconnect, alternate endpoint probe or mode change. The native
package/partial guard is retained only while that session remains eligible.
Terminal cleanup closes listeners/sockets and removes private temporary files
before recovery controls become available.

Native interface enumeration replaces the default-route UDP probe and needs no
Internet route. The dialog shows the selected interface and its local addresses,
allows refresh after a network change, and requires an explicit choice when
multiple recognizable Wi-Fi/Ethernet interfaces are available. Tunnel, loopback
and unknown interfaces are never selected automatically. Recognition uses
platform interface names and the point-to-point flag; it is a convenience, not
a guarantee about physical hardware or network trust. Listeners bind selected
addresses, never wildcard addresses. Connectors bind a matching source address.
On adapters with multiple compatible addresses, the most specific matching OS
subnet prefix selects the source. If none matches, the connector reports ambiguity
instead of guessing; use a peer address on that network or choose another
network. A single compatible source still permits routed local transfers.
OS routing and firewall policy remain authoritative. Global-only IPv6 networks
require transfer-file fallback. IPv6 link-local scope comes from the connecting
device's selected interface, never the remote device's numeric zone. Windows
adapter identity groups both address families while retaining their separate
interface indices.

App startup creates or repairs the data and cache roots as owner-only directories
on Unix (`0700`).
Transfer packages use cryptographically random names, atomic exclusive creation,
and owner-only Unix permissions (`0600`); their scope guard removes them after
normal completion or failure. Resumable receive partials use the same file
permissions.

Startup removes transfer packages and partials left by an earlier process. They
cannot be resumed because pairing credentials are intentionally not persisted.
Within one running app session, authenticated retries keep their partial file;
transfer files left untouched for seven days are also removed on the next
transfer operation. Other cache entries are never matched by this cleanup.

Only pre-Ready insufficient-storage rejection can retain the same session for
retry after a storage check. Invalid schemas, checksums, payloads and admitted
restore failures are terminal. Native requests and status messages use structured
error codes and optional storage counts, so the UI never parses human error text
to decide whether retry or import is allowed. Preparation cancellation still
waits for the synchronous package build, then discards it before networking.

Native responsibilities are split by job:

- `network.rs`: one generation, IPC, device-wide network/import reservation,
  foreground ownership, immutable result/status snapshots and terminal dismissal.
- `network/operation.rs`: prepared-file/private-partial guards, listener lifetime
  and expiry, explicit retry, the shared channel-to-importer adapter and safe
  diagnostic mapping.
- `network/v4/`: authentication, bounded codec/streaming, resume pins and atomic
  restore admission. `network/security.rs` owns ephemeral TLS/code primitives.
- `network/endpoints.rs`: interface/address policy and all-or-nothing binding;
  `network/transport.rs`: cancellable source-bound TCP setup; `network/session.rs`:
  cancellation, atomic native admission and interface watching.
- `package.rs`/`mod.rs`: canonical storage/package validation and the same importer
  used by transfer-file import. Library bytes never pass through the WebView.

React uses one start/status/cancel/retry/dismiss IPC contract. It polls by native
generation, displays native byte/import facts, ignores stale replies/events and
never reconstructs eligibility from progress. On view recreation it can reattach
to active work or a retained terminal result, without restarting networking or
import. Successful receiver results update frontend audiobook references and
refresh the library once per displayed generation. Back/Close acknowledges a
terminal result; a stale dismissal cannot clear a newer owner. File-import
progress retains the existing event subscription. Local generation IDs stay out
of authenticated peer progress messages and copied diagnostic reports.

A single native reservation covers network preparation/waiting/retry/import and
file import; cleanup releases it before terminal recovery is observable.
Background/close/network loss cancels pre-import work. After atomic admission it
still tears down the network while preserving local restore and its result.
This does not promise execution after OS suspension/process termination. The
existing identical-address network replacement limitation remains pending native
network identity adapters; interface polling alone cannot identify every switch.

iOS and macOS bundles include `NSLocalNetworkUsageDescription`, and the transfer
starts only from a user action while Papercut is foregrounded. Android currently
targets SDK 36 and uses its existing normal `INTERNET` permission. Before raising
the target to Android 17 / SDK 37, implement the required permission or supported
system-picker path described in the connection design below.

## Transfer Diagnostics

Every transfer dialog exposes **Show Details** and **Copy Diagnostic Report**,
including after failure or completion. Rust owns a process-local history of the
most recent **200 events across attempts**. New attempts do not clear earlier
failures. The UI loads a snapshot on opening and state changes and polls once a
second while work is active; missed progress events cannot erase native history.
It stops polling on unmount. Diagnostic read failures are explicit, retain the
last display as potentially stale, disable copying, and offer Retry. Copying reads
a fresh snapshot so a terminal error between polls is included. Clipboard failure
is also visible. No diagnostic file, background upload or new logging
service is created; quitting Papercut discards this history.

Events contain UTC timestamps, monotonic elapsed time, local attempt number and
role, stage, severity, fixed diagnostic code and optional numeric progress. Local
app version, OS, architecture, protocol capability and a build SHA when available
are included in the report header. The remote app version is not negotiated or
inferred. Network stages distinguish address/code validation, preparation,
interface selection/listener binding, TCP connection/accept, TLS, protocol
compatibility, pairing, package headers/storage checks, transfer/resume,
verification/restore and completion acknowledgement. File transfers retain
preparation, observed import phases and terminal outcome. Progress is sampled at
most once per second within a phase, with first/final counts retained and repeated
identical samples omitted. Phase changes, terminal outcomes and errors are kept;
the omitted-event count discloses bounded-history eviction.

Privacy is enforced when recording, not by attempting to scrub arbitrary text
later. Only fixed labels and an allowlist of numeric facts enter the history.
Socket errors retain their OS error kind/code; storage failures retain required
and available bytes. A fatal listener accept error records both the I/O facts and
a terminal failure, then closes that attempt against late events. Restore summaries
keep aggregate imported/skipped/failed
counts (documents plus audiobooks), including partial outcomes, without item names
or per-item error strings. Endpoint details retain port and IPv4 scope (private,
link-local or loopback), **never the IP address**. Keys, TLS/PAKE material, request
objects, document content/titles, private paths and raw error display strings are
excluded. The copy formatter also projects only these allowed fields, so adding
fields to command results cannot silently expand copied reports. Local technical
stage/error identifiers remain stable English tokens; diagnostic controls and
privacy/recovery text are localized in all eight languages.

Retryable sender interruptions show their existing diagnostic outside Details,
beside Waiting to Resume. Progress-subscription failures no longer disappear as
unhandled rejections; a localized warning points to native details without
stopping the transfer. Pre-authentication failures remain nonfatal and do not
consume a pairing code, but now retain diagnostic evidence; a completed wrong-code
exchange still consumes the session. Failures forwarding import progress or its
acknowledgement are recorded without undoing successfully restored content. The
protocol, key handling, timeouts, listener selection, archive limits and consent
behavior are unchanged.

The `v1.9.0` and `v1.9.2` release tags use the same `PCLAN003` implementation and
package versions. Matching app versions are not a transfer prerequisite. Earlier
incompatible development protocols still fail closed. The original release-pair
failure was not independently reproduced. Subsequent user
evidence identifies inbound firewall policy as the strongest lead; the foundation
follow-up confirmed a live listener for the newer failure below. Diagnostics expose
the failing stage rather than claiming to repair the user's network.

Automated regressions cover bounded history, sampling/final counts, retained
attempt failures, redaction, protocol mismatch, wrong-code consumption, real
localhost TLS resume, refusal with OS facts, visible retryable errors, lost UI
progress subscription, native snapshot recovery, copy errors, polling cleanup and
native diagnostic Retry without starting transfer work. These are automated
Linux/jsdom boundaries, not installed-device acceptance. Further smoke remains
deferred. During the eventual consolidated pass, test Show Details and copying
on both devices through refusal/timeout, wrong key, interrupted resume, partial
import, cancellation and completion; verify redaction and retained errors after
reopening, touch/RTL/keyboard/screen-reader behavior and clipboard permissions.

Validation for this follow-up: **311 frontend passed / 1 optional skipped in 57 files**, **196 default Rust passed / 5 opt-in ignored**, plus lint, locale parity, TypeScript, Vite+Pagefind build, Rust formatting and diff checks. The sender hidden-error regression also rejects removing its production diagnostic. New tests do not claim physical-network, mobile clipboard, screen-reader, provider-inference or current-source CI acceptance.

## Deferred LAN Redesign — 2026-10-15

**History:** bounded diagnostics from `bfc7245` were retained in the merged
search-v2 work. On 2026-10-16 the follow-up moved to `refactor/library-transfer`.
The selected connection design below supersedes the earlier candidate list;
this commit documents decisions only. The current transport remains manual,
and the reported connection failure is not claimed repaired. Existing release
acceptance requirements still apply; device smoke remains deferred.

### Reported Failure and Remaining Uncertainty

The user tested Linux 1.9.2 on CachyOS sending to Android 1.9.0 on the same Wi-Fi,
using the displayed address and pairing key, with no Linux VPN. Android reported
`Could not connect to the source device: connection timed out`. The source report
reached listener binding on private IPv4/TCP port `41803`, certificate setup and
`waiting`, with no `tcp-accepted` event in the captured history. This places the
observed failure before TLS, protocol negotiation or pairing; changing the key or
requiring identical app versions does not address this evidence.

UFW was active with default incoming deny/outgoing allow and only KDE Connect
ports allowed. That policy does not permit the advertised Papercut port. However,
the supplied `ss -ltn` check showed **no listener** on that port at the time of
inspection. A firewall does not hide a listener from `ss`: an expired/cancelled
session, app closure or stale endpoint must also be ruled out. The earlier bind
event is historical evidence, not a current liveness check. No packet capture or
successful restricted-rule retry establishes a single root cause yet.

CachyOS documents UFW as enabled by default with incoming traffic blocked. A
different firewall configuration plausibly explains the earlier Mint success;
there is no demonstrated distro-specific socket defect. Same Wi-Fi alone does
not guarantee peer reachability. See [CachyOS firewall documentation](https://wiki.cachyos.org/configuration/post_install_setup/#configuring-firewall-ufw).

The foundation follow-up reproduced the failure with the current Android build
`7525feb`: Receive timed out in `tcp-connect` at approximately 30 seconds and
Send recorded no accepted connection. A read-only host check now confirmed a
live Papercut listener on the selected Wi-Fi IPv4 address, TCP `53318`. A fresh
UFW screenshot still showed incoming deny and only KDE Connect allowances.
This resolves the earlier missing-listener uncertainty for that observation and
supports host filtering as the leading cause; no packet capture or successful
controlled allowance proves the sole cause. Do not infer a TLS/pairing defect.

A narrow per-peer rule was suggested only as a diagnostic experiment. The owner
declined that setup requirement, and no firewall changes or successful retry
were performed. Terminal commands/per-device rules are not the shipping UX.
Any later developer investigation must separately record current listener,
firewall and routing evidence; do not expand access just to classify a timeout.
[UFW supports interface, source, destination and port restrictions and removal of the original rule](https://manpages.ubuntu.com/manpages/noble/man8/ufw.8.html).

### LocalSend Comparison and Recommended Direction

LocalSend's documented v2 protocol uses configurable defaults of TCP/UDP
`53317`, multicast discovery at `224.0.0.167`, and a normal upload flow in which
the receiver listens and accepts or rejects a transfer request. It also documents
HTTP discovery fallback. Discovery removes routine address entry; a stable port
makes firewall configuration reusable. Neither bypasses packet filtering.
[LocalSend protocol](https://github.com/localsend/protocol)

LocalSend documents HTTPS for normal app transfers, but still requires firewall
access and warns about router client isolation. Its existence is not evidence
that an application can safely avoid all network permission prompts.
[LocalSend setup](https://github.com/localsend/localsend/blob/main/README.md#setup)

The familiar **Allow through firewall** prompt is an OS facility on Windows,
not a universal LocalSend authorization protocol. Windows can retain an
application allow rule after consent; denial, existing rules and administrator
policy affect whether a prompt appears and whether access works. A native grant
is a viable Papercut path on supported systems, with the network scope and
revocation requirements below. Do not repeatedly trigger prompts after denial.
[Windows firewall application rules](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/rules#applications-rules)

LocalSend's Linux setup still documents UFW/firewalld commands; its evaluated
Windows installer at `d01fdb1` has no explicit firewall-rule installation step.
Neither establishes an equivalent one-time UFW/AppImage approval API. Showing our
own confirmation would express intent but would not grant OS privileges. This
comparison is source/documentation review, not a test of installed prompts.
[LocalSend installer](https://github.com/localsend/localsend/blob/d01fdb14c978fa8b2b13e29a50cc0feaefe5c1b9/support/scripts/compile_windows_exe-inno.iss)

In contrast, [croc's transport selection](https://github.com/schollz/croc#data-transport-selection)
uses traversal and relay infrastructure when a direct path is unavailable.
That improves reachability but does not establish a strictly offline LAN path.
Papercut will not silently add a public relay or adopt that infrastructure to
hide a blocked local connection.

### Selected Connection Design — 2026-10-16

**Revised after the foundation device test and no-terminal requirement.** This
supersedes the earlier recommendation to retain sender-listens as the permanent
design. The endpoint foundation is retained under the production v4 cutover.
Manual modes are implemented; discovery, OS authorization and packaged acceptance
remain gates rather than claims of universal reachability.

Reuse the package/import boundary, maintained TLS/SPAKE2 primitives, native
streaming, one-use codes, authenticated resume and bounded diagnostics. Choose
**receiver-listens / sender-connects by default**, with an explicit
**receiver-connects / sender-listens** option for incoming-blocked receivers.
Use one new protocol and shared transfer engine for both, not two legacy stacks.
Content roles and connection roles must be explicit rather than inferred from
generic names such as client/server throughout storage and import code.

| Option | Evaluation and decision |
| --- | --- |
| Keep sender-listens | Preserves the current wire path but unnecessarily requires inbound Linux access for the reported Linux-to-Android send. Keep it as the current implementation, not the future UX target. |
| Receiver-listens | Standard upload pattern used by LocalSend. Linux sending to Android uses Linux's permitted outgoing path; Linux receiving still needs incoming access. Selected default for the next protocol. |
| Explicit receiver-connects | Select before pairing when the receiver cannot accept incoming connections. Android hosts the selected package and Linux connects out to receive it. Recommended alternative before adding a privileged UFW helper; the other device still needs a reachable listener. |
| Automatically negotiate either connection direction | Could avoid an inbound restriction when the other endpoint is reachable, but needs coordinated discovery, role binding, simultaneous-attempt cleanup and fallback rules. Defer until the simpler flow and native authorization are tested; do not add speculative dual listeners or a custom rendezvous protocol. |
| External relay or traversal service | Helps when direct traffic is blocked, but changes the offline, privacy and operating-cost contract. Excluded from this LAN-only design. |

The new normal flow is: receiver opens Receive, grants required OS access and
displays an ephemeral name/code; sender selects content and the receiver, enters
that receiver's code, then uploads the existing package. Starting foreground
Receive is explicit consent to accept a correctly authenticated transfer during
the short session; no background inbox or persistent device trust is introduced.
Only after authentication may package metadata/data be accepted. Discovery
replaces routine address entry, not authentication. Manual endpoints remain an
advanced fallback; file export/import remains available without network access.

#### Explicit Outbound Receiving — Implemented, Device Acceptance Pending

**Recommend an explicit connection choice, not automatic direction negotiation.**
TCP is bidirectional: opening a connection does not determine which device sends
the library. The earlier v3 transport listened only on the sender. The v4
controller retains that useful direction as the explicit alternative and adds
receiver-listens as the default. This is code and automated-test evidence, not
packaged Android/Linux acceptance.

| Content direction | Listening device / code owner | Connecting device | Package flow |
| --- | --- | --- | --- |
| Linux sends to Android (default) | Android receiver | Linux sender | Linux uploads over its outgoing connection. |
| Android sends to incoming-blocked Linux (explicit alternative) | Android sender | Linux receiver | Linux downloads over its outgoing connection. |

This avoids a new **incoming transfer-TCP** allowance on Linux in those cases
when outgoing traffic and replies are permitted. It neither changes UFW nor
guarantees discovery, Android listener permission, routing or router peer access.
Two incoming-blocked devices still need authorized access on one side or file
transfer; no LAN-only architecture can promise reachability under every policy.

Use these bounded UI/session rules:

1. Keep ordinary **Send** / **Receive** controls. Default Receive waits for the
   sender. Offer **Use the Other Device's Code** under **Other Connection Method**
   before an attempt or as explicit recovery; the other device selects content
   and chooses **Show a Pairing Code Instead**. Explain
   the two actions in place without requiring firewall diagnosis or port entry.
   This is a session choice, not a persistent setting or an OS-name heuristic.
2. The chosen listener alone displays the ephemeral name and one-use code.
   The connecting device selects that nearby session and enters its code. In
   either mode, the content sender explicitly selects and prepares the package;
   the peer cannot browse its library or ask it to export different content.
3. A mode switch cancels/closes the old generation before starting another,
   withdraws its advertisement and discards its pairing credentials. Keep the
   content selection, but start a fresh session. No timeout, discovery hint or
   authentication failure silently changes direction or retries another mode.
   Once restore starts, disable mode switching with cancellation until its
   terminal result; do not imply rollback of imported data.
4. Publish a sending listener only after packaging is ready. A connecting sender
   also prepares before connecting; human code entry completes before TLS/PAKE
   I/O begins. If a waiting receiver expires during preparation, require a fresh
   session rather than extending its deadline or holding a half-paired socket.

A single authenticated connection-establishment path should handle listener and
connector roles, then hand its stream to the existing content send/receive
framing. In current code, `send_package` embeds `ServerConnection` and
`receive_package` embeds TCP connect plus `ClientConnection`; those are the seams
to separate, not a reason to duplicate streaming/import code. Native ownership
must distinguish content roles from connection roles: the listener owns socket
readiness/code/expiry, the sender owns its immutable prepared package, and the
receiver owns private staging and restore admission. Preserve one active session,
bounded cancellation and stale-generation protection in both modes.

This is preferable to a custom UFW helper for the reported device pair because
it uses unprivileged sockets and already-maintained TLS/SPAKE2 primitives. It
still requires protocol/UI work and secure role binding; it is not a one-line
swap of existing calls. Do not add a new HTTP/browser API. LocalSend's separate
browser download mode documents unencrypted HTTP; that is not an acceptable
model for Papercut's encrypted native transfer.
[LocalSend reverse transfer](https://github.com/localsend/protocol#5-reverse-file-transfer-http-aka-download-api)

#### Connection Roles and Protocol Cutover

**v4 production integration implemented/tested 2026-10-16; device acceptance pending.**
`network/v4/` and the shared controller/dialog implement the contract below.
The v3 runtime and temporary dead-code allowance have been removed. The wire change
is deliberate: new peers use **only `PCLAN004`**, with no version negotiation or
fallback to v3 after failure. Read the eight-byte marker before any remaining
pairing bytes; a different marker closes the connection and gives a local
update/file-transfer explanation. EOF before a marker is only a connection
failure with compatibility guidance, not evidence of the peer's version. Old
v3 peers remain compatible with each other; package versions 1–4, saved libraries
and audiobook formats are unchanged.

##### Roles and TLS

`C` is the TCP connector/TLS client; `L` is the listener/TLS server. The mode is
selected locally before connecting, never negotiated from discovery or a socket:

| Mode byte | Discovery hint | C's content role | L's content role |
| --- | --- | --- | --- |
| `01` | `m=push` | Sender | Receiver |
| `02` | `m=pull` | Receiver | Sender |

These are the only valid values. Content roles follow the mode, so no redundant
role byte or peer-supplied role assignment is needed. Use TLS 1.3 through the
existing rustls/ring stack, with a fresh self-signed certificate per listener
session. Keep handshake-signature verification; the following PAKE authenticates
the otherwise untrusted certificate/channel. Do not consult public certificate
services, persist trust, enable TLS tickets/resumption or accept 0-RTT. Application
resume always starts a fresh TLS handshake and fresh PAKE instances. This narrows
the new transfer configuration, not TLS settings used by other app features.

Use the existing 12-symbol, 60-bit CSPRNG code and normalization/alphabet. The
listener displays it; it is neither a user-chosen password nor a persistent key.
Also generate a native-owned, random **16-byte session ID** at listener readiness;
retain it only for that foreground session and its eligible resumes. Neither
code nor ID goes into DNS-SD, logs, copied diagnostics or persistent preferences.
Possession of the code authorizes the selected session, not a permanent physical
device identity; anyone given the same code has that authority.

##### Fixed Pairing Frames and Transcript

All bytes below travel **inside completed TLS**. `||` means concatenation; quoted
labels are exact ASCII bytes, without a terminator unless `\0` is shown. SPAKE2
messages are the opaque 33-byte messages from the currently locked
`spake2` 0.4.0 `Ed25519Group`, including its side byte; do not reimplement group
parsing or claim RFC 9382 wire interoperability.

| Message | Direction | Exact encoding | Bytes |
| --- | --- | --- | --- |
| `HelloC` | C → L | `"PCLAN004" \|\| mode:u8 \|\| pake_C:[u8;33]` | 42 |
| `HelloL` | L → C | `"PCLAN004" \|\| mode:u8 \|\| session_id:[u8;16] \|\| pake_L:[u8;33]` | 58 |
| `ConfirmL` | L → C | HMAC-SHA256 tag defined below | 32 |
| `ConfirmC` | C → L | HMAC-SHA256 tag defined below | 32 |

C calls `start_a`, L calls `start_b`; both pass these identities in the same order:
`id_A="papercut-library-transfer-v4-connector"`,
`id_B="papercut-library-transfer-v4-listener"`. Both use the normalized code as
`Password`. Let `K` be the shared bytes returned by `finish`, not a raw group
element. Each attempt uses new PAKE randomness. The exact confirmation inputs are:

```text
E = completed_tls.export_keying_material([0u8; 32],
      b"EXPORTER-papercut-library-transfer-v4", None)?
T = "papercut-library-transfer-v4-confirm\0" || HelloC || HelloL || E
ConfirmL = HMAC-SHA256(K, T || 02)
ConfirmC = HMAC-SHA256(K, T || 01)
```

Exporter failure closes the attempt; never substitute a zero output. The final
`01`/`02` are single hexadecimal connection-role bytes, not ASCII digits or
content-role bytes. Both hello mode bytes must equal the local choice. The fixed layout makes the
transcript unambiguous; its mode determines the ordered content-role mapping.
The session ID and both PAKE messages are included verbatim. No JSON, discovery
name, IP string, locale-dependent text or code is concatenated into `T`.

The only exchange order is `C: HelloC` → `L: HelloL` → `C: ConfirmC` →
`L: ConfirmL`. Flush each flight. L verifies ConfirmC in constant time **before
releasing its own confirmation**: a guessing connector cannot inspect L's tag
and disconnect before presenting the proof that consumes an incorrect attempt.
C verifies ConfirmL in constant time. A side handles content only after
verifying the peer and sending its own confirmation. L may queue its first
content frame after flushing ConfirmL; C checks that tag first. No library
metadata, archive bytes or filesystem effects based on a peer occur before this
gate. Do not add another ready acknowledgement or custom encryption layer.
Drop per-connection PAKE/key material promptly after confirmation; retain only
the native session credentials needed for explicitly permitted resume.

Reject wrong magic/mode/side, malformed group elements and incomplete flights as
unauthenticated traffic. They close that socket without consuming the code or
changing the waiting deadline. At L, a complete invalid ConfirmC is a completed
online guess: close its listener session and discard its code. At C, an invalid
ConfirmL ends the local generation; it cannot command a remote reset or trigger
another connection. A valid ConfirmL
pins the listener/session at C; later reconnects require the same authenticated
session ID. A mismatching untrusted hello must not delete or repurpose a partial.
An attacker can still occupy or terminate a foreground session; code consumption
limits guessing, not denial of service. Accept one handshake at a time, with no
application queue of pending handshakes and no per-peer ban database.

The existing [SPAKE2 API](https://docs.rs/spake2/0.4.0/spake2/)
supports fixed identities and using `finish` output as an HMAC key. It explicitly
reports no independent security audit; reusing it is not an audit claim. The
exporter is the post-handshake exporter, never the early-data exporter.
[TLS exporter contract](https://www.rfc-editor.org/rfc/rfc8446.html#section-7.5)

##### Authenticated Package and Status Framing

After pairing, use `type:u8 || length:u32be || body[length]` for control frames.
All integers are unsigned and big-endian; lengths count bytes, not characters.
Read/validate type, state and length **before allocation**. Unknown types,
out-of-order frames, zero/oversized JSON, malformed UTF-8/JSON, truncated fixed
fields and arithmetic overflow fail closed; do not skip extensions or retry a
different protocol/mode. Only these types exist:

| Type | Direction / allowed state | Body |
| --- | --- | --- |
| `10` (hex), Offer | Content S → R, immediately after pairing | Exactly 40 bytes: archive size `N:u64be` then `SHA256(archive):[u8;32]`. Require `0 < N <= 8 GiB`, the existing `MAX_PACKAGE_BYTES`. |
| `11` (hex), Ready | R → S, after validating Offer and local staging capacity | Exactly 8 bytes: resume offset `O:u64be`, with `0 <= O <= N`. |
| `12` (hex), Status | R → S, before Ready for a rejection, or after all archive bytes | 1–65,536 bytes of UTF-8 JSON using the existing `ReceiverMessage` representation below. Before Ready only `failed` is legal. |

Following Ready, S writes **exactly `N-O` raw bytes**, with no chunk headers;
only then does S read Status frames. R emits no wire progress during byte copy;
both UIs use native local byte progress. An error during raw streaming closes
the socket; do not block trying to exchange duplex control messages. A sender
must not append extra content; only the announced archive is staged/imported.
Keep the existing 64 KiB copy buffer and checked arithmetic. Do not allocate
from `N`, map the whole archive or trust a remote offset as a local file path.

Reuse these JSON shapes, not a second result/error model:
`{"kind":"progress","value":<LibraryTransferProgress>}`,
`{"kind":"complete"}`, and
`{"kind":"failed","value":<LibraryTransferError>}`. Existing camelCase fields
and unsigned numeric bounds apply. Reject unknown variants; progress is display
data, never authority to import, complete, extend pairing expiry or change roles.
Coalesce its UI updates within the existing bounds; render strings as text and
keep raw peer strings/item names out of copied diagnostic reports. `complete`
means the importer returned its result; retain the current receiver-owned
partial/skipped/failed-item details instead of claiming every item was imported.

Compute the archive digest once from the closed, immutable prepared file before
connecting/advertising, using bounded reads with cancellation checks. Offer is
on the authenticated TLS stream. Pin its size
and digest on the first accepted Offer; reject a changed Offer in the same
session rather than silently overwriting a partial. R's native-only resume key
is the tuple `(local generation, mode, authenticated listener session ID,
listener endpoint, N, digest)`. Code/PAKE proof is still required on every
connection; that tuple or a digest alone grants no authority. Do not persist
session credentials to allow process-restart resume or accept a peer filename.

For a retained private partial, validate its local length against N, then request
that length as O. Rehash the prefix with bounded reads if maintaining an
incremental full-file hash; length or filename alone is not a proof of correct
prefix contents. There is no new prefix-hash protocol. Check remaining staging
space before Ready. After receiving the remainder, verify the full staged
archive digest before atomically admitting restore, including when `O=N`.
Every pre-admission hash pass uses bounded reads and checks cancellation.
A digest failure is terminal: remove the untrusted partial, import nothing and
end the session. Existing package checksums, ZIP/path limits, sanitization,
merge rules and restore-space checks remain mandatory afterward.

Before Ready, an explicit `failed` with `insufficient-space` permits user Retry
in the same unexpired session; retain the pinned Offer/package. Other rejections
are terminal. Once restore is admitted, **all** import outcomes are terminal for
this network session, including an insufficient-space error; do not infer safe
network replay from its error code. A later user action may recover through the
existing importer/merge rules, but the wire protocol promises no rollback.

Once bytes are staged, R sends bounded verification/import progress and one
terminal Status. Forwarding is best effort with bounded writes; after a write
failure, disable further forwarding rather than blocking each import callback
again. A lost final message never undoes receiver data. After S has written all
bytes, EOF, timeout or cancellation before terminal Status means **Outcome
Unknown — Check Receiving Device**, not confirmed failure or successful import.
Treat an ambiguous final-write/flush error the same way if all bytes may have
been delivered. Invalid post-copy status also closes the channel with an unknown
import outcome; a parser error is not proof of rollback. An authenticated
`failed` reports the receiver's failure, not that zero items were restored.
Close that session without automatic resend/re-import. No distributed commit,
exactly-once guarantee or result-recovery service is introduced.

##### Session State and Deadlines

Rust owns one foreground generation per device, with independent content and
connection roles. Reserve receiving/import work before exposing readiness or
connecting; retain the sender's package guard until terminal cleanup. A listener
generation owns the code/session ID/certificate/listener set and its deadline.
The connector retains its selected mode/endpoint and authenticated session ID
only within that generation. The UI never reconstructs these from progress events.

| State | Entry / permitted next states |
| --- | --- |
| Preparing / Selecting / Permission | Local selection and sender packaging; then listener Binding or connector Connecting. Cancel/failure is terminal. No socket waits for human code entry. |
| Binding → Waiting | Bind all eligible addresses, create credentials/TLS, then mark Ready and advertise. Any required bind failure closes the entire set before failure. |
| Connecting → Authenticating | Explicit peer/code selection; TCP, TLS and the fixed exchange. A listener also enters Authenticating after accept. One socket attempt owns this state. |
| Waiting after unauthenticated noise | Close only the rejected socket; listener returns to its existing waiting generation/deadline. A connector may offer explicit Retry without a mode switch. |
| Authenticated → Offer / Ready → Streaming | Withdraw advertising and allow only the grammar above. No second peer or simultaneous attempt. Pin package identity before staging. |
| Waiting to Resume | Only recoverable I/O before local raw-copy completion, or pre-Ready storage rejection, while foreground and unexpired. L may re-advertise; C explicitly reconnects to the same endpoint/session using fresh TLS/PAKE. Both sessions must still be eligible; a terminal peer cannot be revived. Preserve the partial/package, not old TLS/PAKE state. |
| Verifying → Finishing Import | R validates the staged digest, then wins atomic restore admission against cancellation. After admission no cancel, mode switch, new connection or network replay can start another restore in this generation. |
| Awaiting Result | S has written all raw bytes; only Status frames are accepted. Confirmed terminal Status ends the session; connection loss becomes Outcome Unknown, not automatic Waiting to Resume. |
| Complete / Failed / Cancelled / Expired / Outcome Unknown | Terminal snapshots close listeners, sockets and registrations before reporting recovery controls; clear code/session ID/deadlines and release reservations/guards. Retain bounded safe diagnostics and immutable result facts. |

Pairing/resume eligibility is a **600-second absolute monotonic deadline** from
listener readiness. Discovery, noise and retries never renew it; authentication
must finish before it. Successful streaming/import can outlive that window, but
no later reconnect is admitted. C has one cancellable **30-second total TCP
connect budget**, and also enforces its own unextended 600-second foreground
attempt window from its first TCP connect, after preparation/code entry. A retry
does not reset it. Do not transmit wall-clock deadlines or assume clock agreement;
only L can display its authoritative code expiry. C's local budget is not a claim
about L's remaining time.
Each TLS-plus-PAKE attempt has **one five-second absolute budget**, capped by
remaining eligibility, not a timeout reset for each byte. Use shutdown-based
cancellation to interrupt blocked I/O in both roles.

Authenticated header/body writes/reads and raw streaming use **30-second I/O
inactivity limits**, except that S allows one **600-second absolute wait for
Ready/rejection** after Offer, accommodating local partial-file reads and space
checks. S uses **600 seconds of inactivity** while awaiting post-copy receiver
status, matching the existing import-wait allowance. Parsing each control frame
also has a 30-second absolute assembly budget after its first header byte;
trickling bytes cannot renew it. A Ready wait never exceeds its 600-second budget.
There is no new total archive-transfer deadline: bounded size, inactivity limits,
foreground ownership and explicit cancellation accommodate large audiobooks.
An observing sender timeout cannot cancel or roll back admitted local import.

Before restore admission, cancel, actual mobile background, close or selected
network loss wins over I/O and closes that generation; changing mode creates a
new generation/code and cannot reuse a partial. During admitted restore, finish
safely even if transport/UI visibility is lost, suppress further network starts
and report the local result. Network teardown must still close sockets/listeners
and disable forwarding without turning that into importer cancellation. This
does not promise continued execution after the OS kills/suspends the process.
Never wait for a sender acknowledgement to commit or keep a listener open just
to recover a lost result. Exact resume requires both
native sessions to remain eligible; no promise survives backgrounding, app exit,
network replacement, a new peer/code/mode or a terminal sender outcome.

##### Review Record and Required Implementation Checks

The review assumes one user-selected foreground transfer, up to the existing
8 GiB archive limit, with an untrusted LAN and arbitrary peers before pairing.
It traces `security.rs`, `transport.rs`, `network.rs`, restore admission and the
existing package/status types. These design ambiguities were resolved before
implementation; none is a claim of a new runtime fix:

| Reviewed failure case | Final design rule |
| --- | --- |
| A guessing connector reads L's confirmation and abandons its own proof. | C confirms first; L verifies before releasing its tag. Completed incorrect proofs consume the listener session. |
| Mode, peer hello, connection role or TLS channel is substituted/reflected. | Fixed PAKE identities, exact ordered transcript, role-separated tags and expected mode checks before content. |
| A different package or corrupted old prefix is resumed by matching only address/code/length. | Session/mode/endpoint/package pinning, bounded private partial handling and full archive SHA-256 before restore, even at offset N. |
| A slow pre-authentication header renews timeouts indefinitely. | Fixed-size parsing, one absolute authentication budget, immutable pairing deadline and one active handshake. |
| Final result is lost after import starts, or a storage error triggers a second restore. | Phase-gated resume; admitted restore is terminal for replay. Sender reports unknown outcome after ambiguous delivery; receiver retains data. |
| A digest pass over a large partial or stalled progress writer blocks the workflow. | Bounded reads and Ready wait, bounded status assembly/writes, and forwarding disabled after its first failure without cancelling import. |

Mechanical fixtures below fix the serialization/confirmation contract. They use
**synthetic** message/key bytes, not valid SPAKE2 group encodings or a real TLS
session: `K=00..1f`, `E=20..3f`, `session_id=00..0f`,
`pake_C=41 || 00..1f`, `pake_L=42 || 20..3f` (hex inclusive byte ranges).
`HelloC=42`, `HelloL=58`, `T=169` bytes in both modes; the confirmation domain
is 37 bytes including its final NUL. Expected tags are:

| Mode | ConfirmC (hex) | ConfirmL (hex) |
| --- | --- | --- |
| `01` | `72d53e91684808e350f4b0482158d9abe7cc4449b55580af7ebc964445814013` | `defef37eecaf9d9d5fb7b61a1cd227433cf1941073499979abd1fb2fe36b632a` |
| `02` | `409b69ad2a0ecb8995db9ae0506c93d5ab1e890100f01dcafe0ac4ce8016bfe0` | `8267d4fb112bf47405cc095dc773fd1e904e36256860856467882880039dc836` |

Control-header fixtures are `1000000028` (Offer/40), `1100000008` (Ready/8),
and `1200010000` (maximum Status/65,536). These checks establish lengths,
byte order and transcript sensitivity, not PAKE/TLS security or interoperability.

Before release, implementation tests must use **real rustls plus the locked PAKE
library** in both role mappings, not only these synthetic fixtures. Require
wrong-code, wrong-side/role/mode, changed session ID/exporter, replay/reflection,
truncation at every framing boundary, oversized/out-of-state frames, invalid
UTF-8/JSON and offsets `0`, `N`, `N+1`/overflow. Exercise changed package/prefix,
storage rejection before Ready versus after admission, final-write/flush/result
loss, deadline trickling, both-mode cancellation/expiry/network changes, stale
callbacks and the cancellation/restore race. Preserve existing sanitizer,
path/ZIP bounds, partial import, private staging and diagnostic-redaction tests.

**Review verdict:** the native core now has executable codec/state and real
TLS/PAKE regressions; this is not a release-ready transport or an independent
cryptographic audit. Packaged platform builds and LAN acceptance remain gates. Smoke and app version bump stay deferred.

##### Production Integration Record — 2026-10-16

The core introduced by `7a9a503` is now used by production commands and both UI
modes. Old v3 pairing/framing and split sender/receiver orchestration were
removed. The implementation reuses the existing TLS verifier, code generation,
locked crypto dependencies, error/status models and importer; no dependency,
app-version, machine-policy or firewall changes accompany this cutover.

The controller publishes a generation immediately so preparation can be
cancelled once its ID is known. Its worker owns all bound listeners, the closed
prepared archive, its digest, the unique private staging file and the global
reservation. A channel must release ownership before another attempt can start.
Explicit connector Retry keeps these objects and the authenticated pins without
renewing eligibility; terminal cleanup drops them before publishing recovery.
A lost or late result never turns a cancelled generation into success.

The UI has localized, explicit action labels in all eight locales. Listener-only
port controls and code/address displays follow connection role rather than Send
or Receive. Active work blocks navigation and mode changes; cancellation waits
for native cleanup. Import admission rejects cancellation while lifecycle
teardown can close the socket without aborting restore. Partial receiver results
remain distinct from sender package counts. There is no automatic direction
fallback, new route/wizard, probe or repeated result replay.

Diagnostics remain bounded to 200 safe events and exclude pairing codes, native
session IDs, addresses, paths, document contents and peer error/item strings.
They now identify connector/listener plus Push/Pull, report `PCLAN004`, retain
TCP OS errors and byte facts, and distinguish wrong proof, protocol mismatch,
expiry, digest/identity failure and unknown outcomes. UI status/cancel/retry/
dismissal errors remain visible; stale generations cannot replace current work.

Validation on Linux: full Rust suite **234 passed, 4 existing opt-in tests ignored**;
full frontend suite **328 passed, 1 existing skip**. Existing core coverage is
retained and extended with a final-delivery disconnect case; obsolete v3-only
regressions were replaced rather than preserving a second protocol. Production
adapter tests use real loopback TLS/PAKE and private files in both modes, verify
exactly one importer call, retain partial results after acknowledgement loss,
reject corrupt full-length staging before import and make admitted storage
errors terminal. Additional regressions cover listener closure/rebinding before
terminal publication, reservation release, stale dismissal/commands, explicit
UI mode requests, cancellation refusal, generation-scoped retry, status failures,
late replies, reopened results and navigation. TCP cancellation/refusal and code
normalization regressions remain in place. `npm run build` passes i18n, TypeScript,
Vite and Pagefind; lint and Rust formatting pass. Vite retains its existing
module/chunk-size warnings. Clippy completes with unrelated existing repository
warnings; new transfer warnings were corrected.

This was automated testing, not device smoke. CI/native platform packaging and
physical Linux ↔ Android acceptance in both modes are still required. On Linux
with incoming-deny UFW, keep Linux on the connecting side: default Send for
Linux → Android, explicit receiver-connects mode for Android → Linux. No UFW rule
or terminal step was added. If neither device can accept inbound connections,
use transfer-file recovery rather than promising LAN success.

##### Production Integration Review — 2026-10-16

Ponytail Review covered the uncommitted production cutover against `7a9a503`,
including the IPC callers, dialog, native lifecycle hooks, endpoint policy,
TLS/PAKE core, importer boundary, diagnostics and regression tests. Expected
load is one foreground transfer on a personal device, including large libraries
and interrupted connections. The saved MemSearch journal confirmed the explicit
direction, no-firewall-helper decisions and the existing synchronous-preparation
and same-IP network-change limitations; these remain documented limits rather
than newly introduced findings. This review is not an independent crypto audit.

Two P2 recovery defects were reproduced during review and are now fixed:

1. **Resolve native ownership before enabling new work.** Previously, changing
   roles while the initial status request was pending invalidated its reply,
   leaving an active native transfer without a status poll or Cancel control.
   Role/mode changes, file import/export and network starts now remain disabled
   until the lookup resolves. A failed lookup shows a localized error and Retry;
   Close remains available. A successful lookup reattaches the native generation
   and restores polling/cancellation. Replies from disposed effects are ignored,
   including the repeated setup/cleanup used by React Strict Mode.
2. **Acknowledge terminal native state before returning to setup.** Change
   Selection previously cleared only React state, so closing and reopening could
   restore the old failure, expiry or unknown-outcome panel. Change Selection and
   role/mode changes now await dismissal of the matching generation before
   clearing their UI. Controls and navigation stay locked while dismissal is
   pending. A dismissal failure preserves the result and exposes the error so
   the user can retry; stale dismissal still cannot remove a newer native owner.
   Active-transfer cancellation and admitted-import protection are unchanged.

Permanent jsdom regressions use the real dialog and existing IPC harness to
cover delayed active ownership, blocked actions/navigation, cancellation after
reattachment, lookup failure/retry, stale Strict Mode replies, Change Selection
followed by Close/reopen for all three warning/error states, failed dismissal,
role-change acknowledgement and a newer generation surviving a delayed stale
dismissal. The earlier temporary reproductions remain superseded by these tests.

Validation after the fixes: **335 frontend tests passed, 1 existing skip** across
57 files; the focused transfer suite passes **38 tests**. Frontend build (locale
parity, TypeScript, Vite and Pagefind), lint and documentation checks pass. No
native code, protocol, dependency, app version or firewall policy changed in
this follow-up; the preceding full native review run remains **234 passed,
4 existing opt-in tests ignored**. Physical transfers were not exercised.

**Follow-up verdict: both review code blockers are resolved.** CI/native
platform packaging and physical Linux ↔ Android acceptance remain outstanding;
no claim is made that the original UFW timeout is fixed on actual devices.
Device smoke and the app version bump remain deferred. Bounded system DNS-SD
adapters and nearby-session selection followed this review; see the system
discovery implementation record below. All four backends are now implemented;
platform packaging and acceptance remain separate gates.

#### DNS-SD Feasibility and Platform Choice

Use standard DNS-SD over local mDNS, with **system discovery adapters** rather
than an app-owned multicast protocol. Source/API inspection establishes a viable
integration path. All four backends are now implemented as described below;
physical acceptance and full platform packaging remain outstanding:

| Platform | Selected adapter | Packaging and permission implications |
| --- | --- | --- |
| iOS / macOS | System `DNSServiceRegister`, `DNSServiceBrowse`, resolve/address callbacks and explicit reference disposal. Register the chosen session listener's Rust port on the selected interface. | Apple supplies Bonjour. Declare `_papercut-xfer._tcp` in `NSBonjourServices`, retain the local-network usage description, and handle permission failure. Use the C API for compatibility with the current macOS 10.13 / iOS 15.0 floors; this does not require a custom multicast entitlement or an OS-version bump. |
| Android | Kotlin `NsdManager` through a narrow Tauri native plugin, with a selected `Network`. | Network-scoped browse and `NsdServiceInfo.setNetwork` are available from API 33. The implementation uses API 34 cancellable service-info callbacks. API 26–33 retains manual/file transfer and reports discovery as unsupported; API 33 extension-specific support is deferred. Keep the current minimum SDK. |
| Windows | Native `DnsServiceBrowse`, `DnsServiceResolve`, `DnsServiceRegister` and cancellation/deregistration APIs, through Rust bindings. | Windows 10 desktop APIs avoid requiring a separate Bonjour installation. Availability and callback cleanup must be checked in the existing Windows build; missing APIs disable discovery, not manual transfer. |
| Linux | Avahi's system D-Bus API: service browser/resolver and entry groups scoped to the selected interface. | Discovery requires an accessible system bus and running Avahi service. Use a maintained Rust D-Bus binding; do not bundle, install or start a daemon. AppImage hosts without Avahi retain manual/file transfer with an explicit unavailable state. |

Apple documents interface-scoped discovery and service-reference cleanup in its
[DNS-SD browsing](https://developer.apple.com/library/archive/documentation/Networking/Conceptual/dns_discovery_api/Articles/browse.html)
and [registration](https://developer.apple.com/library/archive/documentation/Networking/Conceptual/dns_discovery_api/Articles/registering.html)
guides. Declared Bonjour services avoid the restricted entitlement required for
custom multicast/broadcast traffic; local-network consent still applies.
[Apple privacy guidance](https://developer.apple.com/news/?id=0oi77447)

Android's [NSD APIs](https://developer.android.com/reference/android/net/nsd/NsdManager)
and [network-scoped service information](https://developer.android.com/reference/android/net/nsd/NsdServiceInfo#setNetwork(android.net.Network))
support the selected-network policy. Target SDK 36 currently uses `INTERNET`;
do not add or request `ACCESS_LOCAL_NETWORK` at this target. Before targeting
SDK 37, implement its runtime permission for manual/general LAN access; the
supported system NSD picker can grant access to a selected discovered endpoint
without broad permission, but does not authorize either mode's advertising/listening or
arbitrary manual addresses. [Android permission guidance](https://developer.android.com/privacy-and-security/local-network-permission)

Microsoft documents [Windows DNS-SD browsing](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsservicebrowse)
and [interface-scoped registration](https://learn.microsoft.com/en-us/windows/win32/api/windns/ns-windns-dns_service_register_request).
Resolve optional discovery entry points without making app startup depend on
their presence. On Linux, check for an existing Avahi bus owner and disable
[D-Bus service activation](https://dbus.freedesktop.org/doc/dbus-specification.html);
discovery must not implicitly start the daemon.
Avahi's [server](https://github.com/avahi/avahi/blob/master/avahi-daemon/org.freedesktop.Avahi.Server.xml)
and [entry-group](https://github.com/avahi/avahi/blob/master/avahi-daemon/org.freedesktop.Avahi.EntryGroup.xml)
interfaces provide the Linux browser, resolver and publisher boundary.

The pure-Rust [mdns-sd alternative](https://docs.rs/mdns-sd/0.21.5/mdns_sd/)
supports desktop discovery without Avahi, but adds an in-process multicast
parser, worker and cache to bound and maintain. Its desktop support does not
establish mobile permission compliance. Do not adopt it or add a second
discovery backend in the initial redesign; accepting an optional Avahi dependency
and graceful fallback is the smaller system-service design. The implementation uses the already-locked `dbus` 0.9.12 Rust binding (MIT/Apache-2.0)
as a direct Linux dependency and already-locked `windows-sys` 0.61.2 (MIT/Apache-2.0)
bindings as a direct Windows dependency. The bindings also compile in host tests
against fake APIs. No package version or transitive package was added/updated
in the lockfile; these add direct dependency edges only. Its existing
libdbus build/runtime dependency remains part of Linux packaging validation.

Reuse the existing [native-plugin pattern](../src-tauri/plugins/document-scanner/src/mobile.rs),
not the scanner plugin itself. Tauri supports Rust-to-Kotlin calls and native
callbacks; discovery adapters return bounded metadata to the Rust session owner.
The WebView neither opens sockets nor parses DNS packets. Apple/Windows system
API calls and Linux D-Bus stay in platform-specific Rust modules; Android alone
needs the Kotlin bridge. [Tauri mobile plugin guide](https://v2.tauri.app/develop/plugins/develop-mobile/)

#### Endpoints, Port and Compatibility

- Use **TCP 53318** as Papercut's default, distinct from LocalSend's 53317. It is
  a private-use default, not an IANA assignment or a promise that the port is
  free. Bind failure must offer Retry or an explicit advanced port override;
  never silently pick another random port. Do not use UDP 53318: discovery uses
  the platform mDNS service on UDP 5353. [IANA port ranges](https://www.iana.org/assignments/service-names-port-numbers/)
- Enumerate active interfaces and their addresses through native APIs. Prefer
  an eligible Wi-Fi/Ethernet interface, ask when ambiguous, and make the selected
  network visible. Do not restore the former default-route UDP probe or require
  Internet access. Never automatically select a VPN/tunnel, cellular or loopback
  interface; an unknown or advanced interface requires explicit selection.
- Bind the eligible addresses of the selected interface, never wildcard
  `0.0.0.0` / `::`. If one required bind fails, close the others and report the
  failure before advertising readiness. Permit
  private/link-local IPv4 and ULA/link-local IPv6; keep loopback for explicit
  same-device testing. Reject public, multicast, unspecified and broadcast peer
  destinations. For IPv6 link-local addresses, derive the scope from the local
  selected interface, not a peer-supplied interface number. Global-only IPv6
  networks remain outside this policy and need file fallback.
- Use a typed endpoint containing address family, port and local interface
  scope. Attempt only eligible addresses for the user-selected session, within
  one cancellable 30-second total connect budget. Discovery must not trigger
  TCP probes. Stop on authentication/protocol failure rather than trying more
  endpoints with the same one-use code.
- Revalidate addresses on selection and immediately before connecting. If the
  selected address/interface disappears, stop the session and withdraw its
  advertisement; do not migrate a code or partial transfer silently to another
  network. A replacement session gets a new code and public discovery name.
- The endpoint policy retains explicit `IPv4:port` input, including
  random ports, bare-IP defaults and IPv6 literals. The new receiver-listens
  manual fallback enters the receiving endpoint on the sender; receiver-connects mode
  instead enters the sending listener's endpoint on the receiver. Do not imply that
  an address makes incompatible wire versions interoperate. App versions need
  not match exactly; authenticated wire and package compatibility are decisive.
- Advertise only an implemented wire capability. Discovery for the v4 flow must not
  announce or impersonate v3 to preserve old discovery behavior. Incompatible
  peers receive a clear update/file-transfer option, not automatic downgrade.

#### Discovery Contract and Bounds

Publish `_papercut-xfer._tcp.local.` only after the chosen TCP listeners are
ready. Browse only on the connecting device while its transfer dialog is
foregrounded. Use an ephemeral random instance name shown on the listening
device; TXT contains only `v=1` (the discovery schema), the implemented
`p` wire capability and `m=push` (receiver listens) or `m=pull` (sender listens).
Reject missing, malformed or unknown mode values in the proposed schema.
The mode hint can filter the nearby list, but must match the explicitly selected
mode and subsequent authenticated exchange. It cannot trigger a mode switch,
callback connection or firewall action. The production discovery path uses `p=PCLAN004`; it never advertises v3.
SRV supplies the port.
Do not add pairing codes, titles, counts, paths or persistent Papercut device IDs.
Normal DNS-SD/IP metadata and OS-managed hostnames may still be visible on the
LAN; this is not anonymous discovery. Names and compatibility hints are untrusted
display data, not proof of identity or authority to import.

Keep at most 64 application peer entries, 8 addresses per peer, 512 bytes of TXT
data per peer, and 128 queued adapter events. Validate DNS name limits and treat
labels as plain text. Coalesce UI updates to at most four per second. Browse in
30-second windows with an explicit Refresh; remove lost records immediately and
discard retained entries after at most 60 seconds or when the dialog closes.
Bound outstanding resolve handles as well as the displayed list, and stop
discovery with a recoverable error if an adapter cannot enforce its queue bounds.
These are Papercut-owned bounds, not claims about system-daemon internal caches.
Let system mDNS perform protocol retransmission; add no subnet scans or custom
announcement loops. A result only becomes a connection attempt after explicit
selection and code entry. Native validation rejects out-of-policy addresses even
when a discovered name resolves to them.

#### System Discovery Implementation — 2026-10-16

Scope includes all four backends together: Linux, Android, Apple (macOS/iOS)
and Windows, as explicitly requested. Every platform retains manual/file recovery. This
stage changes no app version, wire/package format, pairing proof or firewall
policy. It adds no relay, multicast parser, background service or daemon startup.
The review below found four Android defects, now fixed with native Android JVM
regressions. The real Tauri plugin and Android framework compile/test together;
packaged-device acceptance remains a separate gate.

- **Linux:** a private system-bus connection looks up Avahi's existing unique
  owner. Every Avahi method disables D-Bus auto-start. The browser, resolver
  and entry-group objects use the selected interface index and `.local` service
  type. At most 64 resolver handles exist per worker; IPv4 and IPv6 sources merge into a
  peer without losing the other address family when one source disappears.
  All client objects are disposed with the private connection; publication is
  explicitly freed as well. Missing bus/owner and policy denial become discovery
  errors while manual transfer remains available. No Avahi installation or
  firewall instruction is added to the user flow.
- **Android:** the existing library-transfer plugin calls `NsdManager` on the
  activity thread. It maps the explicitly selected interface to exactly one
  `Network`; it never substitutes the default route, binds the whole process or
  browses all networks. API 34+ uses cancellable service-info callbacks, eight
  addresses per peer, a 128-event queue and bounded callback handles. API 26–33
  returns unsupported. Although network-scoped NSD exists at API 33, its older
  one-shot resolver is not reliably cancellable without extension-specific
  support. Keeping that optional support deferred is smaller and safer than
  leaving resolutions alive after teardown. The normal `ACCESS_NETWORK_STATE`
  manifest permission enables mapping; target SDK 36 and minimum SDK 26 stay
  unchanged. No broad local-network runtime permission is requested.
- **Apple:** the system Bonjour C API runs on the existing discovery worker.
  Browse, resolve, address-query and registration references use the selected
  interface index and `.local.` domain. Hostnames outside `.local.` are rejected
  before address lookup. File descriptors are polled without blocking before
  processing results; references and their callback contexts are disposed on
  that same worker. Resolver/address/publication tokens reject stale replies,
  and address deletion or TTL zero removes prior source data. Native errors
  distinguish local-network policy denial. `NSBonjourServices` declares
  `_papercut-xfer._tcp` in the macOS/iOS plist sources and generated iOS plist;
  the usage descriptions and macOS 10.13 / iOS 15.0 floors remain unchanged.
- **Windows:** the system DNS-SD browse/resolve/register APIs are loaded once
  from `dnsapi.dll` in System32. Missing entry points report unsupported rather
  than preventing app startup; no Bonjour installation is required. Requests
  specify the selected nonzero interface index, publication is mDNS-only, and
  registration includes only selected local addresses and the three nonsecret
  TXT fields. Resolver results are checked for matching interface, local service/
  host names and bounded metadata before entering the shared endpoint filter.
  Callback contexts are opaque IDs in a bounded registry, not pointers into a
  dropped worker. Request/query/cancel buffers and input instances stay alive
  until completion, including late callbacks after cancellation. Output records
  and instances are freed through their matching system APIs. Registration that
  succeeds during cancellation is explicitly deregistered. Re-publication waits
  for the old completion, with a five-second limit, rather than overlapping
  registrations or renewing the pairing window.
- **Lifecycle:** only ready listener generations advertise. A random
  `Papercut-…` name is shown after the system confirms registration; Android's
  collision-renamed name is reflected in the UI. Successful v4 pairing withdraws
  publication; an eligible interruption can publish the same session name again
  without renewing the pairing window. Cancellation, expiry, network loss and
  terminal cleanup dispose the advertisement. Android also detects loss of its
  captured `Network` handle. Linux/Apple/Windows retain the documented
  identical-interface/address replacement limitation of the shared address watch;
  these adapters do not prove Wi-Fi access-point identity. Android backgrounding immediately stops native
  handles. Browser teardown signals cancellation without joining a system-API
  worker from a native UI callback. At most two workers, including cancellation
  tails waiting on system APIs, can exist; further starts fail visibly until a
  slot is released. Cancelled caches clear immediately. Linux/Apple/Windows
  use 64-event queues per worker, preserving the 128-event overlap budget. A new listener can therefore overlap one closing browser
  without blocking the activity thread or accumulating blocked workers. Listener
  cleanup closes the discovery owner before publishing terminal transfer state.
  Apple reference disposal is synchronous; Android/Windows OS acknowledgement
  can follow the cancellation request. Old callbacks cannot republish results,
  and a replacement waits for confirmed cleanup. OS failure to acknowledge can
  keep a bounded pending registration until process exit; the native transfer
  generation and its listener are still closed, and no pairing key is advertised.
  Windows caps all outstanding requests, including cancelled tails after worker
  exit, at 130 (two sets of 64 resolvers plus a browser/publication each). Further
  requests fail closed while manual/file recovery remains available.
- **Bounds and selection:** native code rejects malformed TXT, mismatched mode/
  protocol and out-of-policy endpoints; invalid updates remove old source data.
  It caps peers/addresses/TXT/events, expires retained rows at 60 seconds and
  stops the browser at 30 seconds. Refresh starts a fresh native generation.
  The dialog polls at most four times per second and drops stale replies. It
  selects nothing automatically. Choosing a row asks Rust to validate the live
  row, retention deadline, selected interface and endpoint, then fills the manual
  address field. Starting still requires code entry and the normal endpoint
  validation/PAKE boundary; a name is not an authenticated identity. A lost row
  cannot be selected from cache after the OS reports its loss. An already-filled
  address remains manual input,
  rather than a claim that the DNS-SD record remains live. In particular, Windows
  native browse callbacks do not promise immediate removal notifications; TTL-zero
  records are removed when delivered and the shared 60-second retention limit
  bounds unsignalled stale hints. There are no automatic TCP reachability probes.
- **Failure handling:** denied, unavailable, unsupported and resource-limit
  states are separate from TCP readiness. Manual/file recovery remains visible.
  On Android, replacement discovery waits for all previous handles to confirm
  teardown. Only the latest pending request is kept; no extra registration starts
  during cleanup. The existing native poll loop advances it without blocking the
  activity thread, with a five-second cleanup deadline. Cancel, backgrounding or
  timeout discards the pending request; late callbacks cannot restart it.
  Safe fixed discovery codes enter the existing 200-event diagnostic history;
  peer labels, interface names, IPs, TXT and pairing codes do not.

Regression coverage includes an isolated private D-Bus/fake Avahi service
(interface/type/port/TXT signatures, disabled activation, resolve/loss and
publication cleanup), metadata and endpoint rejection, IPv6 local scope, source
loss, peer/address/event limits, retention, browser deadlines/cancellation,
background teardown, optional permission failure preserving manual pairing and
captured-network loss cancelling pairing. A real loopback v4 handshake verifies
withdrawal after authentication and re-publication after an eligible disconnect.
UI tests cover explicit selection, escaped labels, stale/lost rows, late Refresh
responses, close during startup and unsupported-discovery manual sending.
The Linux CI check explicitly installs `dbus-daemon` for the isolated test bus;
this is a test dependency, not an app-started production daemon.

Initial discovery-stage validation: **244 native tests passed / 4 existing opt-in tests ignored**, and
**344 frontend tests passed / 1 existing skip** in 58 files; formatting, lint,
locale parity and the TypeScript/Vite/Pagefind build pass. The Android Kotlin
sources compile with Kotlin 2.1 and Google's API 34 reference jar, Tauri's actual
argument annotations/JSON type and temporary bridge stubs. This is an API check,
not a full Android plugin/Rust/Gradle build or device acceptance. Android's
one-time `getAllNetworks` snapshot is deprecated but is used only to map an
explicit interface rather than silently using the default network. CI must
compile the real plugin and manifests. No smoke was run on real Avahi, mDNS or
Android devices; those checks remain deferred.

All-platform follow-up validation: **256 native tests passed / 4 existing ignored**
and **344 frontend tests passed / 1 existing skip**. The nine Android JVM tests
remain green from the preceding fix. Twelve Apple/Windows fake-API regressions were added. They cover actual API
arguments, both modes' TXT, byte order, IPv6 and
TTL loss, malformed/null metadata, selected-interface filtering, stale callbacks,
reference disposal, cancellation/completion races, compensation deregistration,
cleanup timeout, queue caps and pending-request bounds. These tests call no real
Bonjour or Windows discovery service. CI runs the native discovery subset on
macOS/Windows in addition to the full Linux native suite, and the Android JVM
subset after APK build.

The actual adapter source files also pass isolated `cargo check` for
`x86_64-pc-windows-gnu`, `x86_64-apple-darwin` and `aarch64-apple-ios`, using the
same locked libc/windows-sys versions in a temporary host. This checks platform
FFI types/configuration, not full Tauri builds, native linking or runtime consent.
Frontend build/i18n/typecheck, lint, Rust formatting, Clippy, plist declarations,
CI YAML and docs checks pass. Clippy completes with existing unrelated warnings
and none in discovery. No physical smoke or platform acceptance is claimed.

The subsequent joint review/fixes and full CI are complete; see
[Current Validation and PR Readiness](#current-validation-and-pr-readiness).
Validate packaged cross-platform discovery, permission denial, address updates
and teardown when smoke resumes. Version bump and physical smoke remain deferred.

#### Discovery Review and Fixes — 2026-10-16

Ponytail Review traced the common discovery owner, Avahi adapter, Android plugin,
UI selection/polling and connected transfer lifecycle. Expected load is one
foreground transfer per app, up to 64 nearby records, plus bounded cleanup
overlap. Manual/file fallback and the documented identical-address network
replacement limit remain explicit decisions. Apple/Windows adapters were added
after this Android-focused review when the all-platform scope was confirmed.

The four reproduced Android findings are now addressed:

1. **P1 — Null TXT values could crash discovery.** Android permits null attribute
   values, but the callback read their length before Rust metadata validation.
   `TransferDiscovery` now rejects null values before sizing/encoding and emits
   loss for that source, removing any previously selectable row. Android JVM
   tests cover null values in each of the three expected keys, followed by a
   valid update. See the platform's
   [attribute contract](https://developer.android.com/reference/android/net/nsd/NsdServiceInfo).
2. **P1 — Publication escaped the selected network.** Kotlin's `apply` receiver
   made `setNetwork(network)` read the new service object's null field. Android
   interprets null as all connected networks. The call now explicitly references
   `this@TransferDiscovery.network`. A test with two available networks inspects
   the actual registration and browse arguments. Pairing authentication and TCP
   listener binding are unchanged. See Android's
   [network scoping contract](https://developer.android.com/reference/android/net/nsd/NsdServiceInfo#setNetwork(android.net.Network)).
3. **P2 — Old resolver callbacks could remove their replacements.** A service
   could disappear and return with callback B before callback A finished.
   Update/loss/failure now require ownership of the current callback, while old
   unregistration/failure acknowledgements still release their handles. A
   regression drives loss → rediscovery → late old loss/update/failure, verifies
   B remains usable, and checks callbacks after close cannot publish data.
4. **P2 — Refresh failed during normal asynchronous cleanup.** The plugin
   previously requested stop, immediately required `released`, and returned
   unavailable while acknowledgements were still pending. It now retains one
   latest pending request, returns starting, and begins it through the existing
   poll command once every old handle is released. Cleanup has a monotonic
   five-second deadline; expiry returns one unavailable event and requires an
   explicit new request. Stale polling/Stop cannot advance or cancel a newer
   request. Tests cover delayed browser/resolver cleanup, repeated Refresh,
   cancellation, background/resume, exact timeout, network revalidation, and
   withdrawal/re-publication of the same listener generation. No activity-thread
   sleep, extra timer, unbounded request queue or simultaneous registration is
   introduced.

Android's
[asynchronous unregistration contract](https://developer.android.com/reference/android/net/nsd/NsdManager#unregisterServiceInfoCallback(android.net.nsd.NsdManager.ServiceInfoCallback))
remains the cleanup boundary. No new registration is started while an old handle
is unconfirmed. An OS cleanup failure may therefore leave discovery unavailable;
manual/file transfer remains available. This keeps the resource bound rather
than accumulating abandoned registrations.

**Follow-up validation:** all **9 Android JVM tests passed** through
`:tauri-plugin-library-transfer:testDebugUnitTest`, using JUnit 4.13.2 and
Robolectric 4.17 with API 34 framework classes and deterministic system-service
shadows. Gradle 8.14.3/AGP 8.11.0/Kotlin 1.9.25 compiled the production plugin,
manifest and the actual Tauri 2.11.5 Android module against compile SDK 36. The
local test-only Gradle host and downloaded SDK/build outputs stayed in `/tmp`;
no custom build script or platform stubs were added to production. The permanent
tests are in the plugin's normal `src/test` source set, and Android CI now runs
that same task after APK build. These are test dependencies only; the app
version, minimum/target SDK, wire protocol and production dependencies did not
change in this follow-up.

The **10 shared/Linux discovery native regressions** and **47 transfer UI tests**
also passed again. CI YAML parsing, local doc links/fences and whitespace checks
passed. The earlier full-suite results remain recorded above; they were not
rerun for this Android-only fix. Existing deprecation warnings remain, including
the selected-interface `getAllNetworks` snapshot.

**The four review code blockers are resolved.** Re-review the fixes before CI
packaging. No full Rust/Gradle APK was built locally, and no real Avahi/mDNS or
physical-device acceptance was run. Packaged LAN behavior, permissions and
background cleanup remain acceptance gates when smoke resumes. Smoke, the app
version bump stay deferred. Apple/Windows adapters are now implemented above
and need the same review and platform acceptance. This is not a cryptographic
certification.

#### Session Lifecycle and Cancellation

Rust owns one foreground session generation, its package guard, listener set,
active socket, cancellation state, discovery registrations and cleanup. React
reads a snapshot and receives state updates; late callbacks from a replaced
generation cannot revive old entries or alter the current session. Separate
listener readiness from discovery availability so an unavailable browser does
not disable a working manual path. A historical bind event is not a liveness
check, and a local Ready state is not proof of remote reachability.

In either selected mode, the listener owns binding, readiness and the waiting/code
window; the connector owns the outgoing connection. The content sender owns
package preparation and the content receiver owns staging/import. Both expose
authentication, streaming, receiver import and a terminal result. Expose the
native waiting/pairing deadline and expiry reason. The exact v4 deadlines and
phase-gated resume rules above apply to both modes;
the ten-minute pairing window remains unextended by discovery or retries.
Withdraw advertising after successful pairing, re-advertise only
after an eligible authenticated interruption, and preserve the same code/package
only within its unexpired session. Incomplete unauthenticated traffic must not
consume the code; a completed incorrect proof still does.

Retain cancellation for connect, TLS/pairing and staging in the new roles. It
must interrupt native I/O and release the restore reservation, not merely hide
the dialog or await the full connect timeout. Once package restore starts, show **Finishing
Import** and disable cancellation until the existing importer reaches its result;
do not claim rollback of already imported items. Preserve retryable partials
under the existing authenticated-resume policy and retain successful imports
even if the sender disconnects before acknowledgement.

On mobile app backgrounding, cancel the foreground network session and stop
browsing and advertising; finish an already-started import safely. Permission or
system-picker presentation is not user backgrounding, and desktop focus loss or
minimization does not cancel an explicitly started transfer. On close, cancel, expiry,
network loss or terminal failure, unregister/dispose all discovery handles and
close sockets idempotently. Resume is promised only for an authenticated
interruption within the same unexpired foreground session and endpoint, not
after app termination, background cancellation or network replacement. Do not
start a background service or automatically restart sharing on foreground entry.

#### Firewall, Errors and Diagnostics

Expose separate states for unavailable/denied discovery, no results, local bind
failure, expiry, connection refusal/timeout, protocol incompatibility and pairing
failure. Preserve safe OS error facts in the existing 200-event diagnostic
history. Do not label every timeout as a firewall failure or claim discovery
proves the TCP port is reachable. Keep IPs, interface names, device labels and
pairing secrets out of copied reports; show the chosen endpoint only in the
interactive transfer UI.

**Use native consent, not terminal instructions or per-device exceptions.** On
supported hosts, offer an **Allow Local Transfers** action that invokes the OS
permission/authorization flow, then retries only with consent. An **Open Network
Settings** action is useful where supported, but is not by itself proof that
setup is usable. A timeout cannot establish that a firewall change is needed;
show safe diagnostics and offer Retry or file transfer without opening access
automatically. Explain port details in Show Details, not as routine setup work.

| Platform | Authorization path and remaining constraint |
| --- | --- |
| Windows | Use the existing application-firewall consent/settings and supported firewall APIs, respecting active profiles and administrator policy. Check whether local policy modification is allowed; do not claim a rule overrides managed policy. |
| macOS | Use system local-network consent and application-firewall settings for the signed application. Respect denial; do not run Papercut as administrator or replace system prompts. |
| Android / iOS | Use system local-network/discovery permissions appropriate to the target OS and operation. These are not permission to modify a desktop firewall, nor proof that the router permits peers. Recheck the Android permission scope when switching to receiver-listens. |
| Linux with running firewalld | Evaluate its supported system D-Bus API and polkit authorization for a narrowly scoped runtime rule with native expiry. Resolve the selected interface's actual zone; do not silently choose a default/public zone, reassign the interface, or request broad `authorizeAll` access. If the zone or backend cannot enforce the intended scope, fail explicitly. |
| Linux with UFW, especially portable AppImage | Source review below found no supported, narrow Papercut authorization flow in the evaluated GUIs. Profiles plus an existing GUI are optional administrator recovery, not the fully guided transfer UX. Keep AppImage unprivileged; do not adopt a custom helper just to fill this gap. |

Primary platform references: [Windows firewall policy API](https://learn.microsoft.com/en-us/windows/win32/api/netfw/nn-netfw-inetfwpolicy2),
[Apple application firewall](https://support.apple.com/en-au/guide/mac-help/mh34041/mac),
[firewalld D-Bus API](https://firewalld.org/documentation/man-pages/firewalld.dbus.html),
[firewalld authorization policies](https://firewalld.org/documentation/man-pages/firewalld.policies.html),
[UFW application integration](https://manpages.ubuntu.com/manpages/noble/man8/ufw.8.html)
and [polkit's mechanism/authority boundary](https://polkit.pages.freedesktop.org/polkit/polkit.8.html).

Any authorization implementation must meet these requirements before adoption:

- **Consent and scope:** show the application, selected network, receiving
  purpose and duration. Prefer one application/network grant over rules for each
  phone. Default guided authorization is for TCP 53318 only; advanced port
  overrides need a separate reviewed policy. System mDNS on UDP 5353 is a
  different path and may already be permitted; do not open UDP 53318 or a range.
- **Network trust:** an interface name, private address or SSID is not evidence
  of trust. Use verified OS network/profile information and explicit user
  consent. Enforce the selected interface/local subnet or an equivalent narrow
  native scope; do not equate every private subnet or every interface in a shared
  zone with the selected network. Public/unknown networks require an explicit
  policy decision, not automatic broad permission.
- **Ownership:** distinguish existing administrator rules from Papercut-owned
  grants. Revoke only the latter; never remove or rewrite unrelated policy.
  Report effective access honestly when another policy still blocks traffic.
  A Linux port/service rule is not executable identity enforcement: it may also
  permit another process using that port. TLS-bound pairing remains mandatory;
  firewall permission is never authority to import content.
- **Lifetime:** prefer native expiring grants renewed only while Receive is
  active. Revoke on cancel, expiry and network change; account for crashes,
  service reload and uninstall. UFW profiles do not provide this expiry. A
  persistent application grant needs separate explicit consent, a visible
  revocation path and enforceable network scope; do not simulate safety with a
  detached shell timer that may never run.
- **Privilege boundary:** prefer an existing supported OS service/GUI. If those
  cannot satisfy the UFW/AppImage case, stop at a separate security and packaging
  review before proposing a helper. Any justified privileged mechanism must be
  system-installed/root-owned, accept only narrow typed allow/revoke operations,
  derive caller identity from the OS, validate scope independently and have a
  maintained update/uninstall path. No generic command runner, arbitrary rule
  text, executable paths or ports supplied by the WebView or discovered peer.
- **Portable builds:** a writable AppImage or its temporary mount is not a
  trusted privileged-helper location. Installing system policy requires explicit
  OS authorization and a supported packaging story; portable launch alone must
  not install a root service or persistent privilege mechanism.

Never collect an administrator password inside Papercut, elevate the whole app,
or add a setuid application. If a later reviewed mechanism uses `pkexec`, it must
invoke a fixed trusted system path through the desktop authentication agent and
disable the internal text-agent fallback. `pkexec` does not validate arguments;
the privileged mechanism must do so itself. Denied authorization, absent agents
and managed policy restrictions are explicit recoverable states, not reasons to
fall back to a terminal password prompt.
[pkexec security model](https://polkit.pages.freedesktop.org/polkit/pkexec.1.html)

Do not start Avahi, disable a firewall, change its default policy, configure
UPnP/router forwarding, add Internet traversal/relays or silently expand network
scope. Native authorization cannot repair router client isolation or an
administrator's prohibition. File transfer remains available, but does not count
as passing the no-terminal LAN acceptance gate. If supported UFW/AppImage setup
cannot meet that gate, report the limitation before implementation rather than
claiming universal zero-setup transfer.

#### UFW/AppImage Feasibility Outcome — 2026-10-16

**Recommendation: keep the portable app unprivileged and do not implement a
Papercut UFW helper now.** Package-managed profiles plus an existing firewall GUI
are the smallest maintainable administrator fallback, but do **not** satisfy the
full no-address/no-port-knowledge, selected-network consent flow. That acceptance
gate remains unmet for an incoming listener under UFW. The subsequent outbound
evaluation above selects an explicit receiver-connects alternative before
privileged firewall software. That avoids the Linux listener in the target
device pair; it does not grant UFW access or establish device acceptance.

The source review found these concrete limits:

- UFW's documented application integration names ports in
  `/etc/ufw/applications.d`; it neither asks for consent nor provides rule expiry.
  Installing a profile is not permission to add a rule. Do not change the global
  application default or run an install-time allow operation.
  [UFW manual](https://manpages.ubuntu.com/manpages/noble/man8/ufw.8.html)
- Gufw snapshot `be5dc3f` loads its preset catalog from
  `/etc/gufw/app_profiles`, a different directory and schema from UFW profiles.
  Its preset-add path supplies ports but no interface/source restriction; those
  require advanced rule fields. Its launcher authorizes the firewall GUI, not
  a bounded Papercut request. A UFW profile alone therefore does not establish
  a scoped Gufw workflow, and its named Home profile is not proof of OS network
  trust. No supported request interface to prefill and approve our scoped rule
  was found in this inspected source.
  [Gufw rule/catalog source](https://github.com/costales/gufw/blob/be5dc3f8615cece46857171000e163d917f87e92/gufw/gufw/view/add.py),
  [launcher policy](https://github.com/costales/gufw/blob/be5dc3f8615cece46857171000e163d917f87e92/policykit/actions/com.ubuntu.pkexec.gufw.policy)
- KDE Plasma Firewall snapshot `405cec9` reads UFW application profiles and
  supports scoped rule data, but uses its own KAuth helper with a general
  `org.kde.ufw.modify` action. Calling that private backend is not a portable,
  least-privilege Papercut API. Launching System Settings does not preconfigure
  or verify a rule. Do not automate either GUI's widgets or import its backend.
  [KDE UFW client](https://github.com/KDE/plasma-firewall/blob/405cec9620178f72a42706e938b38f1beba914e4/kcm/backends/ufw/ufwclient.cpp),
  [helper dispatch](https://github.com/KDE/plasma-firewall/blob/405cec9620178f72a42706e938b38f1beba914e4/kcm/backends/ufw/helper/helper.cpp)

These are source/API findings, not acceptance of every distribution's packaged
GUI version. Neither inspected path supplies Papercut-owned, session-expiring
UFW grants. Rules deliberately created through an administrator's GUI remain
administrator-owned; Papercut must not delete them on session exit.

Packaging options and their consequences:

| Option | Decision |
| --- | --- |
| Portable AppImage alone | Retain as an unprivileged artifact. Files bundled inside its AppDir are not installed into the host's UFW/polkit directories. Desktop integration does not grant firewall permission; moving or updating the image cannot be a trusted privileged installation path. |
| Existing `.deb` / `.rpm` packages with static profiles | Preferred packaging for any later system integration on matching distributions. Use Tauri's declarative package-file mappings, not a new setup script. Install descriptors only; no automatic rule, firewall enable/reload or manager replacement. A separate Gufw catalog entry would need its own compatibility check or upstream contribution. |
| Data-only companion package for AppImage | Could install descriptors through an existing package-manager GUI without elevating Papercut, but still cannot authorize a scoped session. Adds release/update/removal work and a separate package per supported family; defer rather than claim it solves the UX. |
| Papercut-owned root helper with polkit | A legitimate privilege-separation pattern if justified, but not the smallest solution here. Requires package-managed installation, narrow input/caller validation, serialized rule ownership, crash-safe lifetime, upgrades, revocation and uninstall. No implementation selected; do not elevate a bundled helper or call unrestricted `pkexec ufw`. |
| Existing firewalld service | Retain its supported D-Bus/polkit runtime-rule candidate where already present, subject to the scope/expiry gates above. Do not install it over UFW, run both managers to solve one transfer, or require users to switch firewall software. |

Papercut already builds `.deb`, `.rpm` and AppImage artifacts through
`src-tauri/tauri.linux.conf.json` and `.github/workflows/release.yml`; neither
config installs firewall descriptors or a helper. Tauri supports package-owned
files for [Debian](https://v2.tauri.app/distribute/debian/#custom-files) and
[RPM](https://v2.tauri.app/distribute/rpm/), while AppImage's
[file mapping](https://v2.tauri.app/distribute/appimage/) packages files inside
the image. The existing Debian/RPM artifacts do not supply an Arch/CachyOS
integration package. Do not suggest installing a foreign package as a fix or
quietly add an Arch/companion-package maintenance commitment.

**Connection direction takes precedence over privileged UFW integration.** Use
the selected default/explicit alternative above with one chosen listener and
authenticated content roles. Keep native permission/grant handling where the OS
supports it, and optional GUI recovery for a device that must listen. Automatic
negotiation, speculative simultaneous listeners, probes, downgrade and relays
remain excluded from this implementation scope.

If neither peer can accept LAN connections and authorized OS access is
unavailable, direct LAN transfer cannot be promised. An existing firewall GUI is
optional administrator recovery and file transfer is recovery; neither silently
relaxes the ordinary-user requirement. The selected outbound alternative needs
protocol tests and packaged acceptance before claiming that requirement met;
networks blocking both directions remain an explicit product limitation.

### Endpoint/Session Foundation — Historical v3 Record, 2026-10-16

This earlier stage implemented the manual `PCLAN003` endpoint/session foundation, not the
new connection direction, firewall authorization or nearby-device discovery.
The production v4 record above supersedes its transport/IPC descriptions; the
original evidence and test results below remain historical.
`if-addrs` **0.15.0** (MIT OR BSD-3-Clause) supplies native
interface enumeration; `socket2` **0.6.3** and Unix `libc` already existed in the
lockfile and are now direct dependencies for scoped listeners and cancellable
nonblocking TCP connection. No unrelated dependencies were upgraded. Pairing,
package versions 1–4, library merge rules, existing audiobooks and search indexes
retain their existing contracts.

The small `src-tauri/plugins/library-transfer/` plugin reports Android `onStop` /
`onResume` and iOS background/foreground notifications to Rust. It does not use
WebView timers, Android `onPause`, iOS resign-active notifications or desktop
focus/minimize events as cancellation signals. Permission prompts therefore do
not cancel a foreground transfer. LAN commands fail closed until the first mobile
visibility callback; transfer-file fallback stays available if visibility tracking
cannot initialize. Closing the main window, actual backgrounding or selected
interface/address loss cancels network work without cancelling an admitted
restore. A visibility epoch also rejects a start request interrupted by a
background/foreground cycle before its session could be installed. Returning to
the foreground never restarts sharing.

The interface watcher exists only during network sessions and stops on cleanup.
It detects operational/interface/address changes, not an SSID change that
retains an identical interface and address set. Network-identity integration and
system-picker coordination belong with the native discovery adapters; no such
picker is introduced by this stage. The connect budget remains one 30-second
attempt that polls cancellation every 25 ms, followed by existing socket
timeouts. Failed status polling is visible and never unlocks navigation while
the native command is still active. Receive-progress events before the first
native generation snapshot are ignored; the native diagnostic history retains
them.

The foundation review fixes preserve native subnet prefixes without exposing
them through IPC. Prefix changes also invalidate an active network snapshot.
Switching Send/Receive clears terminal sender state so network controls and
Receive errors stay visible, while retaining the content selection. A matching
terminal sender snapshot clears pending cancellation even when failure or expiry
wins the race; snapshots from another generation cannot unlock recovery.

Regression coverage includes offline/ambiguous interface selection, explicit
advanced interfaces, legacy `IPv4:port`, bare-IP default ports, IPv6 scope
replacement, unsafe destinations, dual-family partial-bind rollback, expiry and
terminal credential cleanup, stale generations, close/background admission
races, network loss, connect-budget cancellation, blocked TLS cancellation,
authenticated staging cancellation with private resume files, and the atomic
restore boundary. Existing wrong-code, channel binding, malformed traffic,
authenticated resume, package validation and bounded/redacted diagnostics tests
remain in place. Source-selection regressions cover multiple IPv4/ULA subnets,
overlapping prefixes, reversed enumeration, link-local scope and ambiguous
routed destinations. Dialog regressions cover selection/override validation,
expiry, stale polls/progress, cancellation errors, cancellation/failure races,
mode changes after terminal Send states, visible Receive errors and Finishing Import.

Validation on Linux after the review fixes: `npm test` passed **324 tests** (one existing skip);
`cargo test --manifest-path src-tauri/Cargo.toml --lib --no-default-features`
passed **212 tests** (four existing ignored tests). All seven new regression cases
failed against the previous implementation and passed after the fixes. `npm run build` passed i18n,
TypeScript, Vite and Pagefind; lint, Rust formatting and documentation-link checks
passed. Vite retains its module/chunk-size warnings. The failed CI run above
confirms the audit result only. The owner subsequently reported a successful APK
rebuild; a complete passing CI record is not independently verified here.

Discovery implementation, further physical/network smoke and the app version
bump remain deferred. Linux automated checks do not establish platform acceptance.
The failed Android retry and confirmed live listener are recorded above; a
controlled network diagnosis and successful transfer remain outstanding. Stable
port selection is not claimed to fix incoming-deny policy.

### Delivery Order and Validation Gates

The default/explicit alternative and their wire/state contract are reviewed.
Implementation remains staged; native lifecycle/permission integration still
needs platform validation before claiming the complete transfer UX:

1. **Endpoint/session foundation (implemented; platform acceptance pending):** typed interface-aware endpoints,
   stable binding, authoritative readiness/deadlines, cancellable receive and
   cleanup, reused by the v4 production path. All four discovery backends are implemented;
   v3 is no longer a runtime compatibility path.
2. **Authorization/packaging source evaluation complete; inbound UFW gate unmet:** the
   inspected UFW GUIs and profile-only packages do not provide the required
   scoped approval flow. Keep AppImage unprivileged and a custom helper
   unselected. Prefer the explicit outbound-receive mode for the target device
   pair; no direction or firewall fallback may run without user intent.
   Firewalld/native permission paths still need scope, denial, expiry, crash and
   packaging validation. Static profiles/GUI recovery are not acceptance.
3. **v4 production integration implemented/tested:** both manual dialog modes
   use one authenticated stream, native owner/reservation and importer boundary.
   The cutover review and recovery fixes are complete. All four system DNS-SD
   backends are implemented and need joint review/platform acceptance. Add only
   supported OS permission paths, not an automatic UFW grant. Compile
   and link adapters on the supported minimum OS; verify callback teardown,
   Linux missing-bus/missing-Avahi behavior and explicit unsupported discovery.
   No minimum-OS increase, third-party daemon installation or dual-stack protocol
   compatibility is implied by this design.
4. **Automated regression gate:** retain port conflicts, partial-bind cleanup,
   permission errors, ambiguous/no-default-route interfaces, scoped IPv6,
   endpoint revalidation, expiry, background/close cleanup, late callbacks and
   cancellation in every pre-import phase. Add malformed/oversized/spoofed/stale
   discovery, event/resolve bounds, denial/unavailability and no automatic TCP
   probes. Cover new/old wire rejection without downgrade, TLS/PAKE role and
   channel binding in both content/connection role mappings, reflected or
   tampered mode/role preludes, wrong-code consumption and authenticated resume
   with changed-package rejection. Exercise explicit mode switches during
   preparation/waiting/connect, stale callbacks and credential/listener cleanup.
   Retain partial import, acknowledgement failure and report redaction; bounded
   diagnostic codes may identify the chosen mode/connection role without secrets.
   Any authorization implementation must verify
   narrow scope, untrusted-input rejection, owned-rule cleanup and crash expiry
   without changing administrator-owned rules.
5. **Deferred physical acceptance:** diagnose the failed connection using both
   reports and current listener/firewall/routing evidence. Verify Linux sending
   to Android without a new Linux inbound transfer-TCP grant. Linux receiving
   must use a reviewed connection/authorization flow without terminal or
   per-phone exceptions. Exercise both directions across UFW/firewalld, Windows,
   Android and Apple builds. Keep
   SDK 26–33 manual fallback, SDK 34+ scoped NSD, Linux without Avahi, multiple
   interfaces/VPNs, network changes, blocked multicast, isolated Wi-Fi and grant
   revocation cases. File fallback is recovery, not proof of LAN success. Smoke
   and the app version bump stay deferred until explicitly scheduled.

The foundation touches session/endpoint ownership, transport cancellation, native
visibility callbacks, transfer IPC, dialog controls, locale resources and their
tests. Package/import storage remains reusable; changing connection roles now
requires an explicit protocol/security review. The earlier design-only commit
`7a69843` contains the initial DNS-SD evaluation, whose permanent sender-listens
recommendation is superseded here. The current stage implements manual v4
transport/UI integration without dependencies or firewall mutation.

## Delivery Checklist

- [x] Architecture, package contract, merge rules, and staged scope documented.
- [x] Stage 1: export generic uploaded documents and folder organization.
- [x] Stage 1: import, verify, sanitize, and rebuild target search data.
- [x] Stage 1: expose file-based transfer from App Settings.
- [x] Stage 1: cover package validation, duplicate handling, and folder mapping.
- [x] Stage 2: make native audiobook manifests the authoritative completed-audio registry.
- [x] Stage 2: add individually selectable completed-audiobook payloads, defaulting to excluded.
- [x] Stage 3: add foreground, authenticated same-network transfer using this package.
- [x] Stage 3: add one-use expiry and foreground sender cancellation.
- [x] Foundation: explicit interfaces, stable port/override, scoped IPv6,
  generation-owned snapshots and cancellable receive with protected restore.
- [x] Foundation platform builds: full CI run 39003088519 on `d0409fe`.
- [ ] Physical lifecycle/network acceptance (smoke deferred).
- [x] Revised LAN-only/no-terminal UX, connection direction and native
  authorization constraints documented; native v4 cutover implemented below.
- [x] Evaluate UFW/AppImage GUI and packaging options against upstream source;
  record profile/catalog differences and reject an assumed approval API.
- [x] Evaluate/select explicit outbound receiving before privileged UFW integration;
  distinguish the listening device/code owner from the content sender/receiver.
- [x] Finalize/review both modes' v4 framing, transcript, state/consent, resume,
  deadlines and old-peer cutover; validate mechanical confirmation fixtures.
- [x] Implement/test the v4 codec/authentication and per-generation state contract
  with real TLS/PAKE before integrating the shared stream boundary and dialog controls.
- [x] Integrate v4 with production listener/package/staging ownership, native
  reservations/lifecycle/diagnostics and both dialog modes; remove the temporary
  dead-code allowance and cut over from v3 without fallback.
- [ ] Validate the no-terminal flow with packaged builds; inbound UFW approval
  remains unsupported without separately reviewed integration or administrator setup.
- [ ] Implement/test supported native OS permission paths without a custom UFW helper.
- [x] Implement native DNS-SD adapters for Linux, Android, Apple and Windows,
  with nearby-session UI and bounded callback regressions.
- [x] Joint discovery review/fixes and full native/platform CI.
- [ ] Packaged discovery acceptance (smoke deferred).
- [x] Stage 3: add byte-level transfer and item-level restore phases.
- [x] Stage 3: reject package staging and restoration when storage is insufficient.
- [x] Stage 3: resume interrupted transfers, especially large audio.
- [x] Stage 3: separate session orchestration, transport framing, and pairing security.
- [x] Security hardening: use 60-bit pairing codes and bind only the selected
      local interface's eligible addresses.
- [x] Security hardening: atomically reserve send preparation and serialize
      file/LAN restore operations at the native boundary.
- [x] Security hardening: disclose that saved transfer files are readable,
      unencrypted copies that should be protected and deleted after use.
- [x] Security hardening: bound standard and ZIP64 central-directory allocation
      before archive parsing.
- [x] Security hardening: ignore malformed pre-authentication connections while
      keeping complete incorrect pairing attempts one-use.
- [x] Security hardening: replace the pairing proof with a standard PAKE and key
      confirmation for transfers on public networks.
- [x] Stage 3: keep nearby transfer primary and progressively disclose file fallback actions.
- [x] Keep active send instructions, progress, cancellation, and results above the fold.
- [x] Add selective uploaded-document transfer with audiobook dependency inclusion and folder pruning.
- [x] Review hardening: constrain audiobook document URLs and cross-platform archive paths.
- [x] Review hardening: verify transferred WAV contents against measured playback metadata.
- [x] Review hardening: classify terminal receiver failures and interrupt active sends on cancel.
- [x] Review hardening: align local-address policy and prune stale transfer cache files.
- [x] Follow-up design: [select DNS-SD backends, endpoints, compatibility and
      session lifecycle](#selected-connection-design--2026-10-16).
- [ ] Implement the connection redesign on `refactor/library-transfer` following
      the documented delivery order; retain diagnostics and defer device smoke.
- [ ] Later: evaluate an optional reading-data category for bookmarks and preferences.

## Release Acceptance Gate

The automated library-transfer suite covers package validation, private staging,
SPAKE2 agreement, TLS-channel and role binding, wrong-code rejection, malformed
traffic, and resumable streaming in v4. It does not establish packaged-device
discovery, permission, firewall or WebView behavior. Before release, run the
following manual cases with packaged builds and record the platform, network
type, wire version and result; these device checks remain deferred in this stage:

| Case | Action | Pass criteria |
| --- | --- | --- |
| Task-first entry and empty library | Open Transfer Library, choose Send with and without content, select items and Continue, then Change Selection. Switch to Receive. | Fresh Send starts with nothing selected and Continue/export disabled; explicit selection enables them. No address form precedes selection; empty Send explains the missing content; selections survive the step change; Receive needs no local content; no discovery or transfer starts merely by choosing a task. |
| Mobile step layout | On Android/iOS, walk task choice, content selection, pairing, progress and results; expand manual settings/diagnostics, rotate, enlarge text and open the keyboard in the pairing field. Repeat in RTL. | Dialog/navigation stay within usable safe-area bounds as steps change. The header identifies the task, controls do not collapse, focused inputs and lower actions can be reached, and no horizontal scrolling is needed. Record actual keyboard/WebView behavior separately from browser-fixture results. |
| Normal transfer | Send selected documents with and without a saved audiobook between two current builds. | Only selected content arrives; required source documents, folders, EPUB images, PDF sources, and playable audio remain intact. |
| Incorrect code | Enter a complete valid-format code that differs from the listening device's code. | Both devices report pairing failure, no package bytes move, and a new session and code are required. |
| Unrelated traffic | While the listening device waits, connect to its displayed port with a browser or raw TCP client where available. | It stays waiting without showing an unrelated transfer failure. |
| Resume | Interrupt a large authenticated transfer, restore the same connection path, and receive again with the same address and code. | Transfer resumes from retained bytes and the completed package passes full checksum and import validation. |
| Cancel and recovery | Cancel while waiting and during transfer; retry after an insufficient-storage failure. Exercise cancellation near expiry/failure and switch from a terminal Send session to Receive. | Sockets close promptly, controls recover, partial files do not accumulate, and retry succeeds after storage is available. Change Selection stays usable after terminal failure; Receive retains its network controls and shows errors. |
| Public network | Try a guest or public Wi-Fi network with client isolation, then one that permits device-to-device traffic. | Isolation produces a clear connection failure; when peer traffic is permitted, only the correct code authenticates and content remains encrypted in transit. |
| Interface selection | Exercise representative Wi-Fi, Ethernet, hotspot, and explicitly selected advanced routes supported by the test devices, including an adapter with multiple local subnets where available. | The displayed address matches the listening interface; other interfaces are not exposed implicitly. The connecting side uses the peer's matching subnet, or reports an ambiguous source instead of choosing an unrelated address. |
| Protocol mismatch | Connect current `PCLAN004` builds to released v3 peers in both directions. | Fail closed without downgrade or import; a different received marker proves incompatibility, while EOF only warrants connection/update/file guidance. |
| New protocol cutover | Pair either v4 mode with v3 in both content directions; spoof compatibility/mode metadata. | Clear incompatible-peer/update/file recovery; no direction switch, authenticated-protocol downgrade or partial import. |
| Linux sending to Android | Use incoming-deny Linux with no Papercut transfer rule; start Receive on Android and follow the new nearby-device/code flow. | Linux uses an outgoing connection; no Linux inbound transfer-TCP exception or terminal setup is required. Validate mDNS/discovery access separately. |
| Linux receiving | Under Other Connection Method, use Use the Other Device's Code on receiving Linux/AppImage, and Show a Pairing Code Instead on Android after selecting content and Continue. Separately exercise default listening on OSes with supported grants. | Linux receives over an outgoing authenticated connection without a Linux incoming transfer-TCP rule, terminal or per-phone exceptions. Android must be reachable and foregrounded; discovery/permissions remain separate gates. Any OS grant explains scope/duration. |
| Explicit mode and role safety | Switch modes before pairing and near cancellation/expiry; tamper or reflect the advertised/authenticated mode and content roles. | Old listeners/codes/advertisements cannot survive or alter the new generation. Only complementary, mutually authenticated expected roles move metadata/data; no automatic fallback, callback connection or second listener. |
| Resume in both modes | Interrupt an authenticated transfer in each connection direction; retry the same session, then offer a different prepared package or switch modes. | Same-session data resumes safely. Changed package/invalid offset is rejected before append; new mode/code/peer selection cannot reuse a prior partial. Import and acknowledgement behavior remain unchanged. |
| Completed-byte/result ambiguity | Lose the last write/flush or final Status, inject invalid post-copy Status, and disconnect/background either device during admitted import. | Sender reports Outcome Unknown where delivery may be complete and never automatically resends. Receiver finishes admitted work safely while allowed by the OS; network teardown cannot cancel import or reopen the generation for replay. |
| Full-length or corrupt partial | Retry with `O=N`, corrupt the retained prefix, or change the authenticated Offer. | Full archive SHA-256 still gates restore; wrong content imports nothing. A new Offer cannot repurpose the partial. Storage Retry is same-session only before Ready, not after restore admission. |
| UFW integration packaging | If descriptors or privileged integration are later selected, inspect matching distro packages, upgrade/remove them and compare KDE/Gufw catalog behavior. | Descriptors appear in the intended GUI without installing rules; no app launch writes system policy. Foreign packages, a generic settings button or an unscoped preset do not establish guided consent. |
| Authorization denial | Deny/cancel OS consent, remove the agent/service, or enforce administrator restrictions. | Visible recovery without password collection, text-agent fallback, privilege bypass or indefinite waiting. |
| Grant lifecycle | Cancel/expire Receive, change networks, crash/restart, reload the firewall service and uninstall; retain an unrelated administrator rule. | Temporary grants expire/revoke within their declared bound; persistent consent is separately explicit and revocable. Unrelated rules survive and the app never silently broadens scope. |
| LAN-only operation | Transfer with Internet unavailable; also test mobile cellular enabled and multiple adapters. Inspect traffic where available. | Pairing, discovery and data use the selected LAN path; no external rendezvous/relay, automatic tunnel/cellular selection or Internet fallback. |
| Transfer-file fallback | Save, move, and import a `.papercut-library` file after reviewing its privacy notice. | The file route preserves the same selection and validation behavior and never implies that the saved package is encrypted. |

The v1.9.0 packaged-build smoke record covers the original manual transfer flow;
it does not establish acceptance of the endpoint/session foundation. The newer
failed device test is recorded above; further smoke remains deferred. In addition
to the matrix, the foundation needs packaged
checks for occupied/default/override ports, IPv4-to-legacy peers, IPv6-only and
scoped link-local peers, Wi-Fi without Internet (including mobile cellular enabled),
network/address changes, actual backgrounding versus permission prompts, main-window
closure, cancellation during connect/TLS/staging and safe completion after restore
starts. Verify desktop focus/minimization does not cancel and revoke every
test-owned temporary grant after testing without changing unrelated policy.

## Deferred Decisions

- Saved transfer files intentionally remain unencrypted. Passphrase-based
  authenticated encryption needs an explicit user requirement because it adds
  password, recovery, compatibility, and package-version UX.
- Android 17 local-network permission/system-picker handling is required when
  Papercut raises its Android target from SDK 36 to SDK 37; a selected-endpoint
  picker grant does not cover arbitrary manual connections or advertising/listening.
- Automatic negotiation of either connection direction is deferred until a
  demonstrated need justifies its discovery, authentication and lifecycle cost.
  The explicit receiver-connects alternative is selected and does not authorize
  automatic negotiation. External relays remain excluded from the LAN-only design.
- Original EPUB archives are not transferable because the current upload
  pipeline stores only generated, sanitized reading HTML. Retaining originals
  would be a separate storage and migration feature.
