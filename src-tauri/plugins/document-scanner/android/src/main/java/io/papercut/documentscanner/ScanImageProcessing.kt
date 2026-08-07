package io.papercut.documentscanner

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.pdf.PdfDocument
import androidx.exifinterface.media.ExifInterface
import java.io.File
import java.io.FileOutputStream
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.roundToInt

data class ScanPoint(val x: Float, val y: Float)

object ScanImageProcessing {
    /** Native capture uses a tighter page ceiling than general PDF import so a
     * camera session cannot become an unbounded bitmap-encoding job. */
    const val MAX_DOCUMENT_PAGES = 500
    /** Matches Rust's canonical PDF byte limit; Rust validates it again after
     * the native plugin returns because platform output crosses IPC. */
    const val MAX_PDF_BYTES = 250L * 1024L * 1024L

    private const val MAX_DECODE_DIMENSION = 4096
    private const val MAX_PREVIEW_DIMENSION = 1600
    private const val MAX_OUTPUT_DIMENSION = 3000
    private const val MAX_THUMBNAIL_DIMENSION = 160
    private const val PDF_SOURCE_DPI = 300f

    /** Decodes a camera JPEG near the maximum useful OCR resolution and applies
     * EXIF orientation up front, keeping crop coordinates stable thereafter. */
    fun decodeUpright(file: File): Bitmap {
        val source = decodeBounded(file, MAX_DECODE_DIMENSION)

        val orientation = ExifInterface(file).getAttributeInt(
            ExifInterface.TAG_ORIENTATION,
            ExifInterface.ORIENTATION_NORMAL,
        )
        val matrix = Matrix().apply {
            when (orientation) {
                ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> setScale(-1f, 1f)
                ExifInterface.ORIENTATION_ROTATE_180 -> setRotate(180f)
                ExifInterface.ORIENTATION_FLIP_VERTICAL -> setScale(1f, -1f)
                ExifInterface.ORIENTATION_TRANSPOSE -> {
                    setRotate(90f)
                    postScale(-1f, 1f)
                }
                ExifInterface.ORIENTATION_ROTATE_90 -> setRotate(90f)
                ExifInterface.ORIENTATION_TRANSVERSE -> {
                    setRotate(-90f)
                    postScale(-1f, 1f)
                }
                ExifInterface.ORIENTATION_ROTATE_270 -> setRotate(-90f)
            }
        }
        if (matrix.isIdentity) return source
        return Bitmap.createBitmap(source, 0, 0, source.width, source.height, matrix, true).also {
            if (it !== source) source.recycle()
        }
    }

    /** Decodes one accepted page for management without returning to the
     * camera-sized bitmap retained for crop quality. Only this preview is held. */
    fun decodePreview(file: File): Bitmap = decodeBounded(file, MAX_PREVIEW_DIMENSION)

    fun rotateClockwise(source: Bitmap): Bitmap = Bitmap.createBitmap(
        source,
        0,
        0,
        source.width,
        source.height,
        Matrix().apply { setRotate(90f) },
        true,
    )

    /** Normalizes an existing photo to the same white, OCR-sized page boundary
     * as a full-frame camera crop without introducing a second image pipeline. */
    fun normalizeImportedPage(source: Bitmap): Bitmap = rectify(
        source,
        listOf(
            ScanPoint(0f, 0f),
            ScanPoint((source.width - 1).toFloat(), 0f),
            ScanPoint((source.width - 1).toFloat(), (source.height - 1).toFloat()),
            ScanPoint(0f, (source.height - 1).toFloat()),
        ),
    )

