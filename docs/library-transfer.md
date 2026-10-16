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

The header has icon-only **Back** and **Close** controls on a separate row above
the title. Back returns to Settings; Close dismisses the entire Settings flow.
During export, import, preparation, receiving or waiting/sending, both controls
are disabled and Escape/backdrop dismissal is blocked. Session cancellation
remains an explicit **Change Selection** or **Stop Sending** action in the body.

Completion also replaces the active controls in place and keeps the result open
until the user chooses **Back** or **Close**. The source reports the document and audiobook
counts sent because the protocol returns only a final receiver acknowledgement;
the target shows imported, already-present, and failed counts plus expandable
failure details. Partial completion uses a distinct warning state instead of
presenting every completed operation as an unqualified success.

## Same-Network Transport

The implemented `PCLAN003` path remains foreground and manual. The revised
receiver-listens design below is not implemented:

1. The source builds an app-owned temporary `.papercut-library` package.
2. It binds TCP **53318**, or an explicit advanced override (1024–65535), on
   every eligible address of the selected interface. IPv4 private/link-local and
   IPv6 ULA/link-local addresses are supported; loopback requires explicit
   selection for same-device testing. A twelve-character base-32 pairing code provides
   60 bits of entropy and is grouped as `XXXX-XXXX-XXXX` for transcription. The
   waiting/pairing session expires after ten minutes. A port conflict fails
   visibly without silently changing ports; any earlier successful binds are closed.
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

Native interface enumeration replaces the default-route UDP probe and needs no
Internet route. The dialog shows the selected interface and its local addresses,
allows refresh after a network change, and requires an explicit choice when
multiple recognizable Wi-Fi/Ethernet interfaces are available. Tunnel, loopback
and unknown interfaces are never selected automatically. Recognition uses
platform interface names and the point-to-point flag; it is a convenience, not
a guarantee about physical hardware or network trust. Listeners bind selected
addresses, never wildcard addresses. Receives bind a matching source address.
On adapters with multiple compatible addresses, the most specific matching OS
subnet prefix selects the source. If none matches, Receive reports ambiguity
instead of guessing; use a peer address on that network or choose another
network. A single compatible source still permits routed local transfers.
OS routing and firewall policy remain authoritative. Global-only IPv6 networks
require transfer-file fallback. IPv6 link-local scope comes from the receiving
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

The receiver marks only explicit insufficient-storage failures as retryable.
Invalid schemas, checksums, payloads, or restore data fail the sender session
instead of offering a retry that would reproduce the same result. Cancelling the
source closes its active socket, including while it waits for target-side import.
The receiver can cancel TCP connection, TLS/pairing and package staging. Once
restore is admitted atomically, native status becomes **Finishing Import** and
cancellation is disabled; successful imports are retained even if acknowledgement
fails. Sender cancellation during preparation waits for the current synchronous
package build to finish, then discards it without exposing a listener.
Transfer failures cross the LAN and Tauri boundaries as a structured code,
diagnostic message, and optional storage byte counts, so retry decisions and
localized storage guidance never depend on parsing human-readable text.

The command and session lifecycle lives in
`src-tauri/src/library_transfer/network.rs`, resumable package streaming and
progress framing live in `network/transport.rs`, interface/address policy and
all-or-nothing binding live in `network/endpoints.rs`, socket cancellation and
interface watching live in `network/session.rs`, and TLS plus one-use SPAKE2
pairing live in `network/security.rs`. Storage remains owned by `package.rs` and
`mod.rs`. React polls native send/receive snapshots and listens
for locale-neutral receiver progress events. Native snapshots identify session
generations, listener readiness and the waiting deadline. Terminal sender
snapshots clear every endpoint, pairing code and deadline after closing listeners.
Cancel commands and local receive-progress events carry generation IDs; stale
polls/events cannot overwrite or cancel a newer session. Local event IDs are not
added to `PCLAN003` progress frames. Rust reports transferred bytes,
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

