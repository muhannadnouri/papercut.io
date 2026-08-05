package io.papercut.documentscanner

import android.Manifest
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Bundle
import android.os.StatFs
import android.provider.Settings
import android.view.Gravity
import android.view.Surface
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.HorizontalScrollView
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import java.io.File
import java.util.UUID
import java.util.Collections
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/** Owns Android's capture/review lifecycle and returns one finalized PDF path.
 * The surrounding Tauri plugin deliberately receives no image buffers, while
 * Rust remains responsible for persistence, indexing, and OCR after capture. */
class DocumentScannerActivity : ComponentActivity() {
    companion object {
        const val EXTRA_OUTPUT_PATH = "documentScannerOutputPath"
        const val EXTRA_PAGE_COUNT = "documentScannerPageCount"
        const val EXTRA_ERROR = "documentScannerError"

        private const val STATE_SESSION_ID = "documentScannerSessionId"
        private const val STATE_PAGE_IMAGES = "documentScannerPageImages"
        private const val STATE_PAGE_THUMBNAILS = "documentScannerPageThumbnails"
        private const val MIN_FREE_SPACE_BYTES = 64L * 1024L * 1024L
        private const val STALE_SESSION_AGE_MS = 7L * 24L * 60L * 60L * 1000L
    }

    private val worker: ExecutorService = Executors.newSingleThreadExecutor()
    private data class AcceptedPage(val image: File, val thumbnail: File)

    private val acceptedPages = mutableListOf<AcceptedPage>()
    private lateinit var sessionDirectory: File
    private lateinit var outputFile: File
    private var cameraProvider: ProcessCameraProvider? = null
    private var imageCapture: ImageCapture? = null
    private var previewView: PreviewView? = null
    private var reviewBitmap: Bitmap? = null
    private var managedPageBitmap: Bitmap? = null
    private var reviewCapture: File? = null
    private var cropOverlay: CropOverlayView? = null
    private var awaitingPermissionSettings = false
    private var processing = false

