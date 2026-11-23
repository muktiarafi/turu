package com.muktiarafi.turu

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.Control
import com.muktiarafi.turu.models.RailAct
import com.muktiarafi.turu.models.ScriptsAct
import com.muktiarafi.turu.models.ScriptsView
import com.muktiarafi.turu.models.Status

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

    /** Runs `action`, `name` showing as [busy] meanwhile. */
    private fun act(name: String, action: ScriptsAct) {
        busy = name
        error = null
        models.act(Api.scripts, action) { failed ->
            busy = null
            error = failed
        }
    }

    fun start() = act("start", ScriptsAct.start)
    fun stop() = act("stop", ScriptsAct.stop)
    fun pause() = act("pause", ScriptsAct.pause)
    fun resume() = act("resume", ScriptsAct.resume)
    fun favourite() = act("favourite", ScriptsAct.favourite)
    fun clearLog() = models.clearLog("script")

    /** The favourite or recent script being started, for its row's spinner. */
    var starting by mutableStateOf<String?>(null)
        private set

    /** A favourite or recent script: loaded and started (the chip's list). */
    fun startQuick(path: String) {
        if (busy != null) return
        busy = "start"
        starting = path
        error = null
        models.act(Api.rail, RailAct.start_quick(path)) { failed ->
            busy = null
            starting = null
            error = failed
        }
    }

    /** Search Scripts, native: a page pushed over this one (set by the shell). */
    var openSearch: () -> Unit = { openWindow("script_repo") }

    /** Script Options, native: a page pushed over this one (set by the shell). */
    var openOptions: () -> Unit = { openWindow("script_options") }

    fun searchScripts() = openSearch()
    fun editOptions() = openOptions()
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

/**
 * The Script Loader (docs/UI.md, "Native shells"), laid out by state. With no script: the status
 * and Search scripts in Start's place. With one loaded: Start, Search and Options. While it runs:
 * the status, Stop and Pause (Search and Options under ⋮), so the log gets the height. The top
 * scrolls within 55% of the page while the log has lines; the log follows its last line.
 */
@Composable
fun ScriptsPage(state: ShellState, scripts: ScriptsPanel, fullWindow: () -> Unit) {
    val view = scripts.view
    val active = view?.status?.isActive() == true
    Column(Modifier.fillMaxSize()) {
        PageBar("Script Loader", onBack = { state.back() }) {
            scripts.tab?.let {
                Text(
                    it.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = Turu.Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 96.dp),
                )
            }
            FullWindowMenu("The log's Find and Copy, opening the script in an editor", fullWindow) { close ->
                // While a script runs, the page keeps only its run controls: setup lives here.
                if (active) {
                    DropdownMenuItem(text = { Text("Search scripts") }, onClick = { close(); scripts.searchScripts() })
                    DropdownMenuItem(
                        text = { Text("Script options") },
                        enabled = view.controls.editOptions.enabled,
                        onClick = { close(); scripts.editOptions() },
                    )
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                }
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
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val cap = if (scripts.log.isEmpty()) maxHeight else maxHeight * 0.55f
            Column(Modifier.fillMaxSize()) {
                // The head changes with the state (Start alone, Stop with Pause, setup and quick
                // scripts while stopped): its height follows on the panel's spring, the controls
                // cross over, and what comes or goes unfolds or folds rather than jumping.
                val layout = PanelMotion
                val controls = MaterialTheme.motionScheme
                Column(
                    Modifier
                        .animateContentSize(layout.defaultSpatialSpec())
                        .heightIn(max = cap)
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 12.dp)
                        .padding(bottom = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    StatusCard(view, scripts)
                    AnimatedContent(
                        targetState = view,
                        contentKey = { runKey(it) },
                        transitionSpec = {
                            (fadeIn(layout.defaultEffectsSpec()) + scaleIn(controls.fastSpatialSpec(), initialScale = 0.94f))
                                .togetherWith(fadeOut(layout.fastEffectsSpec()))
                        },
                        label = "run controls",
                    ) { shown -> Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { RunControls(shown, scripts) } }
                    scripts.error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodySmall) }
                    AnimatedVisibility(
                        view.script != null && !active,
                        enter = fadeIn(layout.defaultEffectsSpec()) + expandVertically(layout.defaultSpatialSpec()),
                        exit = fadeOut(layout.fastEffectsSpec()) + shrinkVertically(layout.fastSpatialSpec()),
                        label = "setup",
                    ) { Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { Setup(view, scripts) } }
                    AnimatedVisibility(
                        !active,
                        enter = fadeIn(layout.defaultEffectsSpec()) + expandVertically(layout.defaultSpatialSpec()),
                        exit = fadeOut(layout.fastEffectsSpec()) + shrinkVertically(layout.fastSpatialSpec()),
                        label = "quick scripts",
                    ) { Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { QuickScripts(scripts) } }
                }
                ScriptLog(scripts, running = view.status != Status.EMPTY && view.status != Status.LOADED && view.status != Status.ERROR)
            }
        }
    }
}

