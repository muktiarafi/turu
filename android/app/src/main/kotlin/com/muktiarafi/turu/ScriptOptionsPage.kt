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
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
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
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.PopupProperties
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.OptionValue
import com.muktiarafi.turu.models.ScriptOptionGroupView
import com.muktiarafi.turu.models.ScriptOptionKind
import com.muktiarafi.turu.models.ScriptOptionRowView
import com.muktiarafi.turu.models.ScriptOptionsAct
import com.muktiarafi.turu.models.ScriptOptionsView
import kotlinx.serialization.json.Json

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

    /** The snackbar: a mode a tick switched, with Undo. */
    var notice by mutableStateOf<Notice?>(null)

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        following = tab
        // The tab before: the page left it. With its script waiting, its own window takes the
        // editor back; else the draft goes (`leave`).
        client?.let { old -> old.act(Api.scriptOptions, ScriptOptionsAct.leave) { old.stop() } }
        client = null
        view = null
        notice = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf(Api.scriptOptions), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ScriptOptionsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/script-options doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act(Api.scriptOptions, ScriptOptionsAct.open)
        }
    }

    fun act(action: ScriptOptionsAct, done: () -> Unit = {}) {
        error = null
        client?.act(Api.scriptOptions, action) { failed ->
            error = failed
            if (failed == null) done()
        }
    }

    fun set(group: ScriptOptionGroupView, row: ScriptOptionRowView, value: Any) =
        act(ScriptOptionsAct.set(group.name, row.name, optionValue(value)))

    /**
     * A checklist the script ignores under its choice's present value (a merge's Select while the
     * mode is All): picking from it sets the choice that uses it, saying so, with Undo.
     */
    fun useChecklist(group: ScriptOptionGroupView) {
        if (group.unusedNote.isEmpty()) return
        val before = group.useNow
        act(ScriptOptionsAct.set(group.useCategory, group.useOption, OptionValue.text(group.useChoice)))
        notice = Notice("“${group.useLabel}” set to ${group.useWords}") {
            act(ScriptOptionsAct.set(group.useCategory, group.useOption, OptionValue.text(before)))
        }
    }
}

/** A filter box shows from this many options on, a checklist counting as one. */
private const val FILTER_FROM = 10

/** A checklist this long gets the filter by itself. */
private const val FILTER_CHECKLIST_FROM = 20

/** A choice of at most this many, each explained, shows as radio buttons with its explanations. */
private const val RADIO_UP_TO = 6

