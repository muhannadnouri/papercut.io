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

## 4. Verify the complete interaction

- Exercise both Books → Manage → Done and Documents → Manage → Done. Check keyboard focus, filtered results, visible counts, and the saved view preference after reopening the library.
- Check a narrow Arabic RTL layout with enlarged text and a desktop LTR layout. The category group, Manage label, long titles, View affordance, and focus indicators should fit and remain operable.
- Run the affected DocumentsPanel, Gallery, tree, and Search Scope tests, the relevant Chromium regressions, type checking, and the frontend build. Manually open an uploaded document by title and by View, then manage a disposable upload and return to the same gallery. After acceptance, update the gallery behavior in `docs/user-document-search.md`.
