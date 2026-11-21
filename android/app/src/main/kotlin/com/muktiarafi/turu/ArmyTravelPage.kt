package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.ArmyRow
import com.muktiarafi.turu.models.ArmyTravelView
import com.muktiarafi.turu.models.TravelDest
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * Army travel's copy (`turu_frontend::models::army_travel`): the last jump, the clients' visited
 * maps, the saved presets, the players and the online servers, read while a page or picker shows.
 * One per process like Army Control; the pages using it count themselves in and out.
 */
class ArmyTravelPanel {
    var view by mutableStateOf<ArmyTravelView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    private var users = 0
    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun open() { users++ }

    fun close() {
        users = maxOf(0, users - 1)
        if (users == 0) follow(null)
    }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) error = null
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("army-travel"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ArmyTravelView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/army-travel doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: String, body: JSONObject = JSONObject(), done: (String?) -> Unit = {}) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("army-travel", "do", body.put("action", action)) { e ->
            error = e
            done(e)
        }
    }
}

/** Follows both panels while a travel page or picker shows. */
@Composable
private fun FollowTravel(state: ShellState, army: ArmyPanel, travel: ArmyTravelPanel) {
    DisposableEffect(Unit) {
        army.open()
        travel.open()
        onDispose {
            travel.close()
            army.close()
        }
    }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) {
        army.follow(state.tabs.active)
        travel.follow(state.tabs.active)
    }
}

/** The rows commands reach: the checked live ones, else every live one. */
private fun targets(army: ArmyPanel): Pair<List<ArmyRow>, Boolean> {
    val live = army.view?.rows?.filter { !it.ended }.orEmpty()
    val everyone = army.checked.isEmpty()
    return (if (everyone) live else live.filter { it.key in army.checked }) to everyone
}

/** "Use current" reads this client: the only checked row, else the tab on screen. */
private fun currentKey(state: ShellState, army: ArmyPanel): String? =
    army.checked.keys.singleOrNull() ?: army.view?.rows?.firstOrNull { it.shellTab == state.tabs.shown.toLong() }?.key

private fun jumpBody(d: TravelDest) =
    JSONObject().put("map", d.map).put("room", d.room).put("cell", d.cell).put("pad", d.pad)

/** "map-room · cell, pad" for a status line. */
private fun place(d: TravelDest): String {
    val map = if (d.room.isBlank()) d.map.trim() else "${d.map.trim()}-${d.room.trim()}"
    return "$map · ${d.cell.ifBlank { "Enter" }}, ${d.pad.ifBlank { "Spawn" }}"
}

/** VibeSkua's "Jump to Map / Cell" text pasted into Map: `map,room,cell,pad`. */
private fun parsePasted(text: String): TravelDest? {
    val parts = text.split(',').map { it.trim() }
    val map = parts.firstOrNull().orEmpty().ifEmpty { return null }
    val (room, cell) = when (parts.size) {
        1 -> "" to "Enter"
        2 -> "" to parts[1]
        else -> parts[1] to parts[2]
    }
    return TravelDest(name = "", map = map, room = room.filter { it.isDigit() }, cell = cell.ifEmpty { "Enter" }, pad = parts.getOrNull(3).orEmpty().ifEmpty { "Spawn" }, label = "")
}

/**
 * Army Control > Travel > Map Travel (VibeSkua `MenuItem_MapTravel_Click`, docs/UI.md, "Native
 * shells"): who it sends, then Go to (map, room, cell, pad; Use current; Jump), the maps the
 * clients visited and the saved presets. A tap on a destination fills Go to; its Jump sends it.
 */
