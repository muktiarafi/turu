package com.muktiarafi.turu

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Control
import com.muktiarafi.turu.models.ScriptsView
import com.muktiarafi.turu.models.Status
import org.json.JSONObject

/**
 * The Script Loader's actions on the tab on screen, over its models ([TabModels]): the view is
 * `models.scripts`, the log `models.scriptLog`.
 */
class ScriptsPanel(
    val models: TabModels,
    /** Opens one of the slot's egui windows ("script_repo", "script_options") and closes the panel. */
    private val openWindow: (String) -> Unit,
) {
    val view: ScriptsView? get() = models.scripts
    val tab: ShellTab? get() = models.tab
    val log get() = models.scriptLog

    /** An action's error, until the next action. */
    var error by mutableStateOf<String?>(null)
        private set

    /** The action under way (its name), for a spinner. */
    var busy by mutableStateOf<String?>(null)
        private set

    private fun act(action: String, body: JSONObject = JSONObject()) {
        busy = action
        error = null
        models.act("scripts", action, body) { failed ->
            busy = null
            error = failed
        }
    }

    fun start() = act("start")
    fun stop() = act("stop")
    fun pause() = act("pause")
    fun resume() = act("resume")
    fun favourite() = act("favourite")
    fun clearLog() = models.clearLog("script")

    /** A favourite or recent script: loaded and started (the chip's list). */
    fun startQuick(path: String) {
        busy = "start"
        error = null
        models.act("rail", "start-quick", JSONObject().put("path", path)) { failed ->
            busy = null
            error = failed
        }
    }

    /** Search Scripts, native: a page pushed over this one (set by the shell). */
    var openSearch: () -> Unit = { openWindow("script_repo") }

    fun searchScripts() = openSearch()
    fun editOptions() = openWindow("script_options")
}

/** "45s", "12m", "3h 05m": the egui card's clock. */
fun durationText(secs: Long): String = when {
    secs < 60 -> "${secs}s"
    secs < 3600 -> "${secs / 60}m"
    else -> "${secs / 3600}h ${"%02d".format(secs / 60 % 60)}m"
}

private fun statusColor(status: Status): Color = when (status) {
    Status.RUNNING -> Turu.Success
    Status.PAUSED, Status.COMPILING, Status.STOPPING -> Turu.Warning
    Status.ERROR -> Turu.Danger
    Status.EMPTY, Status.LOADED -> Turu.Muted
}

/** The Script Loader: status, the run controls, setup, the script log (docs/UI.md, "Native shells"). */
@Composable
fun ScriptsPage(state: ShellState, scripts: ScriptsPanel) {
    val view = scripts.view
    Column(Modifier.fillMaxSize()) {
        PageBar("Script Loader", onBack = { state.back() }) {
            scripts.tab?.let {
                Text(
                    it.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = Turu.Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = androidx.compose.ui.text.style.TextAlign.End,
                    modifier = Modifier.padding(end = 16.dp).width(96.dp),
                )
            }
        }
        if (view == null) {
            Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                val starting = scripts.tab?.api == null
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(12.dp))
                    Text(if (starting) "The tab is starting…" else "Reading the tab…", color = Turu.Muted)
                }
            }
            return@Column
        }
        Column(Modifier.padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatusCard(view, scripts)
            RunControls(view, scripts)
            scripts.error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodySmall) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = scripts::searchScripts, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                    Icon(Icons.Rounded.Search, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Search", maxLines = 1)
                }
                val options = view.controls.editOptions
                OutlinedButton(
                    onClick = scripts::editOptions,
                    enabled = options.enabled,
                    modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                ) {
                    Icon(Icons.Rounded.Settings, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Options", maxLines = 1)
                }
            }
            if (view.script != null) Reason(view.controls.editOptions)
            QuickScripts(scripts, active = view.status.isActive())
        }
        ScriptLog(scripts, running = view.status != Status.EMPTY && view.status != Status.LOADED && view.status != Status.ERROR)
    }
}

private fun Status.isActive() = this == Status.COMPILING || this == Status.RUNNING || this == Status.PAUSED || this == Status.STOPPING

