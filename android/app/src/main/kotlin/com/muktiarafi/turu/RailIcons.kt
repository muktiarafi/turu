package com.muktiarafi.turu

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

/**
 * The rail's icons Material Symbols' core set lacks, drawn on a 24-unit grid in one 1.8 stroke
 * with round caps, as the egui rail's (`nav_bar.rs`, `RailIcon`) so both rails read alike.
 */
object RailIcons {
    private fun icon(name: String, filled: Boolean = false, draw: PathBuilder.() -> Unit): ImageVector =
        ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
            path(
                fill = if (filled) SolidColor(Color.Black) else null,
                stroke = if (filled) null else SolidColor(Color.Black),
                strokeLineWidth = 1.8f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathBuilder = draw,
            )
        }.build()

    private fun PathBuilder.circle(cx: Float, cy: Float, r: Float) {
        moveTo(cx - r, cy)
        arcTo(r, r, 0f, isMoreThanHalf = true, isPositiveArc = true, x1 = cx + r, y1 = cy)
        arcTo(r, r, 0f, isMoreThanHalf = true, isPositiveArc = true, x1 = cx - r, y1 = cy)
        close()
    }

    /** A crosshair: combat automation. */
    val Auto = icon("Auto") {
        circle(12f, 12f, 7f)
        moveTo(12f, 2.5f); lineTo(12f, 6.5f)
        moveTo(12f, 17.5f); lineTo(12f, 21.5f)
        moveTo(2.5f, 12f); lineTo(6.5f, 12f)
        moveTo(17.5f, 12f); lineTo(21.5f, 12f)
        circle(12f, 12f, 0.6f)
    }

    /** A chest. */
    val Bank = icon("Bank") {
        moveTo(4f, 10f); lineTo(20f, 10f); lineTo(20f, 19f); lineTo(4f, 19f); close()
        moveTo(4f, 10f); lineTo(4f, 8f)
        arcTo(4f, 3f, 0f, isMoreThanHalf = false, isPositiveArc = true, x1 = 8f, y1 = 5f)
        lineTo(16f, 5f)
        arcTo(4f, 3f, 0f, isMoreThanHalf = false, isPositiveArc = true, x1 = 20f, y1 = 8f)
        lineTo(20f, 10f)
        moveTo(11f, 10f); lineTo(11f, 13.5f); lineTo(13f, 13.5f); lineTo(13f, 10f)
    }

    /** A page with lines: a script. */
    val Script = icon("Script") {
        moveTo(6f, 3.5f); lineTo(14f, 3.5f); lineTo(18.5f, 8f); lineTo(18.5f, 20.5f); lineTo(6f, 20.5f); close()
        moveTo(14f, 3.5f); lineTo(14f, 8f); lineTo(18.5f, 8f)
        moveTo(9f, 12f); lineTo(15.5f, 12f)
        moveTo(9f, 15.5f); lineTo(15.5f, 15.5f)
    }

    /** Lines of text: a log. */
    val Logs = icon("Logs") {
        moveTo(5f, 6f); lineTo(19f, 6f)
        moveTo(5f, 10f); lineTo(19f, 10f)
        moveTo(5f, 14f); lineTo(15f, 14f)
        moveTo(5f, 18f); lineTo(12f, 18f)
    }

    /** Two people: the army. */
    val Army = icon("Army") {
        circle(9f, 8.5f, 3f)
        moveTo(3.5f, 19.5f)
        arcTo(5.5f, 5f, 0f, isMoreThanHalf = false, isPositiveArc = true, x1 = 14.5f, y1 = 19.5f)
        moveTo(15.5f, 5.8f)
        arcTo(3f, 3f, 0f, isMoreThanHalf = false, isPositiveArc = true, x1 = 15.5f, y1 = 11.2f)
        moveTo(17f, 14.5f)
        arcTo(5f, 5f, 0f, isMoreThanHalf = false, isPositiveArc = true, x1 = 20.5f, y1 = 19.5f)
    }

    /** An arrow into a tray: an update to download. */
    val Update = icon("Update") {
        moveTo(12f, 3.5f); lineTo(12f, 14f)
        moveTo(7.5f, 9.5f); lineTo(12f, 14f); lineTo(16.5f, 9.5f)
        moveTo(4.5f, 14.5f); lineTo(4.5f, 19.5f); lineTo(19.5f, 19.5f); lineTo(19.5f, 14.5f)
    }

    val Play = icon("Play", filled = true) {
        moveTo(8f, 5.5f); lineTo(18.5f, 12f); lineTo(8f, 18.5f); close()
    }

    val Pause = icon("Pause", filled = true) {
        moveTo(7f, 5.5f); lineTo(10f, 5.5f); lineTo(10f, 18.5f); lineTo(7f, 18.5f); close()
        moveTo(14f, 5.5f); lineTo(17f, 5.5f); lineTo(17f, 18.5f); lineTo(14f, 18.5f); close()
    }

    val Stop = icon("Stop", filled = true) {
        moveTo(7f, 7f); lineTo(17f, 7f); lineTo(17f, 17f); lineTo(7f, 17f); close()
    }
}
