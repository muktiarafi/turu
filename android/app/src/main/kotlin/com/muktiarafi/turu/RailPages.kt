package com.muktiarafi.turu

import android.content.Context
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.edit
import com.muktiarafi.turu.models.AutoAct
import com.muktiarafi.turu.models.AutoView
import com.muktiarafi.turu.models.Control
import com.muktiarafi.turu.models.JumpAct
import com.muktiarafi.turu.models.MenuItem

/** Why a control is off, under it (a touch screen has no tooltips). */
@Composable
private fun Reason(control: Control?) {
    if (control != null && !control.enabled && control.reason.isNotEmpty()) {
        Text(control.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
    }
}

/** While the tab's models aren't read yet. */
@Composable
private fun Waiting(models: TabModels) {
    Text(
        if (models.tab?.api == null) "The tab is starting…" else "Reading the tab…",
        color = Turu.Muted,
        modifier = Modifier.padding(20.dp),
    )
}

/**
 * Auto (VibeSkua's Auto view), "Combat Automation": what to fight with first (Class, Mode, the
 * monster IDs), the line saying what a Start fights with, then Attack and Hunt with a line each on
 * how they differ. While one runs, a status block leads (what runs, how long, with what) with Stop
 * beside it, and the rest steps back (`turu_frontend::models::auto`).
 */
@Composable
fun AutoPage(state: ShellState, models: TabModels) {
    val view = models.auto
    var error by remember { mutableStateOf<String?>(null) }
    var sending by remember { mutableStateOf<AutoAct?>(null) }
    val act = { action: AutoAct ->
        error = null
        models.act("auto", action, AutoAct.JsonSerializer) { error = it }
    }
    // The tapped start or Stop shows a spinner until the view says it took.
    val press = { action: AutoAct ->
        sending = action
        error = null
        models.act("auto", action, AutoAct.JsonSerializer) { error = it; if (it != null) sending = null }
    }
    LaunchedEffect(view?.running) { sending = null }
    Column(Modifier.fillMaxSize()) {
        PageBar("Combat Automation", onBack = { state.back() })
        if (view == null) return@Column Waiting(models)
        val running = view.running
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // The running card and the start buttons trade places as a run starts and stops:
            // each unfolds or folds on the panel's spring rather than jumping.
            val layout = PanelMotion
            AnimatedVisibility(
                running,
                enter = fadeIn(layout.defaultEffectsSpec()) + expandVertically(layout.defaultSpatialSpec()),
                exit = fadeOut(layout.fastEffectsSpec()) + shrinkVertically(layout.fastSpatialSpec()),
                label = "running card",
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Running(view, stopping = sending == AutoAct.stop) { press(AutoAct.stop) }
                    error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodySmall) }
                }
            }
            // While a run goes, the settings are the next run's: they step back (and fade there).
            val dim by animateFloatAsState(if (running) 0.6f else 1f, layout.defaultEffectsSpec(), label = "settings")
            val settings = Modifier.alpha(dim)
            Column(settings, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                ClassPicker(view, onOpen = { act(AutoAct.reload_classes) }) { act(AutoAct.`class`(it)) }
                Picker(
                    label = "Skills",
                    none = "Base",
                    value = view.mode ?: "",
                    options = view.modes.map { it to it },
                    loading = false,
                    empty = if (view.`class` == null) "Pick a class for its saved skills" else "No saved skills for this class",
                ) { act(AutoAct.mode(it)) }
                var mapIds by remember(models.tab?.id) { mutableStateOf(view.mapIds) }
                OutlinedTextField(
                    value = mapIds,
                    onValueChange = { typed ->
                        mapIds = typed
                        act(AutoAct.map_ids(typed))
                    },
                    label = { Text("Monster IDs to fight first") },
                    placeholder = { Text("Optional, e.g. 1, 2") },
                    supportingText = {
                        val ids = mapIds.split(',', ' ').mapNotNull { it.trim().toLongOrNull() }
                        Text(
                            when {
                                mapIds.isBlank() -> "Their map IDs, in order; empty fights any"
                                ids.isEmpty() -> "No IDs read: numbers, split by commas"
                                ids.size == 1 -> "Fights ${ids[0]} first"
                                else -> "Fights ${ids.joinToString(", ")}, in that order"
                            },
                        )
                    },
                    singleLine = true,
                    shape = MaterialTheme.shapes.medium,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            AnimatedVisibility(
                !running,
                enter = fadeIn(layout.defaultEffectsSpec()) + expandVertically(layout.defaultSpatialSpec()),
                exit = fadeOut(layout.fastEffectsSpec()) + shrinkVertically(layout.fastSpatialSpec()),
                label = "start buttons",
            ) { Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                // The equipped class by name once the class list has been read.
                val with = if (view.`class` != null && view.mode != null) {
                    "${view.`class`}, ${view.mode} skills"
                } else {
                    "${view.equipped ?: "the equipped class"}, Base skills"
                }
                Text("Fights with $with", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 4.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StartButton("Attack", "Fights what's in your cell", sending == AutoAct.attack, view.start.enabled && sending == null, Modifier.weight(1f)) { press(AutoAct.attack) }
                    StartButton("Hunt", "Goes to the cells its targets are in", sending == AutoAct.hunt, view.start.enabled && sending == null, Modifier.weight(1f)) { press(AutoAct.hunt) }
                }
                Reason(view.start)
                error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodySmall) }
            } }
            Spacer(Modifier.heightIn(min = 12.dp))
        }
    }
}

