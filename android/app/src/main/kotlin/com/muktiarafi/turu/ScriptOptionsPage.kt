package com.muktiarafi.turu

import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
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
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.ScriptOptionGroupView
import com.muktiarafi.turu.models.ScriptOptionKind
import com.muktiarafi.turu.models.ScriptOptionRowView
import com.muktiarafi.turu.models.ScriptOptionsView
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * Script Options for the tab on screen (`turu_frontend::models::script_options`): the draft, the
 * script's waiting request and the last error are the slot's, shared with its egui window. Read
 * only while the page shows.
 */
class ScriptOptionsPanel {
    var view by mutableStateOf<ScriptOptionsView?>(null)
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
            api, token, models = listOf("script-options"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ScriptOptionsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/script-options doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("script-options", "do", JSONObject().put("action", "open"))
        }
    }

    fun act(action: String, body: JSONObject = JSONObject(), done: () -> Unit = {}) {
        error = null
        client?.act("script-options", "do", body.put("action", action)) { failed ->
            error = failed
            if (failed == null) done()
        }
    }

    fun set(group: ScriptOptionGroupView, row: ScriptOptionRowView, value: Any) =
        act("set", JSONObject().put("category", group.name).put("name", row.name).put("value", value))
}

/** From this many rows on, the page has a filter box. */
private const val FILTER_FROM = 10

/**
 * Scripts > Edit Script Options (docs/UI.md, "Native shells"; VibeSkua's Script Options): the
 * loaded script's options (or a queued entry's), each with its description and editor, then each
 * group under its title; a group of many switches is a checklist (All, None, Hide owned, and a
 * note with a fix when a choice elsewhere leaves it unused); an item option picks from the
 * account's items or takes a typed name. Save writes them; Cancel and Back drop them. While the
 * script waits for them it says so, and either lets it go on. ⋮: Reset to defaults, Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ScriptOptionsPage(state: ShellState, panel: ScriptOptionsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var filter by rememberSaveable { mutableStateOf("") }
    var resetting by remember { mutableStateOf(false) }
    val cancel = { panel.act("cancel") { state.back() } }
    BackHandler(enabled = view?.editing == true) { cancel() }

    Column(Modifier.fillMaxSize()) {
        PageBar("Script Options", onBack = { if (view?.editing == true) cancel() else state.back() }) {
            FullWindowMenu("The same options, with a picker under each item option", fullWindow) { close ->
                DropdownMenuItem(
                    text = { Text("Reset to defaults…") },
                    enabled = view?.editing == true,
                    onClick = { close(); resetting = true },
                )
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        if (!view.editing) {
            Text(view.message, style = MaterialTheme.typography.bodyMedium, color = Turu.Muted, modifier = Modifier.padding(16.dp))
            return@Column
        }
        val typing = WindowInsets.isImeVisible
        if (!typing) {
            Text(
                listOfNotNull(if (view.queued) "A queued entry's options" else null, view.savedIn).joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            if (view.waiting) {
                Text(
                    "The script waits for these options: Save or Cancel lets it go on.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Warning,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 2.dp),
                )
            }
        }
        val rowCount = view.groups.sumOf { it.rows.size }
        val q = filter.trim().lowercase()
        if (rowCount >= FILTER_FROM) {
            OutlinedTextField(
                value = filter,
                onValueChange = { filter = it },
                placeholder = { Text("Filter options") },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
                trailingIcon = { if (filter.isNotEmpty()) IconButton(onClick = { filter = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the filter") } },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }
        LazyColumn(Modifier.weight(1f)) {
            var any = false
            for (group in view.groups) {
                val groupHit = q.isNotEmpty() && group.title.lowercase().contains(q)
                val rows = group.rows.filter { groupHit || q.isEmpty() || it.label.lowercase().contains(q) || it.name.lowercase().contains(q) }
                if (rows.isEmpty() && !(group.checklist && group.hidingOwned)) continue
                any = true
                if (group.title.isNotEmpty()) {
                    item(key = "t:${group.name}") {
                        Text(group.title, style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp))
                    }
                }
                if (group.checklist) {
                    item(key = "c:${group.name}") { ChecklistHead(group, rows, panel) }
                    items(rows, key = { "r:${group.name}:${it.name}" }) { row -> CheckRow(group, row, panel) }
                } else {
                    items(rows, key = { "r:${group.name}:${it.name}" }) { row -> OptionRow(group, row, panel) }
                }
            }
            if (!any) {
                item { Text("No options match \"${filter.trim()}\".", color = Turu.Muted, modifier = Modifier.padding(16.dp)) }
            }
        }
        if (typing) return@Column
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        (panel.error ?: view.error.ifEmpty { null })?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 6.dp))
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            if (view.readingItems) {
                Text("Reading your items…", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(start = 4.dp))
            }
            Spacer(Modifier.weight(1f))
            TextButton(onClick = cancel, modifier = Modifier.heightIn(min = 48.dp)) { Text("Cancel") }
            Spacer(Modifier.width(8.dp))
            Button(onClick = { panel.act("save") { state.back() } }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Save") }
        }
    }

    if (resetting) {
        AlertDialog(
            onDismissRequest = { resetting = false },
            title = { Text("Reset to defaults?") },
            text = { Text("Every option goes back to the script's default. Nothing is saved until you press Save.") },
            confirmButton = { TextButton(onClick = { resetting = false; panel.act("reset") }) { Text("Reset") } },
            dismissButton = { TextButton(onClick = { resetting = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** A checklist's head: how many are picked, All and None for the rows shown, Hide owned, and the note when it's unused. */