/** Favourite and recent scripts, a tap loading and starting one (the egui chip's list), while none runs. */
@Composable
private fun QuickScripts(scripts: ScriptsPanel, active: Boolean) {
    val rail = scripts.models.rail ?: return
    if (active || (rail.favourites.isEmpty() && rail.recent.isEmpty())) return
    Column {
        for ((title, list) in listOf("Favourites" to rail.favourites, "Recent" to rail.recent)) {
            if (list.isEmpty()) continue
            Text(title, style = MaterialTheme.typography.labelMedium, color = Turu.Muted, modifier = Modifier.padding(top = 4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 4.dp)) {
                for (quick in list.take(3)) {
                    androidx.compose.material3.AssistChip(
                        onClick = { scripts.startQuick(quick.path) },
                        label = { Text(quick.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        modifier = Modifier.weight(1f, fill = false),
                    )
                }
            }
        }
    }
}

@Composable
private fun StatusCard(view: ScriptsView, scripts: ScriptsPanel) {
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(Turu.Card)
            .padding(12.dp)
            .animateContentSize(),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(10.dp).clip(CircleShape).background(statusColor(view.status)))
            Spacer(Modifier.width(8.dp))
            val run = view.lastRun
            val state = when {
                run != null && view.status == Status.ERROR -> "${view.state} · ran ${durationText(run.ranSecs.toLong())}"
                run != null -> "${if (run.finished) "Finished" else "Stopped"} · ran ${durationText(run.ranSecs.toLong())}"
                view.elapsedSecs != null -> "${view.state} · ${durationText(view.elapsedSecs!!.toLong())}"
                else -> view.state
            }
            Text(state, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        }
        view.lastLog?.let { log ->
            val ago = if (log.quietSecs.toLong() < 5) "just now" else "${durationText(log.quietSecs.toLong())} ago"
            Text("Last log $ago", style = MaterialTheme.typography.bodySmall, color = if (log.stale) Turu.Warning else Turu.Muted)
        }
        view.waits?.let { Text("Waits for you: $it", style = MaterialTheme.typography.bodySmall, color = Turu.Warning) }
        val script = view.script
        if (script == null) {
            Text("Search Scripts to pick one", style = MaterialTheme.typography.bodyMedium, color = Turu.Muted)
        } else {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    script.name,
                    fontFamily = FontFamily.Monospace,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = scripts::favourite) {
                    Icon(
                        Icons.Rounded.Star,
                        contentDescription = if (script.favourite) "Take it off the favourites" else "Add to the favourites",
                        tint = if (script.favourite) Turu.Warning else Turu.Edge,
                    )
                }
            }
        }
        view.error?.let { Text(it.firstLine, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, maxLines = 2, overflow = TextOverflow.Ellipsis) }
    }
}

/** Start alone while nothing runs; Stop with Pause or Resume while it runs; Force Stop while it stops. */
@Composable
private fun RunControls(view: ScriptsView, scripts: ScriptsPanel) {
    val c = view.controls
    val busy = scripts.busy != null
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        c.start?.let { start ->
            Button(onClick = scripts::start, enabled = start.enabled && !busy, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                Text("Start script")
            }
        }
        c.stop?.let { stop ->
            Button(
                onClick = scripts::stop,
                enabled = stop.enabled && !busy,
                colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Color.White),
                modifier = Modifier.weight(1.6f).heightIn(min = 48.dp),
            ) { Text("Stop") }
        }
        c.forceStop?.let { force ->
            Button(
                onClick = scripts::stop,
                enabled = force.enabled && !busy,
                colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Color.White),
                modifier = Modifier.weight(1f).heightIn(min = 48.dp),
            ) { Text("Force stop", maxLines = 1) }
        } ?: c.pause?.let {
            if (c.paused) {
                Button(onClick = scripts::resume, enabled = !busy, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) { Text("Resume") }
            } else {
                FilledTonalButton(onClick = scripts::pause, enabled = !busy, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) { Text("Pause") }
            }
        }
    }
    // A touch screen has no tooltips: why the control is off, under it.
    if (view.script != null) {
        c.start?.let { Reason(it) }
        c.forceStop?.let { Reason(it) }
    }
}

@Composable
private fun Reason(control: Control) {
    if (!control.enabled && control.reason.isNotEmpty()) {
        Text(control.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
    }
}

@Composable
private fun ScriptLog(scripts: ScriptsPanel, running: Boolean) {
    val list = rememberLazyListState()
    LaunchedEffect(scripts.log.size) { if (scripts.log.isNotEmpty()) list.scrollToItem(scripts.log.size - 1) }
    Column(Modifier.fillMaxSize().padding(top = 8.dp)) {
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Script log", style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
            TextButton(onClick = scripts::clearLog, enabled = scripts.log.isNotEmpty()) { Text("Clear") }
        }
        if (scripts.log.isEmpty()) {
            Text(
                if (running) "Nothing logged yet" else "The script's log shows here when it runs",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }
        LazyColumn(state = list, modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            itemsIndexed(scripts.log) { _, (time, text) ->
                Row(Modifier.padding(vertical = 2.dp)) {
                    Text(clock(time), style = MaterialTheme.typography.bodySmall, color = Turu.Muted, fontFamily = FontFamily.Monospace)
                    Spacer(Modifier.width(8.dp))
                    Text(text, style = MaterialTheme.typography.bodySmall, color = Turu.Text)
                }
            }
        }
    }
}

/** "14:02:31" from an RFC 3339 time. */
private fun clock(time: String): String = time.substringAfter('T').take(8)
