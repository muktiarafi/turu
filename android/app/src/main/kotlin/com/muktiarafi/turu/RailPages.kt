package com.muktiarafi.turu

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
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
import androidx.compose.material3.VerticalDivider
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Control
import com.muktiarafi.turu.models.MenuItem
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
                colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = MaterialTheme.colorScheme.onError),
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

/**
 * Logs: the script's and the client's, line-fed (`models::logs`); Find and Copy in the full
 * window.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LogsPage(state: ShellState, models: TabModels, full: () -> Unit) {
    var kind by rememberSaveable { mutableStateOf("script") }
    val lines = if (kind == "script") models.scriptLog else models.clientLog
    val list = rememberLazyListState()
    LaunchedEffect(kind, lines.size) { if (lines.isNotEmpty()) list.scrollToItem(lines.size - 1) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Logs", onBack = { state.back() }) {
            TextButton(onClick = { models.clearLog(kind) }, enabled = lines.isNotEmpty()) { Text("Clear") }
            FullWindowMenu(adds = "Find and Copy", open = full)
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
    "logs" -> Page.Logs
    else -> null
}

/**
 * More: VibeSkua's menus as this platform has them (`models::rail`), list and detail so none is
 * more than a tap away: the menus down the left, the one picked on the right (kept while the
 * panel is closed). A native window opens its page (a chevron says so), the rest the slot's egui
 * window. The bar holds what the rail folded and no menu has (Bank, Update) and Hide Side Bar.
 */
@Composable
fun MorePage(state: ShellState, models: TabModels, window: (String) -> Unit, bank: () -> Unit, hideRail: () -> Unit) {
    val rail = models.rail
    val menus = rail?.menus
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
                TextButton(onClick = { window("client_update") }) {
                    BadgedBox(badge = { Badge(containerColor = if (update.failed != null) Turu.Danger else Turu.Accent) }) {
                        Icon(RailIcons.Update, contentDescription = null, modifier = Modifier.size(18.dp))
                    }
                    Spacer(Modifier.width(6.dp))
                    Text("Update")
                }
            }
            TextButton(onClick = hideRail) { Text("Hide Side Bar") }
        }
        if (menus == null) return@Column Waiting(models)
        val shown = menus.firstOrNull { it.title == state.moreMenu } ?: menus.firstOrNull() ?: return@Column
        Row(Modifier.fillMaxSize()) {
            Column(
                Modifier
                    .width(152.dp)
                    .fillMaxHeight()
                    .verticalScroll(rememberScrollState())
                    .padding(start = 8.dp, end = 8.dp, bottom = 8.dp)
                    .selectableGroup(),
            ) {
                for (menu in menus) {
                    val selected = menu.title == shown.title
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 48.dp)
                            .clip(RoundedCornerShape(12.dp))
                            .background(if (selected) Turu.AccentTint else Color.Transparent)
                            .selectable(selected = selected, role = Role.Tab) { state.moreMenu = menu.title }
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        Text(
                            menu.title,
                            style = MaterialTheme.typography.labelLarge,
                            color = if (selected) Turu.Text else Turu.Muted,
                        )
                    }
                }
            }
            VerticalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            AnimatedContent(
                targetState = shown.title,
                transitionSpec = { fadeIn(tween(150, delayMillis = 60)).togetherWith(fadeOut(tween(90))) },
                label = "more menu",
            ) { title ->
                val menu = menus.firstOrNull { it.title == title } ?: return@AnimatedContent
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 0.dp)) {
                    items(menu.items, key = { it.window }) { item ->
                        MenuRow(item, nativePage(item.window)) { page ->
                            if (page != null) state.push(page) else window(item.window)
                        }
                    }
                }
            }
        }
    }
}

/** A row of More: the window's name, why it's off under it, a chevron if it opens a page here. */
@Composable
private fun MenuRow(item: MenuItem, page: Page?, open: (Page?) -> Unit) {
    val enabled = item.control.enabled
    ListItem(
        modifier = Modifier
            .clip(RoundedCornerShape(12.dp))
            .clickable(enabled = enabled) { open(page) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        headlineContent = {
            Text(item.label, color = if (enabled) Turu.Text else Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        supportingContent = item.control.takeIf { !it.enabled && it.reason.isNotEmpty() }?.let { c ->
            { Text(c.reason, color = Turu.Muted) }
        },
        trailingContent = if (page != null && enabled) {
            { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Muted) }
        } else {
            null
        },
    )
}
