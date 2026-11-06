# Erase all data

Settings > Reset > **Erase all data** removes everything Papercut stores on the
device and returns the app to its starting state. It is available in native builds
only. It can't be undone, and it isn't a secure overwrite: it deletes files through
the filesystem.

## Flow

| Step | Desktop | Android and iOS |
| --- | --- | --- |
| Refuse if a job is active | Yes | Yes |
| Write marker `pending-native` | Yes | Yes |
| Wipe | At next startup, after `request_restart` | In-process, after releasing audio and engines |
| Marker becomes `pending-webview` | Yes | Yes |
| Clear WebView storage | Boot gate in `src/main.tsx`, before app modules load | Same, after `location.reload()` |
| Remove marker, show notice | Yes | Yes |

Desktop wipes at startup because Windows can't delete open files, and because TTS
installs and the startup index rebuild have no cancel. Mobile has no relaunch API,
so it wipes in-process, where unlinking open files is safe.

WebView storage is cleared at boot rather than before reloading, since `pagehide`
flushes can rewrite localStorage during shutdown. Modules read localStorage at
import time, so `main.tsx` imports `renderApp.tsx` only after the gate.

## Rules to preserve

- **Allowlist only.** `ERASABLE_CHILDREN` in `src-tauri/src/app_reset.rs` names what
  goes. Never delete a root: on Android `app_data_dir` is the sandbox, and on Linux
  and Windows the roots hold the WebView profile.
- **Add new stores to the list.** A new top-level directory under app data or cache
  survives an erase until it is listed.
- **Wipe at most once per request.** The marker advances even after a partial
  failure. Retrying on a later launch would destroy documents imported since.
- **Exact marker content.** A stray or truncated file must never trigger a wipe.
- **Never block launch.** Boot gate and wipe failures are logged or shown, not fatal.

## What the dialog shows

Counts, total size, and on desktop the app data and cache folders, so users can inspect
them. Paths are hidden on mobile, where users can't browse the sandbox. The total covers
the erase list only, so it excludes WebView storage. The folders also hold WebView files
that stay after an erase, and the dialog says so.

## Not removed

- Exports, and any file outside the app's directories
- OS backups. Android backup is on by default, and iOS backs up Application Support,
  so a restore can bring erased data back
- `PAPERCUT_SILMA_MODEL_DIR` and other external development paths
- iOS `Documents/` save-dialog placeholders and any OS-created `Documents/Inbox` copies
- Debug-build logs

Library export is not a full backup, and the dialog says so. It omits bookmarks and
settings, and includes audiobooks only when selected.

## Testing safely

Dev and release desktop builds share `io.papercut.desktop`, so `tauri dev` uses your
real library. On Linux, always use a disposable profile:

```bash
XDG_DATA_HOME=/tmp/papercut-erase/data XDG_CACHE_HOME=/tmp/papercut-erase/cache npm run tauri dev
```

Unit tests cover the wipe, the marker and the dialog. Device acceptance is tracked in
[bug-squash-smoke-tests.md](bug-squash-smoke-tests.md).

## Removing this feature

1. Delete `src-tauri/src/app_reset.rs`, `src/app-reset/` and this file.
2. In `src-tauri/src/lib.rs`, remove the module, the four commands, `erased` and its
   `queue_cli_paths` guard.
3. Remove the `is_busy`, `is_running`, `unload`, `unload_engine` and `forget_finished`
   accessors, and make `cleanup_orphaned_transfer_files` private again.
4. Remove `forgetNativeAudio` from `src/tts/playback/nativeMobileAudio.ts`.
5. Move `src/renderApp.tsx` back to `src/main.tsx`, restoring `import './index.css'` and
   dropping `AppResetNotice`.
6. In `AppSettings.tsx`, `AppSettings.css` and `App.tsx`, remove the erase section, view
   and `eraseBlockedBy`.
7. Remove `settings.erase` from all 8 locale files.
