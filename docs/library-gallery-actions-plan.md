# Library gallery actions

**Approved scope.** In a document browse row, clicking either the title or **View** opens the document. In Gallery, a separate text button labeled **Manage** sits beside the **Books / Documents** selector. From either category it opens the existing **User Uploads** manager in one step. **Done** returns to the gallery.

## 1. Make the browse title and View one open action (complete)

Flat browse rows in `DocumentList` now use one native button containing the full bidirectional title and the visible **View** label. Both activate the existing open callback. The button takes one Tab stop and shows **Opening…** and a disabled state while its document opens. Delete remains a sibling. Rows without an open callback keep their plain titles.

The Chromium regression uses a long mixed-direction title at a 320px viewport. It exercises title and View clicks, native Tab/Enter/Space input, a disabled opening row, an independent Delete action, and title-to-checkbox selection in Search Scope mode.

## 2. Enter and leave the existing manager from Gallery (complete)

**Manage** uses the same translation as List and sits beside the Books / Documents group in a wrapping toolbar. `DocumentsPanel` opens a temporary management view without writing the saved library view or category. It keeps the text, bookmark, and saved-audio filters and presents the existing User Uploads tree across both categories. Selecting, moving, deleting, and confirming still run through that tree.

Gallery entry starts with the uploads root expanded, an empty selection, and focus on **Done**. Done restores the gallery and focuses **Manage**. If the last manageable item disappears, focus returns to the active category. Empty folders support management when visible. Import startup exits gallery management, consistent with List's edit reset.

Component regressions cover entry from both categories with a text filter, focus on entry and return, unchanged view preference, loss of the final upload, folder-only availability, and ordinary List management. Stage 3 handles removing inline Delete where this route is available.

## 3. Remove redundant browse-row Delete actions (complete)

Gallery → Documents now passes the single-delete callback to browse rows only when the gallery has no Manage entry. With Manage available, deletion goes through the existing upload tree. List already routes manageable uploads through that tree. Callers that provide only single-document deletion keep their inline Delete action in both views.

Component tests cover entering Manage with an empty selection, selecting one upload, cancelling its confirmation, and confirming deletion for that upload alone. A saved-audio case verifies that View Audiobooks opens without calling deletion. Separate Gallery and List fallback cases verify that inline Delete calls the single-document handler for the chosen row.

## 4. Verify the complete interaction (complete)

Both entry points now open one uploads-only Manage mode owned by `DocumentsPanel`. The uploaded tree owns selections and folder operations. During management the panel hides the view switch and other document sources. Done returns to the entry view and restores focus. Wide panels center the Gallery selector independently. In compact panels the selector shares a row with Manage and can shrink. Manage wraps when space is too tight for both. Decorative book and page icons precede the translated category labels.

Chromium exercises Books and Documents through Manage and Done at 460px, 644px, and 900px in English and 320px in Arabic with 24px root text, checking control positions and focus. Component tests cover entering from a collapsed List root and the visibility of bundled titles and the view switch on entry and return.

When filtering removes the last match, the manager closes and an already-focused filter keeps keyboard focus. When the final upload is deleted and no folders remain, List keeps the dismissible result and hides Manage. Component regressions cover filtering down to zero and back, and the final deletion notice.

### Manual acceptance checklist (passed)

The desktop and narrow RTL smoke checks passed before the review fixes. The owner also verified the revised native toolbar at both reported widths (460px and 644px). The revised toolbar and focus behavior have automated coverage.

- [x] On desktop, open Gallery → Documents. Open an upload by title, return, and open it again with View.
- [x] From Books, activate Manage by keyboard. Check that Done has focus, both kinds of uploads are available, and the bundled documents and view switch are hidden. Activate Done and check focus returns to Manage in Books.
- [x] Repeat from Documents with a filter matching an upload. Check the filter, visible counts, and active category after Done. Reopen the library and check the saved view and category.
- [x] Switch to List, collapse User Uploads, then activate Manage. Check that uploads expand, bundled titles and the view switch disappear, and Done restores List and focus.
- [x] Select a disposable upload, cancel deletion, then confirm deletion of that upload only. Check that the library count updates.
- [x] In Arabic with large text at a narrow width, check that both category labels fit, Manage appears below them on the left, and keyboard focus stays visible through Manage → Done.