private fun Status.isActive() = this == Status.COMPILING || this == Status.RUNNING || this == Status.PAUSED || this == Status.STOPPING

/** Which set of run controls [view] shows: the controls cross over only when this changes. */
private fun runKey(view: ScriptsView): List<Boolean> {
    val c = view.controls
    return listOf(view.script == null && !view.status.isActive(), c.start != null, c.stop != null, c.forceStop != null, c.paused)
}

/** A button's label, with a spinner beside it while its action is under way. */
@Composable
private fun RowScope.Working(on: Boolean, label: String) {
    // The spinner slides in beside the label on the theme's fast spring, and out again.
    val motion = MaterialTheme.motionScheme
    AnimatedVisibility(
        on,
        enter = fadeIn(motion.fastEffectsSpec()) + expandHorizontally(motion.fastSpatialSpec()),
        exit = fadeOut(motion.fastEffectsSpec()) + shrinkHorizontally(motion.fastSpatialSpec()),
        label = "working",
    ) {
        Row {
            CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = LocalContentColor.current)
            Spacer(Modifier.width(8.dp))
        }
    }
    Text(label, maxLines = 1)
}

/** Search and Options, with a script loaded and stopped (the reason Options is off under them). */
@Composable
private fun Setup(view: ScriptsView, scripts: ScriptsPanel) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        OutlinedButton(onClick = scripts::searchScripts, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
            Icon(Icons.Rounded.Search, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(6.dp))
            Text("Search", maxLines = 1)
        }
        OutlinedButton(
            onClick = scripts::editOptions,
            enabled = view.controls.editOptions.enabled,
            modifier = Modifier.weight(1f).heightIn(min = 48.dp),
        ) {
            Icon(Icons.Rounded.Settings, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(6.dp))
            Text("Options", maxLines = 1)
        }
    }
    Reason(view.controls.editOptions)
}

/**
 * Favourite and recent scripts, while none runs: a tap loads and starts one (the egui chip's
 * list), so each row says so with its play mark, and all wait while one starts.
 */
@Composable
private fun QuickScripts(scripts: ScriptsPanel) {
    val rail = scripts.models.rail ?: return
    if (rail.favourites.isEmpty() && rail.recent.isEmpty()) return
    val idle = scripts.busy == null
    Column {
        for ((title, list) in listOf("Start a favourite" to rail.favourites, "Start a recent script" to rail.recent)) {
            if (list.isEmpty()) continue
            Text(
                title,
                style = MaterialTheme.typography.labelMedium,
                color = Turu.Muted,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp, bottom = 2.dp),
            )
            for (quick in list.take(3)) {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clip(MaterialTheme.shapes.medium)
                        .clickable(enabled = idle, onClickLabel = "start ${quick.name}") { scripts.startQuick(quick.path) }
                        .heightIn(min = 48.dp)
                        .padding(horizontal = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        quick.name,
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (idle) Turu.Text else Turu.Muted,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) {
                        if (scripts.starting == quick.path) {
                            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = Turu.Accent)
                        } else {
                            Icon(Icons.Rounded.PlayArrow, contentDescription = null, tint = if (idle) Turu.Accent else Turu.Muted)
                        }
                    }
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
            .clip(MaterialTheme.shapes.medium)
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
                view.elapsedSecs != null -> "${view.state} · ${durationText(view.elapsedSecs.toLong())}"
                else -> view.state
            }
            Text(state, style = MaterialTheme.typography.titleSmallEmphasized, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        }
        view.lastLog?.let { log ->
            val ago = if (log.quietSecs.toLong() < 5) "just now" else "${durationText(log.quietSecs.toLong())} ago"
            Text("Last log $ago", style = MaterialTheme.typography.bodySmall, color = if (log.stale) Turu.Warning else Turu.Muted)
        }
        view.waits?.let { Text("Waits for you: $it", style = MaterialTheme.typography.bodySmall, color = Turu.Warning) }
        view.script?.let { script ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    script.name,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = scripts::favourite) {
                    Icon(
                        Icons.Rounded.Star,
                        contentDescription = if (script.favourite) "Take it off the favourites" else "Add to the favourites",
                        tint = if (script.favourite) Turu.Warning else Turu.Muted,
                    )
                }
            }
        }
        view.error?.let { ErrorText(it.firstLine, it.text) }
    }
}

