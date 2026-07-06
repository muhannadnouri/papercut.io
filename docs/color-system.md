# Papercut Color System

This palette is a testable baseline for the static landing page and a future reference for the React app theme tokens.

The static site supports system, light, and dark previews. It defaults to the visitor's system preference and lets visitors cycle themes without persisting the choice across refreshes.

## Goals

- Meet WCAG AA contrast for normal text, controls, focus states, and meaningful UI outlines.
- Make controls, menus, inputs, and selectable chips easier to distinguish in light and dark themes.
- Keep Papercut quiet, technical, and document-focused while preserving a clear product accent.

## Palette Direction

Papercut uses a Slate + Indigo palette.

- Slate provides neutral document surfaces, borders, and text.
- Indigo marks primary actions, selected state, and focus affordances.
- Green is reserved for positive status badges.

## Light Tokens

```css
--bg: #f8fafc;
--panel: #ffffff;
--surface-muted: #f1f5f9;
--surface-subtle: #f8fafc;
--surface-selected: #eef2ff;

--text: #0f172a;
--muted: #475569;
--quiet: #64748b;

--line: #cbd5e1;
--soft-line: #e2e8f0;
--control-border: #7c8798;

--accent: #4f46e5;
--accent-text: #4f46e5;
--accent-hover: #4338ca;
--accent-soft: #eef2ff;
--accent-soft-text: #3730a3;
--focus-ring: #2563eb;
```

## Dark Tokens

```css
--bg: #0f172a;
--panel: #111827;
--surface-muted: #1e293b;
--surface-subtle: #172033;
--surface-selected: #312e81;

--text: #f8fafc;
--muted: #cbd5e1;
--quiet: #94a3b8;

--line: #334155;
--soft-line: #263244;
--control-border: #64748b;

--accent: #4f46e5;
--accent-text: #818cf8;
--accent-hover: #4338ca;
--accent-soft: #312e81;
--accent-soft-text: #c7d2fe;
--focus-ring: #818cf8;
```

## Usage Rules

- Use `--text` for primary copy and headings.
- Use `--muted` for body copy, descriptions, and secondary metadata.
- Use `--quiet` only for nonessential labels and small metadata.
- Use `--soft-line` for decorative dividers.
- Use `--line` for card borders and section separation.
- Use `--control-border` for inputs, menus, topic chips, icon buttons, and other interactive controls.
- Use `--accent` for primary actions.
- Use `--accent-text` for links, icons, secondary-button hover text, and selected borders.
- Use `--accent-soft` plus `--accent-soft-text` for selected and active states.
- Use `--focus-ring` for visible keyboard focus.

## React Adoption Notes

When moving this into the app, map these landing page tokens onto the existing `src/index.css` theme variables instead of introducing a parallel naming system.

Recommended first pass:

- `--app-bg` -> `--bg`
- `--surface` -> `--panel`
- `--surface-muted` -> `--surface-muted`
- `--text` / `--text-body` -> `--text`
- `--text-muted` -> `--muted`
- `--text-soft` -> `--quiet`
- `--border` -> `--line`
- `--border-strong` -> `--control-border`
- `--accent` -> `--accent`
- add `--accent-text` or map it to the existing link/accent text token
- `--accent-hover` -> `--accent-hover`
- `--surface-accent` -> `--accent-soft`
- `--accent-ink` -> `--accent-soft-text`
- `--shadow-focus` -> `--focus-shadow`

Keep decorative borders subtle, but use `--control-border` for actionable UI. This distinction matters most on mobile, where controls have less surrounding whitespace.