In contrast, [croc's transport selection](https://github.com/schollz/croc#data-transport-selection)
uses traversal and relay infrastructure when a direct path is unavailable.
That improves reachability but does not establish a strictly offline LAN path.
Papercut will not silently add a public relay or adopt that infrastructure to
hide a blocked local connection.

### Selected Connection Design — 2026-10-16

**Revised after the foundation device test and no-terminal requirement.** This
supersedes the earlier recommendation to retain sender-listens as the permanent
design. The implemented `PCLAN003` foundation remains unchanged and useful; the
changes below are design decisions and implementation gates only.

Reuse the package/import boundary, maintained TLS/SPAKE2 primitives, native
streaming, one-use codes, authenticated resume and bounded diagnostics. Choose
**receiver-listens / sender-connects for new peers** as the smallest next design.
Content roles and connection roles must be explicit rather than inferred from
generic names such as client/server throughout storage and import code.

| Option | Evaluation and decision |
| --- | --- |
| Keep sender-listens | Preserves the current wire path but unnecessarily requires inbound Linux access for the reported Linux-to-Android send. Keep it as the current implementation, not the future UX target. |
| Receiver-listens | Standard upload pattern used by LocalSend. Linux sending to Android uses Linux's permitted outgoing path; Linux receiving still needs incoming access. Selected for the next protocol. |
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

#### Connection Roles and Protocol Cutover

The current `security.rs` explicitly binds sender/server and receiver/client to
`PCLAN003`, ordered SPAKE2 identities and a versioned TLS exporter. Reversing TCP
alone is not a compatible refactor. Require a reviewed new wire version
(`PCLAN004` is the proposed marker, not an implemented capability), exporter
domain and explicit content-role confirmations. Reuse the cryptographic
libraries; do not invent negotiation or weaken certificate/channel binding.
Finish human code entry before starting short authentication I/O timeouts.

Prefer a deliberate protocol cutover with a clear update/file-transfer fallback
for old peers over indefinitely maintaining two network implementations. The
compatibility decision and new framing/pairing transcript must be reviewed before
code changes. Do not silently retry a failed authenticated exchange using v3;
DNS-SD metadata cannot authorize a downgrade. Existing v3 peers continue to work
with each other until cutover; package versions 1–4 and saved libraries are not
changed by the new network protocol. App version/release timing remains deferred.

#### DNS-SD Feasibility and Platform Choice

Use standard DNS-SD over local mDNS, with **system discovery adapters** rather
than an app-owned multicast protocol. Source/API inspection establishes a viable
integration path, not a compiled or device-tested implementation:

| Platform | Selected adapter | Packaging and permission implications |
| --- | --- | --- |
| iOS / macOS | System `DNSServiceRegister`, `DNSServiceBrowse`, resolve/address callbacks and explicit reference disposal. Register the receiving session's one Rust listener port on the selected interface. | Apple supplies Bonjour. Declare `_papercut-xfer._tcp` in `NSBonjourServices`, retain the local-network usage description, and handle permission failure. Use the C API for compatibility with the current macOS 10.13 / iOS 15.0 floors; this does not require a custom multicast entitlement or an OS-version bump. |
| Android | Kotlin `NsdManager` through a narrow Tauri native plugin, with a selected `Network`. | Network-scoped browse and `NsdServiceInfo.setNetwork` are available from API 33. For the app's API 26–32 range, retain manual/file transfer and report discovery as unsupported rather than silently searching/advertising on all networks. Keep the current minimum SDK. |
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
without broad permission, but does not authorize receiver advertising/listening or
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
and graceful fallback is the smaller system-service design. Exact binding-crate
versions and their dependency/license audits belong to implementation, with no
dependency changes in this design commit.

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
- The existing v3 foundation retains full legacy `IPv4:port` input, including
  random ports, bare-IP defaults and IPv6 literals. The new receiver-listens
  manual fallback enters the receiving endpoint on the sender. Do not imply that
  an address makes incompatible wire versions interoperate. App versions need
  not match exactly; authenticated wire and package compatibility are decisive.
- Advertise only an implemented wire capability. The proposed v4 flow must not
  announce or impersonate v3 to preserve old discovery behavior. Incompatible
  peers receive a clear update/file-transfer option, not automatic downgrade.

#### Discovery Contract and Bounds

Publish `_papercut-xfer._tcp.local.` only after the receiving TCP listeners are
ready. Browse only that service while Send is foregrounded. Use an ephemeral
random instance name, shown on the receiver for recognition; TXT contains only
`v=1` (discovery schema) and the implemented `p` wire capability. The proposed v4
path uses `p=PCLAN004` only after cutover; no discovery is implemented today.
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

#### Session Lifecycle and Cancellation

Rust owns one foreground session generation, its package guard, listener set,
active socket, cancellation state, discovery registrations and cleanup. React
reads a snapshot and receives state updates; late callbacks from a replaced
generation cannot revive old entries or alter the current session. Separate
listener readiness from discovery availability so an unavailable browser does
not disable a working manual path. A historical bind event is not a liveness
check, and a local Ready state is not proof of remote reachability.

In the new flow, Receive owns binding, readiness and the waiting/code window;
Send owns package preparation and the outgoing connection. Both expose
authentication, streaming, receiver import and a terminal result. Expose the
native waiting/pairing deadline and expiry reason. Retain the existing ten-minute
pairing window, authentication timeouts and authenticated-stream/import timeouts;
discovery refreshes do not
extend them. Withdraw advertising after successful pairing, re-advertise only
after an eligible authenticated interruption, and preserve the same code/package
only within its unexpired session. Incomplete unauthenticated traffic must not
consume the code; a completed incorrect proof still does.

Retain cancellation for connect, TLS/pairing and staging in the new roles. It
must interrupt native I/O and release the restore reservation, not merely hide the dialog or
await the full connect timeout. Once package restore starts, show **Finishing
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
gate remains unmet for incoming-deny UFW. Prefer evaluating an explicit outbound
connection option for Linux receiving before taking ownership of privileged
firewall software. This is a design recommendation, not a new implemented mode
or a claim that UFW's policy can be bypassed.

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

The next design decision is therefore **connection direction before privileged
UFW integration**. Keep receiver-listens as the proposed normal flow: it avoids
Linux's inbound transfer-TCP requirement when sending to Android, though mDNS
and Android permissions still need validation. For the opposite content direction,
evaluate an explicitly selected peer-hosted session so Linux can connect out
while receiving. Compare its UX, TLS/PAKE role binding and resume cost with the
helper's security/packaging cost before changing the selected protocol design.
Require one chosen listener and authenticated content roles; no speculative
simultaneous listeners, automatic probing, protocol downgrade or external relay.
Automatic bidirectional negotiation remains deferred.

If neither peer can accept LAN connections and authorized OS access is
unavailable, direct LAN transfer cannot be promised. An existing firewall GUI is
optional administrator recovery and file transfer is recovery; neither silently
relaxes the ordinary-user requirement. The next review must select a flow that
meets it or explicitly resolve this product limitation before implementation.

### Endpoint/Session Foundation — Implemented 2026-10-16

This stage implements the manual `PCLAN003` endpoint/session foundation, not the
new connection direction, firewall authorization or nearby-device discovery.
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

The revised direction is selected; platform authorization and the new security
transcript must pass feasibility/review gates before implementation:

1. **Endpoint/session foundation (implemented; platform acceptance pending):** typed interface-aware endpoints,
   stable binding, authoritative readiness/deadlines, cancellable receive and
   cleanup. The manual `PCLAN003` path remains the first integration gate;
   the full discovery redesign is not complete.
2. **Authorization/packaging source evaluation complete; UX gate unmet:** the
   inspected UFW GUIs and profile-only packages do not provide the required
   scoped approval flow. Keep AppImage unprivileged and a custom helper
   unselected. Next compare explicit outbound receiving with privileged system
   integration in the protocol/UX review; do not implement either implicitly.
   Firewalld/native permission paths still need scope, denial, expiry, crash and
   packaging validation. Static profiles/GUI recovery are not acceptance.
3. **Protocol review, then transport/discovery implementation:** resolve the
   outbound-receiving candidate and finalize code ownership, content/connection
   roles, new marker/exporter/transcript, authenticated resume and old-peer
   cutover before coding. Then add the new
   flow, bounded native DNS-SD adapters, device selection and permissions. Compile
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
   channel binding, wrong-code consumption, authenticated resume, partial import,
   acknowledgement failure and report redaction. Authorization tests must verify
   narrow scope, untrusted-input rejection, owned-rule cleanup and crash expiry
   without changing administrator-owned rules.
5. **Deferred physical acceptance:** diagnose the failed connection using both
   reports and current listener/firewall/routing evidence. Verify Linux sending
   to Android without a new Linux inbound transfer-TCP grant. Linux receiving
   must use a reviewed connection/authorization flow without terminal or
   per-phone exceptions. Exercise both directions across UFW/firewalld, Windows,
   Android and Apple builds. Keep
   SDK 26–32 manual fallback, SDK 33+ scoped NSD, Linux without Avahi, multiple
   interfaces/VPNs, network changes, blocked multicast, isolated Wi-Fi and grant
   revocation cases. File fallback is recovery, not proof of LAN success. Smoke
   and the app version bump stay deferred until explicitly scheduled.

The foundation touches session/endpoint ownership, transport cancellation, native
visibility callbacks, transfer IPC, dialog controls, locale resources and their
tests. Package/import storage remains reusable; changing connection roles now
requires an explicit protocol/security review. The earlier design-only commit
`7a69843` contains the initial DNS-SD evaluation, whose permanent sender-listens
recommendation is superseded here. No implementation, dependency change or
firewall mutation accompanies this revised design.

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
- [ ] Foundation platform builds and physical lifecycle/network acceptance.
- [x] Revised LAN-only/no-terminal UX, connection direction and native
  authorization constraints documented; runtime unchanged.
- [x] Evaluate UFW/AppImage GUI and packaging options against upstream source;
  record profile/catalog differences and reject an assumed approval API.
- [ ] Resolve the unmet UFW ordinary-user UX: review explicit outbound receiving
  before adopting privileged integration; validate scope/lifetime on supported OSes.
- [ ] Review receiver-listens wire/security cutover, the explicit outbound-receive
  candidate and old-peer messaging before selecting implementation scope.
- [ ] Implement and test supported native authorization without terminal setup.
- [ ] Native DNS-SD adapters, nearby-session UI and platform discovery acceptance.
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
traffic, and resumable streaming. It does not establish the proposed new protocol
or authorization behavior. Before release, run the following manual cases with
packaged builds and record the platform, network type, wire version and result;
new-design cases become required when that implementation is available:

| Case | Action | Pass criteria |
| --- | --- | --- |
| Normal transfer | Send selected documents with and without a saved audiobook between two current builds. | Only selected content arrives; required source documents, folders, EPUB images, PDF sources, and playable audio remain intact. |
| Incorrect code | Enter a complete valid-format code that differs from the listening device's code. | Both devices report pairing failure, no package bytes move, and a new session and code are required. |
| Unrelated traffic | While the listening device waits, connect to its displayed port with a browser or raw TCP client where available. | It stays waiting without showing an unrelated transfer failure. |
| Resume | Interrupt a large authenticated transfer, restore the same connection path, and receive again with the same address and code. | Transfer resumes from retained bytes and the completed package passes full checksum and import validation. |
| Cancel and recovery | Cancel while waiting and during transfer; retry after an insufficient-storage failure. Exercise cancellation near expiry/failure and switch from a terminal Send session to Receive. | Sockets close promptly, controls recover, partial files do not accumulate, and retry succeeds after storage is available. Change Selection stays usable after terminal failure; Receive retains its network controls and shows errors. |
| Public network | Try a guest or public Wi-Fi network with client isolation, then one that permits device-to-device traffic. | Isolation produces a clear connection failure; when peer traffic is permitted, only the correct code authenticates and content remains encrypted in transit. |
| Interface selection | Exercise representative Wi-Fi, Ethernet, hotspot, and explicitly selected advanced routes supported by the test devices, including an adapter with multiple local subnets where available. | The displayed address matches the listening interface; other interfaces are not exposed implicitly. The connecting side uses the peer's matching subnet, or reports an ambiguous source instead of choosing an unrelated address. |
| Protocol mismatch | Connect a `PCLAN003` build to the last pre-`PCLAN003` development build. | Pairing fails closed and neither build imports partial or unauthenticated data. |
| New protocol cutover | Pair the future receiver-listens build with v3 in both content directions; spoof compatibility metadata. | Clear incompatible-peer/update/file recovery; no authenticated-protocol downgrade or partial import. |
| Linux sending to Android | Use incoming-deny Linux with no Papercut transfer rule; start Receive on Android and follow the new nearby-device/code flow. | Linux uses an outgoing connection; no Linux inbound transfer-TCP exception or terminal setup is required. Validate mDNS/discovery access separately. |
| Linux receiving | Receive under UFW/AppImage and firewalld using the connection/authorization flow selected by the next design review. | No terminal or per-phone rule is required. Any OS grant clearly explains scope/duration; the unresolved ordinary-user UFW flow blocks claiming this acceptance. |
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
  External relays remain excluded from the LAN-only design.
- Original EPUB archives are not transferable because the current upload
  pipeline stores only generated, sanitized reading HTML. Retaining originals
  would be a separate storage and migration feature.
