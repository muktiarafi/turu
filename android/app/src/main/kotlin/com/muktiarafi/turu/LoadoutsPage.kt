package com.muktiarafi.turu

import android.util.Log
import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.GearSlotView
import com.muktiarafi.turu.models.ItemPlace
import com.muktiarafi.turu.models.LoadoutView
import com.muktiarafi.turu.models.LoadoutWork
import com.muktiarafi.turu.models.LoadoutsAct
import com.muktiarafi.turu.models.LoadoutsView
import kotlinx.serialization.json.Json

/**
 * Loadouts for the tab on screen (`turu_frontend::models::loadouts`): the character's list and
 * what a bot thread does for it are the slot's, shared with its egui window. Read only while the
 * page shows; opening it reads the file and the worn gear again.
 */
class LoadoutsPanel {
    var view by mutableStateOf<LoadoutsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("loadouts"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<LoadoutsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/loadouts doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("loadouts", LoadoutsAct.open, LoadoutsAct.JsonSerializer)
        }
    }

    fun act(action: LoadoutsAct) {
        error = null
        client?.act("loadouts", action, LoadoutsAct.JsonSerializer) { error = it }
    }
}

/** The page's open loadout: the live entry, or a saved one by name. */
private sealed interface Open {
    data object Current : Open
    data class Saved(val name: String) : Open
}

/** What a dialog asks before acting. */
private sealed interface Ask {
    data class Rename(val loadout: LoadoutView) : Ask
    data class Delete(val loadout: LoadoutView) : Ask
    data class StopAndEquip(val loadout: LoadoutView, val script: String) : Ask
    data class Enhance(val loadout: LoadoutView) : Ask
}

/**
 * Tools & Helpers > Loadouts (docs/UI.md, "Native shells"; VibeSkua's Loadouts Manager): the
 * character's saved gear sets, then one of them with where each item is now (worn, in the bag, in
 * the bank), Equip, Enhance when enhancements differ, Replace with worn, Rename and Delete.
 * Currently equipped is the live entry, saved as a new loadout from there or from Save current
 * gear at the foot.
 */