    private val permissionRequest = registerForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) showCapture() else showPermissionExplanation()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        title = getString(R.string.scanner_title)
        val path = intent.getStringExtra(EXTRA_OUTPUT_PATH)
        if (path.isNullOrBlank()) {
            fail("A scan destination was not provided")
            return
        }
        outputFile = File(path)
        if (!prepareSession(savedInstanceState)) {
            fail("Unable to prepare temporary scan storage")
            return
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (processing) return
                when {
                    managedPageBitmap != null -> showCapture()
                    reviewBitmap != null -> discardCurrentCapture()
                    else -> confirmCancel()
                }
            }
        })

        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            showCapture()
        } else {
            permissionRequest.launch(Manifest.permission.CAMERA)
        }
    }

    /** Saves only durable accepted-page filenames. Android may recreate this
     * Activity while the host process remains alive; the current camera frame
     * or unaccepted review is deliberately retaken instead of serialized. */
    override fun onSaveInstanceState(outState: Bundle) {
        if (::sessionDirectory.isInitialized) {
            touchSession()
            outState.putString(STATE_SESSION_ID, sessionDirectory.name)
            outState.putStringArrayList(
                STATE_PAGE_IMAGES,
                ArrayList(acceptedPages.map { it.image.name }),
            )
            outState.putStringArrayList(
                STATE_PAGE_THUMBNAILS,
                ArrayList(acceptedPages.map { it.thumbnail.name }),
            )
        }
        super.onSaveInstanceState(outState)
    }

    /** Re-checks permission after the user returns from system settings without
     * launching another scanner Activity or losing already accepted pages. */
    override fun onResume() {
        super.onResume()
        if (awaitingPermissionSettings &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            awaitingPermissionSettings = false
            showCapture()
        }
    }

    /** The Activity handles orientation itself so accepted temporary pages are
     * retained; only CameraX's target rotation needs refreshing. */
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        previewView?.post {
            imageCapture?.targetRotation = previewView?.display?.rotation ?: Surface.ROTATION_0
        }
    }

    override fun onDestroy() {
        cameraProvider?.unbindAll()
        reviewBitmap?.recycle()
        managedPageBitmap?.recycle()
        worker.shutdown()
        // Explicit finish/cancel paths own cleanup. Retaining this directory
        // here is what lets Android recreate an interrupted Activity safely;
        // abandoned sessions are bounded by prepareSession's stale sweep.
        super.onDestroy()
    }

    /** Restores accepted page files from Android's saved Activity state and
     * rejects filenames outside the plugin-owned cache directory. This covers
     * Activity recreation, not a killed process or app restart (Stage 9). */
    private fun prepareSession(savedState: Bundle?): Boolean {
        val root = File(cacheDir, "document-scanner")
        if (!root.mkdirs() && !root.isDirectory) return false
        cleanupStaleSessions(root)

        val restoredId = savedState?.getString(STATE_SESSION_ID)
        val sessionId = if (restoredId != null) {
            try {
                UUID.fromString(restoredId).toString()
            } catch (_: IllegalArgumentException) {
                return false
            }
        } else {
            UUID.randomUUID().toString()
        }
        sessionDirectory = File(root, sessionId)
        if (!sessionDirectory.mkdirs() && !sessionDirectory.isDirectory) return false

        if (restoredId == null) return true
        val images = savedState.getStringArrayList(STATE_PAGE_IMAGES) ?: arrayListOf()
        val thumbnails = savedState.getStringArrayList(STATE_PAGE_THUMBNAILS) ?: arrayListOf()
        if (images.size != thumbnails.size) return false
        for (index in images.indices) {
            val image = restoredSessionFile(images[index], "page-") ?: return false
            val thumbnail = restoredSessionFile(thumbnails[index], "thumbnail-") ?: return false
            acceptedPages.add(AcceptedPage(image, thumbnail))
        }
        cleanupUnacceptedSessionFiles()
        touchSession()
        return true
    }

    private fun restoredSessionFile(name: String, prefix: String): File? {
        if (!name.startsWith(prefix) || name.contains(File.separatorChar)) return null
        return File(sessionDirectory, name).takeIf { it.isFile }
    }

    /** Prevents interrupted or abandoned scans from accumulating forever while
     * leaving recent sessions available to Android's normal Activity restore. */
    private fun cleanupStaleSessions(root: File) {
        val cutoff = System.currentTimeMillis() - STALE_SESSION_AGE_MS
        root.listFiles()?.forEach { session ->
            if (session.isDirectory && session.lastModified() < cutoff) {
                session.deleteRecursively()
            }
        }
    }

    private fun cleanupUnacceptedSessionFiles() {
        val retained = acceptedPages.flatMapTo(mutableSetOf()) {
            listOf(it.image.name, it.thumbnail.name)
        }
        sessionDirectory.listFiles()?.forEach { file ->
            if (file.name !in retained) file.delete()
        }
    }

    private fun touchSession() {
        sessionDirectory.setLastModified(System.currentTimeMillis())
    }

    /** Builds the live camera state. Returning here after accepting a page
     * rebinds CameraX, but previous pages remain as compressed session files. */
    private fun showCapture() {
        processing = false
        reviewBitmap?.recycle()
        reviewBitmap = null
        managedPageBitmap?.recycle()
        managedPageBitmap = null
        reviewCapture = null
        cropOverlay = null

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(0xff101114.toInt())
        }
        root.addView(captureHeader())
        if (acceptedPages.isNotEmpty()) root.addView(acceptedPagesStrip())
        val preview = PreviewView(this).apply {
            implementationMode = PreviewView.ImplementationMode.PERFORMANCE
            scaleType = PreviewView.ScaleType.FILL_CENTER
        }
        previewView = preview
        root.addView(preview, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            0,
            1f,
        ))
        root.addView(actionButton(getString(R.string.scanner_capture)) { capturePage() }.apply {
            contentDescription = getString(R.string.scanner_capture)
        }, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            dp(64),
        ).apply { setMargins(dp(16), dp(12), dp(16), dp(16)) })
        setContentView(root)
        preview.post { bindCamera(preview) }
    }

    /** Shows lightweight on-disk thumbnails rather than keeping accepted page
     * bitmaps alive. Tapping one opens a single bounded management preview. */
    private fun acceptedPagesStrip(): View = HorizontalScrollView(this).apply {
        isHorizontalScrollBarEnabled = false
        addView(LinearLayout(this@DocumentScannerActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dp(8), dp(4), dp(8), dp(4))
            acceptedPages.forEachIndexed { index, page ->
                addView(LinearLayout(this@DocumentScannerActivity).apply {
                    orientation = LinearLayout.VERTICAL
                    gravity = Gravity.CENTER
                    isClickable = true
                    isFocusable = true
                    contentDescription = getString(R.string.scanner_manage_page, index + 1)
                    setOnClickListener { showPageManager(index) }
                    addView(ImageView(this@DocumentScannerActivity).apply {
                        setImageBitmap(BitmapFactory.decodeFile(page.thumbnail.path))
                        scaleType = ImageView.ScaleType.CENTER_CROP
                    }, LinearLayout.LayoutParams(dp(64), dp(72)))
                    addView(TextView(this@DocumentScannerActivity).apply {
                        text = getString(R.string.scanner_page_number, index + 1)
                        gravity = Gravity.CENTER
                        setTextColor(0xffeeeeee.toInt())
                    }, LinearLayout.LayoutParams(dp(64), dp(24)))
                }, LinearLayout.LayoutParams(dp(72), dp(104)).apply {
                    setMargins(dp(4), 0, dp(4), 0)
                })
            }
        })
    }

    private fun captureHeader(): View = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(dp(8), dp(8), dp(8), dp(8))
        addView(actionButton(getString(R.string.scanner_cancel)) { confirmCancel() })
        addView(TextView(this@DocumentScannerActivity).apply {
            text = resources.getQuantityString(
                R.plurals.scanner_pages_captured,
                acceptedPages.size,
                acceptedPages.size,
            )
            setTextColor(0xffeeeeee.toInt())
            gravity = Gravity.CENTER
        }, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        if (acceptedPages.isNotEmpty()) {
            addView(actionButton(getString(R.string.scanner_finish)) { finalizeScan() })
        }
    }

    /** Selects a back camera when available and falls back to any front camera,
     * allowing the plugin's availability check to remain honest on odd devices. */
    private fun bindCamera(previewView: PreviewView) {
        val providerFuture = ProcessCameraProvider.getInstance(this)
        providerFuture.addListener({
            try {
                val provider = providerFuture.get()
                val selector = when {
                    provider.hasCamera(CameraSelector.DEFAULT_BACK_CAMERA) ->
                        CameraSelector.DEFAULT_BACK_CAMERA
                    provider.hasCamera(CameraSelector.DEFAULT_FRONT_CAMERA) ->
                        CameraSelector.DEFAULT_FRONT_CAMERA
                    else -> {
                        fail(getString(R.string.scanner_camera_unavailable))
                        return@addListener
                    }
                }
                val preview = Preview.Builder().build().apply {
                    surfaceProvider = previewView.surfaceProvider
                }
                val capture = ImageCapture.Builder()
                    .setCaptureMode(ImageCapture.CAPTURE_MODE_MAXIMIZE_QUALITY)
                    .setTargetRotation(previewView.display?.rotation ?: Surface.ROTATION_0)
                    .build()
                provider.unbindAll()
                provider.bindToLifecycle(this, selector, preview, capture)
                cameraProvider = provider
                imageCapture = capture
            } catch (error: Exception) {
                fail(error.message ?: getString(R.string.scanner_camera_unavailable))
            }
        }, ContextCompat.getMainExecutor(this))
    }

    /** Captures to a file so CameraX encoding and EXIF metadata stay intact until
     * the worker decodes one bounded review bitmap off the UI thread. */
    private fun capturePage() {
        if (!hasWorkingSpace(sessionDirectory)) {
            showLowStorage()
            return
        }
        val capture = imageCapture ?: return
        val captureFile = File(sessionDirectory, "capture-${System.nanoTime()}.jpg")
        capture.takePicture(
            ImageCapture.OutputFileOptions.Builder(captureFile).build(),
            worker,
            object : ImageCapture.OnImageSavedCallback {
                override fun onImageSaved(output: ImageCapture.OutputFileResults) {
                    try {
                        val bitmap = ScanImageProcessing.decodeUpright(captureFile)
                        runOnUiThread { showReview(bitmap, captureFile) }
                    } catch (_: Throwable) {
                        captureFile.delete()
                        runOnUiThread { showRecoverableFailure() }
                    }
                }

                override fun onError(error: ImageCaptureException) {
                    captureFile.delete()
                    runOnUiThread { showRecoverableFailure() }
                }
            },
        )
        // ImageCapture must remain bound until its callback; only the preview UI
        // is replaced while CameraX finishes writing the page in the background.
        showProgress(getString(R.string.scanner_processing), unbindCamera = false)
    }

    /** Presents the manual four-corner decision before a page joins the scan.
     * Retake and rotation operate only on the current bitmap and leave accepted
     * pages untouched. */
    private fun showReview(bitmap: Bitmap, captureFile: File) {
        processing = false
        cameraProvider?.unbindAll()
        reviewBitmap = bitmap
        reviewCapture = captureFile
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(0xff101114.toInt())
        }
        root.addView(TextView(this).apply {
            text = getString(R.string.scanner_adjust_corners)
            setTextColor(0xffeeeeee.toInt())
            gravity = Gravity.CENTER
            setPadding(dp(16), dp(12), dp(16), dp(12))
        })
        val overlay = CropOverlayView(this).also {
            it.setPageBitmap(bitmap)
            cropOverlay = it
        }
        root.addView(overlay, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            0,
            1f,
        ))
        root.addView(reviewActions())
        setContentView(root)
    }

    private fun reviewActions(): View = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(dp(12), dp(8), dp(12), dp(12))
        addView(actionRow(
            actionButton(getString(R.string.scanner_retake)) { discardCurrentCapture() },
            actionButton(getString(R.string.scanner_rotate)) { rotateReview() },
        ))
        addView(actionRow(
            actionButton(getString(R.string.scanner_add_page)) { acceptReview(false) },
            actionButton(getString(R.string.scanner_finish)) { acceptReview(true) },
        ))
    }

    private fun actionRow(first: Button, second: Button) = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        addView(first, LinearLayout.LayoutParams(0, dp(52), 1f).apply {
            setMargins(0, dp(4), dp(4), dp(4))
        })
        addView(second, LinearLayout.LayoutParams(0, dp(52), 1f).apply {
            setMargins(dp(4), dp(4), 0, dp(4))
        })
    }

    private fun rotateReview() {
        val source = reviewBitmap ?: return
        val rotated = ScanImageProcessing.rotateClockwise(source)
        reviewBitmap = rotated
        cropOverlay?.setPageBitmap(rotated)
        if (rotated !== source) source.recycle()
    }

    /** Loads exactly one accepted page for management. The list itself stores
     * files only, and its order is the final PDF page order. */
    private fun showPageManager(index: Int) {
        val page = acceptedPages.getOrNull(index) ?: return
        managedPageBitmap?.recycle()
        managedPageBitmap = null
        showProgress(getString(R.string.scanner_processing))
        worker.execute {
            try {
                val bitmap = ScanImageProcessing.decodePreview(page.image)
                runOnUiThread { renderPageManager(index, bitmap) }
            } catch (_: Throwable) {
                runOnUiThread { showRecoverableFailure() }
            }
        }
    }

    private fun renderPageManager(index: Int, bitmap: Bitmap) {
        processing = false
        managedPageBitmap = bitmap
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(0xff101114.toInt())
        }
        root.addView(TextView(this).apply {
            text = getString(R.string.scanner_page_position, index + 1, acceptedPages.size)
            setTextColor(0xffeeeeee.toInt())
            gravity = Gravity.CENTER
            setPadding(dp(16), dp(12), dp(16), dp(12))
        })
        root.addView(ImageView(this).apply {
            setImageBitmap(bitmap)
            scaleType = ImageView.ScaleType.FIT_CENTER
            contentDescription = getString(R.string.scanner_manage_page, index + 1)
        }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        root.addView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(12), dp(8), dp(12), dp(12))
            addView(actionRow(
                actionButton(getString(R.string.scanner_move_earlier)) {
                    moveManagedPage(index, index - 1)
                }.apply { isEnabled = index > 0 },
                actionButton(getString(R.string.scanner_move_later)) {
                    moveManagedPage(index, index + 1)
                }.apply { isEnabled = index < acceptedPages.lastIndex },
            ))
            addView(actionRow(
                actionButton(getString(R.string.scanner_delete_page)) {
                    confirmDeleteManagedPage(index)
                },
                actionButton(getString(R.string.scanner_back_to_camera)) { showCapture() },
            ))
        })
        setContentView(root)
    }

    /** Reorders the durable page entries directly; no JPEG is rewritten and
     * PDF assembly later consumes this same list order. */
    private fun moveManagedPage(from: Int, to: Int) {
        if (from !in acceptedPages.indices || to !in acceptedPages.indices) return
        Collections.swap(acceptedPages, from, to)
        touchSession()
        showPageManager(to)
    }

    private fun confirmDeleteManagedPage(index: Int) {
        AlertDialog.Builder(this)
            .setMessage(R.string.scanner_delete_page_confirm)
            .setNegativeButton(R.string.scanner_keep_scanning, null)
            .setPositiveButton(R.string.scanner_delete_page) { _, _ ->
                val removed = acceptedPages.removeAt(index)
                removed.image.delete()
                removed.thumbnail.delete()
                touchSession()
                if (acceptedPages.isEmpty()) showCapture()
                else showPageManager(index.coerceAtMost(acceptedPages.lastIndex))
            }
            .show()
    }

    /** Rectifies and compresses the accepted page on the worker. The UI returns
     * to live capture only after the page file is durable for this session. */
    private fun acceptReview(finishAfterPage: Boolean) {
        if (!hasWorkingSpace(sessionDirectory)) {
            showLowStorage()
            return
        }
        val source = reviewBitmap ?: return
        val captureFile = reviewCapture ?: return
        val corners = cropOverlay?.bitmapCorners() ?: return
        reviewBitmap = null
        showProgress(getString(R.string.scanner_processing))
        worker.execute {
            val pageFile = File(sessionDirectory, "page-${System.nanoTime()}.jpg")
            val thumbnailFile = File(sessionDirectory, "thumbnail-${System.nanoTime()}.jpg")
            try {
                val corrected = ScanImageProcessing.rectify(source, corners)
                // Identity must not depend on list position: deleting a middle
                // page and capturing another must never overwrite a retained JPEG.
                try {
                    ScanImageProcessing.savePage(corrected, pageFile)
                    ScanImageProcessing.saveThumbnail(corrected, thumbnailFile)
                } finally {
                    corrected.recycle()
                }
                captureFile.delete()
                source.recycle()
                runOnUiThread {
                    acceptedPages.add(AcceptedPage(pageFile, thumbnailFile))
                    touchSession()
                    reviewCapture = null
                    if (finishAfterPage) finalizeScan() else showCapture()
                }
            } catch (_: Throwable) {
                pageFile.delete()
                thumbnailFile.delete()
                captureFile.delete()
                source.recycle()
                runOnUiThread {
                    reviewCapture = null
                    showRecoverableFailure()
                }
            }
        }
    }

    /** Writes the reviewed page files to the Rust-supplied destination and only
     * then reports success; partial output is removed by the processing helper. */
    private fun finalizeScan() {
        if (acceptedPages.isEmpty()) return
        val estimatedOutputBytes = acceptedPages.fold(0L) { total, page ->
            val pageBytes = page.image.length()
            if (Long.MAX_VALUE - total < pageBytes) Long.MAX_VALUE else total + pageBytes
        }
        if (!hasWorkingSpace(outputFile, estimatedOutputBytes)) {
            showLowStorage()
            return
        }
        showProgress(getString(R.string.scanner_processing))
        worker.execute {
            try {
                ScanImageProcessing.writePdf(acceptedPages.map { it.image }, outputFile)
                sessionDirectory.deleteRecursively()
                runOnUiThread {
                    setResult(Activity.RESULT_OK, Intent().apply {
                        putExtra(EXTRA_OUTPUT_PATH, outputFile.path)
                        putExtra(EXTRA_PAGE_COUNT, acceptedPages.size)
                    })
                    finish()
                }
            } catch (_: Throwable) {
                runOnUiThread { showFinishFailure() }
            }
        }
    }

    /** Uses Android's filesystem accounting before camera and PDF writes. The
     * fixed reserve covers temporary encoding overhead; finalization also adds
     * the accepted JPEG bytes as a conservative output estimate. */
    private fun hasWorkingSpace(target: File, additionalBytes: Long = 0L): Boolean {
        var anchor = if (target.isDirectory) target else target.parentFile ?: cacheDir
        while (!anchor.exists()) anchor = anchor.parentFile ?: cacheDir
        val required = if (Long.MAX_VALUE - MIN_FREE_SPACE_BYTES < additionalBytes) {
            Long.MAX_VALUE
        } else {
            MIN_FREE_SPACE_BYTES + additionalBytes
        }
        // If Android cannot inspect the volume, let the actual write report its
        // error rather than incorrectly declaring scanning unsupported.
        return runCatching { StatFs(anchor.path).availableBytes >= required }.getOrDefault(true)
    }

    private fun showLowStorage() {
        AlertDialog.Builder(this)
            .setMessage(R.string.scanner_low_storage)
            .setPositiveButton(android.R.string.ok, null)
            .show()
    }

    /** Returns to capture after a transient camera, preview, or page-write
     * failure without deleting pages that were already accepted. */
    private fun showRecoverableFailure() {
        processing = false
        AlertDialog.Builder(this)
            .setMessage(R.string.scanner_operation_failed)
            .setPositiveButton(R.string.scanner_keep_scanning) { _, _ -> showCapture() }
            .setCancelable(false)
            .show()
    }

    /** Keeps accepted page files when final PDF assembly fails so a transient
     * write error never turns a recoverable scan into lost work. */
    private fun showFinishFailure() {
        processing = false
        AlertDialog.Builder(this)
            .setMessage(R.string.scanner_finish_failed)
            .setPositiveButton(R.string.scanner_keep_scanning) { _, _ -> showCapture() }
            .setCancelable(false)
            .show()
    }

    private fun discardCurrentCapture() {
        reviewBitmap?.recycle()
        reviewBitmap = null
        reviewCapture?.delete()
        reviewCapture = null
        showCapture()
    }

    private fun showPermissionExplanation() {
        AlertDialog.Builder(this)
            .setMessage(R.string.scanner_camera_permission)
            .setNegativeButton(R.string.scanner_cancel) { _, _ -> cancel("Camera permission denied") }
            .setPositiveButton(R.string.scanner_open_settings) { _, _ ->
                awaitingPermissionSettings = true
                startActivity(Intent(
                    Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                    Uri.parse("package:$packageName"),
                ))
            }
            .setCancelable(false)
            .show()
    }

    private fun confirmCancel() {
        if (acceptedPages.isEmpty() && reviewBitmap == null) {
            cancel("Document scan cancelled")
            return
        }
        AlertDialog.Builder(this)
            .setMessage(R.string.scanner_discard)
            .setNegativeButton(R.string.scanner_keep_scanning, null)
            .setPositiveButton(R.string.scanner_discard_scan) { _, _ ->
                cancel("Document scan cancelled")
            }
            .show()
    }

    private fun showProgress(message: String, unbindCamera: Boolean = true) {
        processing = true
        if (unbindCamera) cameraProvider?.unbindAll()
        previewView = null
        imageCapture = null
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(dp(24), dp(24), dp(24), dp(24))
            addView(ProgressBar(this@DocumentScannerActivity))
            addView(TextView(this@DocumentScannerActivity).apply {
                text = message
                gravity = Gravity.CENTER
                setPadding(0, dp(16), 0, 0)
            })
        })
    }

    private fun actionButton(label: String, action: () -> Unit) = Button(this).apply {
        text = label
        minHeight = dp(48)
        setOnClickListener { action() }
    }

    private fun cancel(message: String) {
        if (::sessionDirectory.isInitialized) sessionDirectory.deleteRecursively()
        setResult(Activity.RESULT_CANCELED, Intent().putExtra(EXTRA_ERROR, message))
        finish()
    }

    private fun fail(message: String) {
        if (::outputFile.isInitialized) outputFile.delete()
        cancel(message)
    }

    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()

}