@Composable
private fun ChecklistHead(group: ScriptOptionGroupView, rows: List<ScriptOptionRowView>, panel: ScriptOptionsPanel) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
        if (group.unusedNote.isNotEmpty()) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(vertical = 4.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(Turu.Warning.copy(alpha = 0.10f))
                    .padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(group.unusedNote, style = MaterialTheme.typography.bodySmall, color = Turu.Warning, modifier = Modifier.weight(1f))
                TextButton(
                    onClick = { panel.act("set", JSONObject().put("category", group.useCategory).put("name", group.useOption).put("value", group.useChoice)) },
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Use ${group.useChoice.replace('_', ' ')}") }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                "${group.picked} of ${group.total} picked",
                style = MaterialTheme.typography.bodySmall,
                color = if (group.picked > 0u) Turu.Text else Turu.Muted,
                modifier = Modifier.weight(1f),
            )
            val names = { on: Boolean -> JSONArray(rows.filter { it.on != on }.map { it.name }) }
            TextButton(onClick = { panel.act("set-all", JSONObject().put("category", group.name).put("names", names(true)).put("value", true)) }, enabled = rows.any { !it.on }, modifier = Modifier.heightIn(min = 48.dp)) { Text("All") }
            TextButton(onClick = { panel.act("set-all", JSONObject().put("category", group.name).put("names", names(false)).put("value", false)) }, enabled = rows.any { it.on }, modifier = Modifier.heightIn(min = 48.dp)) { Text("None") }
        }
        if (group.owned > 0u) {
            FilterChip(
                selected = group.hidingOwned,
                onClick = { panel.act("hide-owned", JSONObject().put("category", group.name).put("on", !group.hidingOwned)) },
                label = { Text("Hide owned (${group.owned})") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text),
                modifier = Modifier.heightIn(min = 48.dp),
            )
        }
        if (group.hidingOwned && rows.isEmpty()) {
            Text("You own every item here.", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(vertical = 8.dp))
        }
    }
}

/** A checklist row: the item, Owned or In bank, its switch; the whole row toggles it. */
@Composable
private fun CheckRow(group: ScriptOptionGroupView, row: ScriptOptionRowView, panel: ScriptOptionsPanel) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickable { panel.set(group, row, !row.on) }
            .heightIn(min = 48.dp)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(row.label, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (row.owned.isNotEmpty()) {
            Text(row.owned, style = MaterialTheme.typography.labelSmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 8.dp))
        }
        Switch(checked = row.on, onCheckedChange = { panel.set(group, row, it) })
    }
}