@Composable
fun LoadoutsPage(state: ShellState, panel: LoadoutsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var open by remember { mutableStateOf<Open?>(null) }
    var ask by remember { mutableStateOf<Ask?>(null) }
    // A loadout saved from what's worn while the page shows: open it and offer its name.
    var createdSeen by remember { mutableStateOf<UInt?>(null) }
    LaunchedEffect(view?.createdCount) {
        val v = view ?: return@LaunchedEffect
        val seen = createdSeen
        createdSeen = v.createdCount
        if (seen != null && seen != v.createdCount) {
            v.loadouts.firstOrNull { it.name == v.created }?.let {
                open = Open.Saved(it.name)
                ask = Ask.Rename(it)
            }
        }
    }
    // A loadout renamed here: followed under its new name once the slot has it. Deleted (or
    // renamed elsewhere): back to the list.
    var renamedTo by remember { mutableStateOf<String?>(null) }
    val shown: LoadoutView? = when (val o = open) {
        null -> null
        Open.Current -> view?.current
        is Open.Saved -> view?.loadouts?.let { list ->
            list.firstOrNull { it.name == o.name } ?: renamedTo?.let { to -> list.firstOrNull { it.name == to } }
        }
    }
    LaunchedEffect(shown?.name, open) {
        val o = open as? Open.Saved ?: return@LaunchedEffect
        if (view == null) return@LaunchedEffect
        when {
            shown == null -> if (renamedTo == null) open = null
            shown.name != o.name -> { open = Open.Saved(shown.name); renamedTo = null }
        }
    }
    BackHandler(enabled = open != null) { open = null }

    Column(Modifier.fillMaxSize()) {
        PageBar(
            title = shown?.let { if (open is Open.Current) "Currently equipped" else it.name } ?: "Loadouts Manager",
            onBack = { if (open != null) open = null else state.back() },
        ) {
            FullWindowMenu("Both panes side by side, as VibeSkua's", fullWindow) { close ->
                if (shown != null && open is Open.Saved && view != null) {
                    LoadoutItems(shown, view, panel, close) { ask = it }
                }
                DropdownMenuItem(text = { Text("Read again") }, onClick = { close(); panel.act(LoadoutsAct.open) })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        if (view.username.isEmpty()) {
            Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("Not logged in", style = MaterialTheme.typography.titleSmall)
                Text(
                    "Loadouts are each character's. Log in, and this character's show here.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            return@Column
        }
        if (shown == null) {
            ListBody(view, Modifier.weight(1f)) { open = it }
        } else {
            DetailBody(shown, saved = open is Open.Saved, view, Modifier.weight(1f))
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Footer(shown, open is Open.Saved, view, panel) { ask = it }
    }

    when (val a = ask) {
        null -> {}
        is Ask.Rename -> NameDialog(
            title = "Rename loadout",
            initial = a.loadout.name,
            confirm = "Rename",
            onDismiss = { ask = null },
        ) { name ->
            ask = null
            if (name != a.loadout.name) {
                renamedTo = name
                panel.act(LoadoutsAct.rename(a.loadout.name, name))
            }
        }
        is Ask.Delete -> AlertDialog(
            onDismissRequest = { ask = null },
            title = { Text("Delete this loadout?") },
            text = { Text("${a.loadout.name}. Its items stay in your inventory and bank.") },
            confirmButton = { TextButton(onClick = { ask = null; open = null; renamedTo = null; panel.act(LoadoutsAct.delete(a.loadout.name)) }) { Text("Delete", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { ask = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
        is Ask.StopAndEquip -> AlertDialog(
            onDismissRequest = { ask = null },
            title = { Text("Stop ${a.script} and equip?") },
            text = { Text("Equipping changes gear the script may rely on, so it stops first, and stays stopped.") },
            confirmButton = {
                TextButton(onClick = { ask = null; panel.act(LoadoutsAct.equip(a.loadout.name, stop = true)) }) { Text("Stop and equip", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { ask = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
        is Ask.Enhance -> AlertDialog(
            onDismissRequest = { ask = null },
            title = { Text("Enhance as saved?") },
            text = {
                Text(
                    "${a.loadout.changes} worn ${if (a.loadout.changes == 1u) "item is" else "items are"} enhanced differently from ${a.loadout.name}. " +
                        "CoreAdvanced may travel to the enhancement shops and spend gold, then comes back here. Stop in the Script Loader stops it.",
                )
            },
            confirmButton = { TextButton(onClick = { ask = null; panel.act(LoadoutsAct.enhance(a.loadout.name)) }) { Text("Enhance") } },
            dismissButton = { TextButton(onClick = { ask = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** A saved loadout's items at the top of the page's ⋮: Rename, Replace with worn, Delete. */
@Composable
private fun LoadoutItems(loadout: LoadoutView, view: LoadoutsView, panel: LoadoutsPanel, close: () -> Unit, onAsk: (Ask) -> Unit) {
    val idle = view.work == null
    DropdownMenuItem(text = { Text("Rename…") }, onClick = { close(); onAsk(Ask.Rename(loadout)) })
    DropdownMenuItem(
        text = {
            Column {
                Text("Replace with worn")
                Text("Put what you wear now in it instead", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
            }
        },
        enabled = idle,
        onClick = { close(); panel.act(LoadoutsAct.replace(loadout.name)) },
    )
    DropdownMenuItem(text = { Text("Delete…", color = if (idle) Turu.Danger else Turu.Muted) }, enabled = idle, onClick = { close(); onAsk(Ask.Delete(loadout)) })
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
}

/** Currently equipped, then the saved loadouts; a tap opens one. */
@Composable
private fun ListBody(view: LoadoutsView, modifier: Modifier, onOpen: (Open) -> Unit) {
    LazyColumn(modifier) {
        item {
            LoadoutRow(
                name = "Currently equipped",
                detail = if (view.reading) "Reading your gear…" else view.current.`class`,
                worn = false,
            ) { onOpen(Open.Current) }
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
            Text(
                "${view.listTitle} · ${view.loadouts.size}",
                style = MaterialTheme.typography.labelLarge,
                color = Turu.Muted,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 4.dp),
            )
        }
        if (view.loadouts.isEmpty()) {
            item {
                Text(
                    "No loadouts yet. Save current gear keeps what you wear now, under a name, to put back on later.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Turu.Muted,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
        }
        items(view.loadouts, key = { it.name }) { l ->
            LoadoutRow(name = l.name, detail = l.`class`, worn = l.worn) { onOpen(Open.Saved(l.name)) }
        }
    }
}

@Composable
private fun LoadoutRow(name: String, detail: String, worn: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(MaterialTheme.shapes.medium)
            .clickable(onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(detail, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (worn) {
            Chip("Worn", Turu.Success)
            Spacer(Modifier.width(4.dp))
        }
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Muted)
    }
}

/** A small label in its colour, on a faint tint of it. */
@Composable
private fun Chip(text: String, color: androidx.compose.ui.graphics.Color) {
    Text(
        text,
        style = MaterialTheme.typography.labelSmall,
        color = color,
        maxLines = 1,
        modifier = Modifier.clip(CircleShape).background(color.copy(alpha = 0.14f)).padding(horizontal = 8.dp, vertical = 2.dp),
    )
}

/** Where an item is, as a chip: worn green, missing amber, the rest muted. */
private fun ItemPlace.chip(): Pair<String, androidx.compose.ui.graphics.Color>? = when (this) {
    ItemPlace.NONE -> null
    ItemPlace.WORN -> "Worn" to Turu.Success
    ItemPlace.BAG -> "In bag" to Turu.Muted
    ItemPlace.BANK -> "In bank" to Turu.Muted
    ItemPlace.MISSING -> "Not found" to Turu.Warning
    ItemPlace.UNKNOWN -> "Not in bag" to Turu.Warning
}

/** A loadout's seven slots, with what to know before Equip. */
@Composable
private fun DetailBody(loadout: LoadoutView, saved: Boolean, view: LoadoutsView, modifier: Modifier) {
    LazyColumn(modifier) {
        item {
            Text(
                if (saved) "${loadout.`class`} · ${loadout.items} ${if (loadout.items == 1u) "item" else "items"}" else view.wearing,
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
        items(loadout.slots, key = { it.label }) { slot -> SlotRow(slot) }
        item {
            val notes = buildList {
                if (saved && loadout.changes > 0u) {
                    add("${loadout.changes} worn ${if (loadout.changes == 1u) "item is" else "items are"} enhanced differently: Enhance applies the saved ones" to Turu.Warning)
                }
                if (saved && view.script.isNotEmpty() && view.work != LoadoutWork.ENHANCING) {
                    add("${view.script} is running. Equipping stops it first, and it stays stopped." to Turu.Warning)
                }
                if (saved && loadout.slots.any { it.place == ItemPlace.UNKNOWN }) {
                    add("The bank wasn't read yet: Equip looks there for what isn't in the bag." to Turu.Muted)
                } else if (saved && view.bankRead.isNotEmpty() && loadout.slots.any { it.place == ItemPlace.BANK || it.place == ItemPlace.MISSING }) {
                    add("Bank as read ${view.bankRead}." to Turu.Muted)
                }
            }
            Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for ((text, color) in notes) {
                    Row(verticalAlignment = Alignment.Top) {
                        Box(Modifier.padding(top = 6.dp).size(6.dp).clip(CircleShape).background(color))
                        Spacer(Modifier.width(8.dp))
                        Text(text, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
        }
    }
}

/** A slot: its name, the item, the enhancement it keeps (amber while the worn one differs) and where it is. */
@Composable
private fun SlotRow(slot: GearSlotView) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 16.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(slot.label, style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = Modifier.widthIn(min = 72.dp).padding(end = 12.dp))
        Column(Modifier.weight(1f).padding(end = 8.dp)) {
            Text(slot.item.ifEmpty { "Nothing" }, color = if (slot.item.isEmpty()) Turu.Muted else Turu.Text, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (slot.enhancement.isNotEmpty()) {
                Text(
                    if (slot.wornWith.isNotEmpty()) "${slot.enhancement} · worn with ${slot.wornWith}" else slot.enhancement,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (slot.wornWith.isNotEmpty()) Turu.Warning else Turu.Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        slot.place.chip()?.let { (text, color) -> Chip(text, color) }
    }
}

/** The outcome or what a bot thread is doing, then the page's actions. */
@Composable
private fun Footer(loadout: LoadoutView?, saved: Boolean, view: LoadoutsView, panel: LoadoutsPanel, onAsk: (Ask) -> Unit) {
    val note = panel.error?.let { it to Turu.Danger } ?: view.note?.let { it.text to it.tint.color() }
    note?.let { (text, color) ->
        Text(text, style = MaterialTheme.typography.bodySmall, color = color, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 6.dp))
    }
    val idle = view.work == null
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (!idle) {
            CircularProgressIndicator(Modifier.padding(start = 4.dp).size(20.dp), strokeWidth = 2.dp)
            Text(
                when (view.work) {
                    LoadoutWork.READING -> "Reading your gear…"
                    LoadoutWork.EQUIPPING -> "Equipping…"
                    else -> "Enhancing…"
                },
                style = MaterialTheme.typography.bodyMedium,
                color = Turu.Muted,
            )
        }
        Spacer(Modifier.weight(1f))
        when {
            loadout == null || !saved -> Button(
                onClick = { panel.act(LoadoutsAct.save_current) },
                enabled = idle && !view.reading,
                modifier = Modifier.heightIn(min = 48.dp),
            ) {
                Icon(Icons.Rounded.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(if (loadout == null) "Save current gear" else "Save as loadout")
            }
            else -> {
                if (loadout.changes > 0u) {
                    FilledTonalButton(
                        onClick = { onAsk(Ask.Enhance(loadout)) },
                        enabled = idle && view.script.isEmpty(),
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("Enhance") }
                }
                Button(
                    onClick = { if (view.script.isNotEmpty()) onAsk(Ask.StopAndEquip(loadout, view.script)) else panel.act(LoadoutsAct.equip(loadout.name, stop = false)) },
                    enabled = idle && loadout.items > 0u,
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Equip", fontWeight = FontWeight.SemiBold) }
            }
        }
    }
    if (saved && loadout != null && loadout.items == 0u) {
        Text("This loadout holds no items", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 6.dp))
    }
}