    /** Maps the reviewed quadrilateral to a rectangle. Output is capped because
     * larger camera frames materially increase memory without helping the OCR
     * target beyond the roughly 300-DPI source retained here. */
    fun rectify(source: Bitmap, corners: List<ScanPoint>): Bitmap {
        require(corners.size == 4) { "Four crop corners are required" }
        val top = distance(corners[0], corners[1])
        val right = distance(corners[1], corners[2])
        val bottom = distance(corners[2], corners[3])
        val left = distance(corners[3], corners[0])
        val rawWidth = max(top, bottom).coerceAtLeast(1f)
        val rawHeight = max(left, right).coerceAtLeast(1f)
        val scale = minOf(1f, MAX_OUTPUT_DIMENSION / max(rawWidth, rawHeight))
        val width = (rawWidth * scale).roundToInt().coerceAtLeast(1)
        val height = (rawHeight * scale).roundToInt().coerceAtLeast(1)

        val sourcePoints = corners.flatMap { listOf(it.x, it.y) }.toFloatArray()
        val destinationPoints = floatArrayOf(
            0f, 0f,
            width.toFloat(), 0f,
            width.toFloat(), height.toFloat(),
            0f, height.toFloat(),
        )
        val transform = Matrix()
        check(transform.setPolyToPoly(sourcePoints, 0, destinationPoints, 0, 4)) {
            "Unable to apply the selected page crop"
        }

        return Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).also { output ->
            Canvas(output).apply {
                drawColor(Color.WHITE)
                drawBitmap(source, transform, Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG))
            }
        }
    }

    fun savePage(bitmap: Bitmap, output: File) {
        output.parentFile?.mkdirs()
        FileOutputStream(output).use { stream ->
            check(bitmap.compress(Bitmap.CompressFormat.JPEG, 92, stream)) {
                "Unable to save the reviewed page"
            }
        }
    }

    /** Persists a deliberately small navigation image. Keeping thumbnails on
     * disk lets the scanner release every full page between review actions. */
    fun saveThumbnail(bitmap: Bitmap, output: File) {
        val scale = minOf(
            1f,
            MAX_THUMBNAIL_DIMENSION.toFloat() / max(bitmap.width, bitmap.height),
        )
        val width = (bitmap.width * scale).roundToInt().coerceAtLeast(1)
        val height = (bitmap.height * scale).roundToInt().coerceAtLeast(1)
        val thumbnail = if (width == bitmap.width && height == bitmap.height) {
            bitmap
        } else {
            Bitmap.createScaledBitmap(bitmap, width, height, true)
        }
        try {
            output.parentFile?.mkdirs()
            FileOutputStream(output).use { stream ->
                check(thumbnail.compress(Bitmap.CompressFormat.JPEG, 84, stream)) {
                    "Unable to save the page thumbnail"
                }
            }
        } finally {
            if (thumbnail !== bitmap) thumbnail.recycle()
        }
    }

    /** Builds the canonical image-only PDF one page bitmap at a time. Accepted
     * JPEGs remain the session boundary, so multi-page scans do not retain every
     * decoded page in memory while the PDF is assembled. Page and final-byte
     * checks remain here as the common backstop for camera and photo input. */
    fun writePdf(pages: List<File>, output: File) {
        require(pages.isNotEmpty()) { "The document scan has no pages" }
        require(pages.size <= MAX_DOCUMENT_PAGES) {
            "The scanned document exceeds the $MAX_DOCUMENT_PAGES-page limit"
        }
        output.parentFile?.mkdirs()
        val document = PdfDocument()
        try {
            pages.forEachIndexed { index, pageFile ->
                val bitmap = BitmapFactory.decodeFile(pageFile.path)
                    ?: error("Unable to read scanned page ${index + 1}")
                try {
                    val widthPoints = (bitmap.width * 72f / PDF_SOURCE_DPI).roundToInt().coerceAtLeast(1)
                    val heightPoints = (bitmap.height * 72f / PDF_SOURCE_DPI).roundToInt().coerceAtLeast(1)
                    val page = document.startPage(
                        PdfDocument.PageInfo.Builder(widthPoints, heightPoints, index + 1).create(),
                    )
                    page.canvas.drawColor(Color.WHITE)
                    page.canvas.drawBitmap(
                        bitmap,
                        null,
                        Rect(0, 0, widthPoints, heightPoints),
                        Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG),
                    )
                    document.finishPage(page)
                } finally {
                    bitmap.recycle()
                }
            }
            FileOutputStream(output).use(document::writeTo)
            require(output.length() <= MAX_PDF_BYTES) {
                "The scanned document exceeds the 250 MB PDF limit"
            }
        } catch (error: Throwable) {
            output.delete()
            throw error
        } finally {
            document.close()
        }
    }

    private fun distance(first: ScanPoint, second: ScanPoint): Float =
        hypot(second.x - first.x, second.y - first.y)

    /** Uses power-of-two sampling so Android never allocates the original
     * camera bitmap merely to create a bounded review or management preview. */
    private fun decodeBounded(file: File, maxDimension: Int): Bitmap {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, bounds)
        if (bounds.outWidth < 1 || bounds.outHeight < 1) {
            error("Unable to read the captured page")
        }

        var sampleSize = 1
        while (bounds.outWidth / sampleSize > maxDimension ||
            bounds.outHeight / sampleSize > maxDimension
        ) {
            sampleSize *= 2
        }
        return BitmapFactory.decodeFile(
            file.path,
            BitmapFactory.Options().apply {
                inSampleSize = sampleSize
                inPreferredConfig = Bitmap.Config.ARGB_8888
            },
        ) ?: error("Unable to decode the captured page")
    }
}