@Composable
fun ArmyTravelPage(state: ShellState, army: ArmyPanel, travel: ArmyTravelPanel) {
    FollowTravel(state, army, travel)
    val view = travel.view
    var deleting by remember { mutableStateOf<TravelDest?>(null) }
    var notice by remember { mutableStateOf<Notice?>(null) }
    Box(Modifier.fillMaxSize()) {
    Column(Modifier.fillMaxSize()) {
            PageBar("Map Travel", onBack = { state.back() })
            if (view == null || army.view == null) {
                val down = state.tabs.active?.down != null || state.tabs.active?.api == null
                Text(
                    if (down) "This tab is starting again. Map Travel is back with it." else "Reading the tab…",
                    color = Turu.Muted,
                    modifier = Modifier.padding(20.dp),
                )
                return@Column
            }
            val (targets, everyone) = targets(army)
            var goTo by remember { mutableStateOf(view.goTo) }
            val here = currentKey(state, army)?.let { key -> view.here.firstOrNull { it.key == key }?.here }
            val jump = { d: TravelDest -> army.send("jump", jumpBody(d)) }
            LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = 12.dp)) {
                item { ArmyTarget(targets, everyone) }
                item {
                    GoTo(
                        goTo = goTo,
                        pads = view.pads,
                        reach = targets.size,
                        here = here,
                        change = { goTo = it },
                        jump = { jump(goTo) },
                    )
                }
                item {
                    Heading("Recent destinations") {
                        TextButton(
                            onClick = {
                                val n = view.history.size
                                travel.act("clear-history") { e ->
                                    if (e == null) notice = Notice(if (n == 1) "Cleared 1 visited map" else "Cleared $n visited maps") { travel.act("undo-clear-history") }
                                }
                            },
                            enabled = view.history.isNotEmpty(),
                            modifier = Modifier.heightIn(min = 48.dp),
                        ) {
                            Text("Clear")
                        }
                    }
                }
                if (view.history.isEmpty()) {
                    item { Muted("The maps the clients travel to show here, the newest first.") }
                } else {
                    item { Muted("Tap one to fill Go to; Jump sends the clients there now.") }
                }
                items(view.history, key = { "h:${it.map}:${it.room}" }) { d ->
                    Destination(d.label, d.name, canJump = targets.isNotEmpty(), fill = { goTo = d }, jump = { jump(d) })
                }
                item {
                    Heading("Saved presets") {
                        // Go to's destination when one is typed, else where the client is (VibeSkua's editor).
                        val start = (if (goTo.map.isNotBlank()) goTo else here)?.copy(name = "")
                        TextButton(onClick = { state.push(Page.ArmyPreset(null, start)) }, modifier = Modifier.heightIn(min = 48.dp)) { Text("New…") }
                    }
                }
                if (view.presets.isEmpty()) {
                    item { Muted("Save a destination to jump to it again: New… starts from Go to, or where the client is.") }
                }
                items(view.presets, key = { "p:${it.name}" }) { d ->
                    Destination(d.label, place(d), canJump = targets.isNotEmpty(), fill = { goTo = d }, jump = { jump(d) }) { close ->
                        DropdownMenuItem(text = { Text("Edit…") }, onClick = { close(); state.push(Page.ArmyPreset(d)) })
                        DropdownMenuItem(text = { Text("Delete", color = Turu.Danger) }, onClick = { close(); deleting = d })
                    }
                }
            }
            val failed = travel.error ?: army.error
            if (failed != null) {
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                Text(failed, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            } else {
                army.view?.results?.lastOrNull()?.let {
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                    Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                }
            }
        }
        NoticeHost(notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { notice = null }
    }
    deleting?.let { d ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text("Delete ${d.name}?") },
            text = { Text("The preset goes; the clients stay where they are.", color = Turu.Muted) },
            confirmButton = { TextButton(onClick = { deleting = null; travel.act("delete-preset", JSONObject().put("name", d.name)) }) { Text("Delete", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** Go to: map and room, cell and pad, then Use current and Jump, and what a jump does. */
@Composable
private fun GoTo(goTo: TravelDest, pads: List<String>, reach: Int, here: TravelDest?, change: (TravelDest) -> Unit, jump: () -> Unit) {
    val valid = goTo.map.isNotBlank()
    val go = KeyboardActions(onGo = { if (valid && reach > 0) jump() })
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
        Heading("Go to", inset = false) {
            TextButton(onClick = { here?.let { change(it) } }, enabled = here != null, modifier = Modifier.heightIn(min = 48.dp)) { Text("Use current") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = goTo.map,
                onValueChange = { text -> change(if (text.contains(',')) parsePasted(text) ?: goTo.copy(map = text) else goTo.copy(map = text)) },
                label = { Text("Map") },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, autoCorrectEnabled = false, imeAction = ImeAction.Go),
                keyboardActions = go,
                modifier = Modifier.weight(1f),
            )
            OutlinedTextField(
                value = goTo.room,
                onValueChange = { text -> change(goTo.copy(room = text.filter { it.isDigit() })) },
                label = { Text("Room") },
                placeholder = { Text("Any") },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Go),
                keyboardActions = go,
                modifier = Modifier.width(112.dp),
            )
        }
        Spacer(Modifier.size(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = goTo.cell,
                onValueChange = { change(goTo.copy(cell = it)) },
                label = { Text("Cell") },
                placeholder = { Text("Enter") },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, autoCorrectEnabled = false, imeAction = ImeAction.Go),
                keyboardActions = go,
                modifier = Modifier.weight(1f),
            )
            Column(Modifier.weight(1f)) {
                Picker("Pad", goTo.pad.ifBlank { "Spawn" }, pads.map { it to it }, loading = false) { change(goTo.copy(pad = it)) }
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            val note = when {
                reach == 0 -> "No client to send to"
                !valid -> "Type a map; the room, cell and pad are optional"
                else -> "Sends ${if (reach == 1) "1 client" else "$reach clients"} to ${place(goTo)}"
            }
            Text(note, style = MaterialTheme.typography.bodySmall, color = if (reach == 0) Turu.Warning else Turu.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f).padding(end = 8.dp))
            Button(onClick = jump, enabled = valid && reach > 0, modifier = Modifier.heightIn(min = 48.dp)) { Text("Jump") }
        }
    }
}

