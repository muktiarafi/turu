package com.muktiarafi.turu

import android.util.Log
import androidx.compose.animation.core.animate
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.CoreBotsView
import com.muktiarafi.turu.models.RoleView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * CoreBots Options for the tab on screen: the draft is the slot's (`models::core_bots`), opened
 * with the page and saved when it closes, as the egui window does; this reads its view and sends
 * what changes. The page and its role pages share one draft: going between them doesn't close it.
 */
class CoreBotsPanel {
    var view by mutableStateOf<CoreBotsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** Said once and gone: the character the page left was saved (tabs switched under it). */
    var notice by mutableStateOf<Notice?>(null)

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        // The tab before: its draft saved, as closing the window does; said when it had changes
        // and the page stays (another tab came on screen), since nothing else shows it.
        val left = view?.takeIf { it.unsaved && it.username.isNotEmpty() }?.shownName
        client?.let { old -> old.act("core-bots", "do", JSONObject().put("action", "close")) { old.stop() } }
        if (left != null && tab != null) notice = Notice("Saved CoreBots for $left")
        following = tab
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("core-bots"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<CoreBotsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/core-bots doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("core-bots", "do", JSONObject().put("action", "open"))
        }
    }

    fun act(action: String, body: JSONObject = JSONObject()) {
        error = null
        client?.act("core-bots", "do", body.put("action", action)) { error = it }
    }

    fun role(index: Int, field: String, value: Any) = act("role", JSONObject().put("index", index).put(field, value))

    /** [into] takes [from]'s class, mode, equipment switch and gear (one action, then a slot each). */
    fun copyRole(into: Int, from: RoleView) {
        act(
            "role",
            JSONObject().put("index", into).put("class", from.`class`).put("mode", from.mode).put("equipment", from.useEquipment),
        )
        from.gear.forEachIndexed { slot, item -> act("role", JSONObject().put("index", into).put("slot", slot).put("item", item)) }
    }
}

private enum class CboTab(val label: String) { Loadout("Loadout"), Options("Options"), Other("Other") }

/** The page and its role pages: they keep the draft open between them, and close it otherwise. */
@Composable
private fun FollowWhileOnCoreBots(state: ShellState, panel: CoreBotsPanel) {
    DisposableEffect(Unit) {
        onDispose { if (state.page != Page.CoreBots && state.page !is Page.CoreBotsRole) panel.follow(null) }
    }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
}

/** The bar's line under the title: whose file, and what saving will do with it. */
private fun subtitle(view: CoreBotsView): Pair<String, Boolean> {
    val who = view.shownName.ifEmpty { "No character" }
    return when {
        view.busy -> "$who · Reading the game…" to false
        view.username.isEmpty() -> "Not logged in" to true
        view.unsaved && view.askPublic -> "$who · Saves when you close · Private Rooms stays on" to true
        view.unsaved -> "$who · Saves when you close" to true
        else -> "$who · ${view.state}" to false
    }
}

/**
 * The bar's actions, the same on the page and a role page: Save now (secondary: the draft saves
 * when the page closes, as VibeSkua's), and ⋮ with Read classes again, Revert changes (asked
 * first) and the full window, plus what [more] adds.
 */
