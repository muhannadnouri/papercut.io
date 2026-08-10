//! Library organization metadata for uploaded documents.
//!
//! Folders and manual ordering live beside the upload/search tables. They never
//! change the uploaded document URL or stored source file, which keeps search,
//! saved audiobook cache ids, and TTS highlighting stable when users reorganize
//! their library.

use std::collections::hash_map::DefaultHasher;
#[cfg(desktop)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use tauri::Runtime;

use super::storage::now_ms;
use super::store::{db_err, open_db};
use super::types::{
    UploadedDocumentLocation, UploadedLibraryCreateFolderRequest,
    UploadedLibraryDeleteFolderRequest, UploadedLibraryFolder, UploadedLibraryMoveDocumentsRequest,
    UploadedLibraryMoveFolderRequest, UploadedLibraryOrderItem, UploadedLibraryOrganization,
    UploadedLibraryRenameFolderRequest, UploadedLibraryReorderRequest,
};

/// Root folders use depth 0, so a max depth of 4 allows five visible folder levels.
pub(crate) const MAX_FOLDER_DEPTH: usize = 4;
const MAX_FOLDER_NAME_CHARS: usize = 80;
const ORDER_STEP: i64 = 1000;

/// Internal row shape with DB-only fields before converting to the serde DTO.
#[derive(Clone)]
struct FolderRow {
    id: String,
    parent_id: Option<String>,
    name: String,
    depth: usize,
    sort_order: i64,
    created_at_ms: u128,
    updated_at_ms: u128,
}

impl From<FolderRow> for UploadedLibraryFolder {
    /// Drop no fields during conversion; this exists only to keep SQL row loading private.
    fn from(row: FolderRow) -> Self {
        Self {
            id: row.id,
            parent_id: row.parent_id,
            name: row.name,
            depth: row.depth,
            sort_order: row.sort_order,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
        }
    }
}

/// Return the folder tree and document placements without document payloads.
pub(crate) fn list_organization<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<UploadedLibraryOrganization, String> {
    let db = open_db(app)?;
    Ok(UploadedLibraryOrganization {
        folders: list_folders(&db)?,
        document_locations: list_document_locations(&db)?,
    })
}

/// Create one isolated folder tree for a desktop folder import and place only
/// newly imported documents inside it.
///
/// A conflicting root name receives a numeric suffix instead of merging with
/// an existing user tree. The transaction is all-or-nothing, so an error leaves
/// the successfully imported documents at Library root rather than half moved.
#[cfg(desktop)]
pub(crate) fn organize_folder_import<R: Runtime>(
    app: &tauri::AppHandle<R>,
    root_name: &str,
    documents: &[(String, Vec<String>)],
) -> Result<(), String> {
    if documents.is_empty() {
        return Ok(());
    }

    let mut db = open_db(app)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let now = now_ms()?;
    let root_name = available_import_folder_name(&tx, None, root_name)?;
    let root_id = insert_import_folder(&tx, None, &root_name, 0, now)?;
    let mut folders = HashMap::<Vec<String>, String>::new();
    let mut next_orders = HashMap::<String, i64>::new();

    for (document_id, relative_folder) in documents {
        ensure_document_exists(&tx, document_id)?;
        let mut prefix = Vec::new();
        let mut parent_id = root_id.clone();
        for (index, segment) in relative_folder.iter().take(MAX_FOLDER_DEPTH).enumerate() {
            prefix.push(segment.clone());
            parent_id = match folders.get(&prefix) {
                Some(id) => id.clone(),
                None => {
                    let name = available_import_folder_name(&tx, Some(&parent_id), segment)?;
                    let id = insert_import_folder(&tx, Some(&parent_id), &name, index + 1, now)?;
                    folders.insert(prefix.clone(), id.clone());
                    id
                }
            };
        }

        let sort_order = match next_orders.get_mut(&parent_id) {
            Some(order) => {
                let current = *order;
                *order += ORDER_STEP;
                current
            }
            None => {
                let current = next_sort_order(&tx, Some(&parent_id))?;
                next_orders.insert(parent_id.clone(), current + ORDER_STEP);
                current
            }
        };
        tx.execute(
            "INSERT INTO uploaded_document_locations (document_id, folder_id, sort_order) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(document_id) DO UPDATE SET \
               folder_id = excluded.folder_id, sort_order = excluded.sort_order",
            params![document_id, parent_id, sort_order],
        )
        .map_err(db_err)?;
    }

    tx.commit().map_err(db_err)
}