/** What runs, for how long and with what, and Stop: the page's head while Attack or Hunt goes. */
@Composable
private fun Running(view: AutoView, stopping: Boolean, stop: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .clip(MaterialTheme.shapes.medium)
            .background(Turu.Card)
            .padding(start = 12.dp, top = 8.dp, bottom = 8.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(10.dp).clip(CircleShape).background(Turu.Success))
                Spacer(Modifier.width(8.dp))
                Text(
                    listOfNotNull(view.runningMode ?: "Running", view.runningSecs?.let { durationText(it.toLong()) }).joinToString(" · "),
                    style = MaterialTheme.typography.titleSmallEmphasized.copy(fontFeatureSettings = "tnum"),
                )
            }
            view.fighting?.let {
                Text("With $it", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        Button(
            onClick = stop,
            enabled = view.stop.enabled && !stopping,
            colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = MaterialTheme.colorScheme.onError),
            modifier = Modifier.heightIn(min = 48.dp),
        ) {
            if (stopping) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = LocalContentColor.current)
                Spacer(Modifier.width(8.dp))
            }
            Text("Stop")
        }
    }
}

/** Attack or Hunt, with what it does under it, both alike (neither is the default). */
@Composable
private fun StartButton(label: String, about: String, sending: Boolean, enabled: Boolean, modifier: Modifier, onClick: () -> Unit) {
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        FilledTonalButton(onClick = onClick, enabled = enabled, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
            if (sending) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = LocalContentColor.current)
                Spacer(Modifier.width(8.dp))
            }
            Text(label, maxLines = 1)
        }
        Text(about, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 4.dp))
    }
}

