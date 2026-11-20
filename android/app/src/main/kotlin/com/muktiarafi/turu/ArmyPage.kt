package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.ArmyRow
import com.muktiarafi.turu.models.ArmyView
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * Army Control's copy (`turu_frontend::models::army`), read from the tab on screen while the page
 * shows: every tab and launched client, whichever slot answers. The checked rows live here, by
 * key (the same in every slot), and go with each command; none checked means every row.
 */
class ArmyPanel {
    var view by mutableStateOf<ArmyView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** The checked rows' keys. */
    val checked = mutableStateMapOf<String, Unit>()

    /** Misc options as last sent from here (the clients don't say theirs). */
    val options = mutableStateMapOf<String, Boolean>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    /** Pages reading it (Army Control and the pages it opens): the last one leaving stops it. */
    private var users = 0

    fun open() { users++ }

    fun close() {
        users = maxOf(0, users - 1)
        if (users == 0) follow(null)
    }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        following = tab
        client?.stop()
        client = null
        // Another slot answers now, or none while it starts again: no stale copy meanwhile.
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("army"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ArmyView>(text) }
                    .onSuccess { v ->
                        view = v
                        // A row that ended or went can't be sent to.
                        val live = v.rows.filter { !it.ended }.map { it.key }.toSet()
                        checked.keys.retainAll(live)
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/army doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    private fun act(action: String, body: JSONObject = JSONObject()) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("army", action, body) { error = it }
    }

    /** A command to the checked rows, or to every row still running. */
    fun send(command: String, body: JSONObject = JSONObject()) =
        act("send", body.put("command", command).put("keys", JSONArray(checked.keys.toList())))

    fun row(row: ArmyRow, action: String) = act("row", JSONObject().put("key", row.key).put("action", action))

    fun task(action: String) = act(action)

    fun toggle(row: ArmyRow) {
        if (row.ended) return
        if (checked.remove(row.key) == null) checked[row.key] = Unit
    }
}

/**
 * Army > Army Control (docs/UI.md, "Native shells"; VibeSkua's tabbed host menu): the clients,
 * each checkable with its own Start or Stop and a ⋮; then the commands, which go to the checked
 * rows or to every row (Travel: Map Travel's page, the player and server pickers); the last outcome
 * at the foot. A row's ⋮ opens its Gear and Logs pages. The full window adds saving logs to a file.
 */
