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
Stage 3 can send that exact package directly to another Papercut device on the
same network; it does not introduce a second persistence format.

Rust ownership lives under `src-tauri/src/library_transfer/`. React ownership
lives under `src/library-transfer/`; shared dialogs and controls remain in their
existing component modules.

## Canonical And Derived Data

The package carries canonical user data:

- sanitized, normalized `source.html` for each generic HTML or EPUB upload;
- stable document ids and import metadata;
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
appear automatically. Audiobook export is optional and off by default because
these payloads can make a transfer package several gigabytes larger.

The receiver parses and sanitizes every transferred HTML document again, then
rebuilds SQLite metadata, sections, and FTS rows with its installed app version.
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

Both versions are limited to 500 documents. Version 2 is also limited to 500
completed audiobooks and 100,000 audiobook files, with an 8 GiB expanded package
limit. Reading preferences are not represented.

Import rules:

- merge into the target; never replace the target library;
- skip an existing document id and never move that existing document;
- re-sanitize, parse, and re-index every new document;
- merge folders by case-insensitive sibling name and preserve hierarchy;
- place only newly imported documents into the mapped source folders;
- reject unsupported schemas, malformed ids, duplicate archive paths, checksum
  mismatches, unexpected entries, oversized manifests, and oversized payloads;
- never extract archive paths directly onto the filesystem.
- stage and checksum every audiobook file, then require the native registry to
  validate its manifest, chunk set, and cache identity before installation;
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

- **Send** makes nearby transfer the primary action and offers **Save Transfer
  File** as the manual alternative;
- **Receive** accepts the source address and one-use code, with **Import
  Transfer File** as the manual alternative.

The export action offers a default-off **Include saved audiobooks** checkbox when
completed audio exists. The dialog reports document and audiobook counts plus
failures. The same dialog also exposes explicit source and target roles for
same-network transfer. The source displays a local address and one-use code;
the target enters both values and receives the same package through the normal
import boundary.

## Same-Network Transport

The first LAN implementation is intentionally foreground and manual:

1. The source builds an app-owned temporary `.papercut-library` package.
2. It binds an ephemeral IPv4 port and shows `address:port` plus an eight-character
   pairing code. The session expires after ten minutes.
3. The target connects over ephemeral TLS. Both devices prove knowledge of the
   code with HMAC-SHA256 values bound to the TLS exporter, so the self-signed
   channel is authenticated without a permanent app key or certificate prompt.
4. The source streams the package once, then deletes its temporary file. The
   first connection attempt consumes the session to prevent online code guessing.
5. The target stages the bytes in app cache and invokes the same archive parser,
   checksums, sanitizer, merge rules, and index rebuild used by file import.

The socket lifecycle and protocol live in
`src-tauri/src/library_transfer/network.rs`; storage remains owned by
`package.rs` and `mod.rs`. The React UI polls source session state and listens
for locale-neutral receiver progress events. Rust reports transferred bytes,
package verification, document counts, and optional audiobook counts; it never
moves library bytes or networking into the WebView.

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
- [x] Stage 2: add optional completed-audiobook payloads, defaulting to excluded.
- [x] Stage 3: add foreground, authenticated same-network transfer using this package.
- [x] Stage 3: add one-use expiry and foreground sender cancellation.
- [x] Stage 3: add byte-level transfer and item-level restore phases.
- [x] Stage 3: reject package staging and restoration when storage is insufficient.
- [ ] Stage 3: resume interrupted transfers, especially large audio.
- [ ] Stage 4: evaluate automatic discovery only after QR/manual pairing is proven.
- [ ] Later: evaluate an optional reading-data category for bookmarks and preferences.

## Deferred Decisions

- Automatic discovery and QR pairing remain deferred; the first LAN release
  proves the smaller address-and-code workflow before adding multicast or a QR
  dependency.
- Android 17 local-network permission handling is required when Papercut raises
  its Android target from SDK 36 to SDK 37.
- Original EPUB archives are not transferable because the current upload
  pipeline stores only generated, sanitized reading HTML. Retaining originals
  would be a separate storage and migration feature.
