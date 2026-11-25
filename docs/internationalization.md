# UI Internationalization

Papercut's UI localization is independent from the selected TTS language and
from the language of an open document.

## Current Stage

- English (`en`), Arabic (`ar`), Spanish (`es`), French (`fr`), Italian
  (`it`), Brazilian Portuguese (`pt-BR`), Hindi (`hi`), and Simplified Chinese
  (`zh-CN`) locale resources are bundled with the app.
- Spanish, French, Italian, Brazilian Portuguese, Hindi, and Simplified
  Chinese are marked experimental in App Settings. Their first-pass
  translations must receive native-speaker review before that marker is
  removed.
- The first launch uses the first supported browser or operating-system
  language, then stores the user's explicit choice in local storage.
- App Settings, the header subtitle, primary navigation, shared confirmation
  defaults, text-input actions, and the external-link prompt are translated.
- Search and Library are translated, including import/delete status messages,
  folder organization, accessible action labels, and localized plural forms.
- Audiobook setup, save/playback controls, queue and library actions,
  confirmation dialogs, and ordinary progress messages are translated.
  Model names, voice names, persisted identifiers, and raw native or diagnostic
  errors remain unchanged.
- Reader chrome is translated, including Back, Find, reader appearance
  settings, document loading/error states, bookmark actions, and the
  in-document Find bar.
- Native TTS commands return a stable error code with the original diagnostic
  message. React translates known user-actionable codes while diagnostics keep
  the original message and unknown failures remain visible for troubleshooting.
- Document and folder tie-break sorting uses the active UI locale. Search
  relevance and explicit library `sortOrder` values remain authoritative.
- Arabic sets the app shell to `dir="rtl"`, uses the bundled Readex Pro font for
  the interface, and defaults reflowable reader content to Readex Pro. An
  explicit reader font choice remains unchanged when the interface language changes.
- Imported HTML keeps valid root/body `lang` and `dir` metadata on the reader
  surface. EPUB imports preserve package language/direction and chapter
  overrides in the generated reading document.
- Document direction remains independent from the app shell. Content without
  explicit direction uses native `dir="auto"` detection.
- Shared panels, popovers, selectors, and text alignment use logical CSS
  properties. Navigation arrows mirror in RTL, while playback transport
  controls and their collision-avoidance placement remain physically stable.
- User-authored titles and free-form status text use `bdi` or `dir="auto"`.
  URLs, filesystem paths, JSON, and raw diagnostics remain explicitly LTR.
- Unmigrated feature screens still fall back to their existing English text.

The locale foundation lives in `src/i18n/`. `i18next` owns application
messages and fallback behavior; React Aria's `I18nProvider` receives the same
locale for direction-aware component behavior.

## Static Website

The static website is generated from one shared
`site/source/index.template.html` file and the locale catalogs under
`site/source/locales/`. Run `node site/build.mjs` after changing the template or
a catalog. The generator and its source live with the static website instead of
the application build. Netlify runs the same generator before publishing
`site/`, while generated pages remain committed for direct `file://` previews:
English is served from `site/index.html`, Arabic from `site/ar/index.html`,
Spanish from `site/es/index.html`, French from `site/fr/index.html`, Italian from
`site/it/index.html`, Brazilian Portuguese from `site/pt-BR/index.html`, Hindi
from `site/hi/index.html`, and Simplified Chinese from
`site/zh-CN/index.html`.

Do not edit the generated HTML pages directly. The renderer rejects missing or
unused catalog values, keeps all locales on the same markup and scripts, and
preserves direct `file://` previews without shipping client-side translation
code. All pages continue to share `site/styles.css`.

Each page declares canonical and reciprocal `hreflang` links, localized social
metadata, and its semantic document direction. The header language menu links
directly between locale URLs, so navigation and metadata do not depend on
client-side translation code.

Website language names do not use the application's experimental labels. Those
labels describe the app locale review status, not whether a visitor can open a
localized marketing page.

### Downloads and FAQs

