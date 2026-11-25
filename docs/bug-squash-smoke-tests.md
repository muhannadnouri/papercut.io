# PR 85 review fixes: owner smoke checklist

Run against the candidate containing all four review-fix stages. Record the build
or commit, OS/device, book, result, and diagnostics for any failure. Record desktop smoke evidence as described below. The remaining checks need
owner observations on the relevant devices.

Start on desktop and Android; repeat on iOS when a build is available. Leave any
untested platform explicit. Use an existing large EPUB/HTML audiobook, a compatible
older imported audiobook bundle, and an embedded-text PDF with saved audio. Include Arabic
or RTL text and an OCR PDF if available. Existing saved audio needs no regeneration
for these fixes; do not edit saved manifests to manufacture fault cases.

## Desktop smoke coverage

For a local Linux build, run `npm run smoke:desktop` from the repository root.
For pre-PR evidence on both platforms, push the candidate commit and dispatch
`desktop-smoke` on that branch. Record the tested SHA, the result for each OS
and the run URL. Credit a passing leg only for its tested build and platform.
Rerun on the new SHA if the candidate changes. [Local prerequisites and reports](desktop-smoke-e2e.md#running-the-reading-journey) · [Branch dispatch and artifact retrieval](desktop-smoke-e2e.md#run-on-a-pushed-branch)

A pass for `DS-001` (reading and bookmark restoration) covers these portions of the numbered reader checks for a document
created with **Paste Text**. Skip repeating these two observations by hand.

| Reader check | What the desktop run verifies | What still needs an owner |
| --- | --- | --- |
| 1. Reopen from Library | After saving a bookmark and restarting the app, the saved passage is in the viewport and the restoration notice’s Top control works. | Use the large book to check the loading status, a flash of the beginning, a second jump and the number of notices. |
| 3. Open a search result | Opening the named Keyword result brings its off-screen target into the viewport. The journey then bookmarks that target. | Search away from an existing bookmark, use Return to bookmark, change the font or size, search again and clear the mark. |

Run reader check 2 in full. Keep format-specific, playback, accessibility,
installation and device checks on their listed platforms. If a smoke leg fails,
open its command log and read the step and exception in `failure.txt` when
available. For UI failures, inspect the screenshot and visible-text capture
alongside the exception. For setup failures, inspect the failed Actions step.

## Reader restoration and search

1. Save a bookmark well into a large book, close it, and reopen from Library.
   Expect the restoring status, then the saved passage without a second jump or
   a flash of the beginning. Confirm a single restoration notice and usable controls.
2. Close and reopen the same book again within 20 seconds. Its restore must finish
   normally without an earlier open's timer revealing it prematurely. Also open a
   book without a bookmark, then restore a late-page PDF bookmark.
3. Open a search result away from the bookmark. Confirm the result opens at its
   target; Return to bookmark restores the saved passage. Repeat after changing
   reader font/size and after another search, including clearing the search mark.
   No stuck restoration status or jump to an unrelated passage should occur.

## Playback targets and later commands

1. In the existing large book and an imported older bundle, use Play from here at
   early, middle, and late passages. Repeat from a text selection. Listen for the
   intended passage and confirm highlighting follows it. HTML in-chunk timing is
   approximate; PDF/OCR starts at the resolved chunk boundary. A requested passage
   that cannot be verified must show an unavailable result rather than play from
   the beginning or an unrelated passage.
2. Repeat while already playing. Then issue an ordinary jump to that same chunk;
   it should use the chunk boundary rather than inherit the earlier in-chunk offset.
   Check Backward, Forward, and a jump to another chunk as well.
3. Start Play from here and quickly Stop, then start from the beginning or another
   book. The new playback must not inherit the canceled offset or play old audio.
   Repeat rapid interactions where the controls allow them; exact pending-command
   races are also covered by automated regressions.
4. After sound begins from a proportional start, Pause and Resume. It must continue
   at the paused position. On mobile, background/lock for 20–30 seconds, return,
   and try Pause/Resume and skip. Stop must prevent later ghost playback.

## External imports and visible errors

`DS-002` exercises invalid-file dismissal in Library and the reader, followed
by a valid bundle in the same desktop process. Keep item 2 manual until the
deliberately broken error-notice rehearsal in the [acceptance plan](desktop-smoke-e2e.md#import-journey-acceptance)
is complete. File-manager associations and cold starts still require owner checks.

1. From the OS file manager, open a valid `.papercut-audiobook` with Papercut both
   closed and already running. Where OS multi-open is available, open an EPUB and
   two distinct bundles together. Also open another bundle while an import or
   audiobook operation is busy. Each requested item should finish once, with no
   missing bundle, duplicate work, overlapping import, or stuck busy state.
2. Make a disposable plain-text file with a `.papercut-audiobook` extension. Open
   it while Library is active and again with a reader open. Expect one visible,
   dismissible error on the current surface without forced navigation. A valid
   import must still work afterward. Keep existing valid bundles unchanged.
3. On iOS, alternate valid and invalid bundle imports from local Files and a cloud
   provider for several cycles, then open a normal document. Expect no access
   errors or stuck import. This smoke check can reveal provider failures; it does
   not prove balanced security-scope accounting by itself.

## Keyboard and screen reader

Use the keyboard to open a book, invoke passage playback/Return, and dismiss an
import error. Confirm usable focus, a single status/error announcement, and no
interaction with the hidden book before restoration finishes. Repeat narrow or
large-text layouts used on the target devices.

## Erase all data

Recorded results are under Acceptance in [app-reset.md](app-reset.md). Use a built bundle,
because `tauri dev` can't relaunch, and
disposable data only. Desktop dev and release builds share one profile, so on Linux set
`XDG_DATA_HOME` and `XDG_CACHE_HOME` to temporary folders. Run on Linux, Windows and
macOS, then Android and iOS. Record untested platforms explicitly.

1. Import a PDF with recognized text, an EPUB and a folder. Add a bookmark, save an
   audiobook, install a voice and change the theme. Open Settings > Library & Data > Review Data to
   Erase. Expect a spinner and message until the summary loads.
   Expect correct counts and total size, and a disabled **Erase Everything** button. On
   desktop, expect both folder paths to be correct, selectable and left to right. On
   mobile, expect no paths.
2. Tick the acknowledgement, then choose **Cancel**. Expect Settings to return and
   nothing to change.
3. Start an import, then open the dialog. Repeat with audiobook generation, a voice
   download, semantic indexing and a library transfer. Expect the button to stay disabled
   with a message naming the job.
4. Choose **Transfer Library**, then Back. Expect the erase view again, with the
   transfer note visible.
5. Confirm the erase. Expect a restart on desktop and a reload on mobile, then a one-time
   notice and the starting state: starter documents only, default theme and language, no
   bookmarks, no audiobooks, no installed voices.
6. Open the app-data and cache folders. Expect the listed stores gone and the WebView
   profile intact.
7. Re-import the same file. Expect no stale bookmark. Re-download a voice. Expect audio to
   work.
8. Launch with a file argument (Open With), then erase. Expect the file not to reappear
   after the restart.
9. Keep a file in app data open from another program on Windows, then erase. Expect the
   notice to list the path and later launches not to erase anything further.
10. On AppImage, expect the restart to relaunch the AppImage. On Android and iOS, erase
    during audio playback and expect playback to stop. Then start a job so the erase is
    refused while audio plays, and expect playback to continue.
11. Use the dialog with a keyboard and a screen reader, and in Arabic. Expect focus to
    start away from the destructive button, Escape to be ignored while erasing, and a
    correct right-to-left layout.

## Automated fault coverage and remaining acceptance

Mixed missing/invalid spans, skipped canonical mappings, recovery ordering,
forced synchronous/asynchronous restoration failures, canceled API replacement,
fallback search DOM mutations, exact queue/command races, and sub-50 ms clip
clamping have automated regressions. They do not require damaged user books or
hand-edited metadata for this smoke pass.

Record failures with reproduction steps and exported diagnostics before further
changes. This checklist closes the review-fix smoke pass only; broader performance,
platform, accessibility, and release gates in [play-anywhere.md](play-anywhere.md)
remain subject to their own acceptance evidence.

## Settings layout

Check Settings with large text, landscape, RTL, and TalkBack on Android; verify
destination names, focus, scrolling, and touch targets. On desktop, check Zoom and
Reset alignment. Verify search consent/status, transfer Back, and erase Cancel.
Local browser checks used mocked native calls; installed-device acceptance remains
pending.

## Temporary npm audit exception

The CI npm audit checks the full dependency tree. Until December 1, 2026 at
00:00 UTC, the gate accepts only
[GHSA-fc5q-39xc-vjpf](https://github.com/advisories/GHSA-fc5q-39xc-vjpf) for
the transitive development copy of `picomatch` 4.0.7 and findings caused solely
by it. CI prints the full report. Other high or critical findings still fail
the job. GitHub listed no patched release when the exception was added, and
the production dependency audit reported zero vulnerabilities.

When a compatible patched release ships, update the lockfile, verify
`npm audit --audit-level=high`, and restore the direct audit command in CI.
Remove the temporary checker and its self-test at the same time. If the
advisory is still active at expiry, the gate fails again.