/// Insert one already-validated import folder within the caller's transaction.
#[cfg(desktop)]
fn insert_import_folder(
    db: &Connection,
    parent_id: Option<&str>,
    name: &str,
    depth: usize,
    now: u128,
) -> Result<String, String> {
    let id = folder_id(parent_id, name, now);
    let sort_order = next_sort_order(db, parent_id)?;
    db.execute(
        "INSERT INTO uploaded_folders \
         (id, parent_id, name, depth, sort_order, created_at_ms, updated_at_ms) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            parent_id,
            name,
            depth as i64,
            sort_order,
            now as i64,
            now as i64,
        ],
    )
    .map_err(db_err)?;
    Ok(id)
}

/// Return a bounded sibling-unique name, preserving the selected folder name
/// when possible and otherwise appending ` (2)`, ` (3)`, and so on.
#[cfg(desktop)]
fn available_import_folder_name(
    db: &Connection,
    parent_id: Option<&str>,
    requested_name: &str,
) -> Result<String, String> {
    let base = requested_name.trim();
    let base = if base.is_empty() {
        "Imported Folder"
    } else {
        base
    };
    let mut sequence = 1usize;
    loop {
        let suffix = (sequence > 1).then(|| format!(" ({sequence})"));
        let suffix_len = suffix.as_ref().map_or(0, |value| value.chars().count());
        let mut candidate = base
            .chars()
            .take(MAX_FOLDER_NAME_CHARS - suffix_len)
            .collect::<String>();
        if let Some(suffix) = suffix {
            candidate.push_str(&suffix);
        }
        if !folder_name_exists(db, parent_id, &candidate)? {
            return Ok(candidate);
        }
        sequence += 1;
    }
}

#[cfg(desktop)]
fn folder_name_exists(
    db: &Connection,
    parent_id: Option<&str>,
    name: &str,
) -> Result<bool, String> {
    let id: Option<String> = match parent_id {
        Some(parent_id) => db
            .query_row(
                "SELECT id FROM uploaded_folders WHERE parent_id = ?1 AND lower(name) = lower(?2)",
                params![parent_id, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?,
        None => db
            .query_row(
                "SELECT id FROM uploaded_folders WHERE parent_id IS NULL AND lower(name) = lower(?1)",
                [name],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?,
    };
    Ok(id.is_some())
}

/// Create a folder at root or under a parent folder, enforcing name and depth rules.
pub(crate) fn create_folder<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedLibraryCreateFolderRequest,
) -> Result<UploadedLibraryFolder, String> {
    let db = open_db(app)?;
    let name = normalize_folder_name(&request.name)?;
    let depth = match request.parent_id.as_deref() {
        Some(parent_id) => load_folder(&db, parent_id)?.depth + 1,
        None => 0,
    };
    if depth > MAX_FOLDER_DEPTH {
        return Err("Uploaded document folders can be nested up to 5 levels deep".into());
    }
    ensure_unique_folder_name(&db, request.parent_id.as_deref(), &name, None)?;

    let now = now_ms()?;
    let id = folder_id(request.parent_id.as_deref(), &name, now);
    let sort_order = next_sort_order(&db, request.parent_id.as_deref())?;
    db.execute(
        "INSERT INTO uploaded_folders \
         (id, parent_id, name, depth, sort_order, created_at_ms, updated_at_ms) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            request.parent_id,
            name,
            depth as i64,
            sort_order,
            now as i64,
            now as i64,
        ],
    )
    .map_err(db_err)?;

    Ok(load_folder(&db, &id)?.into())
}

/// Rename a folder without changing its id, parent, children, or contained documents.
pub(crate) fn rename_folder<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedLibraryRenameFolderRequest,
) -> Result<UploadedLibraryFolder, String> {
    let db = open_db(app)?;
    let folder = load_folder(&db, &request.folder_id)?;
    let name = normalize_folder_name(&request.name)?;
    ensure_unique_folder_name(&db, folder.parent_id.as_deref(), &name, Some(&folder.id))?;

    let now = now_ms()?;
    db.execute(
        "UPDATE uploaded_folders SET name = ?1, updated_at_ms = ?2 WHERE id = ?3",
        params![name, now as i64, request.folder_id],
    )
    .map_err(db_err)?;

    Ok(load_folder(&db, &folder.id)?.into())
}

