# Library Transfer

## Purpose

Library Transfer copies Papercut-owned user data from one device to another
without an account, cloud service, or continuous synchronization. The source
device is unchanged and the receiving device merges new content into its
existing library.

The product deliberately says **transfer**, not **sync**. Continuous two-way
sync would require persistent device identity, deletion propagation, conflict
resolution, and background networking that this feature does not need.

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

The sole RustSec waiver is `RUSTSEC-2026-0235`: `rkyv` 0.7 appears only as an
inactive optional Chrono dependency and is absent from Papercut's complete
feature/target dependency graph. Remove the waiver when that upstream optional
edge moves to `rkyv` 0.8 or if a Papercut feature ever activates it.

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
per document. Before the ZIP reader allocates metadata for every entry, Papercut
reads only the fixed standard/ZIP64 end records and rejects more than 125,000
entries or a central directory larger than 64 MiB. The ZIP crate still owns all
archive parsing; the preflight only bounds work that otherwise occurs before the
manifest limits can run. Reading preferences are not represented.

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

App Settings owns the entry point because transfer is device-level data
management rather than another document format. A **Data** section opens a
dedicated **Transfer Library** dialog organized by the user's role:

- **Send** presents **Send over a local network** as the primary action and keeps
  **Save Transfer File** under an expandable manual fallback;
- **Receive** presents **Receive over a local network**, accepts the sending
  device's address and one-use code, and keeps **Import Transfer File** under
  the same secondary fallback.

The role control remains visible throughout the dialog. Local-network transfer
is the default path, while the transfer-file disclosures explain that files can
be copied through USB, shared storage, or another user-chosen method. The save
action also states that the ZIP-based package is not encrypted, contains readable
copies of the selection, and should be kept private and deleted when no longer
needed. This notice remains visible beside the action without adding a blocking
confirmation. The dialog consistently describes transfer as copying selected
content; it does not imply that the source device loses its library.

The send action offers separate collapsed checklists for uploaded books and
documents and for completed audiobooks. Documents default to selected to
preserve the original whole-library behavior; audiobooks default to excluded
because their audio can add gigabytes. Both lists support select-all and
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
source displays a local address and pairing code; the target enters both values
and receives the same package through the normal import boundary.

While Papercut checks and packages the selected content, the initiating action
stays disabled and an adjacent live status explains that large items can take
longer. Preparation remains indeterminate because hashing and ZIP construction
do not expose a trustworthy total; pairing details appear only after the package
is ready.

After sending starts, the selection lists and transfer-file fallback are
replaced in place by a compact guided state so current work remains above the
fold. Preparation leads to two numbered pairing instructions, the address and
one-use code, existing byte and receiver-import progress, and the final result.
**Change Selection** cancels a waiting session and restores the retained choices;
an in-flight transfer instead exposes **Stop Sending**. This uses the existing
dialog and session state rather than introducing routes or a multi-page wizard.

## Same-Network Transport

The first LAN implementation is intentionally foreground and manual:

1. The source builds an app-owned temporary `.papercut-library` package.
2. It binds an ephemeral port only on the private, link-local, or loopback IPv4
   address shown to the user. A twelve-character base-32 pairing code provides
   60 bits of entropy and is grouped as `XXXX-XXXX-XXXX` for transcription. The
   session expires after ten minutes.
3. The target connects over ephemeral TLS. The target and source use the
   one-use code as the password for a role-separated SPAKE2 exchange, then send
   mutual HMAC-SHA256 key confirmations bound to the TLS exporter. This prevents
   a captured exchange from becoming an offline code-guessing oracle and
   authenticates the self-signed channel without a permanent app key or
   certificate prompt. The `PCLAN003` marker versions these wire semantics so
   incompatible builds fail closed instead of interpreting another protocol.
4. The target reports the size of any partial package retained for this source
   address and pairing code. The source seeks to that offset and sends only the
   remaining bytes. An authenticated interruption returns the source to its
   waiting state, and choosing **Receive Library** again resumes the transfer.