The build resolves version badges, filenames, archive and installer URLs, and the
release-note link from `site/source/release.mjs`. Catalogs own the translated
download labels and may use `{{release.tag}}` in a versioned description.
Recommended buttons and full platform listings reference the same assets.

Installation instructions for Android, Linux, macOS and Windows live inside
their respective download groups. System requirements and the link to release
notes remain in the setup FAQ. The other FAQ groups cover library and privacy,
voices and languages, and audio rights and sharing.

### Homepage walkthrough

The primary action in the hero and floating navigation links to
`#downloads-title`. The walkthrough keeps the existing `#preview-title` anchor
and provides direct links to `#find`, `#read`, and `#listen`. Keep each
translated example consistent across the search result, reader, and audiobook.
The `walk.before` and `walk.after` strings include the spacing needed around
the highlighted query in that language.

The current product frames are labeled placeholders. The hero and Listen step
depict the same playback state so they can share a capture for each device
size. Keep captions and walkthrough headings in the locale catalogs when the
real captures arrive.

## Translation Rules

- Use semantic keys rather than English sentences as keys.
- Translate complete messages instead of concatenating translated fragments.
- Keep Papercut, model names, voice names, user titles, paths, and raw
  diagnostics unchanged.
- Use `dir="auto"` or `bdi` around user and document text when its direction is
  not known.
- Keep app-shell direction separate from document `lang` and `dir`.
- Prefer CSS logical properties such as `margin-inline-start` and
  `border-inline-start`.
- Navigation arrows mirror in RTL; media playback controls do not.

`npm run check:i18n` verifies that every locale contains the same non-empty
message families as English, including the CLDR plural categories required by
each locale, and runs automatically during `npm run build`.

## Bundled reader fonts

