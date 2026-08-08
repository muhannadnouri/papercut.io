package io.papercut.documentscanner

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.view.Gravity
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import java.io.File

/** Virtualizes accepted-page thumbnails so long scans retain only the bitmaps
 * visible in the strip instead of decoding every page into an ImageView. */
internal class AcceptedPageThumbnailAdapter(
    context: Context,
    private val thumbnails: List<File>,
    private val pageLabel: (Int) -> String,
    private val manageLabel: (Int) -> String,
    private val onPageSelected: (Int) -> Unit,
) : RecyclerView.Adapter<AcceptedPageThumbnailAdapter.PageViewHolder>() {
    private val density = context.resources.displayMetrics.density

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): PageViewHolder {
        val context = parent.context
        val image = ImageView(context).apply {
            scaleType = ImageView.ScaleType.CENTER_CROP
        }
        val label = TextView(context).apply {
            gravity = Gravity.CENTER
            setTextColor(0xffeeeeee.toInt())
        }
        val item = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            isClickable = true
            isFocusable = true
            addView(image, LinearLayout.LayoutParams(dp(64), dp(72)))
            addView(label, LinearLayout.LayoutParams(dp(64), dp(24)))
            layoutParams = RecyclerView.LayoutParams(dp(72), dp(104)).apply {
                setMargins(dp(4), 0, dp(4), 0)
            }
        }
        return PageViewHolder(item, image, label)
    }

    override fun onBindViewHolder(holder: PageViewHolder, position: Int) {
        holder.bind(
            BitmapFactory.decodeFile(thumbnails[position].path),
            pageLabel(position + 1),
            manageLabel(position + 1),
        ) {
            val selected = holder.bindingAdapterPosition
            if (selected != RecyclerView.NO_POSITION) onPageSelected(selected)
        }
    }

    override fun onViewRecycled(holder: PageViewHolder) {
        holder.releaseBitmap()
        super.onViewRecycled(holder)
    }

    override fun getItemCount(): Int = thumbnails.size

    private fun dp(value: Int) = (value * density).toInt()

    /** Owns the one decoded thumbnail attached to this recycled row. Releasing
     * it before rebinding keeps rapid scrolling within RecyclerView's window. */
    internal class PageViewHolder(
        item: LinearLayout,
        private val image: ImageView,
        private val label: TextView,
    ) : RecyclerView.ViewHolder(item) {
        private var bitmap: Bitmap? = null

        fun bind(bitmap: Bitmap?, pageText: String, description: String, onClick: () -> Unit) {
            releaseBitmap()
            this.bitmap = bitmap
            image.setImageBitmap(bitmap)
            label.text = pageText
            itemView.contentDescription = description
            itemView.setOnClickListener { onClick() }
        }

        fun releaseBitmap() {
            image.setImageDrawable(null)
            bitmap?.recycle()
            bitmap = null
        }
    }
}