/// Delete only empty folders so document loss or surprising recursive moves are impossible.
pub(crate) fn delete_folder<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedLibraryDeleteFolderRequest,
) -> Result<(), String> {
    let mut db = open_db(app)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    load_folder(&tx, &request.folder_id)?;

    let child_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM uploaded_folders WHERE parent_id = ?1",
            [request.folder_id.as_str()],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    let document_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM uploaded_document_locations WHERE folder_id = ?1",
            [request.folder_id.as_str()],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    if child_count > 0 || document_count > 0 {
        return Err("Folder must be empty before it can be deleted".into());
    }

    tx.execute(
        "DELETE FROM uploaded_folders WHERE id = ?1",
        [request.folder_id.as_str()],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok(())
}

/// Move documents into a folder or back to root by editing metadata only.
pub(crate) fn move_documents<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedLibraryMoveDocumentsRequest,
) -> Result<UploadedLibraryOrganization, String> {
    let mut db = open_db(app)?;
    let document_ids = unique_ids(&request.document_ids)?;
    if document_ids.is_empty() {
        return Err("Select at least one uploaded document to move".into());
    }
    if let Some(folder_id) = request.folder_id.as_deref() {
        load_folder(&db, folder_id)?;
    }
    for document_id in &document_ids {
        ensure_document_exists(&db, document_id)?;
    }

    let mut sort_order = next_sort_order(&db, request.folder_id.as_deref())?;
    let tx = db.transaction().map_err(db_err)?;
    for document_id in document_ids {
        tx.execute(
            "INSERT INTO uploaded_document_locations (document_id, folder_id, sort_order) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(document_id) DO UPDATE SET \
               folder_id = excluded.folder_id, sort_order = excluded.sort_order",
            params![document_id, request.folder_id, sort_order],
        )
        .map_err(db_err)?;
        sort_order += ORDER_STEP;
    }
    tx.commit().map_err(db_err)?;

    Ok(UploadedLibraryOrganization {
        folders: list_folders(&db)?,
        document_locations: list_document_locations(&db)?,
    })
}

/// Move a folder while preventing self-parenting, cycles, and excessive depth.
pub(crate) fn move_folder<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedLibraryMoveFolderRequest,
) -> Result<UploadedLibraryOrganization, String> {
    let mut db = open_db(app)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let folder = load_folder(&tx, &request.folder_id)?;
    if request.parent_id.as_deref() == Some(folder.id.as_str()) {
        return Err("A folder cannot be moved inside itself".into());
    }

    let new_parent_id = request.parent_id.as_deref();
    let new_depth = match new_parent_id {
        Some(parent_id) => {
            if is_descendant(&tx, parent_id, &folder.id)? {
                return Err("A folder cannot be moved inside one of its child folders".into());
            }
            load_folder(&tx, parent_id)?.depth + 1
        }
        None => 0,
    };
    let max_subtree_depth = max_subtree_depth(&tx, &folder.id)?;
    let deepest_after_move = new_depth + max_subtree_depth.saturating_sub(folder.depth);
    if deepest_after_move > MAX_FOLDER_DEPTH {
        return Err("Uploaded document folders can be nested up to 5 levels deep".into());
    }
    ensure_unique_folder_name(&tx, new_parent_id, &folder.name, Some(&folder.id))?;

    let now = now_ms()?;
    let sort_order = next_sort_order(&tx, new_parent_id)?;
    let depth_delta = new_depth as i64 - folder.depth as i64;
    tx.execute(
        "UPDATE uploaded_folders \
         SET parent_id = ?1, depth = ?2, sort_order = ?3, updated_at_ms = ?4 \
         WHERE id = ?5",
        params![
            new_parent_id,
            new_depth as i64,
            sort_order,
            now as i64,
            folder.id.as_str(),
        ],
    )
    .map_err(db_err)?;
    tx.execute(
        "WITH RECURSIVE descendants(id) AS (
           SELECT id FROM uploaded_folders WHERE parent_id = ?1
           UNION ALL
           SELECT f.id FROM uploaded_folders f
           JOIN descendants d ON f.parent_id = d.id
         )
         UPDATE uploaded_folders
         SET depth = depth + ?2, updated_at_ms = ?3
         WHERE id IN (SELECT id FROM descendants)",
        params![folder.id.as_str(), depth_delta, now as i64],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;

    Ok(UploadedLibraryOrganization {
        folders: list_folders(&db)?,
        document_locations: list_document_locations(&db)?,
    })
}

