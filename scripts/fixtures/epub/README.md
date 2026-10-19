# EPUB image diagnostics acceptance check

Import [image-diagnostics.epub](image-diagnostics.epub) into the current Papercut
build using Library > Import > Files. The title is **Papercut Image Diagnostics**.
This deliberately incomplete, synthetic book is about 8.5 KB compressed; one
valid SVG expands just beyond the 5 MiB image limit. It contains no publisher
content and does not require network access.

## Expected import result

The book imports successfully. The Library status must remain visible until
dismissed and show **Images unavailable: 9**, with this file named and these
reasons (wording follows the app language):

| Reason | Occurrences |
| --- | ---: |
| Missing file | 3 |
| Unsupported format (BMP) | 1 |
| Remote image | 1 |
| Image size limit | 1 |
| Unconverted inline SVG | 1 |
| Unavailable image (empty PNG) | 1 |
| Invalid archive path | 1 |

The three missing occurrences reference the same file: two in the first chapter
and one in the final image-only chapter. Counts describe placements, not unique
files. The remote URL uses the reserved `.invalid` domain and is removed from the
generated reading HTML.

## Expected reader result

1. A green square and a blue circle display normally: external and inline SVG.
2. Scroll through both chapters. Nine import omissions have labeled placeholders;
   the corrupt PNG produces one additional load-failure placeholder, for **10
   placeholders total**. It is deliberately nonempty, so import stores it before
   the WebView discovers that it cannot decode it. The import total stays at nine.
3. The final image-only chapter remains present with its missing-image placeholder.
   The two links near the beginning reach that chapter and the failed SVG anchor.
4. Alternative text identifies each failed image, including the two repeated
   missing figures. Changing the app language changes the placeholder explanations.
5. Find `copper lantern beside the window`: two occurrences in the book text.
   Narration and Find should not treat generated placeholder explanations as book
   text. A saved text bookmark should still return to its text after reopening.
6. Return to the Library if import opened the reader automatically. Confirm the
   warning remains visible. Dismiss it, reimport the same test book, and confirm
   the duplicate result reports the warnings without creating another book.

This fixture does not exercise the 100 MiB aggregate budget, CSS backgrounds,
external SVG dependencies, or gallery cover metadata. The budget already has a
bounded Rust check; no 100 MiB manual fixture is needed.

## Verification

ZIP integrity and XML well-formedness were checked. The actual EPUB parser
produced the counts above, two chapters, and three stored image assets. Linux
WebKitGTK decoded the two valid SVGs and rejected the corrupt PNG. This does not
replace the full Papercut UI checks above; preparing the fixture did not import it
into local app data.

The parser acceptance check is runnable from the repository root:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --offline --lib manual_image_diagnostics_fixture_matches_acceptance_counts
```

The book's XHTML, OPF, and SVG source can be inspected with any ZIP reader. Missing,
empty, and invalid resources are intentional; this is not a standards-compliance
fixture. Delete this synthetic book from the Library when finished testing.
