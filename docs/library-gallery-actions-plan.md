# Library gallery actions

**Approved scope.** In a document browse row, clicking either the title or **View** opens the document. In Gallery, a separate text button labeled **Manage** sits beside the **Books / Documents** selector. From either category it opens the existing **User Uploads** manager in one step. **Done** returns to the gallery.

## 1. Make the browse title and View one open action (complete)

Flat browse rows in `DocumentList` now use one native button containing the full bidirectional title and the visible **View** label. Both activate the existing open callback. The button takes one Tab stop and shows **Opening…** and a disabled state while its document opens. Delete remains a sibling. Rows without an open callback keep their plain titles.

The Chromium regression uses a long mixed-direction title at a 320px viewport. It exercises title and View clicks, native Tab/Enter/Space input, a disabled opening row, an independent Delete action, and title-to-checkbox selection in Search Scope mode.

## 2. Enter and leave the existing manager from Gallery

- In `LibraryGalleryView`, place **Manage** at the trailing edge of the category row, outside the category button group. Reuse the List-view translation `library.tree.organize`. Allow the action to wrap on a narrow screen while keeping both category buttons together.
- Let `DocumentsPanel` own the temporary transition. Enter the existing `UploadedLibraryTree` in management mode with **User Uploads** expanded, focus its **Done** control, and start with an empty selection. The manager operates on uploaded books and documents across both categories. Preserve the text, bookmark, and saved-audio filters. Bundled documents stay outside the management tree.
- Keep selection, folder actions, confirmations, saved-audio protection, and operation status in the existing tree. Preserve its busy and import-reset behavior. On Done, restore the original gallery category and filters and return focus to Manage. Opening the manager must never write a new Gallery/List preference. Entering Manage from List continues to return to List.
- Offer the gallery entry only when the organizational data and callbacks are available and the tree can render. Cover folders without documents, filtered results with no manageable uploads, and deleting the last visible upload. If the tree disappears, return to Gallery and focus the active category button when Manage is unavailable.

## 3. Remove redundant browse-row Delete actions

Once the round trip works, stop passing the inline deletion action for uploads that can be managed through the tree. Keep single-document Delete available to callers that supply deletion without the organization capabilities. Use the same availability decision for the gallery entry and its replacement of inline Delete so a deletable upload always has a reachable action.

Test the normal path and the single-delete fallback separately. In the manager, verify that selecting a document enables the existing deletion flow, cancelling leaves it present, and saved-audio protection still applies. Confirm that browsing or selecting a category never selects an upload for deletion.

## 4. Verify the complete interaction

- Exercise both Books → Manage → Done and Documents → Manage → Done. Check keyboard focus, filtered results, visible counts, and the saved view preference after reopening the library.
- Check a narrow Arabic RTL layout with enlarged text and a desktop LTR layout. The category group, Manage label, long titles, View affordance, and focus indicators should fit and remain operable.
- Run the affected DocumentsPanel, Gallery, tree, and Search Scope tests, the relevant Chromium regressions, type checking, and the frontend build. Manually open an uploaded document by title and by View, then manage a disposable upload and return to the same gallery. After acceptance, update the gallery behavior in `docs/user-document-search.md`.
