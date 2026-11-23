package com.muktiarafi.turu

import android.util.Log
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.EntryView
import com.muktiarafi.turu.models.Note
import com.muktiarafi.turu.models.SchedulerAct
import com.muktiarafi.turu.models.SchedulerView
import com.muktiarafi.turu.models.Tint
import kotlinx.serialization.json.Json

/**
 * The Scheduler's copy of the tab on screen (`turu_frontend::models::scheduler`): the tab's
 * playlist, read only while the page shows.
 */
class SchedulerPanel {
    var view by mutableStateOf<SchedulerView?>(null)
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
            api, token, models = listOf(Api.scheduler), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<SchedulerView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/scheduler doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: SchedulerAct) {
        error = null
        client?.act(Api.scheduler, action) { error = it }
    }
}

/** A tint in the shell's palette. */
fun Tint.color(): Color = when (this) {
    Tint.PLAIN -> Turu.Text
    Tint.MUTED -> Turu.Muted
    Tint.SUCCESS -> Turu.Success
    Tint.WARNING -> Turu.Warning
    Tint.DANGER -> Turu.Danger
}

/**
 * Scripts > Scheduler (docs/UI.md, "Native shells"; VibeSkua's Script Scheduler): Start / Stop
 * queue with the summary, the playlist top to bottom (each row's Move, Rename, Script options and
 * Remove in its ⋮), and Add scripts (Search Scripts' Queue), Load and Save by name. Script options
 * opens the slot's Script Options window.
 */
