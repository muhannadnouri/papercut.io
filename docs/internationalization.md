# UI Internationalization

Papercut's UI localization is independent from the selected TTS language and
from the language of an open document.

## Current Stage

- English (`en`) and Arabic (`ar`) locale resources are bundled with the app.
- The first launch uses the first supported browser or operating-system
  language, then stores the user's explicit choice in local storage.
- App Settings, the header subtitle, and primary navigation are translated.
- Arabic sets the app shell to `dir="rtl"` and uses the bundled Readex Pro font.
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
keys as English and runs automatically during `npm run build`.

## Remaining Stages

- [ ] Migrate shared dialogs and common actions.
- [ ] Migrate Search and Library, including plural messages and locale-aware sorting.
- [ ] Preserve imported HTML/EPUB language and direction in the reader.
- [ ] Complete the RTL CSS and bidirectional-content audit.
- [ ] Migrate audiobook and TTS status text.
- [ ] Replace expected native English error strings with stable error codes.
- [ ] Add Spanish, French, Italian, Brazilian Portuguese, Hindi, and Simplified Chinese.
- [ ] Complete native-speaker and desktop/mobile visual review.
