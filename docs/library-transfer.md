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
Future same-network transfer must send the same package contract rather than
introducing a second persistence format.

Rust ownership lives under `src-tauri/src/library_transfer/`. React ownership
lives under `src/library-transfer/`; shared dialogs and controls remain in their
existing component modules.

## Canonical And Derived Data

The package carries canonical user data:

- sanitized, normalized `source.html` for each generic HTML or EPUB upload;
- stable document ids and import metadata;
- uploaded-library folders and document placement metadata;
- a manifest with package and payload checksums.

The package does not carry derived or platform-specific data:

- `search.sqlite3`, FTS rows, or Pagefind output;
- TTS models, SILMA runtimes, generated playback caches, or diagnostics;
- incomplete audiobook jobs or application build artifacts.

The receiver parses and sanitizes every transferred HTML document again, then
rebuilds SQLite metadata, sections, and FTS rows with its installed app version.
Document ids come from the manifest rather than a hash of normalized HTML;
otherwise transferred EPUBs would get new URLs because their original archive
bytes are not retained by Papercut.

## Package Version 1

The archive contains:

```text
manifest.json
documents/<document-id>/source.html
```

`manifest.json` identifies `papercut-library`, schema version `1`, creation
time, every document payload and SHA-256 checksum, and folder organization.
Version 1 is limited to 500 documents and generic document uploads. Audiobooks
and reading preferences are intentionally not represented yet.

Import rules:

- merge into the target; never replace the target library;
- skip an existing document id and never move that existing document;
- re-sanitize, parse, and re-index every new document;
- merge folders by case-insensitive sibling name and preserve hierarchy;
- place only newly imported documents into the mapped source folders;
- reject unsupported schemas, malformed ids, duplicate archive paths, checksum
  mismatches, unexpected entries, oversized manifests, and oversized payloads;
- never extract archive paths directly onto the filesystem.

The importer retains successful documents if a later document fails and reports
partial results. This matches Papercut's existing batch-import behavior and
avoids discarding useful work because one payload is damaged.

## User Experience

App Settings owns the entry point because transfer is device-level data
management rather than another document format. A **Data** section opens a
dedicated **Transfer Library** dialog with two explicit actions:

- **Export Library** creates a package from this device;
- **Import Library** merges a selected package into this device.

The dialog reports document counts and failures. Network pairing, device roles,
byte-level progress, resumability, and audiobook selection appear only in the
stages that need them.

## Delivery Checklist

- [x] Architecture, package contract, merge rules, and staged scope documented.
- [x] Stage 1: export generic uploaded documents and folder organization.
- [x] Stage 1: import, verify, sanitize, and rebuild target search data.
- [x] Stage 1: expose file-based transfer from App Settings.
- [x] Stage 1: cover package validation, duplicate handling, and folder mapping.
- [ ] Stage 2: make native audiobook manifests the authoritative completed-audio registry.
- [ ] Stage 2: add optional completed-audiobook payloads, defaulting to excluded.
- [ ] Stage 3: add foreground, authenticated same-network transfer using this package.
- [ ] Stage 3: add transfer phases, cancellation, free-space checks, and resume for large audio.
- [ ] Stage 4: evaluate automatic discovery only after QR/manual pairing is proven.
- [ ] Later: evaluate an optional reading-data category for bookmarks and preferences.

## Deferred Decisions

- Audiobook packages require a native manifest scan so restored audio does not
  depend on WebView `localStorage` records.
- Same-network transport must use standard TLS and one-use session credentials;
  an unauthenticated local HTTP server is not acceptable.
- Android and iOS local-network permissions and discovery belong to the LAN
  stage, not the portable package implementation.
- Original EPUB archives are not transferable because the current upload
  pipeline stores only generated, sanitized reading HTML. Retaining originals
  would be a separate storage and migration feature.