/**
 * Scripts > Edit Script Options (docs/UI.md, "Native shells"; VibeSkua's Script Options): the
 * loaded script's options (or a queued entry's), each with its description and editor, then each
 * group under its title. A short choice the script explains is a radio list with each choice's
 * explanation; a group of many switches is a checklist (All, None, Hide owned, and while a choice
 * elsewhere leaves it unused, its rows dimmed and a note with a fix; picking a row makes the fix,
 * with Undo); an item option picks from the account's items (those with the enhancement asked
 * for first) or takes a typed name. Required options sit under "Needed to run", and Save asks
 * before saving one empty. CoreBots' "Skip this window" is the foot's "Don't show before each
 * run". Save writes them; Cancel and Back drop them. While the script waits for them it says so,
 * and either lets it go on. ⋮: where they're saved, Reset to defaults, Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ScriptOptionsPage(state: ShellState, panel: ScriptOptionsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var filter by rememberSaveable { mutableStateOf("") }
    var resetting by remember { mutableStateOf(false) }
    var askingSave by remember { mutableStateOf(false) }
    // After a Save was asked about, each empty required field says so too.
    var triedSave by remember { mutableStateOf(false) }
    var filterFocused by remember { mutableStateOf(false) }
    val cancel = { panel.act(ScriptOptionsAct.cancel) { state.back() } }
    val save = { panel.act(ScriptOptionsAct.save) { state.back() } }
    BackHandler(enabled = view?.editing == true) { cancel() }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            PageBar("Script Options", onBack = { if (view?.editing == true) cancel() else state.back() }) {
                FullWindowMenu("The same options, with a picker under each item option", fullWindow) { close ->
                    if (!view?.savedIn.isNullOrEmpty()) {
                        DropdownMenuItem(text = { Text(view!!.savedIn, color = Turu.Muted) }, enabled = false, onClick = {})
                    }
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
                val caption = listOfNotNull(
                    if (view.queued) "A queued entry's options" else null,
                    if (view.readingItems) "Reading your items…" else null,
                ).joinToString(" · ")
                if (caption.isNotEmpty()) {
                    Text(caption, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp))
                }
                if (view.waiting) {
                    Text(
                        "The script waits for these options: Save or Cancel lets it go on.",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Warning,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 2.dp),
                    )
                }
            }
            val optionCount = view.groups.sumOf { if (it.checklist) 1 else it.rows.size }
            val showFilter = optionCount >= FILTER_FROM || view.groups.any { it.checklist && it.total.toInt() >= FILTER_CHECKLIST_FROM }
            val q = filter.trim().lowercase()
            LazyColumn(Modifier.weight(1f)) {
                // The filter scrolls with the options, leaving them the room. With the keyboard up
                // for an option, it steps aside so the field has the room.
                if (showFilter && (!typing || filterFocused)) {
                    item(key = "filter") {
                        OutlinedTextField(
                            value = filter,
                            onValueChange = { filter = it },
                            placeholder = { Text("Filter options") },
                            leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
                            trailingIcon = { if (filter.isNotEmpty()) IconButton(onClick = { filter = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the filter") } },
                            singleLine = true,
                            shape = MaterialTheme.shapes.medium,
                            colors = fieldColors(),
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).onFocusChanged { filterFocused = it.isFocused },
                        )
                    }
                }
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
                        item(key = "n:${group.name}") { UnusedNote(group, panel) }
                        stickyHeader(key = "c:${group.name}") { ChecklistHead(group, rows, panel) }
                        items(rows, key = { "r:${group.name}:${it.name}" }) { row -> CheckRow(group, row, panel) }
                    } else {
                        val empties = group.rows.count { it.required && it.text.isBlank() && it.kind.holdsText() }
                        items(rows, key = { "r:${group.name}:${it.name}" }) { row ->
                            if (row.section.isNotEmpty()) SectionHead(row.section, empties)
                            OptionRow(group, row, panel, triedSave)
                        }
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
                if (view.skipCategory.isNotEmpty()) {
                    Row(
                        Modifier
                            .weight(1f)
                            .clip(MaterialTheme.shapes.small)
                            .toggleable(value = view.skipOn, role = Role.Checkbox) {
                                panel.act(ScriptOptionsAct.set(view.skipCategory, "SkipOption", OptionValue.switch(it)))
                            }
                            .heightIn(min = 48.dp)
                            .padding(end = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = view.skipOn, onCheckedChange = null, modifier = Modifier.padding(horizontal = 12.dp))
                        Text("Don't show before each run", style = MaterialTheme.typography.bodyMedium, maxLines = 2)
                    }
                } else {
                    Spacer(Modifier.weight(1f))
                }
                TextButton(onClick = cancel, modifier = Modifier.heightIn(min = 48.dp)) { Text("Cancel") }
                Spacer(Modifier.width(8.dp))
                Button(
                    onClick = { if (view.missingRequired.isEmpty()) save() else { triedSave = true; askingSave = true } },
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Save") }
            }
        }
        NoticeHost(panel.notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 72.dp)) { panel.notice = null }
    }

    if (resetting) {
        AlertDialog(
            onDismissRequest = { resetting = false },
            title = { Text("Reset to defaults?") },
            text = { Text("Every option goes back to the script's default. Nothing is saved until you press Save.") },
            confirmButton = { TextButton(onClick = { resetting = false; panel.act(ScriptOptionsAct.reset) }) { Text("Reset") } },
            dismissButton = { TextButton(onClick = { resetting = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    val missing = view?.missingRequired.orEmpty()
    if (askingSave && missing.isNotEmpty()) {
        AlertDialog(
            onDismissRequest = { askingSave = false },
            title = { Text(if (missing.size == 1) "Save with an option empty?" else "Save with ${missing.size} options empty?") },
            text = { Text("The script stops without ${missing.joinToString(", ")}.") },
            confirmButton = { TextButton(onClick = { askingSave = false; save() }) { Text("Save anyway") } },
            dismissButton = { TextButton(onClick = { askingSave = false }) { Text("Keep editing") } },
            containerColor = Turu.Card,
        )
    }
}

private fun ScriptOptionKind.holdsText() = this == ScriptOptionKind.TEXT || this == ScriptOptionKind.ITEM

/** The heading over a run of required options, with how many are still empty. */
@Composable
private fun SectionHead(title: String, empties: Int) {
    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.titleSmall, modifier = Modifier.weight(1f))
        if (empties > 0) {
            Text(if (empties == 1) "1 empty" else "$empties empty", style = MaterialTheme.typography.bodySmall, color = Turu.Warning)
        }
    }
}

