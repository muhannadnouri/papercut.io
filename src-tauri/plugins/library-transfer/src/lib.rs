//! Native app visibility, independent of WebView timers and focus events.
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Runtime,
};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_library_transfer);

pub fn init<R: Runtime>(
    on_background: impl Fn(&AppHandle<R>, bool) + Send + Sync + 'static,
) -> TauriPlugin<R> {
    #[cfg(desktop)]
    {
        let _ = on_background;
        Builder::new("library-transfer").build()
    }
    #[cfg(mobile)]
    {
        use std::sync::Arc;
        use tauri::{
            ipc::{Channel, InvokeResponseBody},
            Manager,
        };
        let callback = Arc::new(on_background);
        Builder::new("library-transfer")
            .setup(move |app, api| {
                #[cfg(target_os = "android")]
                let handle = api.register_android_plugin(
                    "io.papercut.librarytransfer",
                    "LibraryTransferPlugin",
                )?;
                #[cfg(target_os = "ios")]
                let handle = api.register_ios_plugin(init_plugin_library_transfer)?;
                let app = app.clone();
                let channel: Channel<bool> = Channel::new(move |body| {
                    if let InvokeResponseBody::Json(json) = body {
                        if let Ok(backgrounded) = serde_json::from_str::<bool>(&json) {
                            callback(&app, backgrounded);
                        }
                    }
                    Ok(())
                });
                api.app().manage(Observer {
                    handle,
                    channel,
                    started: std::sync::atomic::AtomicBool::new(false),
                });
                Ok(())
            })
            .on_webview_ready(|view| {
                let observer = view.state::<Observer<R>>();
                if observer
                    .started
                    .swap(true, std::sync::atomic::Ordering::AcqRel)
                {
                    return;
                }
                let handle = observer.handle.clone();
                let channel = observer.channel.clone();
                tauri::async_runtime::spawn(async move {
                    // No initial callback means LAN stays disabled; file transfer remains available.
                    let _result: Result<(), _> = handle
                        .run_mobile_plugin_async("observe", ObserveArgs { on_state: &channel })
                        .await;
                });
            })
            .build()
    }
}

#[cfg(mobile)]
struct Observer<R: Runtime> {
    handle: tauri::plugin::PluginHandle<R>,
    channel: tauri::ipc::Channel<bool>,
    started: std::sync::atomic::AtomicBool,
}
#[cfg(mobile)]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ObserveArgs<'a> {
    on_state: &'a tauri::ipc::Channel<bool>,
}