@Composable
private fun CoreBotsActions(
    view: CoreBotsView,
    panel: CoreBotsPanel,
    fullWindow: () -> Unit,
    onSave: () -> Unit,
    onRevert: () -> Unit,
    more: @Composable (close: () -> Unit) -> Unit = {},
) {
    val editable = view.username.isNotEmpty()
    TextButton(onClick = onSave, enabled = editable && !view.busy, modifier = Modifier.heightIn(min = 48.dp)) { Text("Save now") }
    FullWindowMenu("The same options, laid out as VibeSkua's", fullWindow) { close ->
        more(close)
        DropdownMenuItem(
            text = {
                Column {
                    Text("Read classes again")
                    Text("For a class or item got since; your picks stay", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
            },
            enabled = editable && !view.busy,
            onClick = { close(); panel.act("refresh") },
        )
        DropdownMenuItem(text = { Text("Revert changes") }, enabled = view.unsaved, onClick = { close(); onRevert() })
    }
}

/** What saving asks (Public Rooms), the revert check and the model's messages: the page's and a role page's. */
@Composable
private fun CoreBotsDialogs(view: CoreBotsView?, panel: CoreBotsPanel, askPublic: Boolean, onAskPublic: (Boolean) -> Unit, revert: Boolean, onRevert: (Boolean) -> Unit) {
    if (askPublic) {
        // Keep private saves with Private Rooms kept on; a tap outside doesn't save.
        PublicRoomsDialog(
            onConfirm = { onAskPublic(false); panel.act("confirm-public"); panel.act("save") },
            onKeep = { onAskPublic(false); panel.act("save") },
            onDismiss = { onAskPublic(false) },
        )
    }
    if (revert) {
        AlertDialog(
            onDismissRequest = { onRevert(false) },
            title = { Text("Revert your changes?") },
            text = { Text("The page goes back to the file as last saved. What you changed since is dropped.") },
            confirmButton = { TextButton(onClick = { onRevert(false); panel.act("revert") }) { Text("Revert") } },
            dismissButton = { TextButton(onClick = { onRevert(false) }) { Text("Keep editing") } },
            containerColor = Turu.Card,
        )
    }
    view?.message?.let { (title, text) ->
        AlertDialog(
            onDismissRequest = { panel.act("dismiss") },
            title = { Text(title) },
            text = { Text(text) },
            confirmButton = { TextButton(onClick = { panel.act("dismiss") }) { Text("OK") } },
            containerColor = Turu.Card,
        )
    }
}

/** Logged out: why nothing here can change yet, and what to do. */
@Composable
private fun LoggedOutNotice() {
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 4.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(Turu.Card)
            .padding(12.dp),
    ) {
        Text("Not logged in", style = MaterialTheme.typography.titleSmall, color = Turu.Warning)
        Text(
            "CoreBots' settings are your character's. Log in on this tab and they load here.",
            style = MaterialTheme.typography.bodySmall,
            color = Turu.Muted,
        )
    }
}

/**
 * Options > CoreBots (docs/UI.md, "Native shells"): the four roles as rows (each opens its own
 * page), Options and Other with search. The bar keeps whose file it is and what saving
 * does; the segments step out of the way as the list scrolls and come back on a scroll up, so the
 * short landscape panel is mostly the list.
 */
@Composable
fun CoreBotsPage(state: ShellState, panel: CoreBotsPanel, fullWindow: () -> Unit) {
    FollowWhileOnCoreBots(state, panel)
    val view = panel.view
    var tab by rememberSaveable { mutableStateOf(CboTab.Loadout) }
    var search by rememberSaveable { mutableStateOf("") }
    var askPublic by remember { mutableStateOf(false) }
    var revert by remember { mutableStateOf(false) }
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            val line = view?.let { subtitle(it) }
            PageBar(
                "CoreBots Options",
                onBack = { state.back() },
                subtitle = line?.first,
                subtitleColor = if (line?.second == true) Turu.Warning else Turu.Muted,
            ) {
                if (view != null) {
                    CoreBotsActions(view, panel, fullWindow, onSave = { if (view.askPublic) askPublic = true else panel.act("save") }, onRevert = { revert = true })
                } else {
                    FullWindowMenu("The same options, laid out as VibeSkua's", fullWindow)
                }
            }
            if (view == null) {
                Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
                return@Column
            }
            val editable = view.username.isNotEmpty()
            val keyboard = LocalSoftwareKeyboardController.current
            val focus = LocalFocusManager.current
            CollapsingSegments(Modifier.weight(1f), segments = { mod ->
                ConnectedChoice(CboTab.entries, tab, { tab = it }, mod.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp)) {
                    Text(it.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }) { top ->
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = top, bottom = 12.dp)) {
                    panel.error?.let { e -> item(key = "error") { Text(e, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) } }
                    if (!editable && !view.busy) item(key = "logged out") { LoggedOutNotice() }
                    when (tab) {
                        CboTab.Loadout -> {
                            view.roles.forEachIndexed { i, role ->
                                item(key = "role " + role.name) { RoleRow(role) { state.push(Page.CoreBotsRole(i)) } }
                            }
                        }
                        CboTab.Options, CboTab.Other -> {
                            item(key = "search") {
                                OutlinedTextField(
                                    value = search,
                                    onValueChange = { search = it },
                                    placeholder = { Text("Search CoreBots options") },
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
                                    // Landscape leaves no room for results above the keyboard:
                                    // Search puts it away so the matches show.
                                    keyboardActions = KeyboardActions(onSearch = { keyboard?.hide(); focus.clearFocus() }),
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
                                )
                            }
                            val needle = search.trim().lowercase()
                            val groups = view.sections.filter { s -> (tab == CboTab.Other) == (s.chip in view.other) }
                            var any = false
                            for (s in groups) {
                                val rows = view.options.filter { o ->
                                    o.section == s.chip && (needle.isEmpty() || o.label.lowercase().contains(needle) || o.description.lowercase().contains(needle))
                                }
                                if (rows.isEmpty()) continue
                                any = true
                                item(key = "title " + s.chip) {
                                    Text(
                                        s.title,
                                        style = MaterialTheme.typography.titleSmall,
                                        color = Turu.Muted,
                                        modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 2.dp).semantics { heading() },
                                    )
                                }
                                for (o in rows) {
                                    item(key = o.group) {
                                        OptionRow(
                                            o.copy(differs = false),
                                            edited = false,
                                            inGame = true,
                                            onSet = { panel.act("option", JSONObject().put("tag", o.group).put("value", it)) },
                                            onAction = {},
                                            // Logged out: off for now, not "Coming soon" (that's for options Turu doesn't read).
                                            enabled = editable,
                                        )
                                    }
                                }
                            }
                            if (!any) {
                                item(key = "none") {
                                    Column(Modifier.padding(20.dp)) {
                                        Text("No options match “${search.trim()}”", style = MaterialTheme.typography.titleSmall)
                                        TextButton(onClick = { search = "" }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Show them all") }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        NoticeHost(panel.notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { panel.notice = null }
    }
    CoreBotsDialogs(view, panel, askPublic, { askPublic = it }, revert, { revert = it })
}

/**
 * [segments] over [content]: as the list scrolls down they slide up and out with it, and the
 * first scroll up brings them back (Material's enter-always app bar, for the page's own tabs).
 * [content] gets the room to leave at its top.
 */
@Composable
private fun CollapsingSegments(
    modifier: Modifier,
    segments: @Composable (Modifier) -> Unit,
    content: @Composable (top: Dp) -> Unit,
) {
    val density = LocalDensity.current
    val snap = MaterialTheme.motionScheme.fastSpatialSpec<Float>()
    var height by remember { mutableFloatStateOf(0f) }
    var offset by remember { mutableFloatStateOf(0f) }
    val scroll = remember(snap) {
        object : NestedScrollConnection {
            override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                offset = (offset + available.y).coerceIn(-height, 0f)
                return Offset.Zero
            }

            // When the list settles they snap all the way in or out, never left half under the bar.
            override suspend fun onPostFling(consumed: Velocity, available: Velocity): Velocity {
                val target = if (offset < -height / 2) -height else 0f
                if (offset != target) animate(offset, target, animationSpec = snap) { value, _ -> offset = value }
                return Velocity.Zero
            }
        }
    }
    Box(modifier.nestedScroll(scroll).clipToBounds()) {
        content(with(density) { height.toDp() })
        Box(
            Modifier
                .fillMaxWidth()
                .onSizeChanged { height = it.height.toFloat() }
                .graphicsLayer { translationY = offset }
                .background(Turu.Panel),
        ) { segments(Modifier) }
    }
}

/** A role at a glance: its dot, name and purpose; what it fights with; how much gear is set. */
@Composable
private fun RoleRow(role: RoleView, open: () -> Unit) {
    val set = role.gear.count { it.isNotEmpty() }
    val with = if (role.`class`.isEmpty()) "No class set" else "${role.`class`} · ${role.mode.ifEmpty { "Base" }}"
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(MaterialTheme.shapes.medium)
            .clickable(onClickLabel = "Edit ${role.name}", onClick = open)
            .heightIn(min = 64.dp)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val c = role.color.toLong()
        Box(Modifier.size(10.dp).clip(CircleShape).background(Color(0xFF000000 or c))) // impeccable-disable-line raw-color: the role's own colour, from the model
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text(role.name, style = MaterialTheme.typography.titleSmall)
                Spacer(Modifier.width(8.dp))
                Text(role.purpose, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Text(
                with,
                style = MaterialTheme.typography.bodyMedium,
                color = if (role.`class`.isEmpty()) Turu.Muted else Turu.Text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Text(
            if (role.useEquipment) "Gear $set/${role.gear.size}" else "Gear off",
            style = MaterialTheme.typography.labelMedium.copy(fontFeatureSettings = "tnum"),
            color = Turu.Muted,
        )
    }
}

/**
 * One role on its own page: class and mode (mode once a class is picked), Use equipment, the gear
 * two to a row in the wide panel, and ⋮ > Copy from another role.
 */
@Composable
fun CoreBotsRolePage(state: ShellState, panel: CoreBotsPanel, index: Int, fullWindow: () -> Unit) {
    FollowWhileOnCoreBots(state, panel)
    val view = panel.view
    var askPublic by remember { mutableStateOf(false) }
    var revert by remember { mutableStateOf(false) }
    val role = view?.roles?.getOrNull(index)
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            val line = view?.let { subtitle(it) }
            PageBar(
                role?.name ?: "Role",
                onBack = { state.back() },
                subtitle = line?.first,
                subtitleColor = if (line?.second == true) Turu.Warning else Turu.Muted,
            ) {
                if (view != null && role != null) {
                    CoreBotsActions(view, panel, fullWindow, onSave = { if (view.askPublic) askPublic = true else panel.act("save") }, onRevert = { revert = true }) { close ->
                        view.roles.forEachIndexed { i, other ->
                            if (i != index) {
                                DropdownMenuItem(
                                    text = { Text("Copy from ${other.name}") },
                                    enabled = view.username.isNotEmpty() && other.`class`.isNotEmpty(),
                                    onClick = { close(); panel.copyRole(index, other) },
                                )
                            }
                        }
                    }
                }
            }
            if (view == null || role == null) {
                Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
                return@Column
            }
            val editable = view.username.isNotEmpty()
            Column(
                Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 12.dp).padding(bottom = 12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                panel.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger) }
                if (!editable && !view.busy) LoggedOutNotice()
                Text(role.purpose, style = MaterialTheme.typography.bodyMedium, color = Turu.Muted)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Box(Modifier.weight(1.4f)) {
                        Picker(
                            label = "Class",
                            value = role.`class`,
                            options = view.classes.map { it to it },
                            loading = view.busy,
                            none = "Not set",
                            empty = "No enhanced classes yet (⋮ > Read classes again)",
                            enabled = editable,
                        ) { panel.role(index, "class", it) }
                    }
                    Box(Modifier.weight(1f)) {
                        Picker(
                            label = "Mode",
                            value = role.mode,
                            options = role.modes.ifEmpty { listOf("Base") }.map { it to it },
                            loading = false,
                            none = if (role.`class`.isEmpty()) "Pick a class" else "Base",
                            // A mode means nothing until there's a class to use it with.
                            enabled = editable && role.`class`.isNotEmpty(),
                        ) { panel.role(index, "mode", it) }
                    }
                }
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(
                        value = role.useEquipment,
                        enabled = editable,
                        role = Role.Switch,
                        onValueChange = { panel.role(index, "equipment", it) },
                    ),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text("Use equipment", style = MaterialTheme.typography.bodyLarge)
                        Text("Equips the gear below when this role is used", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                    }
                    Switch(checked = role.useEquipment, enabled = editable, onCheckedChange = null)
                }
                val set = role.gear.count { it.isNotEmpty() }
                Text(
                    "Gear · $set of ${role.gear.size} set",
                    style = MaterialTheme.typography.titleSmall,
                    color = Turu.Muted,
                    modifier = Modifier.padding(top = 8.dp).semantics { heading() },
                )
                // Gear can be picked either way, as in VibeSkua; the switch decides whether it's worn.
                if (!role.useEquipment) {
                    Text("Worn only while Use equipment is on", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
                view.slots.chunked(2).forEachIndexed { row, pair ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        pair.forEachIndexed { col, s ->
                            val slot = row * 2 + col
                            Box(Modifier.weight(1f)) {
                                Picker(
                                    label = s.name,
                                    value = role.gear.getOrElse(slot) { "" },
                                    options = listOf("" to "None") + s.items.map { it to it },
                                    loading = false,
                                    enabled = editable,
                                ) { panel.act("role", JSONObject().put("index", index).put("slot", slot).put("item", it)) }
                            }
                        }
                        if (pair.size == 1) Spacer(Modifier.weight(1f))
                    }
                }
            }
        }
        NoticeHost(panel.notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { panel.notice = null }
    }
    CoreBotsDialogs(view, panel, askPublic, { askPublic = it }, revert, { revert = it })
}
