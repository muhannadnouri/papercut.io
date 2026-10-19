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

## Remaining Stages

- [x] Migrate shared dialogs and common actions.
- [x] Migrate Search and Library, including plural messages and locale-aware sorting.
- [x] Preserve imported HTML/EPUB language and direction in the reader.
- [x] Complete the RTL CSS and bidirectional-content audit.
- [x] Migrate audiobook and TTS controls and ordinary status text.
- [x] Replace expected native English command errors with stable error codes.
- [x] Add Spanish, French, Italian, Brazilian Portuguese, Hindi, and Simplified Chinese.
- [ ] Complete native-speaker and desktop/mobile visual review.