ReaderSettings offers Literata, original Atkinson Hyperlegible, Source Sans 3,
Merriweather, System Serif, System Sans, and the existing Arabic families. Source
Sans 3 and Merriweather are additional choices; the default remains Literata for
non-Arabic locales and Readex Pro for Arabic. Explicit font choices survive reload
and locale changes. PDF page fonts are unchanged. App-wide UI typography is
covered separately under [UI font integration](#ui-font-integration).

The two new families use unmodified upright and italic variable TTFs from Google
Fonts commit
[`7af0d90b4d532ffa4b041937b560c6df4e7b1e95`](https://github.com/google/fonts/tree/7af0d90b4d532ffa4b041937b560c6df4e7b1e95/ofl).
Files and their original OFL notices are under `public/fonts/reader/source-sans-3/`
and `public/fonts/reader/merriweather/`. Both are bundled for offline use. They are
not downloaded from a font service at runtime or subset/converted locally.

| Family | File | SHA-256 |
| --- | --- | --- |
| Source Sans 3 | `SourceSans3[wght].ttf` | `042fe2cc0b933e328410d7acbd0aa6a1873dca5aef81875f4bc214b08825c7b9` |
| Source Sans 3 | `SourceSans3-Italic[wght].ttf` | `39e3ab05ccd7cb94907c31005bb5bec1d5432f0b096a2b782976e217a540eb6c` |
| Merriweather | `Merriweather[opsz,wdth,wght].ttf` | `d0ed0e359e396af7ad05e73dffd11a3a4c326ea0d0283c56bd9361cb2cc86a96` |
| Merriweather | `Merriweather-Italic[opsz,wdth,wght].ttf` | `f68a8f4989258679e4fbaf50aa42400132b5373c2d9d2514ba82ef6e85947a0b` |

The TTF pairs add 1,041,712 bytes for Source Sans 3 and 9,219,248 bytes for
Merriweather (10,260,960 total), plus licenses. These are raw asset sizes, not
installer-size increases. Source Sans 3 exposes weights 200–900; Merriweather
exposes 300–900, with its existing upstream optical-size/width axes retained.
Normal and italic CSS faces declare their supported weight ranges. No weight,
width, or optical-size control has been added to ReaderSettings.

The new Latin families do not provide Arabic, Hindi, or Chinese glyph coverage;
unsupported characters continue through the existing CSS/platform fallback. They
are explicit choices, not new universal fallback families. Atkinson Hyperlegible
Next and the other research candidates are not included.

Font copyright and OFL text are included in `THIRD_PARTY_NOTICES.md` and the
packaged Android notices. The Android notice generator's explicit family list
includes both additions. The font licenses remain separate from Papercut's MIT
source license; the existing Android combined-work licensing statement is unchanged.
The source OFL files and generated excerpts retain upstream whitespace verbatim.

### Validation and remaining acceptance

- The earlier offline specimen comparison passed 48 size/matching/viewport cases.
  That was an emulation check, not proof of physical-panel comfort.
- All 15 focused ReaderSettings and notice-helper tests pass; new regressions
  cover both new stored font values across locale defaults. Locale parity, TypeScript,
  touched-file ESLint, and the production Vite/Pagefind build pass.
- Font binaries and licenses match the pinned comparison hashes, and the built
  `dist/fonts/reader/` copies retain the same bytes. Packaged notices are generated
  with the existing tool and include the new font copyright/license blocks only.
- A real-component Chromium fixture verifies both selectors, actual custom-font
  rendering for normal/bold/italic text, saved choices after reload and locale
  changes, default restoration on Reset, narrow 320px RTL layout, and stable text
  anchor nodes. Local font requests succeed (including normal cached responses),
  with no page errors. This does not certify native bookmark/playback behavior.
- [ ] Review normal/bold/italic text on actual low-density desktop and small
  high-density mobile displays, including text scaling, mixed-script fallback,
  night themes, and long passages.
- [ ] In the installed app, verify reflow, existing bookmarks, Find and playback
  highlighting after font switches and reload. Record actual results before
  accepting the release; local browser checks are not native/device acceptance.

## App-wide font preview

A November 17, 2026 trial compares the current system font with Source Sans 3.
It runs a separate production snapshot with a temporary font override. The app's
committed default is unchanged. Arabic retains Readex Pro and reader text retains
its selected font.

The browser check covers Library, Search, Settings, and the reader in both modes:
128 cases across 320/393/1280px, normal/enlarged text, English/Arabic, and light/dark.
No horizontal overflow was found. Source Sans 3 looks more compact at the current
sizes; this is a useful comparison, not proof that it is easier to read.

Before rollout, review both fonts on the actual monitor and phone. Native dialogs
are absent from this browser preview, and Windows/macOS still need testing.

## Remaining Stages

- [x] Migrate shared dialogs and common actions.
- [x] Migrate Search and Library, including plural messages and locale-aware sorting.
- [x] Preserve imported HTML/EPUB language and direction in the reader.
- [x] Complete the RTL CSS and bidirectional-content audit.
- [x] Migrate audiobook and TTS controls and ordinary status text.
- [x] Replace expected native English command errors with stable error codes.
- [x] Add Spanish, French, Italian, Brazilian Portuguese, Hindi, and Simplified Chinese.
- [ ] Complete native-speaker and desktop/mobile visual review.

## UI font integration

Inter is the default non-Arabic UI font, with a Chinese exception. Arabic retains
Readex Pro; Chinese uses the native font stack so curly quotes and Han characters
stay together. Reader fonts are unchanged. Inter is bundled with its OFL notice.

### Owner smoke acceptance — November 17, 2026

The owner reports that all tests in the current typography smoke pass passed on
latest AppImage and Android APK builds. This includes the requested screen/label,
light/dark, scaling/layout, locale, symbol, reader persistence/reflow, bookmark,
Find/playback highlighting, and Android TalkBack checks. An AppImage screenshot
of the Audiobooks tab was supplied; the Android result is owner-reported without
a screenshot. These are owner-tested native results, separate from the prior
Chromium checks. Exact artifact hashes and device/OS versions were not supplied.

The earlier reader acceptance items above are covered by this report to the
extent of the current smoke checklist. It does not complete native-speaker review
for every locale or Windows/macOS acceptance.

The existing font weights remain unchanged. The separate proposal to trial a
lighter Audio Setup title, audio section labels, and dropdown weights has not
been implemented or authorized by this smoke result.