/// Persist a complete sibling ordering for one folder/root.
pub(crate) fn reorder<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedLibraryReorderRequest,
) -> Result<UploadedLibraryOrganization, String> {
    let mut db = open_db(app)?;
    if let Some(parent_id) = request.parent_id.as_deref() {
        load_folder(&db, parent_id)?;
    }
    validate_reorder_items(&db, request.parent_id.as_deref(), &request.items)?;

    let tx = db.transaction().map_err(db_err)?;
    for (index, item) in request.items.iter().enumerate() {
        let sort_order = index as i64 * ORDER_STEP;
        match item.item_type.as_str() {
            "folder" => {
                tx.execute(
                    "UPDATE uploaded_folders SET sort_order = ?1 WHERE id = ?2",
                    params![sort_order, item.id],
                )
                .map_err(db_err)?;
            }
            "document" => {
                tx.execute(
                    "UPDATE uploaded_document_locations SET sort_order = ?1 WHERE document_id = ?2",
                    params![sort_order, item.id],
                )
                .map_err(db_err)?;
            }
            _ => return Err("Library order item type must be folder or document".into()),
        }
    }
    tx.commit().map_err(db_err)?;

    Ok(UploadedLibraryOrganization {
        folders: list_folders(&db)?,
        document_locations: list_document_locations(&db)?,
    })
}

