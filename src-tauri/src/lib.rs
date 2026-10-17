mod document_scanner;
mod document_uploads;
mod library_transfer;
mod native_tts;
#[cfg(any(
    feature = "native-text-preprocessing-core",
    feature = "experimental-search-embeddings-core"
))]
mod onnx_runtime;
mod open_documents;
mod search_embeddings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // Tauri requires the single-instance plugin to be registered first.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
        open_documents::queue_cli_paths(
            app,
            args.into_iter().map(Into::into),
            std::path::Path::new(&cwd),
        );
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_native_audio::init())
        .plugin(tauri_plugin_document_scanner::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_library_transfer::init(|app, backgrounded| {
            app.state::<library_transfer::LibraryTransferState>()
                .set_backgrounded(backgrounded);
        }))
        .manage(open_documents::OpenDocumentState::default())
        .manage(document_uploads::DocumentUploadState::default())
        .manage(library_transfer::LibraryTransferState::default())
        .manage(native_tts::NativeTtsState::default())
        .manage(search_embeddings::SearchEmbeddingState::default())
        .manage(document_uploads::semantic::SemanticIndexState::default())
        .invoke_handler(tauri::generate_handler![
            document_uploads::semantic::query::search_semantic_query,
            document_uploads::semantic::search_semantic_index_status,
            document_uploads::semantic::search_semantic_index_build,
            document_uploads::semantic::search_semantic_index_cancel,
            document_uploads::semantic::search_semantic_index_remove,
            search_embeddings::search_embedding_status,
            search_embeddings::search_embedding_configure,
            search_embeddings::search_embedding_cache_status,
            search_embeddings::search_embedding_download,
            search_embeddings::search_embedding_cancel_download,
            search_embeddings::search_embedding_remove,
            open_documents::open_documents_take_sources,
            document_uploads::commands::document_uploads_import_batch,
            document_uploads::commands::document_uploads_import_folder,
            document_uploads::commands::document_uploads_import_pasted_text,
            document_uploads::commands::document_uploads_import_paths,
            document_uploads::commands::document_uploads_cancel_import_batch,
            document_uploads::commands::document_uploads_list,
            document_uploads::commands::document_uploads_update_title,
            document_uploads::commands::document_uploads_search,
            document_uploads::commands::document_uploads_concordance,
            document_uploads::commands::document_uploads_find_pdf_text,
            document_uploads::commands::document_uploads_get_source,
            document_uploads::commands::document_uploads_get_cover,
            document_uploads::commands::document_uploads_get_pdf_source,
            document_uploads::commands::document_uploads_get_pdf_asset_path,
            document_uploads::commands::document_uploads_get_pdf_narration_segments,
            document_uploads::commands::document_uploads_get_pdf_page_text,
            document_uploads::commands::document_uploads_pdf_has_ocr_text,
            document_uploads::commands::document_uploads_store_pdf_page_text,
            document_uploads::commands::document_uploads_finalize_pdf,
            document_uploads::commands::document_uploads_delete,
            document_uploads::commands::document_uploads_delete_batch,
            document_uploads::commands::document_uploads_library_organization,
            document_uploads::commands::document_uploads_create_folder,
            document_uploads::commands::document_uploads_rename_folder,
            document_uploads::commands::document_uploads_delete_folder,
            document_uploads::commands::document_uploads_move_documents,
            document_uploads::commands::document_uploads_move_folder,
            document_uploads::commands::document_uploads_reorder_library,
            document_scanner::commands::document_scanner_availability,
            document_scanner::commands::document_scanner_scan,
            document_scanner::commands::document_scanner_import_images,
            library_transfer::diagnostics::library_transfer_diagnostics,
            library_transfer::library_transfer_export,
            library_transfer::library_transfer_import,
            library_transfer::network::library_transfer_network_start,
            library_transfer::network::library_transfer_network_status,
            library_transfer::network::library_transfer_network_cancel,
            library_transfer::network::library_transfer_network_retry,
            library_transfer::network::library_transfer_network_dismiss,
            library_transfer::network::library_transfer_interfaces,
            library_transfer::network::discovery::library_transfer_discovery_start,
            library_transfer::network::discovery::library_transfer_discovery_status,
            library_transfer::network::discovery::library_transfer_discovery_stop,
            library_transfer::network::discovery::library_transfer_discovery_select,
            native_tts::commands::tts_native_capabilities,
            native_tts::commands::tts_model_status,
            native_tts::commands::tts_install_model,
            native_tts::commands::tts_native_audiobook_status,
            native_tts::commands::tts_list_saved_audiobooks,
            native_tts::commands::tts_get_native_audiobook_chunk,
            native_tts::commands::tts_preview_voice,
            native_tts::commands::tts_prepare_native_audiobook_playback,
            native_tts::commands::tts_save_audiobook_native,
            native_tts::commands::tts_cancel_audiobook_save,
            native_tts::commands::tts_export_audiobook_native,
            native_tts::commands::tts_import_audiobook_native,
            native_tts::commands::tts_get_imported_audiobook_source,
            native_tts::commands::tts_get_imported_audiobook_metadata,
            native_tts::commands::tts_delete_audiobook_native,
            native_tts::commands::tts_probe_silma_sidecar,
        ])
        .setup(|app| {
            library_transfer::initialize_storage(app.handle()).map_err(std::io::Error::other)?;
            document_uploads::schedule_search_index_rebuild(app.handle().clone());
            #[cfg(desktop)]
            open_documents::queue_cli_paths(
                app.handle(),
                std::env::args_os().skip(1),
                &std::env::current_dir().unwrap_or_default(),
            );
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(&event, tauri::RunEvent::Exit) {
                app.state::<library_transfer::LibraryTransferState>()
                    .set_backgrounded(true);
            }
            if matches!(&event, tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } if label == "main") {
                app.state::<library_transfer::LibraryTransferState>().cancel_network_sessions("closed");
            }
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = event {
                open_documents::queue_opened_urls(app, urls);
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
            let _ = (app, event);
        });
}