/**
 * The class to fight with: the owned ones, read again when the list opens (VibeSkua
 * `ReloadClasses`), the equipped one first and marked. Picking one equips it (VibeSkua
 * `SelectedClass`); none picked fights with the equipped class. Not typed into: on a landscape
 * phone the keyboard would cover the list it filters.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ClassPicker(view: AutoView, onOpen: () -> Unit, onPick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val equipped = view.equipped
    val shown = listOfNotNull(equipped?.takeIf { it in view.classes }) + view.classes.filter { it != equipped }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = {
        if (it && !open) onOpen()
        open = it
    }) {
        OutlinedTextField(
            // None picked reads as what it means, as Skills reads "Base".
            value = view.`class` ?: "The equipped class",
            onValueChange = {},
            readOnly = true,
            label = { Text("Class") },
            supportingText = { Text("Picking one equips it") },
            trailingIcon = {
                if (view.loading) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else ExposedDropdownMenuDefaults.TrailingIcon(expanded = open)
            },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            if (shown.isEmpty()) {
                DropdownMenuItem(
                    text = {
                        Text(
                            when {
                                view.loading -> "Reading your classes…"
                                else -> "No classes (not logged in?)"
                            },
                            color = Turu.Muted,
                        )
                    },
                    onClick = {},
                    enabled = false,
                )
            }
            for (name in shown) {
                DropdownMenuItem(
                    text = { Text(name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    trailingIcon = if (name == equipped) {
                        { Text("Equipped", style = MaterialTheme.typography.labelMedium, color = Turu.Muted) }
                    } else {
                        null
                    },
                    onClick = {
                        open = false
                        onPick(name)
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

/** Jump (VibeSkua's Fast Map Jump): a cell and a pad, Current and Jump (`models::jump`). */
@Composable
fun JumpPage(state: ShellState, models: TabModels) {
    val view = models.jump
    LaunchedEffect(models.tab?.id, models.tab?.api) { models.act("jump", JumpAct.refresh, JumpAct.JsonSerializer) }
    val act = { action: JumpAct -> models.act("jump", action, JumpAct.JsonSerializer) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Fast Map Jump", onBack = { state.back() })
        if (view == null) return@Column Waiting(models)
        Column(Modifier.padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Picker(
                label = "Cell",
                none = "Select a cell",
                value = view.cell ?: "",
                options = view.cells.map { it to it },
                loading = false,
                onOpen = { act(JumpAct.refresh) },
                empty = "No cells (not in a map?)",
            ) { act(JumpAct.cell(it)) }
            Picker(
                label = "Pad",
                none = "Select a pad",
                value = view.pad,
                options = view.pads.map { it to it },
                loading = false,
            ) { act(JumpAct.pad(it)) }
            if (view.autoJump) {
                Text("Picking a cell or pad jumps at once", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { act(JumpAct.current) }, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                    Text("Current")
                }
                Button(
                    onClick = { act(JumpAct.jump) },
                    enabled = view.jump.enabled,
                    modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                ) { Text("Jump") }
            }
            Reason(view.jump)
            view.status?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Turu.Muted) }
        }
    }
}