/// Load all folder metadata in stable display order.
fn list_folders(db: &Connection) -> Result<Vec<UploadedLibraryFolder>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, parent_id, name, depth, sort_order, created_at_ms, updated_at_ms \
             FROM uploaded_folders ORDER BY parent_id IS NOT NULL, parent_id, sort_order, name",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(UploadedLibraryFolder {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                depth: row.get::<_, i64>(3)? as usize,
                sort_order: row.get(4)?,
                created_at_ms: row.get::<_, i64>(5)? as u128,
                updated_at_ms: row.get::<_, i64>(6)? as u128,
            })
        })
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// Load document-to-folder placements without joining document payload rows.
fn list_document_locations(db: &Connection) -> Result<Vec<UploadedDocumentLocation>, String> {
    let mut stmt = db
        .prepare(
            "SELECT document_id, folder_id, sort_order \
             FROM uploaded_document_locations ORDER BY folder_id IS NOT NULL, folder_id, sort_order",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(UploadedDocumentLocation {
                document_id: row.get(0)?,
                folder_id: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// Fetch one folder row and translate a missing row into a user-facing error.
fn load_folder(db: &Connection, id: &str) -> Result<FolderRow, String> {
    db.query_row(
        "SELECT id, parent_id, name, depth, sort_order, created_at_ms, updated_at_ms \
         FROM uploaded_folders WHERE id = ?1",
        [id],
        |row| {
            Ok(FolderRow {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                depth: row.get::<_, i64>(3)? as usize,
                sort_order: row.get(4)?,
                created_at_ms: row.get::<_, i64>(5)? as u128,
                updated_at_ms: row.get::<_, i64>(6)? as u128,
            })
        },
    )
    .optional()
    .map_err(db_err)?
    .ok_or_else(|| "Uploaded document folder was not found".to_string())
}

/// Validate a document id from the UI before using it in move/reorder operations.
fn ensure_document_exists(db: &Connection, id: &str) -> Result<(), String> {
    if !is_valid_id(id) {
        return Err("Uploaded document id is invalid".into());
    }
    let exists: Option<String> = db
        .query_row(
            "SELECT id FROM uploaded_documents WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;
    exists
        .map(|_| ())
        .ok_or_else(|| "Uploaded document was not found".to_string())
}

/// Trim and validate names before uniqueness checks so visually empty names cannot persist.
fn normalize_folder_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Folder name cannot be empty".into());
    }
    if name.chars().count() > MAX_FOLDER_NAME_CHARS {
        return Err(format!(
            "Folder name must be {MAX_FOLDER_NAME_CHARS} characters or fewer"
        ));
    }
    Ok(name.to_string())
}

/// Enforce case-insensitive sibling uniqueness.
///
/// `except_id` lets rename/move check the destination while ignoring the folder being edited.
fn ensure_unique_folder_name(
    db: &Connection,
    parent_id: Option<&str>,
    name: &str,
    except_id: Option<&str>,
) -> Result<(), String> {
    let duplicate: Option<String> = match parent_id {
        Some(parent_id) => db
            .query_row(
                "SELECT id FROM uploaded_folders \
                 WHERE parent_id = ?1 AND lower(name) = lower(?2) AND (?3 IS NULL OR id <> ?3)",
                params![parent_id, name, except_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?,
        None => db
            .query_row(
                "SELECT id FROM uploaded_folders \
                 WHERE parent_id IS NULL AND lower(name) = lower(?1) AND (?2 IS NULL OR id <> ?2)",
                params![name, except_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?,
    };
    if duplicate.is_some() {
        return Err("A folder with that name already exists here".into());
    }
    Ok(())
}

/// Append new folders/documents after existing siblings in the same root/folder.
///
/// Folders and documents share one visual list, so both tables contribute to the max order.
fn next_sort_order(db: &Connection, parent_id: Option<&str>) -> Result<i64, String> {
    let folder_max: Option<i64> = match parent_id {
        Some(parent_id) => db
            .query_row(
                "SELECT MAX(sort_order) FROM uploaded_folders WHERE parent_id = ?1",
                [parent_id],
                |row| row.get(0),
            )
            .map_err(db_err)?,
        None => db
            .query_row(
                "SELECT MAX(sort_order) FROM uploaded_folders WHERE parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(db_err)?,
    };
    let document_max: Option<i64> = match parent_id {
        Some(parent_id) => db
            .query_row(
                "SELECT MAX(sort_order) FROM uploaded_document_locations WHERE folder_id = ?1",
                [parent_id],
                |row| row.get(0),
            )
            .map_err(db_err)?,
        None => db
            .query_row(
                "SELECT MAX(sort_order) FROM uploaded_document_locations WHERE folder_id IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(db_err)?,
    };

    Ok(folder_max
        .into_iter()
        .chain(document_max)
        .max()
        .unwrap_or(0)
        + ORDER_STEP)
}

/// Return whether `candidate_id` is already inside `folder_id`'s subtree.
///
/// Used before moving folders so a parent cannot be moved into its own child.
fn is_descendant(db: &Connection, candidate_id: &str, folder_id: &str) -> Result<bool, String> {
    let found: Option<String> = db
        .query_row(
            "WITH RECURSIVE descendants(id) AS (
               SELECT id FROM uploaded_folders WHERE parent_id = ?1
               UNION ALL
               SELECT f.id FROM uploaded_folders f
               JOIN descendants d ON f.parent_id = d.id
             )
             SELECT id FROM descendants WHERE id = ?2 LIMIT 1",
            params![folder_id, candidate_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;
    Ok(found.is_some())
}

/// Find deepest folder depth within a subtree before applying a move depth delta.
fn max_subtree_depth(db: &Connection, folder_id: &str) -> Result<usize, String> {
    let max_depth: i64 = db
        .query_row(
            "WITH RECURSIVE subtree(id, depth) AS (
               SELECT id, depth FROM uploaded_folders WHERE id = ?1
               UNION ALL
               SELECT f.id, f.depth FROM uploaded_folders f
               JOIN subtree s ON f.parent_id = s.id
             )
             SELECT MAX(depth) FROM subtree",
            [folder_id],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    Ok(max_depth as usize)
}

/// Ensure reorder payload exactly matches current siblings in one location.
///
/// This prevents partial reorder requests from dropping hidden folders/documents.
fn validate_reorder_items(
    db: &Connection,
    parent_id: Option<&str>,
    items: &[UploadedLibraryOrderItem],
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for item in items {
        if item.item_type != "folder" && item.item_type != "document" {
            return Err("Library order item type must be folder or document".into());
        }
        if !is_valid_id(&item.id) {
            return Err("Library order item id is invalid".into());
        }
        let key = format!("{}:{}", item.item_type, item.id);
        if !seen.insert(key) {
            return Err("Library order contains duplicate items".into());
        }
    }

    let expected = sibling_keys(db, parent_id)?;
    let actual = items
        .iter()
        .map(|item| format!("{}:{}", item.item_type, item.id))
        .collect::<HashSet<_>>();
    if actual != expected {
        return Err("Library order must include every folder and document in that location".into());
    }
    Ok(())
}

/// Build comparable `folder:{id}` / `document:{id}` keys for reorder validation.
fn sibling_keys(db: &Connection, parent_id: Option<&str>) -> Result<HashSet<String>, String> {
    let mut keys = HashSet::new();
    match parent_id {
        Some(parent_id) => {
            let mut stmt = db
                .prepare("SELECT id FROM uploaded_folders WHERE parent_id = ?1")
                .map_err(db_err)?;
            let rows = stmt
                .query_map([parent_id], |row| row.get::<_, String>(0))
                .map_err(db_err)?;
            for id in rows {
                keys.insert(format!("folder:{}", id.map_err(db_err)?));
            }
        }
        None => {
            let mut stmt = db
                .prepare("SELECT id FROM uploaded_folders WHERE parent_id IS NULL")
                .map_err(db_err)?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(db_err)?;
            for id in rows {
                keys.insert(format!("folder:{}", id.map_err(db_err)?));
            }
        }
    }

    match parent_id {
        Some(parent_id) => {
            let mut stmt = db
                .prepare("SELECT document_id FROM uploaded_document_locations WHERE folder_id = ?1")
                .map_err(db_err)?;
            let rows = stmt
                .query_map([parent_id], |row| row.get::<_, String>(0))
                .map_err(db_err)?;
            for id in rows {
                keys.insert(format!("document:{}", id.map_err(db_err)?));
            }
        }
        None => {
            let mut stmt = db
                .prepare(
                    "SELECT document_id FROM uploaded_document_locations WHERE folder_id IS NULL",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(db_err)?;
            for id in rows {
                keys.insert(format!("document:{}", id.map_err(db_err)?));
            }
        }
    }
    Ok(keys)
}

/// Deduplicate selected document ids while preserving first-seen order.
fn unique_ids(ids: &[String]) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for id in ids {
        if !is_valid_id(id) {
            return Err("Uploaded document id is invalid".into());
        }
        if seen.insert(id.as_str()) {
            unique.push(id.clone());
        }
    }
    Ok(unique)
}

/// Accept only upload ids generated by this feature, currently lowercase/uppercase hex.
fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_hexdigit())
}

/// Generate an opaque folder id; uniqueness does not depend on folder name alone.
fn folder_id(parent_id: Option<&str>, name: &str, created_at_ms: u128) -> String {
    let mut hasher = DefaultHasher::new();
    parent_id.hash(&mut hasher);
    name.hash(&mut hasher);
    created_at_ms.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(desktop)]
    #[test]
    fn imported_root_folders_receive_a_bounded_numeric_suffix() {
        let db = Connection::open_in_memory().expect("open database");
        db.execute_batch(
            "CREATE TABLE uploaded_folders (
               id TEXT PRIMARY KEY,
               parent_id TEXT,
               name TEXT NOT NULL,
               depth INTEGER NOT NULL,
               sort_order INTEGER NOT NULL,
               created_at_ms INTEGER NOT NULL,
               updated_at_ms INTEGER NOT NULL
             );",
        )
        .expect("create folders table");
        for (id, name) in [("one", "Books"), ("two", "Books (2)")] {
            db.execute(
                "INSERT INTO uploaded_folders VALUES (?1, NULL, ?2, 0, 0, 0, 0)",
                params![id, name],
            )
            .expect("insert folder");
        }

        assert_eq!(
            available_import_folder_name(&db, None, "Books").expect("available name"),
            "Books (3)"
        );
        let long_name = "x".repeat(MAX_FOLDER_NAME_CHARS + 20);
        let bounded =
            available_import_folder_name(&db, None, &long_name).expect("bounded available name");
        assert_eq!(bounded.chars().count(), MAX_FOLDER_NAME_CHARS);
    }
}