@Composable
fun SchedulerPage(state: ShellState, panel: SchedulerPanel, openOptions: () -> Unit, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var renaming by remember { mutableStateOf<EntryView?>(null) }
    var loading by remember { mutableStateOf(false) }
    var saving by remember { mutableStateOf(false) }
    // Script options sent: the reads count it was sent at. Once the slot's window took the read,
    // its Script Options window is open (the panel goes) unless a note says why not.
    // (The note shown then is cleared by the slot: only a new one counts.)
    var optionsAt by remember { mutableStateOf<Pair<UInt, Note?>?>(null) }
    LaunchedEffect(view?.reads, view?.note, optionsAt) {
        val (at, before) = optionsAt ?: return@LaunchedEffect
        val v = view ?: return@LaunchedEffect
        when {
            v.reads != at -> {
                optionsAt = null
                if (v.note == null) openOptions()
            }
            // Refused at once (a script runs).
            v.note != null && v.note != before -> optionsAt = null
        }
    }
    Column(Modifier.fillMaxSize()) {
        PageBar("Scheduler", onBack = { state.back() }) {
            FullWindowMenu("Load and save playlists anywhere on the device", fullWindow)
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Button(
                onClick = { panel.act(SchedulerAct.toggle) },
                enabled = view.toggle.enabled,
                colors = if (view.running) ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = MaterialTheme.colorScheme.onError) else ButtonDefaults.buttonColors(),
                modifier = Modifier.heightIn(min = 44.dp),
            ) { Text(if (view.running) "Stop queue" else "Start queue") }
            Spacer(Modifier.width(12.dp))
            Text(view.summary, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        if (!view.toggle.enabled) {
            Text(view.toggle.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp))
        }
        LazyColumn(Modifier.weight(1f).padding(top = 8.dp)) {
            if (view.entries.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("The playlist is empty", style = MaterialTheme.typography.titleSmall)
                        Text(
                            "Add scripts from Search Scripts (Add to the queue) and they run here top to bottom, one after another.",
                            style = MaterialTheme.typography.bodySmall,
                            color = Turu.Muted,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
            }
            itemsIndexed(view.entries, key = { _, e -> e.id }) { i, entry ->
                EntryRow(
                    i, entry, view, panel,
                    onRename = { renaming = entry },
                    onOptions = { optionsAt = view.reads to view.note; panel.act(SchedulerAct.options(entry.id)) },
                )
            }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        (panel.error?.let { it to Tint.DANGER } ?: view.note?.let { it.text to it.tint })?.let { (text, tint) ->
            Text(text, style = MaterialTheme.typography.bodySmall, color = tint.color(), modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { loading = true }, enabled = view.load.enabled, modifier = Modifier.heightIn(min = 44.dp)) { Text("Load") }
            TextButton(onClick = { saving = true }, enabled = view.save.enabled, modifier = Modifier.heightIn(min = 44.dp)) { Text("Save") }
            Spacer(Modifier.weight(1f))
            Button(onClick = { state.push(Page.Search) }, enabled = view.edit.enabled, modifier = Modifier.heightIn(min = 44.dp)) { Text("Add scripts") }
        }
    }
    renaming?.let { entry ->
        NameDialog(
            title = "Rename",
            initial = entry.name,
            confirm = "Rename",
            onDismiss = { renaming = null },
        ) { name -> renaming = null; panel.act(SchedulerAct.rename(entry.id, name)) }
    }
    if (saving) {
        NameDialog(
            title = "Save playlist",
            initial = "playlist",
            confirm = "Save",
            note = view?.playlists?.takeIf { it.isNotEmpty() }?.let { "A name already saved is replaced." },
            onDismiss = { saving = false },
        ) { name -> saving = false; panel.act(SchedulerAct.save(name)) }
    }
    if (loading) {
        AlertDialog(
            onDismissRequest = { loading = false },
            title = { Text("Load playlist") },
            text = {
                val names = view?.playlists.orEmpty()
                if (names.isEmpty()) {
                    Text("No saved playlists yet. Save one, or load another file from the full window.", color = Turu.Muted)
                } else {
                    LazyColumn(Modifier.heightIn(max = 280.dp)) {
                        itemsIndexed(names) { _, name ->
                            Text(
                                name.removeSuffix(".json"),
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clip(MaterialTheme.shapes.small)
                                    .clickable { loading = false; panel.act(SchedulerAct.load(name)) }
                                    .heightIn(min = 48.dp)
                                    .padding(horizontal = 8.dp, vertical = 14.dp),
                            )
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { loading = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** A playlist row: its number, name, status and time, the current one tinted; its ⋮. */
@Composable
private fun EntryRow(index: Int, entry: EntryView, view: SchedulerView, panel: SchedulerPanel, onRename: () -> Unit, onOptions: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    val dim = entry.tint == Tint.MUTED
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(selectionTint(entry.current))
            .heightIn(min = 56.dp)
            .padding(start = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("${index + 1}", style = MaterialTheme.typography.labelMedium, color = Turu.Muted, modifier = Modifier.widthIn(min = 24.dp))
        Column(Modifier.weight(1f).padding(horizontal = 8.dp)) {
            Text(entry.name, fontWeight = FontWeight.SemiBold, color = if (dim) Turu.Muted else Turu.Text, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    entry.status,
                    style = MaterialTheme.typography.labelSmall,
                    color = entry.tint.color(),
                    modifier = Modifier
                        .clip(CircleShape)
                        .background(entry.tint.color().copy(alpha = 0.14f))
                        .padding(horizontal = 8.dp, vertical = 2.dp),
                )
                Text(entry.time, style = MaterialTheme.typography.labelSmall, fontFamily = FontFamily.Monospace, color = Turu.Muted)
            }
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "${entry.name}: move, rename, options, remove", tint = Turu.Muted) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                val editable = view.edit.enabled
                DropdownMenuItem(text = { Text("Move up") }, enabled = editable && index > 0, onClick = { menu = false; panel.act(SchedulerAct.move(entry.id, -1)) })
                DropdownMenuItem(text = { Text("Move down") }, enabled = editable && index + 1 < view.entries.size, onClick = { menu = false; panel.act(SchedulerAct.move(entry.id, 1)) })
                DropdownMenuItem(text = { Text("Rename") }, onClick = { menu = false; onRename() })
                DropdownMenuItem(text = { Text("Script options") }, onClick = { menu = false; onOptions() })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                DropdownMenuItem(text = { Text("Remove", color = if (editable) Turu.Danger else Turu.Muted) }, enabled = editable, onClick = { menu = false; panel.act(SchedulerAct.remove(entry.id)) })
                if (!editable) {
                    Text(view.edit.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp).widthIn(max = 240.dp))
                }
            }
        }
    }
}

/**
 * Asks for a name: Rename, Save. The name is selected, so typing replaces it; the keyboard's Done
 * confirms (in landscape the keyboard covers the dialog's buttons).
 */
@Composable
fun NameDialog(title: String, initial: String, confirm: String, note: String? = null, onDismiss: () -> Unit, onConfirm: (String) -> Unit) {
    var field by remember { mutableStateOf(TextFieldValue(initial, TextRange(0, initial.length))) }
    val text = field.text
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                OutlinedTextField(
                    value = field,
                    onValueChange = { field = it },
                    singleLine = true,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { if (text.isNotBlank()) onConfirm(text.trim()) }),
                    modifier = Modifier.focusRequester(focus),
                )
                note?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 6.dp)) }
            }
        },
        confirmButton = { TextButton(onClick = { onConfirm(text.trim()) }, enabled = text.isNotBlank()) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}