@Composable
fun ArmyPage(
    state: ShellState,
    panel: ArmyPanel,
    travel: ArmyTravelPanel,
    select: (Int) -> Unit,
    fullWindow: () -> Unit,
) {
    DisposableEffect(Unit) {
        panel.open()
        onDispose { panel.close() }
    }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    // A command for every row, asked first: its label and how to send it.
    var asking by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    var history by remember { mutableStateOf(false) }
    var picking by remember { mutableStateOf<TravelPick?>(null) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Army Control", onBack = { state.back() }) {
            FullWindowMenu("Save a client's logs to a file", fullWindow) { close ->
                if (view?.launched == true) {
                    DropdownMenuItem(text = { Text("Clear ended") }, onClick = { close(); panel.task("clear-ended") })
                    DropdownMenuItem(
                        text = { Text("Close all launched clients", color = Turu.Danger) },
                        onClick = { close(); asking = "Close all launched clients?" to { panel.task("close-all") } },
                    )
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                }
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Army Control is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        val live = view.rows.filter { !it.ended }
        val targets = if (panel.checked.isEmpty()) live else live.filter { it.key in panel.checked }
        val everyone = panel.checked.isEmpty()
        val askTwice = everyone && targets.size > 1
        val any = targets.isNotEmpty()
        // Stop script and Log out ask first when they'd go to every client.
        fun guarded(label: String, command: String) {
            if (askTwice) asking = "$label in all ${targets.size} clients?" to { panel.send(command) } else panel.send(command)
        }
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = 12.dp)) {
            item { ClientsHeader(view, live, panel) }
            items(view.rows, key = { it.key }) { row ->
                ClientRow(row, panel, shown = row.shellTab == state.tabs.shown.toLong(), select = select, open = { state.push(it) })
            }
            items(view.waiting, key = { "waiting:$it" }) { name ->
                Text(
                    "$name: waiting to start",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    modifier = Modifier.padding(start = 60.dp, top = 8.dp, bottom = 8.dp),
                )
            }
            if (view.rows.size < 2) {
                item {
                    Text(
                        "Open accounts in more tabs (Tabs > New tab) to command them together.",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }
            item {
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(top = 8.dp))
                ArmyTarget(targets, everyone)
            }
            item {
                Group("Script") {
                    ButtonGrid(
                        listOf(
                            Action("Start script", primary = true, enabled = any) { panel.send("start-script") },
                            Action("Stop script", enabled = any) { guarded("Stop the script", "stop-script") },
                            Action("Search scripts…", enabled = any) {
                                state.scriptPick = ScriptPick.Army(panel.checked.keys.toList(), targets.singleOrNull()?.label ?: "${targets.size} clients")
                                state.push(Page.Search)
                            },
                            Action("This tab's script", enabled = any) { panel.send("tab-script") },
                        ),
                    )
                }
            }
            item {
                Group("Account") {
                    ButtonGrid(
                        listOf(
                            Action("Log in", enabled = any) { panel.send("login") },
                            Action("Log out", enabled = any) { guarded("Log out", "logout") },
                            Action("Relog", enabled = any) { panel.send("relog") },
                            Action("Go to house", enabled = any) { panel.send("house") },
                        ),
                    )
                }
            }
            item {
                Group("Auto-Attack") {
                    ButtonGrid(
                        listOf(
                            Action("Start Auto-Attack", enabled = any) { panel.send("auto-attack") },
                            Action("Stop Auto-Attack", enabled = any) { panel.send("stop-auto") },
                        ),
                    )
                    Reason("Each client's equipped class; clients running a script are skipped.")
                }
            }
            item { Fold("Quest") { QuestForm(any) { id, reward -> panel.send("quest", JSONObject().put("id", id).apply { reward?.let { put("reward", it) } }) } } }
            item {
                Fold("Misc options") {
                    view.options.forEach { option ->
                        val on = panel.options[option.name] ?: false
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 48.dp)
                                .clickable(enabled = any) {
                                    panel.options[option.name] = !on
                                    panel.send("option", JSONObject().put("name", option.name).put("on", !on))
                                }
                                .padding(horizontal = 16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(option.label, modifier = Modifier.weight(1f), color = if (any) Turu.Text else Turu.Muted)
                            Switch(
                                checked = on,
                                enabled = any,
                                onCheckedChange = {
                                    panel.options[option.name] = it
                                    panel.send("option", JSONObject().put("name", option.name).put("on", it))
                                },
                            )
                        }
                    }
                }
            }
            item {
                Group("Travel") {
                    ButtonGrid(
                        listOf(
                            Action("Map Travel…") { state.push(Page.ArmyTravel) },
                            Action("Jump to player…", enabled = any) { picking = TravelPick.Player },
                            Action("Change server…", enabled = any) { picking = TravelPick.Server },
                        ),
                    )
                }
            }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Outcome(panel.error, view.results.lastOrNull()) { history = true }
    }
    when (picking) {
        TravelPick.Player -> PlayerDialog(state, panel, travel) { picking = null }
        TravelPick.Server -> ServerDialog(state, panel, travel) { picking = null }
        null -> {}
    }
    asking?.let { (question, send) ->
        AlertDialog(
            onDismissRequest = { asking = null },
            title = { Text(question) },
            text = { Text("No client is checked, so it goes to every one. Check rows to pick some.", color = Turu.Muted) },
            confirmButton = {
                TextButton(onClick = { asking = null; send() }) { Text("Yes, all", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { asking = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    if (history) {
        AlertDialog(
            onDismissRequest = { history = false },
            title = { Text("Outcomes") },
            text = {
                LazyColumn(Modifier.heightIn(max = 280.dp)) {
                    items(view?.results.orEmpty().reversed()) { line ->
                        Text(line, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, modifier = Modifier.padding(vertical = 4.dp))
                    }
                }
            },
            confirmButton = { TextButton(onClick = { history = false }) { Text("Close") } },
            dismissButton = { TextButton(onClick = { history = false; panel.task("clear-results") }) { Text("Clear") } },
            containerColor = Turu.Card,
        )
    }
}

/** Above the rows: check all or none, how many are checked, and Groups. */
@Composable
private fun ClientsHeader(view: ArmyView, live: List<ArmyRow>, panel: ArmyPanel) {
    val checked = live.count { it.key in panel.checked }
    val box = when {
        checked == 0 -> ToggleableState.Off
        checked == live.size -> ToggleableState.On
        else -> ToggleableState.Indeterminate
    }
    var groups by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        TriStateCheckbox(
            state = box,
            enabled = live.isNotEmpty(),
            onClick = {
                if (box == ToggleableState.Off) live.forEach { panel.checked[it.key] = Unit } else panel.checked.clear()
            },
        )
        val count = when {
            checked > 0 -> "$checked of ${live.size} selected"
            live.size == 1 -> "1 client"
            else -> "${live.size} clients"
        } + if (view.waiting.isNotEmpty()) " · ${view.waiting.size} waiting" else ""
        Text(
            count,
            style = MaterialTheme.typography.labelLarge,
            color = if (checked > 0) Turu.Text else Turu.Muted,
            modifier = Modifier.weight(1f),
        )
        if (view.groups.isNotEmpty()) {
            Box {
                TextButton(onClick = { groups = true }) {
                    Text("Groups")
                    Icon(Icons.Rounded.ArrowDropDown, contentDescription = null)
                }
                DropdownMenu(expanded = groups, onDismissRequest = { groups = false }, containerColor = Turu.Card) {
                    view.groups.forEach { group ->
                        DropdownMenuItem(
                            text = {
                                Column {
                                    Text(group.name)
                                    Text("${group.keys.size} of ${group.accounts} open", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                                }
                            },
                            onClick = {
                                groups = false
                                panel.checked.clear()
                                group.keys.forEach { panel.checked[it] = Unit }
                            },
                        )
                    }
                }
            }
        }
    }
}

/**
 * A client: its check box, a dot for its state, its name and kind; under them what it runs and
 * where it is. Its own Start or Stop and a ⋮ at the end. A tap anywhere checks it.
 */
@Composable
private fun ClientRow(row: ArmyRow, panel: ArmyPanel, shown: Boolean, select: (Int) -> Unit, open: (Page) -> Unit) {
    val checked = row.key in panel.checked
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (checked) Turu.AccentTint else Color.Transparent)
            .clickable(enabled = !row.ended) { panel.toggle(row) }
            .heightIn(min = 60.dp)
            .padding(end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = checked, enabled = !row.ended, onCheckedChange = { panel.toggle(row) })
        Column(Modifier.weight(1f).padding(end = 8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(row.tint.color()))
                Spacer(Modifier.width(8.dp))
                Text(
                    row.label,
                    fontWeight = FontWeight.SemiBold,
                    color = if (row.ended) Turu.Muted else Turu.Text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                val kind = if (shown) "on screen" else row.kind
                if (kind.isNotEmpty()) {
                    Text(kind, style = MaterialTheme.typography.labelSmall, color = Turu.Muted, modifier = Modifier.padding(start = 8.dp))
                }
            }
            val doing = when {
                row.auto.isNotEmpty() -> row.auto to Turu.Success
                row.script.isNotEmpty() -> "${row.script}${if (row.running) "" else " (loaded)"}" to if (row.running) Turu.Success else Turu.Muted
                else -> null
            }
            Text(
                listOfNotNull(doing?.first, row.summary).joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = when {
                    row.error.isNotEmpty() -> Turu.Danger
                    doing != null -> doing.second
                    else -> Turu.Muted
                },
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (row.error.isNotEmpty()) {
                Text(row.error, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        when {
            row.ended && row.kind != "tab" -> OutlinedButton(onClick = { panel.row(row, "restart") }, border = edge(true), modifier = Modifier.heightIn(min = 48.dp)) { Text("Restart") }
            row.ended -> {}
            row.running -> OutlinedButton(onClick = { panel.row(row, "stop") }, border = edge(true), modifier = Modifier.heightIn(min = 48.dp)) { Text("Stop") }
            else -> FilledTonalButton(onClick = { panel.row(row, "start") }, enabled = row.start.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Start") }
        }
        // A tab that's down has nothing to offer here: the shell restarts it (Tabs).
        if (row.ended && row.kind == "tab") {
            Spacer(Modifier.width(48.dp))
            return@Row
        }
        Box {
            IconButton(onClick = { menu = true }) {
                Icon(Icons.Rounded.MoreVert, contentDescription = "${row.label}: log in or out, relog, gear, logs, Auto-Attack", tint = Turu.Muted)
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                if (row.ended) {
                    DropdownMenuItem(text = { Text("Remove") }, onClick = { menu = false; panel.row(row, "close") })
                    return@DropdownMenu
                }
                if (row.shellTab >= 0 && !shown) {
                    DropdownMenuItem(text = { Text("Show its tab") }, onClick = { menu = false; select(row.shellTab.toInt()) })
                }
                if (row.loggedIn) {
                    DropdownMenuItem(text = { Text("Log out") }, onClick = { menu = false; panel.row(row, "logout") })
                } else {
                    DropdownMenuItem(text = { Text("Log in") }, onClick = { menu = false; panel.row(row, "login") })
                }
                DropdownMenuItem(text = { Text("Relog") }, onClick = { menu = false; panel.row(row, "relog") })
                DropdownMenuItem(text = { Text("Gear…") }, onClick = { menu = false; open(Page.ArmyGear(row.key, row.label)) })
                DropdownMenuItem(text = { Text("Logs…") }, onClick = { menu = false; open(Page.ArmyLogs(row.key, row.label)) })
                if (row.auto.isNotEmpty()) {
                    DropdownMenuItem(text = { Text("Stop ${row.auto}") }, onClick = { menu = false; panel.row(row, "stop-auto") })
                } else {
                    DropdownMenuItem(
                        text = {
                            Column {
                                Text("Start Auto-Attack")
                                if (!row.autoAttack.enabled) Text(row.autoAttack.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                            }
                        },
                        enabled = row.autoAttack.enabled,
                        onClick = { menu = false; panel.row(row, "auto-attack") },
                    )
                }
                if (row.kind != "tab") {
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                    DropdownMenuItem(text = { Text("Restart") }, onClick = { menu = false; panel.row(row, "restart") })
                    DropdownMenuItem(text = { Text("Close", color = Turu.Danger) }, onClick = { menu = false; panel.row(row, "close") })
                }
                if (!row.start.enabled && !row.running) {
                    Text(
                        "Start: ${row.start.reason}",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp).width(220.dp),
                    )
                }
            }
        }
    }
}

/** Who the commands below go to; amber while that's every client. */
@Composable
fun ArmyTarget(targets: List<ArmyRow>, everyone: Boolean) {
    val text = when {
        targets.isEmpty() -> "No client to send to"
        everyone && targets.size == 1 -> "To the only client"
        everyone -> "To all ${targets.size} clients · check rows to pick"
        targets.size <= 3 -> "To ${targets.joinToString(", ") { it.label }}"
        else -> "To the ${targets.size} selected"
    }
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = if (everyone && targets.size > 1) Turu.Warning else Turu.Text,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp),
    )
}

/** A heading and its controls. */
@Composable
private fun Group(title: String, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(top = 14.dp)) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = Modifier.padding(bottom = 6.dp))
        content()
    }
}

/** A heading that opens and closes what's under it (closed at first, as the window's are). */
@Composable
private fun Fold(title: String, content: @Composable () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().padding(top = 6.dp)) {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable { open = !open }
                .heightIn(min = 48.dp)
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(title, style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = Modifier.weight(1f))
            Icon(
                if (open) Icons.Rounded.KeyboardArrowDown else Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = if (open) "Fold $title" else "Open $title",
                tint = Turu.Muted,
            )
        }
        if (open) content()
    }
}

private class Action(val label: String, val primary: Boolean = false, val enabled: Boolean = true, val onClick: () -> Unit)

/**
 * Buttons in even columns, as many to a line as fit at [min] wide (wider with a larger font, so
 * labels wrap to fewer columns rather than cut), balanced so the lines keep the same count (2 + 2,
 * not 3 + 1).
 */
@Composable
private fun ButtonGrid(actions: List<Action>, min: Dp = 128.dp) {
    val least = min * LocalDensity.current.fontScale
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val gap = 8.dp
        val fit = ((maxWidth + gap) / (least + gap)).toInt().coerceIn(1, actions.size)
        val lines = (actions.size + fit - 1) / fit
        val columns = (actions.size + lines - 1) / lines
        Column(verticalArrangement = Arrangement.spacedBy(gap)) {
            actions.chunked(columns).forEach { line ->
                Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                    line.forEach { action ->
                        val modifier = Modifier.weight(1f).heightIn(min = 48.dp)
                        val padding = PaddingValues(horizontal = 12.dp)
                        if (action.primary) {
                            Button(onClick = action.onClick, enabled = action.enabled, modifier = modifier, contentPadding = padding) {
                                Text(action.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        } else {
                            OutlinedButton(
                                onClick = action.onClick,
                                enabled = action.enabled,
                                modifier = modifier,
                                contentPadding = padding,
                                border = edge(action.enabled),
                            ) {
                                Text(action.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                    repeat(columns - line.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

/** An outlined button's border: the shell's edge, still drawn while it's off, so it reads as a button. */
private fun edge(enabled: Boolean) = BorderStroke(1.dp, if (enabled) Turu.Edge else Turu.Edge.copy(alpha = 0.45f))

@Composable
private fun Reason(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 6.dp))
}

/** Quest ID and an optional reward ID, then Accept quest. */
@Composable
private fun QuestForm(any: Boolean, accept: (Long, Long?) -> Unit) {
    var quest by remember { mutableStateOf("") }
    var reward by remember { mutableStateOf("") }
    val id = quest.trim().toLongOrNull()?.takeIf { it > 0 }
    // In landscape the keyboard covers Accept quest: its Done accepts.
    val done = KeyboardActions(onDone = { if (any) id?.let { accept(it, reward.trim().toLongOrNull()) } })
    Column(Modifier.padding(horizontal = 16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = quest,
                onValueChange = { quest = it.filter(Char::isDigit) },
                label = { Text("Quest ID") },
                singleLine = true,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next),
                modifier = Modifier.weight(1f),
            )
            OutlinedTextField(
                value = reward,
                onValueChange = { reward = it.filter(Char::isDigit) },
                label = { Text("Reward ID (optional)") },
                singleLine = true,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
                keyboardActions = done,
                modifier = Modifier.weight(1f),
            )
        }
        OutlinedButton(
            onClick = { id?.let { accept(it, reward.trim().toLongOrNull()) } },
            enabled = any && id != null,
            border = edge(any && id != null),
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp).heightIn(min = 48.dp),
        ) { Text("Accept quest") }
        if (id == null) Reason("Enter a quest ID")
    }
}

/** The last outcome (or the error sending it), on one line, with the earlier ones a tap away. */
@Composable
private fun Outcome(error: String?, last: String?, history: () -> Unit) {
    val text = error ?: last ?: "Outcomes show here"
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.bodySmall,
            fontFamily = if (error == null && last != null) FontFamily.Monospace else null,
            color = when {
                error != null -> Turu.Danger
                last == null -> Turu.Muted
                else -> Turu.Text
            },
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (last != null) TextButton(onClick = history, modifier = Modifier.heightIn(min = 48.dp)) { Text("History") }
    }
}
