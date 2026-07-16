# UI Internationalization

Papercut's UI localization is independent from the selected TTS language and
from the language of an open document.

## Current Stage

- English (`en`) and Arabic (`ar`) locale resources are bundled with the app.
- The first launch uses the first supported browser or operating-system
  language, then stores the user's explicit choice in local storage.
- App Settings, the header subtitle, primary navigation, shared confirmation
  defaults, text-input actions, and the external-link prompt are translated.
- Search and Library are translated, including import/delete status messages,
  folder organization, accessible action labels, and localized plural forms.
- Document and folder tie-break sorting uses the active UI locale. Search
  relevance and explicit library `sortOrder` values remain authoritative.
- Arabic sets the app shell to `dir="rtl"` and uses the bundled Readex Pro font.
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
- [ ] Migrate audiobook and TTS status text.
- [ ] Replace expected native English error strings with stable error codes.
- [ ] Add Spanish, French, Italian, Brazilian Portuguese, Hindi, and Simplified Chinese.
- [ ] Complete native-speaker and desktop/mobile visual review.