/** The error's first line, and the whole of it on a tap (the log may have scrolled past it). */
@Composable
private fun ErrorText(first: String, whole: String) {
    var open by remember(whole) { mutableStateOf(false) }
    val more = whole.trim() != first.trim()
    Column(
        Modifier
            .fillMaxWidth()
            .clip(MaterialTheme.shapes.small)
            .then(if (more) Modifier.clickable(onClickLabel = if (open) "show less" else "show the whole error") { open = !open } else Modifier)
            .heightIn(min = if (more) 48.dp else 0.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            if (open) whole.trim() else first,
            style = MaterialTheme.typography.bodySmall,
            color = Turu.Danger,
            maxLines = if (open) Int.MAX_VALUE else 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (more) {
            Text(if (open) "Show less" else "Show the whole error", style = MaterialTheme.typography.labelMedium, color = Turu.Muted)
        }
    }
}

/**
 * No script: Search scripts in Start's place. Loaded: Start alone. Running: Stop with Pause or
 * Resume. Stopping: Force stop. The tapped control shows a spinner until the tab answers.
 */
@Composable
private fun RunControls(view: ScriptsView, scripts: ScriptsPanel) {
    val c = view.controls
    val busy = scripts.busy
    if (view.script == null && !view.status.isActive()) {
        Button(onClick = scripts::searchScripts, shapes = ButtonDefaults.shapes(), modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
            Icon(Icons.Rounded.Search, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text("Search scripts")
        }
        return
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        c.start?.let { start ->
            Button(onClick = scripts::start, shapes = ButtonDefaults.shapes(), enabled = start.enabled && busy == null, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                Working(busy == "start", "Start script")
            }
        }
        c.stop?.let { stop ->
            Button(
                onClick = scripts::stop,
                shapes = ButtonDefaults.shapes(),
                enabled = stop.enabled && busy == null,
                colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = MaterialTheme.colorScheme.onError),
                modifier = Modifier.weight(1.6f).heightIn(min = 48.dp),
            ) { Working(busy == "stop", "Stop") }
        }
        // Force stop sends the same stop: the engine escalates a second Stop to a forced one.
        c.forceStop?.let { force ->
            Button(
                onClick = scripts::stop,
                shapes = ButtonDefaults.shapes(),
                enabled = force.enabled && busy == null,
                colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = MaterialTheme.colorScheme.onError),
                modifier = Modifier.weight(1f).heightIn(min = 48.dp),
            ) { Working(busy == "stop", "Force stop") }
        } ?: c.pause?.let {
            if (c.paused) {
                Button(onClick = scripts::resume, shapes = ButtonDefaults.shapes(), enabled = busy == null, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                    Working(busy == "resume", "Resume")
                }
            } else {
                FilledTonalButton(onClick = scripts::pause, shapes = ButtonDefaults.shapes(), enabled = busy == null, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                    Working(busy == "pause", "Pause")
                }
            }
        }
    }
    // A touch screen has no tooltips: why the control is off, under it.
    c.start?.let { Reason(it) }
    c.forceStop?.let { Reason(it) }
}

@Composable
private fun Reason(control: Control) {
    if (!control.enabled && control.reason.isNotEmpty()) {
        Text(control.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
    }
}

/** The script log, following its last line unless scrolled back. */
@Composable
private fun ScriptLog(scripts: ScriptsPanel, running: Boolean) {
    val list = rememberLazyListState()
    val atEnd by remember { derivedStateOf { !list.canScrollForward } }
    LaunchedEffect(scripts.log.size) {
        if (scripts.log.isNotEmpty() && atEnd) list.scrollToItem(scripts.log.size - 1)
    }
    LaunchedEffect(Unit) { if (scripts.log.isNotEmpty()) list.scrollToItem(scripts.log.size - 1) }
    Column(Modifier.fillMaxSize()) {
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Script log", style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
            TextButton(
                onClick = scripts::clearLog,
                enabled = scripts.log.isNotEmpty(),
                colors = ButtonDefaults.textButtonColors(contentColor = Turu.Muted),
            ) { Text("Clear") }
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
