# Settings layout

App Settings uses a flat list with no accordions. Secondary destinations open
existing task views; preference controls apply their changes immediately.

| Group | Contents |
| --- | --- |
| Preferences | Language, Theme, and desktop-only Zoom |
| Search (native builds) | Semantic Search (Experimental), with the observed status beneath its name |
| Library & Data (native builds) | Transfer Library, followed by a separated, red Review Data to Erase row |
| Developer Mode | One directly operable labeled toggle, without a repeated heading |
| About | Legal & Notices and Version |

The whole destination row is clickable. Chevrons indicate another view and mirror
in RTL. Non-destructive destination labels and chevrons use the accent color;
section headings stay neutral, search status stays muted, and erase review stays
red. The search destination has a stable accessible name and a separate changing
status description; opening it does not activate, download, or index anything.
Erase review still requires its existing summary and acknowledgement, refuses
active jobs, and explains the limits of transfer backups. See [app-reset.md](app-reset.md).

Preference labels and controls share a row when space permits, then wrap using
CSS flex layout rather than a fixed phone-width breakpoint. Selected language text
can wrap. The compact Zoom control shares the starting edge of Language and
Theme; its Reset action wraps within that same control column when necessary.
The root settings close control, preference controls, and Settings cog
have a coarse-pointer minimum of 3rem; destination and developer rows also have a
3rem minimum. At a 16px root size this is 48 CSS pixels, not a verified physical
48dp measurement on Android. The existing shared navigation header stays pinned
and the body scrolls. Version is not guaranteed to remain above the fold with
large text, translations, or landscape orientation.

## Rationale

The November 16, 2026 review applied NN/g's
[proximity](https://www.nngroup.com/articles/gestalt-proximity/),
[visual hierarchy](https://www.nngroup.com/articles/principles-visual-design/),
[progressive disclosure](https://www.nngroup.com/articles/progressive-disclosure/),
and [touch target](https://www.nngroup.com/articles/touch-target-size/) guidance.
Existing task views provide disclosure without adding accordions. Full-width tap
areas remain useful; redundant headings, isolated actions, and forced stacking
were the sources of avoidable height. Android's
[48dp guidance](https://support.google.com/accessibility/android/answer/7101858?hl=en)
informs the touch-target check, with installed-device validation still required.

## Local validation

- 86 focused tests pass across Settings, shared dialogs, erase review, and transfer.
  Added DOM regressions exercise search naming/status, opening without preparation,
  Back/Close, related destinations, version placement, and developer label activation.
- Locale parity, TypeScript, touched-file ESLint, production Vite/Pagefind build,
  and whitespace checks pass. Existing bundling warnings remain.
- A temporary real-component Chromium fixture with mocked native IPC checks all
  eight locales at 320px and 393px widths with 16px, 24px, and 32px root text sizes:
  48 combinations, no horizontal body/dialog overflow, and root targets at least
  48 CSS pixels tall. Arabic dark large text retains the pinned header on scroll.
- At 393 × 852 CSS pixels with default English text and touch input, the old body
  was 693px tall in a 681px scroll area; the new body is 602px tall and fits.
  This saves 91px in that fixture, not a measurement on a physical Pixel phone.
- Browser checks cover search/legal/erase Back and Close, developer label activation,
  focus return, disabled erase before acknowledgement, no preparation/erase requests,
  desktop Zoom visibility, and browser-only Settings. No real transfer, download, or deletion was run.

## Alignment and destination-color follow-up

The follow-up changes only root destination color and the Zoom control's layout
container. Shared task buttons, navigation, zoom limits/storage, and native behavior
are unchanged. No locale changes or dependencies are needed.

- 14 existing Settings tests, touched-file ESLint, locale parity, TypeScript,
  production Vite/Pagefind build, and whitespace checks pass.
- A real-component Chromium fixture with mocked native IPC checks 64 combinations:
  four widths (320/393/800/1280px), English/Arabic/French/Portuguese, light/dark,
  and Zoom at 100%/110%. Language, Theme, and compact Zoom share their starting edge
  within 1 CSS pixel, including RTL and the visible Reset action; no horizontal
  body overflow appears. The zoom widget remains narrower than its control column.
- Computed colors confirm accent destination labels/chevrons, neutral section
  headings, muted search status, and red erase review. Keyboard focus remains visible;
  Android-user-agent preview still hides Zoom. Desktop and mobile previews were
  visually inspected. Actual WebView scaling and device acceptance remain pending.

## Next stage: installed-device acceptance

- [ ] On a Pixel-sized Android device, check default and largest font/display sizes,
  portrait/landscape, day/night themes, RTL, scroll reachability, and safe-area spacing.
- [ ] With TalkBack, check destination names/status descriptions, Developer Mode,
  focus order, Back/Close, and touch target bounds using Accessibility Scanner.
- [ ] Verify actual semantic status transitions and consent remain correct, transfer
  returns with Back, and erase review can be cancelled without changing data.
- [ ] Check desktop Zoom/change/reset and browser-only Settings. Run normal PR CI.

Record device/build versions and actual results before marking these complete.
