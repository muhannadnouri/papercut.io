package io.papercut.documentscanner

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/** Keeps Android registration compiling while its CameraX capture/review
 * adapter is implemented as the next independent scanner stage. */
@TauriPlugin
class DocumentScannerPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun availability(invoke: Invoke) {
        val result = JSObject()
        result.put("supported", false)
        result.put("platform", "android")
        result.put("reason", "Android document capture is not available yet")
        invoke.resolve(result)
    }

    @Command
    fun scan(invoke: Invoke) {
        invoke.reject("Android document capture is not available yet")
    }
}