/** While a choice elsewhere leaves the checklist unused: what it's set to, and the fix. */
@Composable
private fun UnusedNote(group: ScriptOptionGroupView, panel: ScriptOptionsPanel) {
    if (group.unusedNote.isEmpty()) return
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(Turu.Warning.copy(alpha = 0.10f))
            .padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(group.unusedNote, style = MaterialTheme.typography.bodySmall, color = Turu.Warning, modifier = Modifier.weight(1f))
        TextButton(
            onClick = { panel.act(ScriptOptionsAct.set(group.useCategory, group.useOption, OptionValue.text(group.useChoice))) },
            modifier = Modifier.heightIn(min = 48.dp),
        ) { Text("Use ${group.useWords}") }
    }
}

/**
 * A checklist's head, kept in view while its rows scroll: how many are picked (or, while it's
 * unused, that it is), All and None for the rows shown, and Hide owned.
 */
@Composable
private fun ChecklistHead(group: ScriptOptionGroupView, rows: List<ScriptOptionRowView>, panel: ScriptOptionsPanel) {
    val unused = group.unusedNote.isNotEmpty()
    Column(Modifier.fillMaxWidth().background(Turu.Panel).padding(horizontal = 16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                if (unused) "Not used while set to ${group.useNowWords}" else "${group.picked} of ${group.total} picked",
                style = MaterialTheme.typography.bodySmall,
                color = when {
                    unused -> Turu.Warning
                    group.picked > 0u -> Turu.Text
                    else -> Turu.Muted
                },
                modifier = Modifier.weight(1f),
            )
            val names = { on: Boolean -> rows.filter { it.on != on }.map { it.name } }
            TextButton(
                onClick = {
                    panel.useChecklist(group)
                    panel.act(ScriptOptionsAct.set_all(group.name, names(true), OptionValue.switch(true)))
                },
                enabled = rows.any { !it.on },
                modifier = Modifier.heightIn(min = 48.dp),
            ) { Text("All") }
            TextButton(onClick = { panel.act(ScriptOptionsAct.set_all(group.name, names(false), OptionValue.switch(false))) }, enabled = rows.any { it.on }, modifier = Modifier.heightIn(min = 48.dp)) { Text("None") }
        }
        if (group.owned > 0u) {
            FilterChip(
                selected = group.hidingOwned,
                onClick = { panel.act(ScriptOptionsAct.hide_owned(group.name, !group.hidingOwned)) },
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

/** A checklist row: the item, Owned or In bank, its switch; the whole row toggles it. Dimmed while the list is unused. */
@Composable
private fun CheckRow(group: ScriptOptionGroupView, row: ScriptOptionRowView, panel: ScriptOptionsPanel) {
    val unused = group.unusedNote.isNotEmpty()
    val toggle = { on: Boolean ->
        if (on) panel.useChecklist(group)
        panel.set(group, row, on)
    }
    Row(
        Modifier
            .fillMaxWidth()
            .toggleable(value = row.on, role = Role.Switch) { toggle(it) }
            .heightIn(min = 48.dp)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(row.label, color = if (unused) Turu.Muted else Turu.Text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (row.owned.isNotEmpty()) {
            Text(row.owned, style = MaterialTheme.typography.labelSmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 8.dp))
        }
        Switch(checked = row.on, onCheckedChange = null)
    }
}

/** An option: its label, description and editor; a short explained choice as radio buttons. */
@Composable
private fun OptionRow(group: ScriptOptionGroupView, row: ScriptOptionRowView, panel: ScriptOptionsPanel, triedSave: Boolean) {
    val radio = row.kind == ScriptOptionKind.CHOICE && row.choices.size <= RADIO_UP_TO && row.choices.any { it.explanation.isNotEmpty() }
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(row.label, fontWeight = FontWeight.Medium)
                if (row.description.isNotEmpty()) Text(row.description, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                if (row.explanation.isNotEmpty() && !radio) {
                    Text(
                        "${row.choices.firstOrNull { it.key == row.text }?.words ?: row.text}: ${row.explanation}",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
            when {
                row.kind == ScriptOptionKind.SWITCH -> {
                    Spacer(Modifier.width(12.dp))
                    Switch(checked = row.on, onCheckedChange = { panel.set(group, row, it) })
                }
                row.kind == ScriptOptionKind.NUMBER -> {
                    Spacer(Modifier.width(12.dp))
                    LiveField(row.number.toString(), "", number = true, modifier = Modifier.width(120.dp)) { t ->
                        t.toLongOrNull()?.let { panel.set(group, row, it) }
                    }
                }
                row.kind == ScriptOptionKind.CHOICE && !radio -> {
                    Spacer(Modifier.width(12.dp))
                    Box(Modifier.widthIn(min = 140.dp, max = 180.dp)) {
                        Picker(label = "", value = row.text, options = row.choices.map { it.key to it.words }, loading = false) { panel.set(group, row, it) }
                    }
                }
                else -> {}
            }
        }
        when {
            radio -> RadioChoices(row) { panel.set(group, row, it) }
            row.kind == ScriptOptionKind.TEXT -> LiveField(row.text, row.defaultWords, number = false, modifier = Modifier.fillMaxWidth().padding(top = 6.dp)) { panel.set(group, row, it) }
            row.kind == ScriptOptionKind.ITEM -> ItemField(group, row, panel)
            else -> {}
        }
        if (row.required && row.text.isBlank() && row.kind.holdsText() && triedSave) {
            Text("Empty: the script stops without it", style = MaterialTheme.typography.bodySmall, color = Turu.Warning, modifier = Modifier.padding(top = 4.dp))
        }
    }
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.4f), modifier = Modifier.padding(horizontal = 16.dp))
}

/** A short choice the script explains: one radio row per choice, its explanation under its words. */
@Composable
private fun RadioChoices(row: ScriptOptionRowView, pick: (String) -> Unit) {
    Column(Modifier.padding(top = 4.dp)) {
        for (choice in row.choices) {
            val picked = choice.key == row.text
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(MaterialTheme.shapes.small)
                    .selectable(selected = picked, role = Role.RadioButton) { if (!picked) pick(choice.key) }
                    .heightIn(min = 48.dp)
                    .padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                RadioButton(selected = picked, onClick = null)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.Center) {
                    Text(choice.words)
                    if (choice.explanation.isNotEmpty()) {
                        Text(choice.explanation, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                    }
                }
            }
        }
    }
}

/**
 * A field that sends each change at once (the slot keeps the draft, so Save sees it) and takes
 * a change from elsewhere (Reset, the full window) while it isn't being typed in.
 */
@Composable
private fun LiveField(
    value: String,
    hint: String,
    number: Boolean,
    modifier: Modifier,
    trailing: (@Composable () -> Unit)? = null,
    onFocus: (Boolean) -> Unit = {},
    onSet: (String) -> Unit,
) {
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
        shape = MaterialTheme.shapes.medium,
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.Number else KeyboardType.Text, imeAction = ImeAction.Done),
        modifier = modifier.onFocusChanged { focused = it.isFocused; onFocus(it.isFocused) },
    )
}