/** An option: its label, description and the picked choice's explanation, and its editor. */
@Composable
private fun OptionRow(group: ScriptOptionGroupView, row: ScriptOptionRowView, panel: ScriptOptionsPanel) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(row.label, fontWeight = FontWeight.Medium)
                if (row.description.isNotEmpty()) Text(row.description, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                if (row.explanation.isNotEmpty()) {
                    Text("${row.text.replace('_', ' ')}: ${row.explanation}", style = MaterialTheme.typography.bodySmall)
                }
            }
            when (row.kind) {
                ScriptOptionKind.SWITCH -> {
                    Spacer(Modifier.width(12.dp))
                    Switch(checked = row.on, onCheckedChange = { panel.set(group, row, it) })
                }
                ScriptOptionKind.NUMBER -> {
                    Spacer(Modifier.width(12.dp))
                    LiveField(row.number.toString(), "", number = true, modifier = Modifier.width(120.dp)) { t ->
                        t.toLongOrNull()?.let { panel.set(group, row, it) }
                    }
                }
                ScriptOptionKind.CHOICE -> {
                    Spacer(Modifier.width(12.dp))
                    Box(Modifier.widthIn(min = 140.dp, max = 180.dp)) {
                        Picker(label = "", value = row.text, options = row.choices.map { it.key to it.words }, loading = false) { panel.set(group, row, it) }
                    }
                }
                else -> {}
            }
        }
        when (row.kind) {
            ScriptOptionKind.TEXT -> LiveField(row.text, row.defaultWords, number = false, modifier = Modifier.fillMaxWidth().padding(top = 6.dp)) { panel.set(group, row, it) }
            ScriptOptionKind.ITEM -> ItemField(group, row, panel)
            else -> {}
        }
    }
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.4f), modifier = Modifier.padding(horizontal = 16.dp))
}

/**
 * A field that sends each change at once (the slot keeps the draft, so Save sees it) and takes
 * a change from elsewhere (Reset, the full window) while it isn't being typed in.
 */
@Composable
private fun LiveField(value: String, hint: String, number: Boolean, modifier: Modifier, trailing: (@Composable () -> Unit)? = null, onSet: (String) -> Unit) {
    var text by remember { mutableStateOf(value) }
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(value) { if (!focused && value != text) text = value }
    OutlinedTextField(
        value = text,
        onValueChange = { next ->
            text = if (number) next.filter { it.isDigit() || it == '-' } else next
            onSet(text)
        },
        placeholder = { if (hint.isNotEmpty()) Text(hint) },
        trailingIcon = trailing,
        singleLine = true,
        shape = RoundedCornerShape(12.dp),
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.Number else KeyboardType.Text, imeAction = ImeAction.Done),
        modifier = modifier.onFocusChanged { focused = it.isFocused },
    )
}

/** An item option: the name (typed), a list of the account's matching items to pick from, and a warning when the name isn't among them. */
@Composable
private fun ItemField(group: ScriptOptionGroupView, row: ScriptOptionRowView, panel: ScriptOptionsPanel) {
    var menu by remember { mutableStateOf(false) }
    Box(Modifier.padding(top = 6.dp)) {
        LiveField(
            row.text,
            "Choose ${if (row.itemNoun.first() in "aeiou") "an" else "a"} ${row.itemNoun}, or type its name",
            number = false,
            modifier = Modifier.fillMaxWidth(),
            trailing = if (row.candidates.isNotEmpty()) {
                { IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.ArrowDropDown, contentDescription = "Your ${row.itemNoun} items") } }
            } else null,
        ) { panel.set(group, row, it) }
        // What's typed narrows the list, unless it already names one of the items.
        val typed = row.text.trim()
        val shown = if (typed.isEmpty() || row.candidates.any { it.name.equals(typed, ignoreCase = true) }) row.candidates
        else row.candidates.filter { it.name.contains(typed, ignoreCase = true) }.ifEmpty { row.candidates }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
            for (item in shown.take(60)) {
                DropdownMenuItem(
                    text = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(item.name, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                            if (item.tag.isNotEmpty()) Text("  ${item.tag}", style = MaterialTheme.typography.labelSmall, color = Turu.Muted)
                        }
                    },
                    onClick = { menu = false; panel.set(group, row, item.name) },
                )
            }
            if (shown.size > 60) {
                Text("Type to narrow ${shown.size} items", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(12.dp))
            }
        }
    }
    if (row.missing.isNotEmpty()) {
        Text(row.missing, style = MaterialTheme.typography.bodySmall, color = Turu.Warning, modifier = Modifier.padding(top = 4.dp))
    }
}
