//! Source-document reads for translation jobs.
//!
//! SQLite supplies provenance; the sanitized reader snapshot supplies the DOM
//! plan and owned assets. Extraction and rendering share that same snapshot.

use rusqlite::OptionalExtension;
use tauri::Runtime;

use crate::document_uploads::{open_document_uploads_db, read_uploaded_document_source};

use super::dom::DomPlan;
use crate::document_uploads::{ParsedDocumentAsset, SourceFormatting};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranslationSourceDocument {
    pub(crate) document_id: String,
    pub(crate) document_url: String,
    pub(crate) title: String,
    pub(crate) format: String,
    pub(crate) view_html: String,
    pub(crate) source_formatting: Option<SourceFormatting>,
    pub(crate) blocks: Vec<TranslationSourceBlock>,
    // Share up to 100 MiB of snapshot bytes across runner/persistence clones.
    pub(crate) assets: Arc<Vec<ParsedDocumentAsset>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranslationSourceBlock {
    pub(crate) ordinal: usize,
    pub(crate) is_heading: bool,
    pub(crate) heading: Option<String>,
    pub(crate) text: String,
}

/// Load the ordered text blocks for an uploaded document URL.
///
/// The caller supplies the same virtual document URL used by the reader and
/// TTS. Looking up by URL keeps translation independent from upload ids while
/// still letting storage retain the real `document_id` for future joins.
pub(crate) fn load_translation_source_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<TranslationSourceDocument, String> {
    let db = open_document_uploads_db(app)?;
    let mut source = query_translation_source_document(&db, document_url)?;
    let (html, assets) = read_uploaded_document_source(app, document_url)?;
    source.view_html = html;
    source.assets = Arc::new(assets);
    source.blocks = DomPlan::new(&source.view_html)
        .blocks
        .into_iter()
        .map(|block| TranslationSourceBlock {
            ordinal: block.ordinal,
            is_heading: block.is_heading,
            heading: block.heading,
            text: block.inline.text,
        })
        .collect();
    if source.blocks.is_empty() {
        return Err("Source document has no translatable sections".into());
    }
    Ok(source)
}

fn query_translation_source_document(
    db: &rusqlite::Connection,
    document_url: &str,
) -> Result<TranslationSourceDocument, String> {
    let metadata = db
        .query_row(
            "SELECT id, url, title, format, source_formatting FROM uploaded_documents WHERE url = ?1",
            [document_url],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(db_err)?;
    let Some((document_id, document_url, title, format, source_formatting)) = metadata else {
        return Err("Source document was not found for translation".into());
    };
    if !matches!(format.as_str(), "html" | "epub") {
        return Err(
            "Translation does not support this document format; choose HTML or EPUB".into(),
        );
    }
    let source_formatting = source_formatting
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|_| "Translation source formatting metadata is invalid".to_string())?;

    Ok(TranslationSourceDocument {
        document_id,
        document_url,
        title,
        format,
        view_html: String::new(),
        source_formatting,
        assets: Arc::default(),
        blocks: Vec::new(),
    })
}

fn db_err(err: rusqlite::Error) -> String {
    format!("Translation source database error: {err}")
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::query_translation_source_document;

    #[test]
    fn reads_provenance_by_url_without_using_stale_index_text() {
        let db = test_db();

        let source =
            query_translation_source_document(&db, "/user-uploads/book.html").expect("source");

        assert_eq!(source.document_id, "doc1");
        assert_eq!(source.document_url, "/user-uploads/book.html");
        assert_eq!(source.title, "Book");
        assert_eq!(source.format, "html");
        assert_eq!(source.view_html, "");
        assert!(source.source_formatting.is_none());
        assert!(source.blocks.is_empty());
    }

    #[test]
    fn rejects_missing_document_url() {
        let db = test_db();
        let error =
            query_translation_source_document(&db, "/missing.html").expect_err("missing document");

        assert!(error.contains("not found"));
    }

    #[test]
    fn retains_import_policy_and_warnings_and_rejects_malformed_provenance() {
        let db = test_db();
        let report = crate::document_uploads::SourceFormatting {
            policy: "source-emphasis-v1".into(),
            warnings: vec!["unread-stylesheet".into()],
        };
        db.execute(
            "UPDATE uploaded_documents SET source_formatting=?1",
            [serde_json::to_string(&report).unwrap()],
        )
        .unwrap();
        let source = query_translation_source_document(&db, "/user-uploads/book.html").unwrap();
        assert_eq!(source.source_formatting, Some(report));
        for invalid in [
            "not json",
            "null",
            "{}",
            r#"{"policy":"source-emphasis-v1","warnings":[],"ready":true}"#,
        ] {
            db.execute(
                "UPDATE uploaded_documents SET source_formatting=?1",
                [invalid],
            )
            .unwrap();
            let error =
                query_translation_source_document(&db, "/user-uploads/book.html").unwrap_err();
            assert!(error.contains("source formatting metadata is invalid"));
            assert!(!error.contains(invalid));
        }
    }

    #[test]
    fn accepts_only_html_and_epub_before_loading_reader_files() {
        let db = test_db();
        for format in ["html", "epub"] {
            db.execute(
                "UPDATE uploaded_documents SET format=?1 WHERE id='doc1'",
                [format],
            )
            .unwrap();
            assert!(query_translation_source_document(&db, "/user-uploads/book.html").is_ok());
        }
        for format in ["pdf", "txt", "docx", "", "unknown"] {
            db.execute(
                "UPDATE uploaded_documents SET format=?1 WHERE id='doc1'",
                [format],
            )
            .unwrap();
            let error =
                query_translation_source_document(&db, "/user-uploads/book.html").unwrap_err();
            assert!(error.contains("choose HTML or EPUB"));
            assert_eq!(
                crate::translation::types::TranslationCommandError::new(error).code,
                "unsupported-translation-option"
            );
        }
    }

    fn test_db() -> Connection {
        let db = test_schema();
        db.execute(
            "INSERT INTO uploaded_documents \
             (id, url, title, format, imported_at_ms, bytes, sections) \
             VALUES ('doc1', '/user-uploads/book.html', 'Book', 'html', 1, 100, 2)",
            [],
        )
        .expect("insert document");
        db.execute(
            "INSERT INTO uploaded_sections (document_id, ordinal, heading, text) \
             VALUES ('doc1', 1, 'Chapter', 'Second block')",
            [],
        )
        .expect("insert second");
        db.execute(
            "INSERT INTO uploaded_sections (document_id, ordinal, heading, text) \
             VALUES ('doc1', 0, 'Intro', 'First block')",
            [],
        )
        .expect("insert first");
        db
    }

    fn test_schema() -> Connection {
        let db = Connection::open_in_memory().expect("open test db");
        db.execute_batch(
            "CREATE TABLE uploaded_documents (
               id TEXT PRIMARY KEY,
               url TEXT NOT NULL UNIQUE,
               title TEXT NOT NULL,
               format TEXT NOT NULL,
               source_formatting TEXT,
               imported_at_ms INTEGER NOT NULL,
               bytes INTEGER NOT NULL,
               sections INTEGER NOT NULL
             );
             CREATE TABLE uploaded_sections (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               document_id TEXT NOT NULL,
               ordinal INTEGER NOT NULL,
               heading TEXT,
               text TEXT NOT NULL
             );",
        )
        .expect("create schema");
        db
    }
}
