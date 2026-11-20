package com.muktiarafi.turu

import android.util.Log
import androidx.compose.animation.AnimatedVisibility
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.CoreBotsView
import com.muktiarafi.turu.models.RoleView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * CoreBots Options for the tab on screen: the draft is the slot's (`models::core_bots`), opened
 * with the page and saved when it closes, as the egui window does; this reads its view and sends
 * what changes.
 */
class CoreBotsPanel {
    var view by mutableStateOf<CoreBotsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        // The tab before: its draft saved, as closing the window does.
        client?.let { old -> old.act("core-bots", "do", JSONObject().put("action", "close")) { old.stop() } }
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
}

private enum class CboTab(val label: String) { Loadout("Loadout"), Options("Options"), Other("Other") }

/** Options > CoreBots (docs/UI.md, "Native shells"): Loadout, Options and Other, then Save. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoreBotsPage(state: ShellState, panel: CoreBotsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var tab by rememberSaveable { mutableStateOf(CboTab.Loadout) }
    var askPublic by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        PageBar("CoreBots Options", onBack = { state.back() }) {
            FullWindowMenu("The same options, laid out as VibeSkua's", fullWindow)
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        Column(Modifier.padding(horizontal = 16.dp)) {
            Text(view.username.ifEmpty { "No character" }, style = MaterialTheme.typography.titleSmall)
            Text(view.state, style = MaterialTheme.typography.bodySmall, color = if (view.unsaved) Turu.Warning else Turu.Muted)
        }
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp)) {
            CboTab.entries.forEachIndexed { i, t ->
                SegmentedButton(
                    selected = tab == t,
                    onClick = { tab = t },
                    shape = SegmentedButtonDefaults.itemShape(i, CboTab.entries.size),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = Turu.AccentTint,
                        activeContentColor = Turu.Text,
                        inactiveContainerColor = Color.Transparent,
                        inactiveContentColor = Turu.Muted,
                        activeBorderColor = Turu.Edge,
                        inactiveBorderColor = Turu.Edge,
                    ),
                ) { Text(t.label) }
            }
        }
        val editable = view.username.isNotEmpty()
        LazyColumn(Modifier.weight(1f)) {
            when (tab) {
                CboTab.Loadout -> {
                    view.roles.forEachIndexed { i, role ->
                        item(key = "role " + role.name) { RoleCard(i, role, view, panel, editable) }
                    }
                }
                CboTab.Options, CboTab.Other -> {
                    val groups = view.sections.filter { s -> (tab == CboTab.Other) == (s.chip in view.other) }
                    for (s in groups) {
                        item(key = "title " + s.chip) {
                            Text(s.title, style = MaterialTheme.typography.labelLarge, color = Turu.Accent, modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 2.dp))
                        }
                        for (o in view.options.filter { it.section == s.chip }) {
                            item(key = o.group) {
                                OptionRow(
                                    o.copy(differs = false, unused = !editable),
                                    edited = false,
                                    inGame = true,
                                    onSet = { panel.act("option", JSONObject().put("tag", o.group).put("value", it)) },
                                    onAction = {},
                                )
                            }
                        }
                    }
                }
            }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        panel.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { panel.act("refresh") }, enabled = !view.busy, modifier = Modifier.heightIn(min = 44.dp)) { Text("Refresh from game") }
            TextButton(onClick = { panel.act("revert") }, enabled = view.unsaved, modifier = Modifier.heightIn(min = 44.dp)) { Text("Revert") }
            Spacer(Modifier.weight(1f))
            Button(
                onClick = { if (view.askPublic) askPublic = true else panel.act("save") },
                enabled = editable && !view.busy,
                modifier = Modifier.heightIn(min = 44.dp),
            ) { Text("Save") }
        }
    }
    if (askPublic) {
        AlertDialog(
            onDismissRequest = { askPublic = false },
            title = { Text("Use public rooms?") },
            text = { Text("With Private Rooms off, the bot joins public rooms, where other players see it. Save keeps Private Rooms on unless you say yes.") },
            confirmButton = { TextButton(onClick = { askPublic = false; panel.act("confirm-public"); panel.act("save") }) { Text("Use public rooms") } },
            dismissButton = { TextButton(onClick = { askPublic = false; panel.act("save") }) { Text("Keep private") } },
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

/** A role: its dot, name and purpose; its class and mode; Use equipment, and its gear folded. */
@Composable
private fun RoleCard(index: Int, role: RoleView, view: CoreBotsView, panel: CoreBotsPanel, editable: Boolean) {
    var gear by remember { mutableStateOf(false) }
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 4.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(Turu.Card)
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            val c = role.color.toLong()
            Box(Modifier.size(10.dp).clip(CircleShape).background(Color(0xFF000000 or c))) // impeccable-disable-line raw-color: the class's own colour, from the model
            Spacer(Modifier.width(8.dp))
            Text(role.name, style = MaterialTheme.typography.titleSmall)
            Spacer(Modifier.width(8.dp))
            Text(role.purpose, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.weight(1.4f)) {
                Picker(
                    label = "Class",
                    value = role.`class`,
                    options = view.classes.map { it to it },
                    loading = false,
                    none = "Not set",
                    empty = "No enhanced classes (Refresh from game)",
                ) { if (editable) panel.role(index, "class", it) }
            }
            Box(Modifier.weight(1f)) {
                Picker(
                    label = "Mode",
                    value = role.mode,
                    options = role.modes.ifEmpty { listOf("Base") }.map { it to it },
                    loading = false,
                ) { if (editable) panel.role(index, "mode", it) }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.heightIn(min = 48.dp)) {
            Text("Use equipment", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
            Switch(checked = role.useEquipment, enabled = editable, onCheckedChange = { panel.role(index, "equipment", it) })
        }
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).clickable { gear = !gear }.heightIn(min = 40.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val set = role.gear.count { it.isNotEmpty() }
            Text("Gear · $set of ${role.gear.size} set", style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = Modifier.weight(1f))
            Icon(if (gear) Icons.Rounded.KeyboardArrowUp else Icons.Rounded.KeyboardArrowDown, contentDescription = if (gear) "Fold the gear" else "Show the gear", tint = Turu.Muted)
        }
        AnimatedVisibility(gear) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                view.slots.forEachIndexed { slot, s ->
                    Picker(
                        label = s.name,
                        value = role.gear.getOrElse(slot) { "" },
                        options = listOf("" to "None") + s.items.map { it to it },
                        loading = false,
                    ) { if (editable) panel.act("role", JSONObject().put("index", index).put("slot", slot).put("item", it)) }
                }
            }
        }
    }
}