/**
 * Logs: the script's and the client's, line-fed (`models::logs`), and the crash reports earlier
 * runs left (`models::crashes`), offered once there is one; Find and Copy in the full window.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LogsPage(state: ShellState, models: TabModels, crashes: CrashesPanel, full: () -> Unit) {
    val kind = state.logsKind
    val reports = crashCount(state, crashes)
    // Most never crash: the segment shows once there is a report, and stays while it's chosen.
    val segments = buildList {
        add("script" to "Script")
        add("debug" to "Client")
        // The count is the list's first line: "Crashes (30)" doesn't fit a third of the panel.
        if (reports > 0 || kind == "crashes") add("crashes" to "Crashes")
    }
    val lines = if (kind == "script") models.scriptLog else models.clientLog
    val list = rememberLazyListState()
    LaunchedEffect(kind, lines.size) { if (kind != "crashes" && lines.isNotEmpty()) list.scrollToItem(lines.size - 1) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Logs", onBack = { state.back() }) {
            if (kind != "crashes") {
                TextButton(onClick = { models.clearLog(kind) }, enabled = lines.isNotEmpty()) { Text("Clear") }
            }
            FullWindowMenu(adds = "Find and Copy in the logs", open = full)
        }
        ConnectedChoice(segments.map { it.first }, kind, { state.logsKind = it }, Modifier.fillMaxWidth().padding(horizontal = 12.dp)) { key ->
            Text(segments.first { it.first == key }.second, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (kind == "crashes") return@Column CrashList(state, crashes, models)
        if (lines.isEmpty()) {
            Text(
                if (kind == "script") "The script's log shows here when it runs" else "Nothing logged yet",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(16.dp),
            )
        }
        LazyColumn(state = list, modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 8.dp)) {
            itemsIndexed(lines) { _, (time, text) ->
                Row(Modifier.padding(vertical = 2.dp)) {
                    Text(time.substringAfter('T').take(8), style = MaterialTheme.typography.bodySmall, color = Turu.Muted, fontFamily = FontFamily.Monospace)
                    Spacer(Modifier.width(8.dp))
                    Text(text, style = MaterialTheme.typography.bodySmall, color = Turu.Text)
                }
            }
        }
    }
}

/** The windows the shell has as pages of the panel, by their egui key (`Windows::open`). */
private fun nativePage(window: String): Page? = when (window) {
    "script_loader" -> Page.Scripts
    "accounts" -> Page.Accounts
    "new_tab" -> Page.NewTab
    "script_repo" -> Page.Search
    "game_options" -> Page.GameOptions
    "application_options" -> Page.AppOptions
    "core_bots" -> Page.CoreBots
    "scheduler" -> Page.Scheduler
    "army" -> Page.Army
    "notifications" -> Page.Notifications
    "stats" -> Page.Stats
    "current_drops" -> Page.CurrentDrops
    "skills" -> Page.Skills
    "loadouts" -> Page.Loadouts
    "fast_travel" -> Page.FastTravel
    "butler" -> Page.Butler
    "cosmetics" -> Page.Cosmetics
    "runtime" -> Page.Runtime
    "notify_drop" -> Page.NotifyDrop
    "loader" -> Page.Loader
    "grabber" -> Page.Grabber
    "junk_items" -> Page.JunkItems
    "console" -> Page.Console
    "packet_logger" -> Page.PacketLogger
    "spammer" -> Page.Spammer
    "interceptor" -> Page.Interceptor
    "script_options" -> Page.ScriptOptions
    "logs" -> Page.Logs
    "client_update" -> Page.Update
    else -> null
}

/**
 * What each of More's windows is for, in a line under its name (and words its search matches):
 * VibeSkua's names don't always say ("Runtime", "Interceptor").
 */
private val ABOUT = mapOf(
    "script_loader" to "Load, start and stop a script; its log",
    "script_repo" to "Find, download and start scripts",
    "script_options" to "The loaded script's own settings",
    "scheduler" to "A playlist of scripts run one after another",
    "game_options" to "Relogin, lag killer, FPS and the game's switches",
    "application_options" to "Turu's own settings",
    "core_bots" to "CoreBots' settings for this character",
    "hotkeys" to "Keyboard shortcuts",
    "loader" to "Load quests and shops by ID; the quest cache",
    "grabber" to "Read inventory, shops, quests, monsters; act on them",
    "current_drops" to "Drops waiting to be picked up",
    "junk_items" to "Mark items as junk and sell them",
    "loadouts" to "Save and equip sets of gear",
    "fast_travel" to "Saved places to travel to in a tap",
    "skills" to "The skill order and its rules",
    "stats" to "Kills, deaths, drops and run time",
    "runtime" to "Drops to pick up, quests to keep accepted, boosts",
    "notifications" to "What happened: drops, deaths, relogins, script events",
    "logs" to "The script's, the client's and the bot's logs",
    "console" to "Run a TypeScript snippet with bot in scope",
    "spammer" to "Send packets on a timer",
    "packet_logger" to "Watch the packets the game sends",
    "interceptor" to "Read and change packets through a proxy",
    "butler" to "Have accounts follow and help each other",
    "cosmetics" to "Copy looks and change your outfit",
    "accounts" to "Saved accounts; start them in tabs",
    "army" to "Command every tab and client at once",
    "army_queue" to "Squads of accounts that run scripts in turn",
    "new_tab" to "Open another account in a tab",
)

/** More's search: the item's name or what it's for, every word typed. */
private fun matches(item: MenuItem, menu: String, typed: String): Boolean {
    val text = "${item.label} ${ABOUT[item.window].orEmpty()} $menu".lowercase()
    return typed.lowercase().split(' ').filter { it.isNotBlank() }.all { it in text }
}