@Composable
private fun Heading(title: String, inset: Boolean = true, action: @Composable () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(start = if (inset) 16.dp else 4.dp, end = if (inset) 4.dp else 0.dp, top = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.titleSmall, color = Turu.Muted, modifier = Modifier.weight(1f))
        action()
    }
}

@Composable
private fun Muted(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp))
}

/** A visited map or a preset: a tap fills Go to, Jump sends it; a preset's ⋮ edits or deletes it. */
@Composable
private fun Destination(
    title: String,
    under: String,
    canJump: Boolean,
    fill: () -> Unit,
    jump: () -> Unit,
    menu: (@Composable (close: () -> Unit) -> Unit)? = null,
) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = fill).heightIn(min = 56.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (under.isNotBlank()) Text(under, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        // A text button: the rows' second action, quieter than Go to's filled Jump.
        TextButton(onClick = jump, enabled = canJump, modifier = Modifier.heightIn(min = 48.dp)) { Text("Jump") }
        if (menu != null) {
            var open by remember { mutableStateOf(false) }
            Box {
                IconButton(onClick = { open = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "$title: edit or delete", tint = Turu.Muted) }
                DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) { menu { open = false } }
            }
        } else {
            Spacer(Modifier.width(8.dp))
        }
    }
}

/**
 * Map Travel's preset editor (VibeSkua `OpenJumpMapPresetEditor`): a name and the destination; a
 * new one starts from Go to's destination, else where the client is, else the last jump. Another preset with the name
 * is replaced.
 */
@Composable
fun ArmyPresetPage(state: ShellState, army: ArmyPanel, travel: ArmyTravelPanel, editing: TravelDest?, start: TravelDest?) {
    FollowTravel(state, army, travel)
    val view = travel.view
    var preset by remember { mutableStateOf(editing ?: start) }
    var error by remember { mutableStateOf<String?>(null) }
    var saving by remember { mutableStateOf(false) }
    LaunchedEffect(view != null) {
        if (preset == null && view != null) preset = view.goTo.copy(name = "")
    }
    val here = view?.let { v -> currentKey(state, army)?.let { key -> v.here.firstOrNull { it.key == key }?.here } }
    Column(Modifier.fillMaxSize()) {
        PageBar(if (editing == null) "New preset" else "Edit preset", onBack = { state.back() }) {
            TextButton(
                onClick = {
                    val p = preset ?: return@TextButton
                    saving = true
                    val body = jumpBody(p).put("name", p.name).put("editing", editing?.name.orEmpty())
                    travel.act("save-preset", body) { e ->
                        saving = false
                        if (e == null) state.back() else error = e
                    }
                },
                enabled = preset != null && !saving,
                modifier = Modifier.heightIn(min = 48.dp),
            ) { Text("Save") }
        }
        val p = preset ?: run {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val field = @Composable { label: String, value: String, number: Boolean, set: (String) -> Unit ->
                OutlinedTextField(
                    value = value,
                    onValueChange = { set(if (number) it.filter { c -> c.isDigit() } else it); error = null },
                    label = { Text(label) },
                    singleLine = true,
                    shape = MaterialTheme.shapes.medium,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.Number else KeyboardType.Ascii, autoCorrectEnabled = false, imeAction = ImeAction.Next),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            field("Preset name", p.name, false) { preset = p.copy(name = it) }
            field("Map", p.map, false) { preset = p.copy(map = it) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) { field("Room (optional)", p.room, true) { preset = p.copy(room = it) } }
                Column(Modifier.weight(1f)) { field("Cell", p.cell, false) { preset = p.copy(cell = it) } }
            }
            Picker("Pad", p.pad.ifBlank { "Spawn" }, view?.pads.orEmpty().map { it to it }, loading = false) { preset = p.copy(pad = it) }
            OutlinedButton(
                onClick = {
                    val h = here ?: run { error = "The selected client hasn't said where it is yet. Log in and wait a moment."; return@OutlinedButton }
                    preset = h.copy(name = p.name)
                    error = null
                },
                modifier = Modifier.heightIn(min = 48.dp),
            ) { Text("Use the client's location") }
            error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger) }
            Spacer(Modifier.size(8.dp))
        }
    }
}