5. The target stages the bytes in app cache and invokes the same archive parser,
   checksums, sanitizer, merge rules, and index rebuild used by file import.
   Starting a different authenticated session removes older partial packages.
6. The target forwards its verification and restore progress over the same TLS
   connection. The source mirrors those phases and reports success only after
   the target confirms that import has completed.

Connections that do not complete TLS and the versioned Papercut PAKE exchange
within five seconds return the source to its waiting state without showing
unrelated network traffic as a transfer failure. A complete but incorrect key
confirmation still consumes the source session to prevent repeated online code
guessing. Partial files survive network interruption and insufficient-space
errors, but are removed after a successful import or a non-recoverable package
error. A resumed package is always checksum-verified in full before restore, so
the byte offset is an optimization rather than a trust boundary.

Source address discovery uses the same private, link-local, or loopback IPv4
policy enforced by the receiver. The listener binds only that displayed address,
so VPN or secondary interfaces are not exposed implicitly. App startup creates
or repairs the data and cache roots as owner-only directories on Unix (`0700`).
Transfer packages use cryptographically random names, atomic exclusive creation,
and owner-only Unix permissions (`0600`); their scope guard removes them after
normal completion or failure. Resumable receive partials use the same file
permissions.

Startup removes transfer packages and partials left by an earlier process. They
cannot be resumed because pairing credentials are intentionally not persisted.
Within one running app session, authenticated retries keep their partial file;
transfer files left untouched for seven days are also removed on the next
transfer operation. Other cache entries are never matched by this cleanup.

The receiver marks only explicit insufficient-storage failures as retryable.
Invalid schemas, checksums, payloads, or restore data fail the sender session
instead of offering a retry that would reproduce the same result. Cancelling the
source closes its active socket, including while it waits for target-side import.
Transfer failures cross the LAN and Tauri boundaries as a structured code,
diagnostic message, and optional storage byte counts, so retry decisions and
localized storage guidance never depend on parsing human-readable text.

The command and session lifecycle lives in
`src-tauri/src/library_transfer/network.rs`, resumable package streaming and
progress framing live in `network/transport.rs`, and TLS plus one-use SPAKE2
pairing live in `network/security.rs`. Storage remains owned by `package.rs` and
`mod.rs`. The React UI polls source session state and listens
for locale-neutral receiver progress events. Rust reports transferred bytes,
package verification, document counts, and optional audiobook counts; it never
moves library bytes or networking into the WebView.

Native reservations are acquired before send preparation enters the blocking
pool and before either file import or LAN receive can mutate the library. The
reservations release automatically on every exit path. This makes Rust—not the
dialog's disabled controls—the authority preventing duplicate send preparation
and overlapping transfer restores.

iOS and macOS bundles include `NSLocalNetworkUsageDescription`, and the transfer
starts only from a user action while Papercut is foregrounded. Android currently
targets SDK 36 and uses its existing normal `INTERNET` permission. Before raising
the target to Android 17 / SDK 37, add and request `ACCESS_LOCAL_NETWORK` as
required by Android's local-network privacy model.

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
- [x] Stage 3: add byte-level transfer and item-level restore phases.
- [x] Stage 3: reject package staging and restoration when storage is insufficient.
- [x] Stage 3: resume interrupted transfers, especially large audio.
- [x] Stage 3: separate session orchestration, transport framing, and pairing security.
- [x] Security hardening: use 60-bit pairing codes and bind only the displayed
      IPv4 interface.
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
- [ ] Later: evaluate an optional reading-data category for bookmarks and preferences.

## Deferred Decisions

- Saved transfer files intentionally remain unencrypted. Passphrase-based
  authenticated encryption needs an explicit user requirement because it adds
  password, recovery, compatibility, and package-version UX.
- Android 17 local-network permission handling is required when Papercut raises
  its Android target from SDK 36 to SDK 37.
- Original EPUB archives are not transferable because the current upload
  pipeline stores only generated, sanitized reading HTML. Retaining originals
  would be a separate storage and migration feature.
