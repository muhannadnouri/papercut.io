package io.papercut.documentscanner

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.util.concurrent.Executors

@InvokeArg
class ScanArgs {
    lateinit var outputPath: String
}

/** Bridges Tauri to a plugin-owned Activity so camera lifecycle and review UI
 * remain native while only the completed PDF path crosses the IPC boundary. */
@TauriPlugin
class DocumentScannerPlugin(private val activity: Activity) : Plugin(activity) {
    private val imageImportWorker = Executors.newSingleThreadExecutor()

    @Command
    fun availability(invoke: Invoke) {
        val supported = activity.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)
        val result = JSObject()
        result.put("supported", supported)
        // ACTION_OPEN_DOCUMENT is part of the Android platform. Querying its
        // handler is unreliable under Android 11 package-visibility rules.
        result.put("photoImportSupported", true)
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

    /** Uses Android's system document picker, which works without Google Play
     * Services or broad media permissions, then builds the PDF off the UI thread. */
    @Command
    fun importImages(invoke: Invoke) {
        try {
            invoke.parseArgs(ScanArgs::class.java)
            startActivityForResult(invoke, imagePickerIntent(), "importImagesResult")
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Unable to open the photo picker")
        }
    }

    /** Copies content-provider images into plugin-owned storage before the
     * picker grant expires; decoding and PDF assembly remain page-bounded. */
    @ActivityCallback
    fun importImagesResult(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode == Activity.RESULT_CANCELED) {
            invoke.reject("Photo import cancelled")
            return
        }
        if (result.resultCode != Activity.RESULT_OK) {
            invoke.reject("Photo import failed")
            return
        }

        val uris = selectedImageUris(result.data)
        if (uris.isEmpty()) {
            invoke.reject("No photos were selected")
            return
        }
        val outputPath = try {
            invoke.parseArgs(ScanArgs::class.java).outputPath
        } catch (error: Exception) {
            invoke.reject(error.message ?: "The photo import destination is invalid")
            return
        }

        imageImportWorker.execute {
            try {
                val pageCount = ImageImportProcessing.writePdf(
                    activity.contentResolver,
                    uris,
                    File(outputPath),
                )
                activity.runOnUiThread {
                    val response = JSObject()
                    response.put("outputPath", outputPath)
                    response.put("pageCount", pageCount)
                    invoke.resolve(response)
                }
            } catch (error: Throwable) {
                activity.runOnUiThread {
                    invoke.reject(error.message ?: "Unable to prepare the selected photos")
                }
            }
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

    private fun imagePickerIntent() = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "image/*"
        putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }

    /** Retains picker order while removing duplicate URIs returned by unusual
     * document providers that populate both data and ClipData. */
    private fun selectedImageUris(data: Intent?): List<Uri> {
        val selected = linkedSetOf<Uri>()
        data?.clipData?.let { clips ->
            for (index in 0 until clips.itemCount) selected.add(clips.getItemAt(index).uri)
        }
        data?.data?.let(selected::add)
        return selected.toList()
    }
}
