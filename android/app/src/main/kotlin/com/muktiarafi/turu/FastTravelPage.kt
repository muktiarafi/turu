package com.muktiarafi.turu

import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
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
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.DestinationView
import com.muktiarafi.turu.models.FastTravelView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * Fast Travel for the tab on screen (`turu_frontend::models::fast_travel`): the destinations, the
 * travel in flight, Get current's answer and the private room are the slot's, shared with its egui
 * window. Read only while the page shows; opening it reads the list again.
 */
class FastTravelPanel {
    var view by mutableStateOf<FastTravelView?>(null)
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
            api, token, models = listOf("fast-travel"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<FastTravelView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/fast-travel doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("fast-travel", "do", JSONObject().put("action", "open"))
        }
    }

    fun act(action: String, body: JSONObject = JSONObject()) {
        error = null
        client?.act("fast-travel", "do", body.put("action", action)) { error = it }
    }

    /** An action on `dest`, named by its place and its name (the slot refuses one that moved). */
    fun on(action: String, dest: DestinationView, body: JSONObject = JSONObject()) =
        act(action, body.put("index", dest.position.toLong()).put("name", dest.name))
}

/** The form's destination: adding (`editing` null) or editing one. */
private data class Form(val editing: DestinationView?, val name: String, val map: String, val cell: String, val pad: String) {
    val valid get() = name.isNotBlank() && map.isNotBlank() && cell.isNotBlank() && pad.isNotBlank()

    fun body(): JSONObject = JSONObject().put("name", name.trim()).put("map", map.trim()).put("cell", cell.trim()).put("pad", pad.trim())

    companion object {
        /** VibeSkua's new destination: no name, `battleon`, `Enter`, `Spawn`. */
        fun new() = Form(null, "", "battleon", "Enter", "Spawn")

        fun of(dest: DestinationView) = Form(dest, dest.name, dest.map, dest.cell, dest.pad)
    }
}

/**
 * Tools & Helpers > Fast Travel (docs/UI.md, "Native shells"; VibeSkua's Fast Travel): the saved
 * destinations, a tap travelling there (joining the private room when it's on), search, and Add
 * or Edit as a page pushed over it with Use where I am (Get current). ⋮ has Clear (asked first),
 * Read again and Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun FastTravelPage(state: ShellState, panel: FastTravelPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var form by remember { mutableStateOf<Form?>(null) }
    var clearing by remember { mutableStateOf(false) }
    var search by rememberSaveable { mutableStateOf("") }

    val f = form
    if (view != null && f != null) {
        FormPage(f, view, panel, onChange = { form = it }, onClose = { form = null })
        return
    }

    Column(Modifier.fillMaxSize()) {
        PageBar("Fast Travel", onBack = { state.back() }) {
            FullWindowMenu("The tiles and the form side by side", fullWindow) { close ->
                DropdownMenuItem(
                    text = { Text("Clear…", color = if (view?.destinations?.isNotEmpty() == true) Turu.Danger else Turu.Muted) },
                    enabled = view?.destinations?.isNotEmpty() == true,
                    onClick = { close(); clearing = true },
                )
                DropdownMenuItem(text = { Text("Read again") }, onClick = { close(); panel.act("open") })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        val typing = WindowInsets.isImeVisible
        OutlinedTextField(
            value = search,
            onValueChange = { search = it },
            placeholder = { Text("Search name, map or cell") },
            leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
            trailingIcon = { if (search.isNotEmpty()) IconButton(onClick = { search = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } },
            singleLine = true,
            shape = RoundedCornerShape(12.dp),
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
        )
        if (!typing) PrivateRoom(view, panel)
        val q = search.trim()
        val shown = view.destinations.filter {
            q.isEmpty() || it.name.contains(q, true) || it.map.contains(q, true) || it.cell.contains(q, true)
        }
        LazyColumn(Modifier.weight(1f)) {
            if (shown.isEmpty()) {
                item {
                    Text(
                        if (view.destinations.isEmpty()) "No destinations yet. Add one: Use where I am fills in the map, cell and pad you're in."
                        else "No destinations match \"$q\".",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Turu.Muted,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
            items(shown, key = { "${it.position}:${it.name}" }) { dest ->
                DestinationRow(dest, busy = view.destinations.any { it.traveling }, panel, onEdit = { form = Form.of(dest) })
            }
        }
        if (typing) return@Column
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            val note = panel.error?.let { it to Turu.Danger } ?: view.note?.let { it.text to it.tint.color() }
            Text(
                note?.first ?: "${view.destinations.size} saved · a tap travels",
                style = MaterialTheme.typography.bodySmall,
                color = note?.second ?: Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(12.dp))
            Button(onClick = { form = Form.new() }, modifier = Modifier.heightIn(min = 48.dp)) {
                Icon(Icons.Rounded.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Add")
            }
        }
    }

    if (clearing) {
        AlertDialog(
            onDismissRequest = { clearing = false },
            title = { Text("Clear Fast Travel?") },
            text = { Text("Every saved destination goes. The defaults don't come back.") },
            confirmButton = { TextButton(onClick = { clearing = false; panel.act("clear") }) { Text("Clear", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { clearing = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** VibeSkua's private room: travels join `map-<room>` while it's on. */
