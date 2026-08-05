package io.papercut.documentscanner

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class ScanArgs {
    lateinit var outputPath: String
}

/** Bridges Tauri to a plugin-owned Activity so camera lifecycle and review UI
 * remain native while only the completed PDF path crosses the IPC boundary. */
@TauriPlugin
class DocumentScannerPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun availability(invoke: Invoke) {
        val supported = activity.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)
        val result = JSObject()
        result.put("supported", supported)
        result.put("platform", "android")
        result.put("reason", if (supported) null else "Document scanning requires a camera")
        invoke.resolve(result)
    }

    /** Starts one native scanning session. The Activity writes directly to the
     * app-owned staging path supplied by Rust, avoiding image bytes over IPC. */
    @Command
    fun scan(invoke: Invoke) {
        try {
            if (!activity.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)) {
                invoke.reject("Document scanning requires a camera")
                return
            }
            val args = invoke.parseArgs(ScanArgs::class.java)
            val intent = Intent(activity, DocumentScannerActivity::class.java).apply {
                putExtra(DocumentScannerActivity.EXTRA_OUTPUT_PATH, args.outputPath)
            }
            startActivityForResult(invoke, intent, "scanResult")
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Unable to start document scanning")
        }
    }

    /** Converts Android's Activity result into the same compact path/page-count
     * response used by iOS; Rust performs the final path and file validation. */
    @ActivityCallback
    fun scanResult(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode == Activity.RESULT_CANCELED) {
            invoke.reject(result.data?.getStringExtra(DocumentScannerActivity.EXTRA_ERROR)
                ?: "Document scan cancelled")
            return
        }
        if (result.resultCode != Activity.RESULT_OK) {
            invoke.reject(result.data?.getStringExtra(DocumentScannerActivity.EXTRA_ERROR)
                ?: "Document scanning failed")
            return
        }

        val outputPath = result.data?.getStringExtra(DocumentScannerActivity.EXTRA_OUTPUT_PATH)
        val pageCount = result.data?.getIntExtra(DocumentScannerActivity.EXTRA_PAGE_COUNT, 0) ?: 0
        if (outputPath.isNullOrBlank() || pageCount < 1) {
            invoke.reject("Document scanner returned an invalid result")
            return
        }
        val response = JSObject()
        response.put("outputPath", outputPath)
        response.put("pageCount", pageCount)
        invoke.resolve(response)
    }
}
