# Desktop smoke E2E implementation plan

Branch `task/desktop-smoke-e2e`, based on `main` at `d642f241`. The deliverable is one manual pre-PR command for the current Linux checkout and one manual CI run that exercises the same reading journey on Linux and Windows at the selected branch SHA. The host-local command is implemented in Stage 3. Stage 4 adds the manual `desktop-smoke` CI scope.

## 1. Establish the baseline and launch targets

Start with the [owner checklist](bug-squash-smoke-tests.md), [reset and profile notes](app-reset.md), [desktop build](../scripts/desktop/build-desktop.js), [single-instance handler](../src-tauri/src/lib.rs), and [CI build jobs](../.github/workflows/ci.yml). Main has frontend component and logic tests, eight optional Chromium fixture suites, Rust native and IPC regressions, and platform packaging jobs. Reuse those layers for edge cases. The missing evidence is a completed task in the built desktop WebView.

The launch chain is a Node smoke entry point → standalone WebdriverIO client → official tauri-driver → platform WebDriver → built Papercut. Keep the client a development dependency. Use a loopback connection and regular UI actions. Keep IPC, indexing, navigation, and persistence real. Use the existing desktop build settings and bundled frontend. [Tauri driver guide](https://v2.tauri.app/develop/tests/webdriver/)

**Acceptance.** Identify the actual launch target and prerequisites for each platform, record available local prerequisites, and identify where the second platform will run. Record a successful launch only after observing Library in a driver session.

**Baseline evidence.** The Linux checkout contains a previously built 1.10.2 amd64 AppImage and has dbus-run-session. tauri-driver, WebKitWebDriver, Xvfb and xvfb-run were absent from PATH during preflight. WebdriverIO was absent from node_modules. The old AppImage has unverified provenance. The Windows target is the release executable with its build-produced resources and DLLs on a disposable Windows runner. The Stage 2 results below supersede this initial prerequisite inventory.

## 2. Prove isolated launch and restart

1. Build with `npm run desktop`. Record the artifact path from this build. Reject missing or ambiguous candidates. Avoid selecting an old versioned AppImage by modification time.
2. On Linux run the driver and app together inside a private D-Bus session and Xvfb display. Allocate a new temporary directory per run, with private runtime, data, config, cache and state directories. Set the corresponding XDG variables for the entire driver process tree. Use the system WebKitWebDriver matching WebKitGTK. Verify WebView storage and single-instance ownership are isolated even with ordinary Papercut running. Launch the AppImage directly where supported. If the CI runner cannot mount it, trial the packaged AppRun from the extracted image and record which path actually ran.
3. On Windows use a fresh hosted runner or disposable user/VM with WebView2 and a compatible msedgedriver. Run the release executable from its prepared resource layout. Establish driver compatibility against the installed WebView2 runtime. Do not use a maintainer's ordinary profile for this trial.
4. Open Library, close the first session, confirm the app process exited, then create a second session against the same artifact and disposable profile. Track the owned process tree, including any AppImage launcher. Bound startup and shutdown. On failure preserve diagnostics and clean up only processes and directories owned by this run.

**Acceptance.** On each OS record artifact, OS/WebView and driver versions, two distinct app process lifetimes, and Library visible in both sessions. On Linux verify an existing ordinary instance retains its library and focus. Record a platform-specific blocker if launch or restart fails.

### Stage 2 evidence, 2026-11-24

**Linux passed using extracted AppRun.** On CachyOS x86-64, a fresh successful `npm run desktop` at `d642f241` produced `Papercut_1.10.2_amd64.AppImage`, SHA-256 `c69d403b31d6eda6766a3c08a246dafe2050dc34fcb5d8d5af5eb6baff500cf4`. Direct launch reached a FUSE mount denial on this host. Extracting that same image with `--appimage-extract` and targeting `squashfs-root/AppRun` preserved the packaged frontend, runtime and launch hooks.

The temporary probe used WebdriverIO 9.32.0, official tauri-driver 2.1.0, WebKitGTK 4.1 at 2.54.3, matching WebKitWebDriver from Arch's webkitgtk-6.0 2.54.3-1 package, and Xvfb 21.1.26. Driver packages were extracted into temporary storage. Launch the whole driver tree inside `dbus-run-session` and `xvfb-run -a`, set `GDK_BACKEND=x11`, clear inherited Wayland and session bus addresses, and give HOME plus all five XDG locations fresh private directories. Keep these directories across both WebDriver sessions. The probe clicked Library and checked the visible tab's selected state, associated its X window with the app PID, deleted the session, and polled for process exit. PIDs 51971 and 52150 both exited.

A second run exercised coexistence with an ordinary, non-automated instance on a separate disposable desktop. It opened a sentinel document through the normal launch argument. Across both smoke cycles (PIDs 52898 and 53079), the control process stayed alive, the control window had the same focus at the end, and the imported source file list and SHA-256 matched before and after. The control sentinel was absent from the smoke app's data. Preserve this isolation when implementing the permanent runner.

**Windows passed on a fresh GitHub-hosted Windows Server 2022 runner.** The temporary probe branch at `8285a50978da9c60eb074c2146d801dffeb1c073` adds only the probe and workflow to the same application revision. The job built Papercut, launched `src-tauri/target/release/app.exe` in its build layout, and used WebdriverIO 9.32.0, tauri-driver 2.1.0 and msedgedriver 131.0.2903.86 matched to the installed WebView2 runtime. Each session selected Library. The probe obtained PIDs 6648 and 1952 by matching the full executable path and confirmed each process exited after session deletion. Both launches shared the disposable runner's user profile. [Windows job and logs](https://github.com/muhannadnouri/papercut.io/actions/runs/45513090361/job/138865532028) · [Probe source](https://github.com/muhannadnouri/papercut.io/blob/8285a50978da9c60eb074c2146d801dffeb1c073/scripts/desktop/stage2-probe.mjs)

**Stage 2 accepted.** Carry the proven session and process checks into Stage 3. Treat extracted AppRun as a supported Linux test target when FUSE is unavailable. Windows evidence covers the built executable. The next stage supplies the first behavioral assertion about persisted user data.

## 3. Implement one host-local entry point and one reading journey

1. Add `npm run smoke:desktop` in [package.json](../package.json) and a small runner under [scripts/desktop](../scripts/desktop). Build the current checkout by default. Give CI a way to pass an explicitly identified artifact from its own build step. Factor only platform provisioning and lifecycle out of the shared journey.
2. Through Library → Paste Text, save a synthetic titled document. Put a unique phrase deep in a later paragraph, several viewports below the beginning at a controlled window size. Use Keyword search, wait for the named result, then open it.
3. Assert both the correct reader title and the target text's bounding rectangle intersecting the visible reader content area. Measure a text range in the reader when necessary. A document-wide text match or a tall container intersecting the viewport can pass at page one. Avoid assertion helpers that scroll the target into view.
4. Bookmark the passage through the UI. Quit and restart as in stage 2. Reopen the book from Library and observe the saved passage after restoration finishes. Check reader controls are usable. Use accessible names, bounded condition waits and diagnostic output for timeouts.
5. Exit nonzero for setup failures, incomplete journeys and assertion failures. Report checkout SHA and dirty state, artifact, stage and durations. Collect driver/app output and a screenshot plus visible reader state on UI failure, before teardown. Keep evidence outside the disposable profile.

**Acceptance.** One shared spec completes from an empty profile on both platforms. Its two viewport assertions distinguish successful search navigation and durable bookmark restoration. A missing driver or an app that never reaches Library gives a useful nonzero result.

### Running the reading journey

For desktop smoke, use Node 22.19 or later (WebdriverIO 10.1.0), run `npm ci`, and install the normal desktop build prerequisites. Use an English desktop for the smoke fixture. Put the official `tauri-driver` (tested with 2.1.0) and the platform tools on PATH before starting.

- Linux requires WebKitWebDriver compatible with the app’s WebKitGTK, dbus-run-session, xvfb-run, Xvfb, xauth and xdotool. The launcher builds the AppImage and extracts it under a new private profile. Ordinary Papercut can remain open.
- Windows requires a fresh disposable account or a fresh GitHub-hosted runner, WebView2, and msedgedriver matching the installed runtime. For a disposable local account set `$env:PAPERCUT_DISPOSABLE_USER = "1"` in PowerShell. Dispose of the account or VM after the run. Native app data uses that account’s known folders. WebView2 uses a temporary user data folder shared by both launches.

From the checkout:

```sh
npm run smoke:desktop
```

To run just the reading case, use `npm run smoke:desktop -- --case DS-001`. Case IDs are case-sensitive. An unknown ID stops the command before it builds. The permanent CI dispatch runs the full suite once per OS. Use the focused command locally to diagnose one case. To exercise a specific build already produced by the caller, use `npm run smoke:desktop -- --artifact PATH`. Pass the Linux AppImage or the Windows app.exe with its resources and DLLs in place. The supplied artifact's provenance is the caller's responsibility.

The terminal records the checkout revision, dirty state, artifact, app PIDs, current step and timings. Each case prints its report directory before launching the driver, for example `papercut-smoke-report-…/DS-001/`. Driver output is in `driver.log`. On UI failure the worker attempts to capture `failure.png` and `visible.txt` before it closes the session. See `failure.txt` for the step and exception. Profiles and reports are separate so the report survives cleanup.

The fixture checks that real keyboard input retained its paragraph breaks and that the unique word starts more than two viewports below the top. On a fresh profile, semantic search is off and the Keyword tab is implicit. The journey explicitly chooses **All Words**, opens the named result and checks the midpoint of the word against the visible reader. After restart it opens **Library → Documents**, filters by title, and checks the same word before using **Top** from the restoration notice. This final action verifies usable controls and a return to the beginning.

### Stage 3 evidence, 2026-11-24

- Linux passed the default build-and-run command on CachyOS with local Stage 3 changes over `9910d881`. The two Papercut processes were 78860 and 79062. One warm run took 133.3 seconds end to end, including 121.7 seconds to build and locate the image and 7.5 seconds in the driver journey. The explicit-artifact run also passed. A missing-driver rehearsal exited nonzero with `spawn tauri-driver ENOENT`; a failed UI selector during development produced the screenshot, text and exception report.
- WebdriverIO 10.1.0 passed the project's npm audit gate with zero high or critical findings. The Stage 2 trial version 9.32.0 introduced blocking transitive findings. The project uses its existing audit policy.
- Windows Server 2022 passed on temporary probe revision `82ec0eb5`, using matching WebView2/EdgeDriver 131.0.2903.86. Earlier runs found the document after restart but failed the passage assertion. The passing implementation supplies the same disposable `webviewOptions.userDataFolder` to both sessions, requests a normal Windows window close, then waits for the app to exit. The two PIDs were 4952 and 5540. Build/preflight took 631.7 seconds, the journey took 26.9 seconds, and the command took 659.2 seconds overall. The final shared worker also passed on Linux. [Windows job and logs](https://github.com/muhannadnouri/papercut.io/actions/runs/45518706951/job/138884761338) · [Microsoft Edge WebView2 capabilities](https://learn.microsoft.com/en-us/microsoft-edge/webdriver/capabilities-edge-options#webviewoptions-object)

**Stage 3 accepted.** The next stage exposes the host command through the permanent manual CI scope. Checklist retirement follows the known-bad rehearsals in Stage 5.

## 4. Expose manual branch validation

Extend the existing CI workflow's dispatch choices and change-detection logic with `desktop-smoke`. Schedule only the relevant Linux and Windows desktop smoke builds for that scope. Reuse build prerequisites and caches, build once per platform, and feed the exact resulting artifact to stage 3. Keep both results when one fails. Upload bounded failure evidence with the tested SHA. Preserve existing push, PR and dispatch behavior for other scopes.

**Acceptance.** Manually dispatch the existing workflow on a pushed feature branch before opening its PR. Verify the selected SHA on both legs, successful report collection, and independent failure reporting. Record the run URL. Ordinary frontend changes need no automatic desktop smoke run.

### Run on a pushed branch

Choose a branch containing the Stage 4 workflow. From an authenticated GitHub CLI, replace `BRANCH` with its remote name:

```sh
gh workflow run ci.yml --repo muhannadnouri/papercut.io --ref BRANCH -f validation_scope=desktop-smoke
gh run list --repo muhannadnouri/papercut.io --workflow ci.yml --branch BRANCH --event workflow_dispatch --limit 3
gh run watch RUN_ID --repo muhannadnouri/papercut.io --exit-status
```

Compare the run’s head SHA with the commit you intend to submit. After the routing job, Linux and Windows each build once and execute the local smoke command. Both jobs can finish independently. Download the seven-day evidence from the run’s **Artifacts** section, or use `gh run download RUN_ID --repo muhannadnouri/papercut.io --pattern 'desktop-smoke-*' --dir smoke-evidence`. The artifact names include the OS and full SHA. Look in `desktop-smoke.log` for the last step and the report directory for driver output and available failure details. For a provisioning error before the command starts, use the failed step’s Actions log.

**Stage 4 verification.** The manual dispatch tested clean revision `24f61427` on Ubuntu 24.04 and Windows Server 2022. Both jobs built Papercut and restored the saved passage in a new process. The Linux command took 7m 05s including a 14.1s journey. The Windows command took 12m 43s including a 23.0s journey. Both downloaded artifacts contained the command log and driver log, with the expected SHA and separate app PIDs in the command log. The run scheduled the routing job and the two smoke legs. Local checks also verified routing for all five previous dispatch scopes and the schedule. Stage 5 records per-platform failure evidence from the same matrix. [Manual run and artifacts](https://github.com/muhannadnouri/papercut.io/actions/runs/45522343407).

## 5. Prove the checks catch regressions

On a disposable trial revision, force search-result navigation to land at the beginning. Repeat separately with bookmark restoration disabled. Run both controls on both platforms. Remove the trial changes, rebuild the good revision, and repeat ten warm journeys per OS without retries using fresh profiles. Separate cold build, warm build and journey timings.

**Acceptance.** The first defect fails at the search passage check and the second fails only after relaunch. Capture representative failure evidence. Record ten consecutive clean passes per OS, timings, flakiness observed and any limits before promoting the journey to manual-checklist coverage.

### Rehearsal evidence

The fault revisions each changed one application behavior over Stage 4 (`7aabf191`). Both used the committed smoke command and assertions. The following failures were expected and verified against the downloaded platform artifacts.

| Disposable revision | Change | Observed on Ubuntu 24.04 and Windows Server 2022 |
| --- | --- | --- |
| [Search control, 538478dc](https://github.com/muhannadnouri/papercut.io/actions/runs/45525109464) | Drop the reader target when opening the named search result. | The first app process opened the document. The viewport assertion failed at `check search destination`, before bookmark save. |
| [Bookmark control, 8abcb975](https://github.com/muhannadnouri/papercut.io/actions/runs/45525116146) | Skip the saved-location seek while letting restoration report completion. | Search navigation, bookmark save and process exit succeeded. A second process reopened the book from Library. The viewport assertion failed at `check restored bookmark`. |

All four failure reports contained `Expected the target word in the visible reader passage`. Each had a screenshot and a text dump containing the target word. The failing observation was its position on screen.

The [clean control, 69e5be3d](https://github.com/muhannadnouri/papercut.io/actions/runs/45525122922) used the Stage 4 application and smoke runner. On each fresh hosted VM it built once without a restored Rust cache, rebuilt immediately against the same target directory, then ran ten consecutive journeys against the warm build. The Linux target was the AppImage and the Windows target was the release executable in its build layout. Each repetition used a fresh profile shared across its two launches. On Windows the trial archived the native stores created by the previous repetition before starting the next. The artifact SHA-256 matched before and after the ten runs.

| Platform | Cold build | Warm build | Journey median (range) | Consecutive passes |
| --- | ---: | ---: | ---: | ---: |
| Ubuntu 24.04 | 10m 06s | 4m 11s | 26.1s (25.7–26.9s) | 10/10 |
| Windows Server 2022 | 12m 09s | 4m 11s | 12.8s (12.5–19.2s) | 10/10 |

**Stage 5 accepted.** All twenty clean journeys completed without retries. Each OS produced ten separate reports and twenty app process launches. Build times are one observation per condition and exclude runner provisioning. Journey times measure the driver and UI portion after build and artifact preparation. This sample covers the pasted-text workflow on these two desktop environments. Stage 6 can retire the corresponding manual assertions for a build with a passing smoke result.

## 6. Hand off the workflow and retire matching checks

Document the proven host command, OS prerequisites, artifact selection, profile isolation, manual branch dispatch and evidence retrieval in the developer docs. In the owner checklist link only the assertions demonstrated for the tested format and platforms. Preserve separate evidence for PDF restoration, transient visual jumps, installed file associations, audible playback, screen-reader output and mobile lifecycle.

**Acceptance.** An owner can run the Linux check from an ordinary shell, obtain Linux and Windows results for one pushed branch SHA, and diagnose a failed step without repeating the whole checklist. Record which checklist portions can be skipped for that build. Next candidate after MVP review is invalid import → dismiss visible error → successfully import a valid document.

**Stage 6 accepted.** The [owner checklist](bug-squash-smoke-tests.md#desktop-smoke-coverage) maps a passing build to the two reader observations it covers and identifies the manual work still required. This completes the desktop reading MVP. The next proposed journey covers [invalid bundle import and recovery](bug-squash-smoke-tests.md#external-imports-and-visible-errors), item 2.

## 7. Specify the external import recovery journey

**Proposed scope for review.** Cover item 2 of the [external import checklist](bug-squash-smoke-tests.md#external-imports-and-visible-errors) on Linux and Windows. Use a valid audiobook bundle for recovery so both attempts exercise the same native importer. The valid-bundle handoff is the first DS-002 checkpoint. Invalid-file dismissal and recovery remain separate acceptance steps.

### Case files and reporting

Keep one independently runnable user journey per file in `scripts/desktop/cases/`. Keep dependent steps together. Saving a bookmark and restoring it after restart share a profile by design. The import case creates its own reader document. This follows WebdriverIO’s guidance on focused specs and Selenium’s guidance on isolated test data. [WebdriverIO suite organization](https://webdriver.io/docs/organizingsuites) · [Selenium test isolation](https://www.selenium.dev/documentation/test_practices/encouraged/avoid_sharing_state/)

| Stable case ID | Case file | User-visible outcome |
| --- | --- | --- |
| `DS-001` | `DS-001-reading-bookmark.mjs` | Find an off-screen passage and restore its bookmark after restarting Papercut. |
| `DS-002` | `DS-002-bundle-import-recovery.mjs` | Open a valid bundle in the running reader. Planned extension: recover from an invalid bundle first. |

Use the filename prefix as the case ID in the owner checklist, log lines and per-case report directories. Keep the ID when wording changes, never recycle it, and reject duplicate IDs. Keep fixture content, selectors and outcome assertions with their case. Keep artifact preparation, session lifecycle, process cleanup and failure capture in the shared runner. Add function comments where platform constraints or fixture intent need explanation.

**First implementation checkpoint.** Extract the existing reading case and run it alone on Linux and Windows. Preserve its real restart, viewport assertion and failure diagnostics. Keep `npm run smoke:desktop` as the full-suite command and add `npm run smoke:desktop -- --case DS-001` for a focused run. An unknown ID must fail before building. Accept the split when both entry points identify the case and produce attributable results.

**Case split accepted.** At verification revision `291779fb`, Ubuntu 24.04 and Windows Server 2022 each passed both the full suite and the focused DS-001 command. Each downloaded artifact contained the case driver log and a command log showing two distinct app PIDs and the restored-bookmark check. Only the temporary verification workflow duplicated the OS jobs. The final scripts received lint formatting and matched the probe scripts after syntax normalization. Both commands also passed against the local Linux AppImage. A deliberate failure in a disposable Linux copy recorded the case ID and search step, screenshot, visible text and driver log. CLI regression tests cover an empty suite, unknown ID and duplicate ID before build. [Four-job verification run](https://github.com/muhannadnouri/papercut.io/actions/runs/45528810475)

### Running independent cases

The manual CI matrix runs Linux and Windows concurrently. Within each OS job the runner builds once, gives each case a fresh desktop profile, and runs cases in sequence. On the disposable Windows account it moves the preceding case’s native stores aside in their original Known Folders before the next case, and uses a fresh WebView2 folder. Dispose of the account after the run. A local change to import handling can be checked with `npm run smoke:desktop -- --case DS-002 --artifact PATH`.

DS-002 launches the executable with one file argument while the original app stays open. The second invocation forwards the file and exits. The existing WebDriver session then reads the expected passage and checks the original app PID. These observations depend on sharing the single-instance endpoint within the case.

If cumulative journey time starts to dominate a pre-PR run, split independent case IDs across isolated workers. Build once per OS and distribute the same artifact to its workers, including the executable’s adjacent resources and DLLs on Windows. Each Linux worker needs its own D-Bus session, display and profile. Use a separate disposable Windows VM or account with an isolated interactive desktop per worker. Give each worker its own driver ports and evidence paths. Keep each case’s steps on one worker. Compare total wall time and runner minutes, including artifact transfer and provisioning, before adopting the split. [WebdriverIO suite organization](https://webdriver.io/docs/organizingsuites)

### Import journey acceptance

1. **Prepare disposable inputs.** Place a plain-text file with the `.papercut-audiobook` suffix and a tiny valid bundle in the smoke workspace. Prefer a synthetic export containing a short passage, a unique title and a known phrase. Keep the fixture reproducible and free of user content, model downloads and playback dependencies. Before testing recovery, confirm the valid fixture alone opens to the expected passage in a fresh smoke profile.
2. **Deliver files through the desktop entry point.** Keep the WebDriver-controlled app open. From the isolated worker, launch the same artifact with the absolute fixture path as one argument. The single-instance callback in `src-tauri/src/lib.rs` forwards it to the running app. In `src/App.tsx`, `useOpenDocumentRequests` routes bundle paths to the audiobook importer. Reuse the Linux worker’s private D-Bus session and the Windows disposable-user checks. Accept delivery when the second invocation exits and the original app PID and WebDriver session handle the UI response.
3. **Prove the error on both surfaces.** From Library, deliver the invalid file, wait for one visible import error, dismiss it using its visible control and confirm Library is still active and usable. Open a known document and repeat from its reader. Confirm the same document remains visible before and after dismissal. Assert the error disappears on each dismissal. Use bounded UI waits and record the failing surface and step.
4. **Prove recovery in the same session.** Immediately after the reader dismissal, deliver the valid bundle through the same path. Require its expected passage to appear in the reader and its unique document title to appear once on return to Library. Keep the same app process and profile across the invalid and valid deliveries so a restart cannot clear a stuck import.
5. **Verify before retiring manual work.** Reuse the existing manual `desktop-smoke` matrix, build each OS artifact once, and retain separate step evidence for reading and import. Require both journeys to pass at the same candidate SHA on Linux and Windows. On a disposable revision suppress the visible import error and confirm the new assertion fails on each platform. Record run links, failed steps and clean results before updating item 2. Installed file associations, cold starts, busy/multi-file delivery, audible playback and mobile provider behavior retain their existing owner checks.

**Valid delivery accepted.** On revision `0fd7109f` both hosted jobs built once and passed DS-001 followed by DS-002. The second executable invocation exited successfully and the original WebDriver session displayed the exact fixture paragraph. Each downloaded report confirmed the original PID and included driver logs for both cases. [Linux and Windows verification](https://github.com/muhannadnouri/papercut.io/actions/runs/45532606012)

| Platform | Build and preflight | DS-001 journey | DS-002 journey | Original PID confirmed after delivery |
| --- | ---: | ---: | ---: | ---: |
| Ubuntu 24.04 | 7m 47s | 16.79s | 1.45s | 25934 |
| Windows Server 2022 | 8m 17s | 18.13s | 3.36s | 4900 |

The [fixture generator](../scripts/fixtures/desktop-smoke/generate.py) reproduces the checked-in 4,218-byte HTML audiobook with a single silent WAV chunk. The smoke command uses the checked-in bundle directly. Locally, a disposable Linux control launched the executable without its file argument. The reader assertion timed out and produced a screenshot and text dump. An earlier hosted Linux run passed delivery, then encountered a document-portal FUSE mount during cleanup. The runner now confines removal to the profile filesystem and retries briefly as the mount clears.

**Next checkpoint.** Extend DS-002 with the Library and reader error-dismissal checks and same-session recovery (steps 3–4). Keep the manual error-recovery check until the clean and deliberately failing rehearsals in step 5.