/**
 * More's memory on this device: the menu on show and the windows opened last (newest first). In
 * preferences, not the panel's state, so a font-size change (the activity made again) keeps them.
 */
private class MoreMemory(context: Context) {
    private val prefs = context.getSharedPreferences("more", Context.MODE_PRIVATE)
    var menu by mutableStateOf(prefs.getString("menu", null))
        private set
    var recent by mutableStateOf(prefs.getString("recent", "").orEmpty().split(',').filter { it.isNotEmpty() })
        private set

    fun show(title: String) {
        menu = title
        prefs.edit { putString("menu", title) }
    }

    fun opened(window: String) {
        recent = (listOf(window) + recent.filter { it != window }).take(RECENT)
        prefs.edit { putString("recent", recent.joinToString(",")) }
    }
}

private const val RECENT = 4

/** The Recent group's title in More's list. */
private const val RECENT_TITLE = "Recent"

/**
 * More: VibeSkua's menus as this platform has them (`models::rail`), list and detail so none is
 * more than a tap away: Recent (the windows opened last) and the menus down the left, the one
 * picked on the right, each window with a line on what it's for. A search above the list looks
 * through every window's name and line. A native window opens its page; one that opens the
 * slot's full egui window says so. The bar holds what the rail folded and no menu has (Bank,
 * Update), and ⋮ Hide Side Bar.
 */
