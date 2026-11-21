package com.muktiarafi.turu

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Status

/** What the rail asks of the shell. */
class RailActions(
    val open: (Page) -> Unit,
    val bank: () -> Unit,
    /** One of the slot's egui windows (the Update window). */
    val window: (String) -> Unit,
    val scripts: ScriptsPanel,
)

/** A rail item: its page (or action), label and icon, and what keeps it on a short screen. */
private class Entry(
    val label: String,
    val icon: ImageVector,
    /** Lower stays longer when the rail is short; More never folds. */
    val keep: Int,
    val page: Page?,
    val badge: Color? = null,
    val busy: Boolean = false,
    /** What the badge or spinner says, for TalkBack ("running", "failed"). */
    val state: String? = null,
    val onClick: () -> Unit,
)

/** A run control under the chip. */
private class Cell(
    val icon: ImageVector,
    val label: String,
    val enabled: Boolean,
    /** Why it's off, read out and shown by the Script Loader it opens instead. */
    val reason: String = "",
    val primary: Boolean = false,
    val danger: Boolean = false,
    val run: () -> Unit,
)

private val CellHeight = 48.dp
private val CellGap = 8.dp

/** The labels' line, in dp at the current font scale: rail items and the chip grow with it. */
@Composable
private fun labelLine(): Dp = with(LocalDensity.current) { MaterialTheme.typography.labelSmall.lineHeight.toDp() }

/** A rail item's height: the 32-dp indicator, the label's line and their gaps (52 dp at 1.0). */
@Composable
fun railItemHeight(): Dp = 36.dp + labelLine()

/**
 * The native rail (docs/UI.md, "Native shells", Android; L5b): the Tabs item, then Logs, Army,
 * Auto, Jump and Bank (and Update while one is out), More last, the script chip and its run
 * controls at the bottom. A short screen folds the items into More, least needed first; the fit
 * always counts two run controls, so starting a script doesn't move the items, and measures the
 * items at the font scale, so larger text folds rather than crowds.
 */
