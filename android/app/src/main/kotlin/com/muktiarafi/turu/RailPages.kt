package com.muktiarafi.turu

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Control
import org.json.JSONObject

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
 * Auto (VibeSkua's Auto view): Auto Attack and Auto Hunt, Stop, the class and mode to fight with
 * and the maps to hunt in (`turu_frontend::models::auto`).
 */
@Composable
fun AutoPage(state: ShellState, models: TabModels) {
    val view = models.auto
    var error by remember { mutableStateOf<String?>(null) }
    val act = { action: String, body: JSONObject -> models.act("auto", action, body) { error = it } }
    Column(Modifier.fillMaxSize()) {
        PageBar("Combat Automation", onBack = { state.back() }) {
            if (view?.running == true) {
                Text(
                    "Running",
                    style = MaterialTheme.typography.labelMedium,
                    color = Turu.Success,
                    modifier = Modifier
                        .padding(end = 16.dp)
                        .clip(RoundedCornerShape(8.dp))
                        .background(Turu.Success.copy(alpha = 0.14f))
                        .padding(horizontal = 8.dp, vertical = 2.dp),
                )
            }
        }
        if (view == null) return@Column Waiting(models)
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { act("attack", JSONObject()) },
                    enabled = view.start.enabled,
                    modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                ) { Text("Auto Attack", maxLines = 1) }
                FilledTonalButton(
                    onClick = { act("hunt", JSONObject()) },
                    enabled = view.start.enabled,
                    modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                ) { Text("Auto Hunt", maxLines = 1) }
            }
            Reason(view.start.takeIf { !view.running })
            Button(
                onClick = { act("stop", JSONObject()) },
                enabled = view.stop.enabled,
                colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Color.White),
                modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
            ) { Text("Stop Automation") }
            Picker(
                label = "Class",
                none = "Select a class",
                value = view.`class` ?: "",
                options = view.classes.map { it to it },
                loading = view.loading,
                // VibeSkua `ReloadClasses`: the owned classes again whenever the list opens.
                onOpen = { act("reload-classes", JSONObject()) },
                empty = if (view.loading) "Loading classes…" else "No classes (not logged in?)",
            ) { act("class", JSONObject().put("class", it)) }
            Picker(
                label = "Mode",
                none = "Select a mode",
                value = view.mode ?: "",
                options = view.modes.map { it to it },
                loading = false,
                empty = if (view.`class` == null) "Pick a class first" else "No saved skills for this class",
            ) { act("mode", JSONObject().put("mode", it)) }
            var mapIds by remember(models.tab?.id) { mutableStateOf(view.mapIds) }
            OutlinedTextField(
                value = mapIds,
                onValueChange = {
                    mapIds = it
                    act("map-ids", JSONObject().put("mapIds", it))
                },
                label = { Text("Manual MapIDs") },
                placeholder = { Text("Optional, e.g. 1, 2") },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth(),
            )
            error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodySmall) }
            Spacer(Modifier.heightIn(min = 12.dp))
        }
    }
}

/** Jump (VibeSkua's Fast Map Jump): a cell and a pad, Current and Jump (`models::jump`). */
@Composable
fun JumpPage(state: ShellState, models: TabModels) {
    val view = models.jump
    LaunchedEffect(models.tab?.id, models.tab?.api) { models.act("jump", "refresh") }
    val act = { action: String, body: JSONObject -> models.act("jump", action, body) }
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
                onOpen = { act("refresh", JSONObject()) },
                empty = "No cells (not in a map?)",
            ) { act("cell", JSONObject().put("cell", it)) }
            Picker(
                label = "Pad",
                none = "Select a pad",
                value = view.pad,
                options = view.pads.map { it to it },
                loading = false,
            ) { act("pad", JSONObject().put("pad", it)) }
            if (view.autoJump) {
                Text("Picking a cell or pad jumps at once", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { act("current", JSONObject()) }, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                    Text("Current")
                }
                Button(
                    onClick = { act("jump", JSONObject()) },
                    enabled = view.jump.enabled,
                    modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                ) { Text("Jump") }
            }
            Reason(view.jump)
            view.status?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Turu.Muted) }
        }
    }
}

/** Logs: the script's and the client's, line-fed (`models::logs`). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LogsPage(state: ShellState, models: TabModels) {
    var kind by rememberSaveable { mutableStateOf("script") }
    val lines = if (kind == "script") models.scriptLog else models.clientLog
    val list = rememberLazyListState()
    LaunchedEffect(kind, lines.size) { if (lines.isNotEmpty()) list.scrollToItem(lines.size - 1) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Logs", onBack = { state.back() }) {
            TextButton(onClick = { models.clearLog(kind) }, enabled = lines.isNotEmpty()) { Text("Clear") }
        }
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
            listOf("script" to "Script", "debug" to "Client").forEachIndexed { i, (key, label) ->
                SegmentedButton(
                    selected = kind == key,
                    onClick = { kind = key },
                    shape = SegmentedButtonDefaults.itemShape(i, 2),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = Turu.AccentTint,
                        activeContentColor = Turu.Text,
                        inactiveContainerColor = Color.Transparent,
                        inactiveContentColor = Turu.Muted,
                        activeBorderColor = Turu.Edge,
                        inactiveBorderColor = Turu.Edge,
                    ),
                ) { Text(label) }
            }
        }
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

/**
 * More: every menu's windows (VibeSkua's menus, what this platform has: `models::rail`), the
 * native ones opening their page, the rest the slot's egui window; then Hide Side Bar.
 */
@Composable
fun MorePage(state: ShellState, models: TabModels, window: (String) -> Unit, hideRail: () -> Unit) {
    val menus = models.rail?.menus
    Column(Modifier.fillMaxSize()) {
        PageBar("More", onBack = { state.back() })
        if (menus == null) return@Column Waiting(models)
        LazyColumn(Modifier.fillMaxSize()) {
            for (menu in menus) {
                item(key = menu.title) {
                    Text(
                        menu.title,
                        style = MaterialTheme.typography.labelMedium,
                        color = Turu.Muted,
                        modifier = Modifier.padding(start = 20.dp, top = 12.dp, bottom = 4.dp),
                    )
                }
                itemsIndexed(menu.items, key = { _, item -> menu.title + item.window }) { _, item ->
                    val native = when (item.window) {
                        "script_loader" -> Page.Scripts
                        "accounts" -> Page.Accounts
                        "new_tab" -> Page.NewTab
                        "script_repo" -> Page.Search
                        "game_options" -> Page.GameOptions
                        "application_options" -> Page.AppOptions
                        else -> null
                    }
                    ListItem(
                        modifier = Modifier
                            .padding(horizontal = 8.dp)
                            .clip(RoundedCornerShape(12.dp))
                            .heightIn(min = 48.dp)
                            .clickable(enabled = item.control.enabled) {
                                if (native != null) state.push(native) else window(item.window)
                            },
                        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                        headlineContent = {
                            Text(item.label, color = if (item.control.enabled) Turu.Text else Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        },
                        supportingContent = item.control.takeIf { !it.enabled && it.reason.isNotEmpty() }?.let { c -> { Text(c.reason, color = Turu.Muted) } },
                    )
                }
            }
            item {
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(top = 8.dp))
                ListItem(
                    modifier = Modifier.padding(horizontal = 8.dp).clip(RoundedCornerShape(12.dp)).clickable(onClick = hideRail),
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    headlineContent = { Text("Hide Side Bar") },
                    supportingContent = { Text("The game takes the whole width; the tab at the left edge brings it back", color = Turu.Muted) },
                )
                Box(Modifier.heightIn(min = 12.dp))
            }
        }
    }
}
