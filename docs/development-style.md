# Code style, naming and editor setup

The lint policy lives in `eslint.config.js`. ESLint owns JavaScript/TypeScript
layout fixes; no second formatter is required. This change installs policy and
editor support without reformatting application files or renaming helpers.
Keep mechanical cleanup separate from behavioral changes and from the policy
commit, so reviewers can distinguish whitespace from logic.

## Checks and fixes

| Command | Behavior |
| --- | --- |
| `npm run lint -- --quiet` | Existing correctness errors and private-method naming errors; used by CI. |
| `npm run lint` | Full workspace diagnostics, including legacy formatting warnings. |
| `npm run lint:strict -- src/path/to/file.ts` | Reject errors and warnings for the selected files. With no paths, checks the whole workspace. |
| `npx eslint src/path/to/file.ts --fix` | Apply safe automatic fixes to selected files; review the diff. |
| `npm run lint:fix` | Explicit whole-workspace fix; produces a separate mechanical cleanup, never part of a feature fix by default. |
| `npm run test:lint-policy` | Exercise actual configured fixers, type/value import preservation, private naming, correctness severity and editor save settings. |

Formatting rules start at warning severity. The initial audit found **10,335
warnings and zero errors in 289 files**. This is an adoption baseline, not a
clean-style claim. CI hides warnings with `--quiet`; it still fails correctness
and naming errors and runs the policy regression check. After a separate cleanup,
promote formatting warnings to errors or adopt strict checks for a clean scope.
Do not add a new style violation merely because older code has one.

The rules keep named imports and object members on separate lines, expand block
bodies, separate variable declarations, use two-space indentation and multiline
trailing commas, and remove trailing whitespace. Mixed `import type` and inline
`type` specifiers retain their meaning. The compatible plugin versions are pinned:
ESLint Stylistic 5.10.0 and import-newlines 1.4.1; import-newlines 2 requires ESLint
10, so upgrading it is a separate toolchain decision.

`max-statements-per-line` reports compressed statements but does not auto-fix
all of them. For example, split `intent++; preparation.abort()` into separate
statements yourself. Fix-on-save does not mean every warning can be rewritten.

Generated Tauri output and vendored PDF.js/Tesseract assets are ignored. They
must retain upstream bytes and are not targets for `lint:fix`. The existing
correctness presets remain scoped to TypeScript; layout also covers JS/MJS.
This policy does not add type-aware linting or change runtime code.

## VS Code

Install the recommended Microsoft ESLint extension (`dbaeumer.vscode-eslint`).
The workspace uses:

- Focus-change Auto Save and `source.fixAll.eslint: "always"`, covering explicit
  save and focus/window-change saves. Timer-based `afterDelay` is not a guarantee
  that every delay invokes code actions.
- ESLint validation for JavaScript, JavaScript React, TypeScript and TypeScript
  React. Layout fixes use the same local configuration as the command line.
- `editor.formatOnSave: false` for those languages so another formatter cannot
  collapse the named imports immediately before/after ESLint expands them.

Existing theme and watcher preferences remain independent. Merge changes to the
workspace settings; do not replace a user's personal preferences. ESLint save
behavior still requires the extension to be installed/enabled; CLI self-checks do
not establish that the user's running editor has activated it.

## Private methods and file-local functions

TypeScript `private` class methods require one underscore and a camelCase base
name. Public/implicit-public/protected methods and ECMAScript `#private` methods
forbid a leading underscore. Private data properties and arrow-function fields
are not covered by this method-only rule; they keep their existing conventions.
The underscore is a readability convention, not runtime access enforcement.

```ts
export class Reader {
  private _resolveTarget() {
    return 1
  }

  #reset() {
    // The # marker already expresses runtime privacy.
  }

  read() {
    this.#reset()
    return this._resolveTarget()
  }
}
```

Use ordinary camelCase for file-local helpers, PascalCase for components, and
`useName` for hooks. A module's exports already define its public API. Requiring
underscores for every non-exported function would also rename nested callbacks,
local components and hooks, and can undermine React's naming-based checks.
`naming-convention` has no plain-function `private` modifier; this change does
not invent a custom rule to infer one. An `_name` requirement for module helpers
would need a separate, explicit policy for these exceptions. Existing functions
are not automatically renamed, and naming violations are not auto-fixed.

## Useful function documentation

Document intent and constraints that a future reader cannot infer from the
signature or local control flow: ownership, lifecycle, ordering, domain semantics,
examples and known gotchas. Explain why a selection must be consumed before
menu cleanup, for example, rather than commenting that a button was clicked.
Keep routine helpers self-explanatory; requiring boilerplate JSDoc on every
function would not establish useful documentation. This remains a review standard.

## Rollout checklist

- [x] Commit the compact reader UI separately (`ff62f4f`).
- [x] Add pinned ESLint plugins, scoped editor save settings and extension recommendation.
- [x] Keep existing correctness errors blocking and introduce layout without source churn.
- [x] Verify import auto-fixes preserve type semantics and private naming matches intended visibility.
- [x] Add policy self-checks to CI; retain the existing frontend/type/build checks.
- [ ] Adopt clean-file strict checks during ordinary work and perform any broad formatting as a separate commit.
- [ ] Confirm fix-on-save in the owner's actual VS Code installation.

References: [ESLint Stylistic](https://eslint.style/),
[import-newlines](https://github.com/SeinopSys/eslint-plugin-import-newlines),
[TypeScript naming convention](https://typescript-eslint.io/rules/naming-convention/),
[VS Code ESLint](https://github.com/microsoft/vscode-eslint).

Validation on 2026-10-22: policy self-checks, zero-error workspace lint,
zero-warning strict lint for the new config/self-check, TypeScript and all
598 frontend tests pass (one existing skip). This does not replace actual VS Code
extension acceptance. The tooling commit is isolated on `dx/linter-rules`, based
on the committed reader UI; returning to `feature/play-anywhere` restores that
branch's tracked configuration. Applying the tooling there later requires an
explicit cherry-pick/merge. Neither operation is part of this change.