@Composable
fun NativeRail(state: ShellState, models: TabModels, actions: RailActions, onTabs: () -> Unit) {
    val open = state.open
    val page = state.page
    val auto = models.auto
    val bank = models.bank
    val update = models.rail?.update
    val entries = listOfNotNull(
        Entry("Logs", RailIcons.Logs, 3, Page.Logs) { actions.open(Page.Logs) },
        Entry("Army", RailIcons.Army, 6, Page.Army) { actions.open(Page.Army) },
        Entry(
            "Auto", RailIcons.Auto, 1, Page.Auto,
            badge = if (auto?.running == true) Turu.Success else null,
            state = if (auto?.running == true) "running" else null,
        ) { actions.open(Page.Auto) },
        Entry("Jump", RailIcons.Jump, 2, Page.Jump) { actions.open(Page.Jump) },
        Entry("Bank", RailIcons.Bank, 4, null, busy = bank?.busy == true, state = if (bank?.busy == true) "opening" else null) {
            actions.bank()
        },
        update?.let {
            Entry(
                "Update", RailIcons.Update, 3, null,
                badge = if (it.failed != null) Turu.Danger else Turu.Accent,
                busy = it.busy != null,
                state = it.failed?.let { why -> "failed: $why" } ?: it.busy ?: "version ${it.version} ready",
            ) { actions.window("client_update") }
        },
        Entry("More", RailIcons.More, 0, Page.More) { actions.open(Page.More) },
    )
    val width = 64.dp + state.cutoutLeftDp.dp
    val item = railItemHeight()
    val chip = 32.dp + labelLine()
    BoxWithConstraints(
        Modifier
            .width(width)
            .fillMaxHeight()
            .background(Turu.Panel)
            .drawBehind {
                drawLine(Turu.Edge, Offset(size.width - 1f, 0f), Offset(size.width - 1f, size.height), strokeWidth = 1f)
            },
    ) {
        // What fits between the Tabs item and the status: More always, then the rest by `keep`.
        val room = maxHeight - (8.dp + item) - chip - 1.dp - CellHeight * 2 - CellGap - 8.dp
        val fit = (room / item).toInt().coerceAtLeast(1)
        val kept = entries.sortedBy { it.keep }.take(fit).toSet()
        val folded = entries.filter { it !in kept }.map { it.label }.toSet()
        SideEffect { if (state.folded != folded) state.folded = folded }
        Column(Modifier.fillMaxSize().padding(start = state.cutoutLeftDp.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            TabsItemContent(state, onTabs)
            for (entry in entries) {
                if (entry in kept) {
                    RailItem(entry, item, selected = open && entry.page != null && entry.page == page)
                }
            }
            Spacer(Modifier.weight(1f))
            ScriptStatus(models, actions, chip, selected = open && page == Page.Scripts)
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun RailItem(entry: Entry, height: Dp, selected: Boolean) {
    val select = if (entry.page != null) {
        Modifier.selectable(selected = selected, role = Role.Tab, onClick = entry.onClick)
    } else {
        Modifier.clickable(role = Role.Button, onClick = entry.onClick)
    }
    Column(
        Modifier
            .width(56.dp)
            .height(height)
            .clip(MaterialTheme.shapes.medium)
            .semantics {
                contentDescription = entry.label
                entry.state?.let { stateDescription = it }
            }
            .then(select),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            Modifier
                .width(52.dp)
                .height(32.dp)
                .clip(CircleShape)
                .background(if (selected) Turu.RailSelected else Color.Transparent),
            contentAlignment = Alignment.Center,
        ) {
            val tint = if (selected) Turu.Text else Turu.Muted
            if (entry.busy) {
                CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = tint)
            } else {
                Icon(entry.icon, contentDescription = null, tint = tint, modifier = Modifier.size(22.dp))
            }
            entry.badge?.let { color ->
                Box(
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(top = 4.dp, end = 11.dp)
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(color),
                )
            }
        }
        Text(
            entry.label,
            style = MaterialTheme.typography.labelSmall,
            color = if (selected) Turu.Text else Turu.Muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}

/** The first of [words] that fits [width] in [style]: the chip's long word, or its short one. */
@Composable
private fun fitting(words: List<String>, style: TextStyle, width: Dp): String {
    val measurer = rememberTextMeasurer()
    val px = with(LocalDensity.current) { width.roundToPx() }
    return remember(words, style, px) {
        words.firstOrNull { measurer.measure(it, style, maxLines = 1).size.width <= px } ?: words.last()
    }
}

/**
 * The chip (its status dot over the clock or the state) and the run controls under it. With no
 * script loaded, the cell under it is Search scripts; an off control still opens the Script
 * Loader, which says why. Two controls stand apart, so a slip from Pause doesn't Stop.
 */
@Composable
private fun ScriptStatus(models: TabModels, actions: RailActions, chipHeight: Dp, selected: Boolean) {
    val view = models.scripts
    val scripts = actions.scripts
    val status = view?.status
    val dot = when (status) {
        Status.RUNNING -> Turu.Success
        Status.PAUSED, Status.COMPILING, Status.STOPPING -> Turu.Warning
        Status.ERROR -> Turu.Danger
        else -> Turu.Muted
    }
    val clock = view?.elapsedSecs?.toLong()
    val candidates = when {
        view == null -> listOf("…")
        clock != null -> listOfNotNull(durationText(clock), if (clock >= 3600) "${clock / 3600}h" else null)
        status == Status.ERROR -> listOf("Error")
        view.lastRun != null -> if (view.lastRun.finished) listOf("Finished", "Done") else listOf("Stopped", "Ended")
        view.script == null -> listOf("No script", "None")
        else -> listOf("Ready")
    }
    val style = MaterialTheme.typography.labelSmall.let { if (clock != null) it.copy(fontFeatureSettings = "tnum") else it }
    val words = fitting(candidates, style, 50.dp)
    val c = view?.controls
    val openLoader = { actions.open(Page.Scripts) }
    val cells = buildList {
        c?.start?.let { start ->
            if (view.script == null) {
                add(Cell(RailIcons.Search, "Search scripts", c.load.enabled, c.load.reason) { actions.open(Page.Search) })
            } else {
                add(Cell(RailIcons.Play, "Start the script", start.enabled, start.reason, primary = true, run = scripts::start))
            }
        }
        c?.pause?.let {
            if (c.paused) add(Cell(RailIcons.Play, "Resume", it.enabled, it.reason, primary = true, run = scripts::resume))
            else add(Cell(RailIcons.Pause, "Pause", it.enabled, it.reason, run = scripts::pause))
        }
        c?.forceStop?.let { add(Cell(RailIcons.Stop, "Force stop", it.enabled, it.reason, danger = true, run = scripts::stop)) }
        c?.stop?.let { add(Cell(RailIcons.Stop, "Stop the script", it.enabled, it.reason, danger = true, run = scripts::stop)) }
    }
    // The theme's medium corner; the chip and its first control share a square seam.
    val round = MaterialTheme.shapes.medium
    val square = CornerSize(0.dp)
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        val joined = cells.isNotEmpty()
        Column(
            Modifier
                .width(56.dp)
                .height(chipHeight)
                .clip(if (joined) round.copy(bottomStart = square, bottomEnd = square) else round)
                .background(if (selected) Turu.RailSelected else Turu.Track)
                .semantics { contentDescription = "Script Loader: ${view?.state ?: "reading the tab"}" }
                .selectable(selected = selected, role = Role.Tab, onClick = openLoader),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(dot))
            Text(
                words,
                style = style,
                color = if (status == Status.ERROR) Turu.Danger else Turu.Text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 4.dp, start = 3.dp, end = 3.dp),
            )
        }
        cells.forEachIndexed { i, cell ->
            // The first control hangs under the chip; the second stands apart.
            Spacer(Modifier.height(if (i == 0) 1.dp else CellGap))
            val shape = if (i == 0) {
                round.copy(topStart = square, topEnd = square)
            } else {
                round
            }
            val idle = scripts.busy == null
            Box(
                Modifier
                    .width(56.dp)
                    .height(CellHeight)
                    .clip(shape)
                    .background(if (cell.primary && cell.enabled) Turu.Accent else Turu.Track)
                    .clickable(
                        enabled = idle,
                        role = Role.Button,
                        onClickLabel = if (cell.enabled) null else "open the Script Loader",
                        onClick = if (cell.enabled) cell.run else openLoader,
                    )
                    .semantics {
                        contentDescription = cell.label
                        if (!cell.enabled) stateDescription = cell.reason.ifEmpty { "off" }
                    },
                contentAlignment = Alignment.Center,
            ) {
                val tint = when {
                    !cell.enabled -> Turu.Muted.copy(alpha = 0.5f)
                    cell.danger -> Turu.Danger
                    cell.primary -> Color.White
                    else -> Turu.Text
                }
                Icon(cell.icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
            }
        }
    }
}

/**
 * While the rail is hidden: a tab at the left edge, halfway down, that brings it back. It shows
 * 24 dp but takes touches 48 dp wide.
 */
@Composable
fun RailHandle(insetDp: Dp, onClick: () -> Unit) {
    Box(
        Modifier
            .width(48.dp + insetDp)
            .height(64.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = "Show the side bar" },
    ) {
        Box(
            Modifier
                .width(24.dp + insetDp)
                .fillMaxHeight()
                .clip(MaterialTheme.shapes.medium.copy(topStart = CornerSize(0.dp), bottomStart = CornerSize(0.dp)))
                .background(Turu.Panel.copy(alpha = 0.85f))
                .padding(start = insetDp),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Text)
        }
    }
}

/** Bank's note (why nothing opened) as a toast: short, needing no answer. */
@Composable
fun BankNote(models: TabModels) {
    val note = models.bank?.note
    val context = androidx.compose.ui.platform.LocalContext.current
    androidx.compose.runtime.LaunchedEffect(note) {
        // impeccable-disable-next-line toast: the panel may be closed, so no snackbar host shows (docs/UI.md)
        if (note != null) android.widget.Toast.makeText(context, "Bank: $note", android.widget.Toast.LENGTH_SHORT).show()
    }
}