@Composable
private fun PrivateRoom(view: FastTravelView, panel: FastTravelPanel) {
    var text by remember(view.room) { mutableStateOf(view.room.toString()) }
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(
            Modifier
                .weight(1f)
                .clip(RoundedCornerShape(12.dp))
                .clickable { panel.act("private", JSONObject().put("on", !view.privateRoom)) }
                .heightIn(min = 48.dp),
            verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
        ) {
            Text("Private room")
            Text(
                if (view.privateRoom) "Travels join this room number" else "Off: travels join the public room",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
            )
        }
        OutlinedTextField(
            value = text,
            onValueChange = { next ->
                text = next.filter { it.isDigit() }.take(9)
                text.toLongOrNull()?.takeIf { it in 1..999_999_999 && it != view.room }?.let { panel.act("private", JSONObject().put("room", it)) }
            },
            enabled = view.privateRoom,
            singleLine = true,
            shape = RoundedCornerShape(12.dp),
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
            modifier = Modifier.width(136.dp),
        )
        Spacer(Modifier.width(12.dp))
        Switch(checked = view.privateRoom, onCheckedChange = { panel.act("private", JSONObject().put("on", it)) })
    }
}

/** A destination: a tap travels there (a spinner while it does); ⋮ edits or removes it. */
@Composable
private fun DestinationRow(dest: DestinationView, busy: Boolean, panel: FastTravelPanel, onEdit: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (dest.traveling) Turu.AccentTint else androidx.compose.ui.graphics.Color.Transparent)
            .clickable(enabled = !busy) { panel.on("travel", dest) }
            .heightIn(min = 56.dp)
            .padding(start = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(32.dp).clip(CircleShape).background(Turu.AccentTint), contentAlignment = Alignment.Center) {
            if (dest.traveling) {
                CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            } else {
                Icon(Icons.Rounded.Place, contentDescription = null, tint = Turu.Accent, modifier = Modifier.size(18.dp))
            }
        }
        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
            Text(dest.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${dest.map} · ${dest.cell} · ${dest.pad}", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "${dest.name}: edit, remove", tint = Turu.Muted) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                DropdownMenuItem(text = { Text("Edit") }, onClick = { menu = false; onEdit() })
                DropdownMenuItem(text = { Text("Remove", color = Turu.Danger) }, onClick = { menu = false; panel.on("remove", dest) })
            }
        }
    }
}

/**
 * Add or Edit, pushed over the list: Name, Map, Cell and Pad, and Use where I am (Get current) to
 * fill the map, cell and pad. Add or Save in the bar (the keyboard covers the page's foot in
 * landscape); Back drops the form.
 */
@Composable
private fun FormPage(form: Form, view: FastTravelView, panel: FastTravelPanel, onChange: (Form) -> Unit, onClose: () -> Unit) {
    BackHandler { onClose() }
    // Get current answered since the form opened: its map, cell and pad come in.
    var hereSeen by remember { mutableStateOf(view.hereCount) }
    LaunchedEffect(view.hereCount) {
        if (view.hereCount != hereSeen) {
            hereSeen = view.hereCount
            view.here?.let { onChange(form.copy(map = it.map, cell = it.cell, pad = it.pad)) }
        }
    }
    val submit = {
        if (form.valid) {
            val e = form.editing
            if (e == null) panel.act("add", form.body()) else panel.act("edit", form.body().put("index", e.position.toLong()).put("was", e.name))
            onClose()
        }
    }
    Column(Modifier.fillMaxSize()) {
        PageBar(if (form.editing == null) "Add a destination" else "Edit ${form.editing.name}", onBack = onClose) {
            Button(onClick = submit, enabled = form.valid, modifier = Modifier.padding(end = 8.dp).heightIn(min = 40.dp)) {
                Text(if (form.editing == null) "Add" else "Save")
            }
        }
        LazyColumn(Modifier.weight(1f)) {
            item {
                FormField("Name", form.name, "Ultra Alteon") { onChange(form.copy(name = it)) }
                FormField("Map", form.map, "battleon") { onChange(form.copy(map = it)) }
                Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.weight(1f)) {
                        OutlinedTextField(
                            value = form.cell,
                            onValueChange = { onChange(form.copy(cell = it)) },
                            label = { Text("Cell") },
                            singleLine = true,
                            shape = RoundedCornerShape(12.dp),
                            colors = fieldColors(),
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                            keyboardActions = KeyboardActions(onDone = { submit() }),
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                    Spacer(Modifier.width(8.dp))
                    Box(Modifier.width(150.dp)) {
                        Picker(label = "Pad", value = form.pad, options = view.pads.map { it to it }, loading = false) { onChange(form.copy(pad = it)) }
                    }
                }
                TextButton(onClick = { panel.act("get-current") }, modifier = Modifier.padding(horizontal = 8.dp).heightIn(min = 48.dp)) {
                    Icon(Icons.Rounded.Place, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Use where I am")
                }
                (panel.error ?: view.note?.takeIf { it.tint == com.muktiarafi.turu.models.Tint.DANGER }?.text)?.let {
                    Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp))
                }
                if (!form.valid) {
                    Text("Fill in the name, map, cell and pad", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                }
            }
        }
    }
}

@Composable
private fun FormField(label: String, value: String, placeholder: String, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        placeholder = { Text(placeholder) },
        singleLine = true,
        shape = RoundedCornerShape(12.dp),
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )
}