/** Which Travel picker Army Control shows. */
enum class TravelPick { Player, Server }

/**
 * "Jump to player…" (VibeSkua `MenuItem_JumpPlayer_Click`): the clients' logged-in players to
 * pick from, or any name typed in.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PlayerDialog(state: ShellState, army: ArmyPanel, travel: ArmyTravelPanel, close: () -> Unit) {
    FollowTravel(state, army, travel)
    val view = travel.view
    var name by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(view?.player) { if (name == null && view != null) name = view.player }
    val text = name.orEmpty()
    AlertDialog(
        onDismissRequest = close,
        title = { Text("Jump to a player") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                if (view != null && view.players.isEmpty()) {
                    Text("No client is logged in to pick from: type a name.", color = Turu.Muted)
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    view?.players?.forEach { p ->
                        FilterChip(
                            selected = p.equals(text, ignoreCase = true),
                            onClick = { name = p },
                            label = { Text(p, maxLines = 1) },
                            colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
                        )
                    }
                }
                OutlinedTextField(
                    value = text,
                    onValueChange = { name = it },
                    label = { Text("Player name") },
                    singleLine = true,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { if (text.isNotBlank()) { army.send("jump-player", JSONObject().put("name", text.trim())); close() } }),
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { army.send("jump-player", JSONObject().put("name", text.trim())); close() }, enabled = text.isNotBlank()) { Text("Jump") }
        },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}

/** "Change server…" (VibeSkua `MenuItem_ChangeServerAll_Click`): the online servers; the clients relog there. */
@Composable
fun ServerDialog(state: ShellState, army: ArmyPanel, travel: ArmyTravelPanel, close: () -> Unit) {
    FollowTravel(state, army, travel)
    val view = travel.view
    var picked by remember { mutableStateOf("") }
    // Ask for the list once the tab answers; forget it when the picker goes.
    LaunchedEffect(view != null) { if (view != null) travel.act("servers") }
    DisposableEffect(Unit) { onDispose { travel.act("forget-servers") } }
    val servers = view?.servers
    AlertDialog(
        onDismissRequest = close,
        title = { Text("Change server") },
        text = {
            Column(Modifier.heightIn(max = 280.dp).verticalScroll(rememberScrollState())) {
                when {
                    servers == null || servers.loading || (servers.names.isEmpty() && servers.error.isEmpty()) -> Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = Turu.Accent)
                        Spacer(Modifier.width(10.dp))
                        Text("Loading servers…", color = Turu.Muted)
                    }
                    servers.error.isNotEmpty() -> Text(servers.error, color = Turu.Danger)
                    else -> {
                        Text("The clients relog on the server picked.", color = Turu.Muted, modifier = Modifier.padding(bottom = 4.dp))
                        servers.names.forEach { s ->
                            Row(
                                Modifier.fillMaxWidth().heightIn(min = 48.dp).selectable(selected = s == picked, role = Role.RadioButton) { picked = s },
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                RadioButton(selected = s == picked, onClick = null)
                                Spacer(Modifier.width(12.dp))
                                Text(s)
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { army.send("change-server", JSONObject().put("server", picked)); close() }, enabled = picked.isNotEmpty()) { Text("Change server") }
        },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}
