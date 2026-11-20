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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material.icons.rounded.Menu
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
    val onClick: () -> Unit,
)

private val ItemHeight = 52.dp
private val CellHeight = 44.dp
private val ChipHeight = 48.dp

/**
 * The native rail (docs/UI.md, "Native shells", Android; L5b): the Tabs item, then Logs, Army
 * and More, then Auto, Jump and Bank (and Update while one is out), the script chip and its
 * run controls at the bottom. A short screen folds the items into More, least needed first; the
 * fit always counts two run controls, so starting a script doesn't move the items.
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
        Entry("More", Icons.Rounded.Menu, 0, Page.More) { actions.open(Page.More) },
        Entry("Auto", RailIcons.Auto, 1, Page.Auto, badge = if (auto?.running == true) Turu.Success else null) {
            actions.open(Page.Auto)
        },
        Entry("Jump", Icons.Rounded.LocationOn, 2, Page.Jump) { actions.open(Page.Jump) },
        Entry("Bank", RailIcons.Bank, 4, null, busy = bank?.busy == true) { actions.bank() },
        update?.let {
            Entry("Update", RailIcons.Update, 3, null, badge = if (it.failed != null) Turu.Danger else Turu.Accent, busy = it.busy != null) {
                actions.window("client_update")
            }
        },
    )
    val width = 64.dp + state.cutoutLeftDp.dp
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
        val room = maxHeight - 60.dp - ChipHeight - CellHeight * 2 - 8.dp - 12.dp
        val fit = (room / ItemHeight).toInt().coerceAtLeast(1)
        val kept = entries.sortedBy { it.keep }.take(fit).toSet()
        val folded = entries.filter { it !in kept }.map { it.label }.toSet()
        SideEffect { if (state.folded != folded) state.folded = folded }
        Column(Modifier.fillMaxSize().padding(start = state.cutoutLeftDp.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.padding(end = 0.dp)) { TabsItemContent(state, onTabs) }
            for (entry in entries) {
                if (entry in kept) {
                    RailItem(entry, selected = open && entry.page != null && entry.page == page)
                }
            }
            Spacer(Modifier.weight(1f))
            ScriptStatus(models, actions, selected = open && page == Page.Scripts)
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun RailItem(entry: Entry, selected: Boolean) {
    Column(
        Modifier
            .width(56.dp)
            .height(ItemHeight)
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = entry.onClick)
            .semantics { contentDescription = entry.label },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            Modifier
                .width(48.dp)
                .height(30.dp)
                .clip(RoundedCornerShape(15.dp))
                .background(if (selected) Turu.AccentTint else Color.Transparent),
            contentAlignment = Alignment.Center,
        ) {
            if (entry.busy) {
                CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = Turu.Text)
            } else {
                Icon(entry.icon, contentDescription = null, tint = Turu.Text, modifier = Modifier.size(22.dp))
            }
            entry.badge?.let { color ->
                Box(
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(top = 3.dp, end = 9.dp)
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
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}

/** The chip (its status dot over the clock or the state) and the run controls under it. */
@Composable
private fun ScriptStatus(models: TabModels, actions: RailActions, selected: Boolean) {
    val view = models.scripts
    val scripts = actions.scripts
    val status = view?.status
    val dot = when (status) {
        Status.RUNNING -> Turu.Success
        Status.PAUSED, Status.COMPILING, Status.STOPPING -> Turu.Warning
        Status.ERROR -> Turu.Danger
        else -> Turu.Muted
    }
    val words = when {
        view == null -> "…"
        view.elapsedSecs != null -> durationText(view.elapsedSecs!!.toLong())
        status == Status.ERROR -> "Error"
        view.lastRun != null -> if (view.lastRun!!.finished) "Finished" else "Stopped"
        view.script == null -> "No script"
        else -> "Ready"
    }
    val c = view?.controls
    val cells = buildList {
        c?.start?.let { add(Triple(RailIcons.Play, "Start the script", it.enabled) to scripts::start) }
        c?.pause?.let {
            if (c.paused) add(Triple(RailIcons.Play, "Resume", true) to scripts::resume)
            else add(Triple(RailIcons.Pause, "Pause", true) to scripts::pause)
        }
        c?.forceStop?.let { add(Triple(RailIcons.Stop, "Force stop", it.enabled) to scripts::stop) }
        c?.stop?.let { add(Triple(RailIcons.Stop, "Stop the script", it.enabled) to scripts::stop) }
    }
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(1.dp)) {
        val top = RoundedCornerShape(topStart = 10.dp, topEnd = 10.dp, bottomStart = if (cells.isEmpty()) 10.dp else 0.dp, bottomEnd = if (cells.isEmpty()) 10.dp else 0.dp)
        Column(
            Modifier
                .width(56.dp)
                .height(ChipHeight)
                .clip(top)
                .background(if (selected) Turu.AccentTint else Turu.Track)
                .clickable { actions.open(Page.Scripts) }
                .semantics { contentDescription = "Script Loader: ${view?.state ?: "reading the tab"}" },
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(dot))
            Text(
                words,
                style = MaterialTheme.typography.labelSmall,
                color = if (status == Status.ERROR) Turu.Danger else Turu.Text,
                maxLines = 1,
                overflow = TextOverflow.Clip,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
        cells.forEachIndexed { i, (cell, run) ->
            val (icon, label, enabled) = cell
            val last = i == cells.lastIndex
            val primary = icon == RailIcons.Play
            Box(
                Modifier
                    .width(56.dp)
                    .height(CellHeight)
                    .clip(RoundedCornerShape(bottomStart = if (last) 10.dp else 0.dp, bottomEnd = if (last) 10.dp else 0.dp))
                    .background(if (primary && enabled) Turu.Accent else Turu.Track)
                    .clickable(enabled = enabled && scripts.busy == null) { run() }
                    .semantics { contentDescription = label },
                contentAlignment = Alignment.Center,
            ) {
                val tint = when {
                    !enabled -> Turu.Muted.copy(alpha = 0.5f)
                    icon == RailIcons.Stop -> Turu.Danger
                    else -> Color.White
                }
                Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
            }
        }
    }
}

/** While the rail is hidden: a tab at the left edge, halfway down, that brings it back. */
@Composable
fun RailHandle(insetDp: Dp, onClick: () -> Unit) {
    Box(
        Modifier
            .width(24.dp + insetDp)
            .height(64.dp)
            .clip(RoundedCornerShape(topEnd = 10.dp, bottomEnd = 10.dp))
            .background(Turu.Panel.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
            .semantics { contentDescription = "Show the side bar" }
            .padding(start = insetDp),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Text)
    }
}

/** Bank's note (why nothing opened) as a toast: short, needing no answer. */
@Composable
fun BankNote(models: TabModels) {
    val note = models.bank?.note
    val context = androidx.compose.ui.platform.LocalContext.current
    androidx.compose.runtime.LaunchedEffect(note) {
        if (note != null) android.widget.Toast.makeText(context, "Bank: $note", android.widget.Toast.LENGTH_SHORT).show()
    }
}