@Composable
fun MorePage(state: ShellState, models: TabModels, window: (String) -> Unit, bank: () -> Unit, hideRail: () -> Unit) {
    val rail = models.rail
    val menus = rail?.menus
    val context = LocalContext.current
    val memory = remember { MoreMemory(context) }
    var search by remember { mutableStateOf("") }
    val open = { item: MenuItem ->
        memory.opened(item.window)
        val page = nativePage(item.window)
        if (page != null) state.push(page) else window(item.window)
    }
    Column(Modifier.fillMaxSize()) {
        PageBar("More", onBack = { state.back() }) {
            if ("Bank" in state.folded) {
                TextButton(onClick = bank) {
                    Icon(RailIcons.Bank, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Bank")
                }
            }
            val update = rail?.update
            if ("Update" in state.folded && update != null) {
                // The version, as the egui bar says it, unless a pre-release's long one would crowd the title.
                val label = if (update.version.length <= 8) "Update ${update.version}" else "Update"
                val said = update.failed?.let { "failed: $it" } ?: update.busy ?: "version ${update.version} ready"
                TextButton(
                    onClick = { state.push(Page.Update) },
                    modifier = Modifier.semantics { stateDescription = said },
                ) {
                    if (update.busy != null) {
                        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = Turu.Accent)
                    } else {
                        BadgedBox(badge = { Badge(containerColor = if (update.failed != null) Turu.Danger else Turu.Accent) }) {
                            Icon(RailIcons.Update, contentDescription = null, modifier = Modifier.size(18.dp))
                        }
                    }
                    Spacer(Modifier.width(6.dp))
                    Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            // A layout setting, not a place to go: under ⋮, so the bar keeps to destinations.
            var more by remember { mutableStateOf(false) }
            Box {
                IconButton(onClick = { more = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "More options", tint = Turu.Muted) }
                DropdownMenu(expanded = more, onDismissRequest = { more = false }, containerColor = Turu.Card) {
                    DropdownMenuItem(
                        text = {
                            Column {
                                Text("Hide Side Bar")
                                Text("The tab at the left edge brings it back", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                            }
                        },
                        onClick = { more = false; hideRail() },
                    )
                }
            }
        }
        if (menus == null) return@Column Waiting(models)
        OutlinedTextField(
            value = search,
            onValueChange = { search = it },
            placeholder = { Text("Search windows: drops, packets…", maxLines = 1) },
            leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
            trailingIcon = if (search.isNotEmpty()) {
                { IconButton(onClick = { search = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } }
            } else {
                null
            },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 8.dp),
        )
        val all = menus.flatMap { m -> m.items.map { m.title to it } }
        if (search.isNotBlank()) {
            val found = all.filter { (menu, item) -> matches(item, menu, search) }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 8.dp)) {
                if (found.isEmpty()) {
                    item {
                        Text(
                            "No window matches. Try a word for what it does: drops, quests, packets.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Turu.Muted,
                            modifier = Modifier.padding(12.dp),
                        )
                    }
                }
                items(found, key = { it.second.window }) { (menu, item) -> MenuRow(item, nativePage(item.window), where = menu) { open(item) } }
            }
            return@Column
        }
        val recent = memory.recent.mapNotNull { w -> all.firstOrNull { it.second.window == w } }
        val titles = (if (recent.isNotEmpty()) listOf(RECENT_TITLE) else emptyList()) + menus.map { it.title }
        val shown = memory.menu?.takeIf { it in titles } ?: menus.first().title
        Row(Modifier.fillMaxSize()) {
            val scroll = rememberScrollState()
            Box(Modifier.width(152.dp).fillMaxHeight()) {
                Column(
                    Modifier
                        .fillMaxSize()
                        .verticalScroll(scroll)
                        .padding(start = 8.dp, end = 8.dp, bottom = 8.dp)
                        .selectableGroup(),
                ) {
                    for (title in titles) {
                        val selected = title == shown
                        Box(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 48.dp)
                                .clip(MaterialTheme.shapes.medium)
                                .background(selectionTint(selected))
                                .selectable(selected = selected, role = Role.Tab) { memory.show(title) }
                                .padding(horizontal = 12.dp, vertical = 8.dp),
                            contentAlignment = Alignment.CenterStart,
                        ) {
                            Text(
                                title,
                                style = MaterialTheme.typography.labelLarge,
                                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                                color = Turu.Text,
                            )
                        }
                    }
                }
                // More menus below the edge: an arrow there says the list scrolls.
                if (scroll.canScrollForward) {
                    Box(
                        Modifier.align(Alignment.BottomCenter).fillMaxWidth().background(Turu.Panel).padding(vertical = 2.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(Icons.Rounded.KeyboardArrowDown, contentDescription = "More menus below", tint = Turu.Muted)
                    }
                }
            }
            VerticalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            val motion = PanelMotion
            AnimatedContent(
                targetState = shown,
                transitionSpec = { fadeIn(motion.defaultEffectsSpec()).togetherWith(fadeOut(motion.fastEffectsSpec())) },
                label = "more menu",
            ) { title ->
                val items = if (title == RECENT_TITLE) recent.map { it.second } else menus.firstOrNull { it.title == title }?.items.orEmpty()
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 0.dp)) {
                    items(items, key = { it.window }) { item -> MenuRow(item, nativePage(item.window)) { open(item) } }
                }
            }
        }
    }
}

/**
 * A row of More: the window's name and, under it, why it's off or what it's for (with its menu
 * in a search). One that opens the slot's full window, not a page here, says so.
 */
@Composable
private fun MenuRow(item: MenuItem, page: Page?, where: String? = null, open: () -> Unit) {
    val enabled = item.control.enabled
    val about = when {
        !enabled && item.control.reason.isNotEmpty() -> item.control.reason
        else -> listOfNotNull(where, ABOUT[item.window]).joinToString(" · ").ifEmpty { null }
    }
    ListItem(
        modifier = Modifier
            .clip(MaterialTheme.shapes.medium)
            .clickable(enabled = enabled, onClick = open),
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        supportingContent = if (about != null || page == null) {
            {
                Column {
                    about?.let { Text(it, color = Turu.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis) }
                    if (page == null && enabled) Text("Opens the full window", style = MaterialTheme.typography.labelMedium, color = Turu.Accent)
                }
            }
        } else {
            null
        },
    ) {
        Text(item.label, color = if (enabled) Turu.Text else Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}
