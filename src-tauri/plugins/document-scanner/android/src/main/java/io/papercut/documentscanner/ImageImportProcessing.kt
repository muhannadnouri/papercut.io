package io.papercut.documentscanner

import android.content.ContentResolver
import android.net.Uri
import java.io.File
import java.io.FileOutputStream
import java.util.UUID

/** Converts system-picker URIs into the same bounded image-only PDF contract
 * as camera capture. The caller owns only the completed PDF, never raw bytes. */
object ImageImportProcessing {
    private const val MAX_SOURCE_BYTES = 64L * 1024L * 1024L

    /** Copies, decodes, normalizes, and releases one selected image at a time.
     * Reviewed JPEG pages stay on disk until PDF assembly, bounding bitmap use
     * independently of the selected page count. */
    fun writePdf(contentResolver: ContentResolver, uris: List<Uri>, output: File): Int {
        require(uris.isNotEmpty()) { "Select at least one photo" }
        require(uris.size <= ScanImageProcessing.MAX_DOCUMENT_PAGES) {
            "Select no more than ${ScanImageProcessing.MAX_DOCUMENT_PAGES} photos at once"
        }
        val session = File(output.parentFile, ".photo-import-${UUID.randomUUID()}")
        val pages = mutableListOf<File>()
        session.mkdirs()
        try {
            uris.forEachIndexed { index, uri ->
                require(ScanImageProcessing.hasWorkingSpace(session)) {
                    "Free some storage before importing these photos"
                }
                val source = File(session, "source-$index")
                copyBounded(contentResolver, uri, source)
                val decoded = ScanImageProcessing.decodeUpright(source)
                val normalized = try {
                    ScanImageProcessing.normalizeImportedPage(decoded)
                } finally {
                    decoded.recycle()
                    source.delete()
                }
                val page = File(session, "page-$index.jpg")
                try {
                    ScanImageProcessing.savePage(normalized, page)
                } finally {
                    normalized.recycle()
                }
                pages.add(page)
            }

            val pageBytes = pages.fold(0L) { total, page ->
                if (Long.MAX_VALUE - total < page.length()) Long.MAX_VALUE else total + page.length()
            }
            require(pageBytes <= ScanImageProcessing.MAX_PDF_BYTES) {
                "The selected photos exceed the 250 MB PDF limit"
            }
            require(ScanImageProcessing.hasWorkingSpace(output, pageBytes)) {
                "Free some storage before importing these photos"
            }
            ScanImageProcessing.writePdf(pages, output)
            return pages.size
        } catch (error: Throwable) {
            output.delete()
            throw error
        } finally {
            session.deleteRecursively()
        }
    }

    /** Enforces a per-image trust-boundary limit without first buffering the
     * content provider's stream or trusting its optional size metadata. */
    private fun copyBounded(contentResolver: ContentResolver, uri: Uri, output: File) {
        val input = contentResolver.openInputStream(uri)
            ?: error("Unable to read a selected photo")
        var total = 0L
        input.use { source ->
            FileOutputStream(output).use { destination ->
                val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
                while (true) {
                    val read = source.read(buffer)
                    if (read < 0) break
                    total += read
                    require(total <= MAX_SOURCE_BYTES) {
                        "A selected photo exceeds the 64 MB limit"
                    }
                    destination.write(buffer, 0, read)
                }
            }
        }
        require(total > 0) { "A selected photo is empty" }
    }
}