/**
 * An item option: the name, typed or picked from the account's matching items (those with the
 * enhancement asked for first, under their own heading; what's typed narrows them, and typing
 * opens them), the one item with that enhancement offered while it's empty, and a warning when
 * the name isn't among the account's items.
 */
@Composable
private fun ItemField(group: ScriptOptionGroupView, row: ScriptOptionRowView, panel: ScriptOptionsPanel) {
    var menu by remember { mutableStateOf(false) }
    var focused by remember { mutableStateOf(false) }
    // What's typed narrows the list, unless it already names one of the items.
    val typed = row.text.trim()
    val shown = if (typed.isEmpty() || row.candidates.any { it.name.equals(typed, ignoreCase = true) }) row.candidates
    else row.candidates.filter { it.name.contains(typed, ignoreCase = true) }
    var width by remember { mutableStateOf(0) }
    val density = LocalDensity.current
    Box(Modifier.padding(top = 6.dp).onSizeChanged { width = it.width }) {
        LiveField(
            row.text,
            "Choose ${if (row.itemNoun.first() in "aeiou") "an" else "a"} ${row.itemNoun}, or type its name",
            number = false,
            modifier = Modifier.fillMaxWidth(),
            trailing = if (row.candidates.isNotEmpty()) {
                // The arrow opens the list without the keyboard (little room for both in landscape).
                {
                    IconButton(onClick = { menu = !menu }) {
                        Icon(
                            Icons.Rounded.ArrowDropDown,
                            contentDescription = "Your ${plural(row.itemNoun)}",
                            modifier = Modifier.rotate(if (menu) 180f else 0f),
                        )
                    }
                }
            } else null,
            onFocus = { focused = it },
        ) {
            panel.set(group, row, it)
            if (focused) menu = true
        }
        // While typing, the list doesn't take the keyboard's focus: what's typed goes on narrowing it.
        DropdownMenu(
            expanded = menu && shown.isNotEmpty(),
            onDismissRequest = { menu = false },
            properties = PopupProperties(focusable = !focused),
            containerColor = Turu.Card,
            modifier = Modifier.width(with(density) { width.toDp() }),
        ) {
            val fitting = shown.count { it.fits }
            if (row.enhancement.isNotEmpty() && fitting == 0 && typed.isEmpty()) {
                Text(
                    "No ${row.itemNoun} with ${row.enhancement} here yet",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Warning,
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp),
                )
            }
            shown.take(60).forEachIndexed { i, item ->
                if (row.enhancement.isNotEmpty() && (i == 0 || i == fitting)) {
                    Text(
                        if (item.fits) "With ${row.enhancement}" else "Other ${plural(row.itemNoun)}",
                        style = MaterialTheme.typography.labelMedium,
                        color = Turu.Muted,
                        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 4.dp),
                    )
                }
                DropdownMenuItem(
                    text = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(item.name, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                            val notes = listOf(item.enhancement, item.tag).filter { it.isNotEmpty() }.joinToString(" · ")
                            if (notes.isNotEmpty()) Text("  $notes", style = MaterialTheme.typography.labelSmall, color = Turu.Muted)
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
    if (row.suggestion.isNotEmpty()) {
        TextButton(onClick = { panel.set(group, row, row.suggestion) }, modifier = Modifier.heightIn(min = 48.dp)) {
            Text("Use ${row.suggestion} (${row.enhancement})", maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
    if (row.missing.isNotEmpty()) {
        Text(row.missing, style = MaterialTheme.typography.bodySmall, color = Turu.Warning, modifier = Modifier.padding(top = 4.dp))
    }
}

/** "cape" -> "capes", "class" -> "classes" (the model's `plural`). */
private fun plural(noun: String) = if (noun.endsWith("s")) "${noun}es" else "${noun}s"
