package io.papercut.documentscanner

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.min

/** Displays one captured page with four large draggable corner handles. The
 * overlay owns only view-space points and maps them back to bitmap coordinates
 * when the Activity accepts the page. */
class CropOverlayView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {
    private val density = resources.displayMetrics.density
    private val imagePaint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG)
    private val shadePaint = Paint().apply { color = 0x99000000.toInt() }
    private val edgePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        style = Paint.Style.STROKE
        strokeWidth = 2f * density
    }
    private val handlePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = 0xff5146e5.toInt()
        style = Paint.Style.FILL
    }
    private val imageRect = RectF()
    private val corners = MutableList(4) { ScanPoint(0f, 0f) }
    private var bitmap: Bitmap? = null
    private var activeCorner = -1

    init {
        contentDescription = context.getString(R.string.scanner_adjust_corners)
    }

    fun setPageBitmap(page: Bitmap) {
        bitmap = page
        resetCorners = true
        invalidate()
    }

    /** Returns the reviewed corners in source-bitmap coordinates in clockwise
     * order: top-left, top-right, bottom-right, bottom-left. */
    fun bitmapCorners(): List<ScanPoint> {
        val page = bitmap ?: error("No captured page is available")
        check(imageRect.width() > 0f && imageRect.height() > 0f) {
            "The captured page is not ready for review"
        }
        return corners.map { point ->
            ScanPoint(
                ((point.x - imageRect.left) / imageRect.width() * page.width)
                    .coerceIn(0f, page.width.toFloat()),
                ((point.y - imageRect.top) / imageRect.height() * page.height)
                    .coerceIn(0f, page.height.toFloat()),
            )
        }
    }

    private var resetCorners = false

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val page = bitmap ?: return
        val scale = min(width.toFloat() / page.width, height.toFloat() / page.height)
        val displayedWidth = page.width * scale
        val displayedHeight = page.height * scale
        imageRect.set(
            (width - displayedWidth) / 2f,
            (height - displayedHeight) / 2f,
            (width + displayedWidth) / 2f,
            (height + displayedHeight) / 2f,
        )
        canvas.drawColor(Color.BLACK)
        canvas.drawBitmap(page, null, imageRect, imagePaint)

        if (resetCorners) {
            val inset = min(imageRect.width(), imageRect.height()) * 0.035f
            corners[0] = ScanPoint(imageRect.left + inset, imageRect.top + inset)
            corners[1] = ScanPoint(imageRect.right - inset, imageRect.top + inset)
            corners[2] = ScanPoint(imageRect.right - inset, imageRect.bottom - inset)
            corners[3] = ScanPoint(imageRect.left + inset, imageRect.bottom - inset)
            resetCorners = false
        }

        val cropPath = cropPath()
        canvas.save()
        canvas.clipOutPath(cropPath)
        canvas.drawRect(imageRect, shadePaint)
        canvas.restore()
        canvas.drawPath(cropPath, edgePaint)
        corners.forEach { canvas.drawCircle(it.x, it.y, 9f * density, handlePaint) }
    }

    /** Keeps the quadrilateral convex while dragging. This prevents crossed
     * corners from producing an inverted or degenerate perspective transform. */
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                activeCorner = nearestCorner(event.x, event.y)
                if (activeCorner < 0) return false
                parent.requestDisallowInterceptTouchEvent(true)
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                if (activeCorner < 0) return false
                val candidate = ScanPoint(
                    event.x.coerceIn(imageRect.left, imageRect.right),
                    event.y.coerceIn(imageRect.top, imageRect.bottom),
                )
                val proposed = corners.toMutableList().also { it[activeCorner] = candidate }
                if (isUsableQuadrilateral(proposed)) {
                    corners[activeCorner] = candidate
                    invalidate()
                }
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                activeCorner = -1
                parent.requestDisallowInterceptTouchEvent(false)
                performClick()
                return true
            }
        }
        return super.onTouchEvent(event)
    }

    override fun performClick(): Boolean {
        super.performClick()
        return true
    }

    private fun cropPath() = Path().apply {
        moveTo(corners[0].x, corners[0].y)
        corners.drop(1).forEach { lineTo(it.x, it.y) }
        close()
    }

    private fun nearestCorner(x: Float, y: Float): Int {
        val touchRadius = 48f * density
        return corners.indices.minByOrNull { index ->
            hypot(corners[index].x - x, corners[index].y - y)
        }?.takeIf { index ->
            hypot(corners[index].x - x, corners[index].y - y) <= touchRadius
        } ?: -1
    }

    private fun isUsableQuadrilateral(points: List<ScanPoint>): Boolean {
        val minimumArea = 48f * density * 48f * density
        val area = abs(points.indices.sumOf { index ->
            val current = points[index]
            val next = points[(index + 1) % points.size]
            (current.x * next.y - next.x * current.y).toDouble()
        }.toFloat()) / 2f
        if (area < minimumArea) return false

        var sign = 0
        for (index in points.indices) {
            val first = points[index]
            val second = points[(index + 1) % points.size]
            val third = points[(index + 2) % points.size]
            val cross = (second.x - first.x) * (third.y - second.y) -
                (second.y - first.y) * (third.x - second.x)
            val nextSign = if (cross >= 0f) 1 else -1
            if (sign != 0 && sign != nextSign) return false
            sign = nextSign
        }
        return true
    }
}
