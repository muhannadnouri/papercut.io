# Desktop smoke E2E implementation plan

Branch `task/desktop-smoke-e2e`, based on `main` at `d642f241`. The deliverable is one manual pre-PR command for the current Linux checkout and one manual CI run that exercises the same reading journey on Linux and Windows at the selected branch SHA. The commands and CI scope below are proposed until their stages pass.

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

## 4. Expose manual branch validation

Extend the existing CI workflow's dispatch choices and change-detection logic with `desktop-smoke`. Schedule only the relevant Linux and Windows desktop smoke builds for that scope. Reuse build prerequisites and caches, build once per platform, and feed the exact resulting artifact to stage 3. Keep both results when one fails. Upload bounded failure evidence with the tested SHA. Preserve existing push, PR and dispatch behavior for other scopes.

**Acceptance.** Manually dispatch the existing workflow on a pushed feature branch before opening its PR. Verify the selected SHA on both legs, successful report collection, and independent failure reporting. Record the run URL. Ordinary frontend changes need no automatic desktop smoke run.

## 5. Prove the checks catch regressions

On a disposable trial revision, force search-result navigation to land at the beginning. Repeat separately with bookmark restoration disabled. Run both controls on both platforms. Remove the trial changes, rebuild the good revision, and repeat ten warm journeys per OS without retries using fresh profiles. Separate cold build, warm build and journey timings.

**Acceptance.** The first defect fails at the search passage check and the second fails only after relaunch. Capture representative failure evidence. Record ten consecutive clean passes per OS, timings, flakiness observed and any limits before promoting the journey to manual-checklist coverage.

## 6. Hand off the workflow and retire matching checks

Document the proven host command, OS prerequisites, artifact selection, profile isolation, manual branch dispatch and evidence retrieval in the developer docs. In the owner checklist link only the assertions demonstrated for the tested format and platforms. Preserve separate evidence for PDF restoration, transient visual jumps, installed file associations, audible playback, screen-reader output and mobile lifecycle.

**Acceptance.** An owner can run the Linux check from an ordinary shell, obtain Linux and Windows results for one pushed branch SHA, and diagnose a failed step without repeating the whole checklist. Record which checklist portions can be skipped for that build. Next candidate after MVP review is invalid import → dismiss visible error → successfully import a valid document.
