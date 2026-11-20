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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.AuraView
import com.muktiarafi.turu.models.RulesView
import com.muktiarafi.turu.models.SetView
import com.muktiarafi.turu.models.SkillsView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * Combat > Skills for the tab on screen (`turu_frontend::models::skills`): the editor and the
 * saved sets are the slot's, shared with its egui window, so Full window keeps what is being
 * built. Read only while the page shows.
 */
class SkillsPanel {
    var view by mutableStateOf<SkillsView?>(null)
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
            api, token, models = listOf("skills"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<SkillsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/skills doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("skills", "do", JSONObject().put("action", "open"))
        }
    }

    fun act(action: String, body: JSONObject = JSONObject()) {
        error = null
        client?.act("skills", "do", body.put("action", action)) { error = it }
    }

    fun editor(field: String, value: Any) = act("editor", JSONObject().put(field, value))

    fun rules(target: String, rules: RulesView) =
        act("rules", JSONObject().put("target", target).put("rules", JSONObject(json.encodeToString(RulesView.serializer(), rules))))
}

private enum class SkillsTab(val label: String) { Edit("Edit"), Saved("Saved sets") }

/** Which rules the rules page edits: the ones added skills get, or one skill's (Edit Rules). */
private enum class RulesTarget { New, Skill }

/** A slot's name: 0 is the auto attack, 5 the potion. */
private fun slotName(slot: String) = when (slot) {
    "0" -> "Auto attack"
    "5" -> "Potion"
    else -> "Skill $slot"
}

/** A sequence line's rules in words (`4 - [Health > 50%] - [Skip ...]`), or "No rules". */
private fun rulesWords(line: String) =
    line.substringAfter(" - ", "").replace("] - [", " · ").removePrefix("[").removeSuffix("]").ifEmpty { "No rules" }

/**
 * Combat > Skills (docs/UI.md, "Native shells"; VibeSkua's Advanced Skills): Edit builds a skill
 * sequence (each skill with its use rules) and saves it as a class's set in one mode; Saved sets
 * finds a class and opens or removes its sets. ⋮ has Refresh, Reset sets (asked first) and Full
 * window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SkillsPage(state: ShellState, panel: SkillsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var tab by rememberSaveable { mutableStateOf(SkillsTab.Edit) }
    var rulesFor by remember { mutableStateOf<RulesTarget?>(null) }
    var resetting by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<SetView?>(null) }
    // The Class field's text as typed: Save sends it, so a name typed just before isn't lost.
    var typedClass by remember { mutableStateOf<String?>(null) }

    // Edit Rules open in the slot (from here or its egui window): the rules page shows it.
    val editing = view?.editing
    val showRules = rulesFor == RulesTarget.New || (editing != null && rulesFor != RulesTarget.New)
    if (view != null && showRules) {
        val target = if (rulesFor == RulesTarget.New || editing == null) RulesTarget.New else RulesTarget.Skill
        val close = {
            if (target == RulesTarget.Skill) panel.act("cancel-rules")
            rulesFor = null
        }
        RulesPage(
            title = if (target == RulesTarget.New) "Rules for added skills" else "Skill ${editing!!.position.toInt() + 1}: rules",
            subtitle = if (target == RulesTarget.New) "Each skill you add gets a copy of these." else "${slotName(editing!!.skill.substringBefore(" "))}: ${rulesWords(editing.skill)}",
            key = if (target == RulesTarget.New) "new" else "skill-${editing!!.position}",
            rules = if (target == RulesTarget.New) view.rules else editing!!.rules,
            onChange = { panel.rules(if (target == RulesTarget.New) "new" else "editing", it) },
            onBack = close,
            onDone = {
                if (target == RulesTarget.Skill) panel.act("confirm-rules")
                rulesFor = null
            },
            confirms = target == RulesTarget.Skill,
        )
        return
    }

    Column(Modifier.fillMaxSize()) {
        PageBar("Skills", onBack = { state.back() }) {
            FullWindowMenu("The editor and the saved list side by side, as VibeSkua's", fullWindow) { close ->
                DropdownMenuItem(text = { Text("Refresh saved sets") }, onClick = { close(); panel.act("refresh") })
                DropdownMenuItem(text = { Text("Reset sets…", color = Turu.Danger) }, onClick = { close(); resetting = true })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        // With the keyboard up the panel has room for the field alone (landscape): the tabs and
        // the footer step aside until it goes.
        val typing = WindowInsets.isImeVisible
        if (!typing) SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) {
            SkillsTab.entries.forEachIndexed { i, t ->
                SegmentedButton(
                    selected = tab == t,
                    onClick = { tab = t },
                    shape = SegmentedButtonDefaults.itemShape(i, SkillsTab.entries.size),
                    colors = segmentColors(),
                ) { Text(t.label) }
            }
        }
        when (tab) {
            SkillsTab.Edit -> EditTab(view, panel, Modifier.weight(1f), onTyping = { typedClass = it }) { rulesFor = RulesTarget.New }
            SkillsTab.Saved -> SavedTab(view, panel, Modifier.weight(1f), onRemove = { removing = it }) { tab = SkillsTab.Edit }
        }
        if (typing) return@Column
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        val note = panel.error?.let { it to true } ?: view.outcome?.let { it.text to it.failed }
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                note?.first ?: when {
                    tab != SkillsTab.Edit -> ""
                    !view.save.enabled -> view.save.reason
                    view.replaces -> "Saving replaces ${view.className.trim()}'s ${view.mode} set"
                    else -> "Save adds ${view.className.trim()}'s ${view.mode} set"
                },
                style = MaterialTheme.typography.bodySmall,
                color = when {
                    note == null -> Turu.Muted
                    note.second -> Turu.Danger
                    else -> Turu.Success
                },
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (tab == SkillsTab.Edit) {
                Spacer(Modifier.width(12.dp))
                Button(onClick = { panel.act("save", JSONObject().apply { typedClass?.let { put("className", it) } }) }, enabled = view.save.enabled || typedClass?.isNotBlank() == true, modifier = Modifier.heightIn(min = 48.dp)) { Text("Save set") }
            }
        }
    }

    if (resetting) {
        AlertDialog(
            onDismissRequest = { resetting = false },
            title = { Text("Reset skill sets?") },
            text = { Text("Every saved set is replaced with the ones Turu ships. Sets you made or changed are lost.") },
            confirmButton = {
                TextButton(onClick = { resetting = false; panel.act("reset-sets") }) { Text("Reset", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { resetting = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    removing?.let { set ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text("Remove this set?") },
            text = { Text("${set.`class`} in ${set.mode}. Scripts that ask for it use the class's other sets instead, or 0 | 1 | 2 | 3 | 4 when none is left.") },
            confirmButton = {
                TextButton(onClick = { removing = null; panel.act("remove-set", JSONObject().put("index", set.position.toLong())) }) { Text("Remove", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

@Composable
private fun segmentColors() = SegmentedButtonDefaults.colors(
    activeContainerColor = Turu.AccentTint,
    activeContentColor = Turu.Text,
    inactiveContainerColor = Color.Transparent,
    inactiveContentColor = Turu.Muted,
    activeBorderColor = Turu.Edge,
    inactiveBorderColor = Turu.Edge,
)

/** A small heading over a group of rows. */
@Composable
private fun GroupLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp))
}

/** The editor: class and mode, the sequence, the slot buttons, how skills are used. */
@Composable
private fun EditTab(view: SkillsView, panel: SkillsPanel, modifier: Modifier, onTyping: (String) -> Unit, openNewRules: () -> Unit) {
    LazyColumn(modifier) {
        item {
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                DebouncedText(
                    value = view.className,
                    label = "Class",
                    placeholder = "Void Highlord",
                    modifier = Modifier.weight(1f),
                    onTyping = onTyping,
                ) { panel.editor("className", it) }
                Spacer(Modifier.width(8.dp))
                Box(Modifier.width(132.dp)) {
                    Picker(label = "Mode", value = view.mode, options = view.modes.map { it to it }, loading = false) { panel.editor("mode", it) }
                }
            }
        }
        item { GroupLabel("Sequence · used top to bottom, then again") }
        if (view.sequence.isEmpty()) {
            item {
                Text(
                    "No skills yet. Add them below in the order the bot should use them.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Turu.Muted,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
        }
        itemsIndexed(view.sequence) { i, line -> SkillRow(i, line, view.sequence.size, panel) }
        item {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                for (slot in 0..5) {
                    FilledTonalButton(
                        onClick = { panel.act("add", JSONObject().put("slot", slot)) },
                        contentPadding = ButtonDefaults.TextButtonContentPadding,
                        modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                    ) { Text("$slot", style = MaterialTheme.typography.titleSmall) }
                }
            }
            Text(
                "Add a skill: 0 is the auto attack, 5 a potion.",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .clickable(onClick = openNewRules)
                    .heightIn(min = 56.dp)
                    .padding(horizontal = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text("Rules for added skills")
                    Text(
                        if (view.rules.useRule) "On: added skills get these rules" else "Off: added skills are used whenever their turn comes",
                        style = MaterialTheme.typography.bodySmall,
                        color = if (view.rules.useRule) Turu.Accent else Turu.Muted,
                    )
                }
                Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Muted)
            }
        }
        item { GroupLabel("How skills are used") }
        item {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
                listOf(false to "Use if available", true to "Wait for cooldown").forEachIndexed { i, (wait, label) ->
                    SegmentedButton(
                        selected = view.waitForCooldown == wait,
                        onClick = { panel.editor("waitForCooldown", wait) },
                        shape = SegmentedButtonDefaults.itemShape(i, 2),
                        colors = segmentColors(),
                    ) { Text(label, maxLines = 1) }
                }
            }
            Text(
                if (view.waitForCooldown) "Waits for each skill to cool down, up to the timeout." else "A skill still cooling down is skipped.",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
            LabeledRow("Skill timeout", "Longest wait for one skill") {
                NumberBox(view.timeout, 0, 5000, "ms") { panel.editor("timeout", it) }
            }
            SwitchRow("Reset on target change", "Start the sequence again when the target changes or is lost", view.resetOnTarget) {
                panel.editor("resetOnTarget", it)
            }
            Spacer(Modifier.heightIn(min = 8.dp).padding(bottom = 8.dp))
        }
    }
}

/** A skill of the sequence: its slot, its rules in words; a tap opens its rules, ⋮ the rest. */
@Composable
private fun SkillRow(index: Int, line: String, count: Int, panel: SkillsPanel) {
    var menu by remember { mutableStateOf(false) }
    val slot = line.substringBefore(" ")
    val rules = rulesWords(line)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .clickable { panel.act("edit-rules", JSONObject().put("index", index)) }
            .heightIn(min = 56.dp)
            .padding(start = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("${index + 1}", style = MaterialTheme.typography.labelMedium, color = Turu.Muted, modifier = Modifier.widthIn(min = 20.dp))
        Box(
            Modifier.size(32.dp).clip(CircleShape).background(Turu.AccentTint),
            contentAlignment = Alignment.Center,
        ) { Text(slot, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold) }
        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
            Text(slotName(slot))
            Text(
                rules,
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Box {
            IconButton(onClick = { menu = true }) {
                Icon(Icons.Rounded.MoreVert, contentDescription = "Skill ${index + 1}: rules, move, remove", tint = Turu.Muted)
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                val at = JSONObject().put("index", index)
                DropdownMenuItem(text = { Text("Edit rules") }, onClick = { menu = false; panel.act("edit-rules", at) })
                DropdownMenuItem(text = { Text("Move up") }, enabled = index > 0, onClick = { menu = false; panel.act("move", at.put("down", false)) })
                DropdownMenuItem(text = { Text("Move down") }, enabled = index + 1 < count, onClick = { menu = false; panel.act("move", at.put("down", true)) })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                DropdownMenuItem(text = { Text("Remove", color = Turu.Danger) }, onClick = { menu = false; panel.act("remove", at) })
            }
        }
    }
}

/** The saved sets: find a class, then its sets; a tap opens one in the editor. */
@Composable
private fun SavedTab(view: SkillsView, panel: SkillsPanel, modifier: Modifier, onRemove: (SetView) -> Unit, opened: () -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    Column(modifier) {
        if (view.`class`.isEmpty()) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Find a class") },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
                trailingIcon = {
                    if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") }
                },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
            )
            val shown = view.classes.filter { it.contains(query.trim(), ignoreCase = true) }
            LazyColumn(Modifier.weight(1f)) {
                item {
                    Text(
                        if (query.isBlank()) "${view.classes.size} classes have saved sets" else "${shown.size} of ${view.classes.size} classes",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    )
                }
                if (shown.isEmpty()) {
                    item {
                        Text(
                            if (view.classes.isEmpty()) "No saved sets. Save one from Edit, or Reset sets in ⋮ for the shipped ones." else "No class matches \"${query.trim()}\". Save a set for it from Edit.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Turu.Muted,
                            modifier = Modifier.padding(16.dp),
                        )
                    }
                }
                items(shown) { name ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp)
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { panel.act("class", JSONObject().put("class", name)) }
                            .heightIn(min = 48.dp)
                            .padding(horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(name, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Muted)
                    }
                }
            }
        } else {
            Row(Modifier.fillMaxWidth().padding(end = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = { panel.act("class", JSONObject().put("class", "")) }) {
                    Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = "All classes")
                }
                Column(Modifier.weight(1f)) {
                    Text(view.`class`, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text("${view.sets.size} saved ${if (view.sets.size == 1) "set" else "sets"} · tap one to edit it", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
            }
            LazyColumn(Modifier.weight(1f)) {
                items(view.sets, key = { it.position.toLong() }) { set ->
                    SetRow(set, onOpen = { panel.act("open-set", JSONObject().put("index", set.position.toLong())); opened() }, onRemove = { onRemove(set) })
                }
            }
        }
    }
}

/** A saved set: its mode, how it uses skills, the sequence. */
@Composable
private fun SetRow(set: SetView, onOpen: () -> Unit, onRemove: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onOpen)
            .heightIn(min = 64.dp)
            .padding(start = 12.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    set.mode,
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.clip(RoundedCornerShape(50)).background(Turu.AccentTint).padding(horizontal = 10.dp, vertical = 2.dp),
                )
                Text(
                    set.useMode + if (set.resetOnTarget) " · resets on target" else "",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(set.sequence, style = MaterialTheme.typography.bodyMedium, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "${set.mode} set: open, remove", tint = Turu.Muted) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                DropdownMenuItem(text = { Text("Open in the editor") }, onClick = { menu = false; onOpen() })
                DropdownMenuItem(text = { Text("Remove…", color = Turu.Danger) }, onClick = { menu = false; onRemove() })
            }
        }
    }
}

/**
 * A skill's use rules, pushed over the page: health, mana and party HP checks, an aura check (or
 * several), the wait before use and Skip if not available. The page keeps its own copy and sends
 * it whole on each change; `confirms` (Edit Rules) has Done keep it and Back drop it, the added
 * skills' rules apply as they change.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RulesPage(
    title: String,
    subtitle: String,
    key: String,
    rules: RulesView,
    onChange: (RulesView) -> Unit,
    onBack: () -> Unit,
    onDone: () -> Unit,
    confirms: Boolean,
) {
    var r by remember(key) { mutableStateOf(rules) }
    // Each change shows at once; the slot gets the copy a moment after the last one, and at once
    // before Done or Back, so nothing typed is lost or arrives after Edit Rules closed.
    var sent by remember(key) { mutableStateOf(rules) }
    val set = { next: RulesView -> r = next }
    val flush = { if (r != sent) { sent = r; onChange(r) } }
    LaunchedEffect(r) {
        if (r != sent) {
            delay(300)
            flush()
        }
    }
    val back = { flush(); onBack() }
    BackHandler { back() }
    Column(Modifier.fillMaxSize()) {
        PageBar(title, onBack = back) {
            TextButton(onClick = { set(defaultRules()) }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Reset") }
            Button(onClick = { flush(); onDone() }, modifier = Modifier.padding(end = 8.dp).heightIn(min = 40.dp)) { Text(if (confirms) "Done" else "Close") }
        }
        Text(
            subtitle,
            style = MaterialTheme.typography.bodySmall,
            color = Turu.Muted,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
        )
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        LazyColumn(Modifier.weight(1f)) {
            item {
                SwitchRow("Use rules", if (r.useRule) "The skill is used only when these hold" else "Off: the skill is used whenever its turn comes", r.useRule) {
                    set(r.copy(useRule = it))
                }
            }
            item {
                Column(Modifier.alpha(if (r.useRule) 1f else 0.38f)) {
                    GroupLabel("Checks · 0 means no check")
                    AmountRow("Health", r.healthGreater, r.health, r.healthPercent, r.useRule) { g, v, p -> set(r.copy(healthGreater = g, health = v, healthPercent = p)) }
                    AmountRow("Mana", r.manaGreater, r.mana, r.manaPercent, r.useRule) { g, v, p -> set(r.copy(manaGreater = g, mana = v, manaPercent = p)) }
                    AmountRow("Party HP", r.partyHealthGreater, r.partyHealth, r.partyHealthPercent, r.useRule) { g, v, p ->
                        set(r.copy(partyHealthGreater = g, partyHealth = v, partyHealthPercent = p))
                    }
                    GroupLabel("Aura")
                    SwitchRow("Several auras", "Check more than one aura", r.multiAura, enabled = r.useRule) { set(r.copy(multiAura = it)) }
                    if (!r.multiAura) {
                        AuraEditor(
                            AuraView(r.auraName, r.auraStacks, r.auraGreater, r.auraTarget),
                            enabled = r.useRule,
                            onRemove = null,
                        ) { a -> set(r.copy(auraName = a.name, auraStacks = a.stacks, auraGreater = a.greater, auraTarget = a.target)) }
                    } else {
                        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text("Holds when", modifier = Modifier.weight(1f))
                            Toggle(listOf("All of them", "Any of them"), if (r.aurasOr) 1 else 0, r.useRule, Modifier.width(220.dp)) { set(r.copy(aurasOr = it == 1)) }
                        }
                        r.auras.forEachIndexed { i, aura ->
                            AuraEditor(aura, enabled = r.useRule, onRemove = {
                                // As the slot does: the last aura removed turns Several auras off.
                                val left = r.auras.filterIndexed { j, _ -> j != i }
                                set(r.copy(auras = left, multiAura = left.isNotEmpty()))
                            }) { a ->
                                set(r.copy(auras = r.auras.mapIndexed { j, old -> if (j == i) a else old }))
                            }
                        }
                        TextButton(
                            onClick = { set(r.copy(auras = r.auras + AuraView("", 0.0, true, false))) },
                            enabled = r.useRule,
                            modifier = Modifier.padding(horizontal = 8.dp).heightIn(min = 48.dp),
                        ) { Text("Add aura") }
                    }
                    GroupLabel("Timing")
                    LabeledRow("Wait before use", "Pause this long first") {
                        NumberBox(r.wait, 0, 5000, "ms", r.useRule) { set(r.copy(wait = it)) }
                    }
                    SwitchRow("Skip if not available", "A check that fails moves on to the next skill", r.skip, enabled = r.useRule) { set(r.copy(skip = it)) }
                    Spacer(Modifier.padding(bottom = 16.dp))
                }
            }
        }
    }
}

/** The rules VibeSkua's editor starts from (`SkillRules::default`). */
private fun defaultRules() = RulesView(
    useRule = false, multiAura = false,
    healthGreater = true, health = 0, healthPercent = true,
    manaGreater = true, mana = 0, manaPercent = true,
    partyHealthGreater = true, partyHealth = 0, partyHealthPercent = true,
    wait = 0, skip = false,
    auraGreater = true, auraStacks = 0.0, auraTarget = false, auraName = "",
    auras = emptyList(), aurasOr = false,
)

/** A health, mana or party HP check: at least or at most, the amount, % or a flat value. */
@Composable
private fun AmountRow(label: String, greater: Boolean, value: Long, percent: Boolean, enabled: Boolean, onSet: (Boolean, Long, Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, modifier = Modifier.width(72.dp))
        Toggle(listOf("≥", "≤"), if (greater) 0 else 1, enabled, Modifier.width(96.dp), descriptions = listOf("At least", "At most")) { onSet(it == 0, value, percent) }
        Spacer(Modifier.width(8.dp))
        NumberBox(value, 0, if (percent) 100 else Int.MAX_VALUE.toLong(), null, enabled, Modifier.weight(1f)) { onSet(greater, it, percent) }
        Spacer(Modifier.width(8.dp))
        Toggle(listOf("%", "#"), if (percent) 0 else 1, enabled, Modifier.width(96.dp), descriptions = listOf("Percent", "Flat value")) {
            val toPercent = it == 0
            onSet(greater, if (toPercent) value.coerceAtMost(100) else value, toPercent)
        }
    }
}

/** One aura check: its name, stacks, at least or at most, on yourself or the target. */
@Composable
private fun AuraEditor(aura: AuraView, enabled: Boolean, onRemove: (() -> Unit)?, onSet: (AuraView) -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 4.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(Turu.Card)
            .padding(horizontal = 8.dp, vertical = 6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = aura.name,
                onValueChange = { onSet(aura.copy(name = it)) },
                label = { Text("Aura") },
                placeholder = { Text("Focus") },
                singleLine = true,
                enabled = enabled,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                modifier = Modifier.weight(1f),
            )
            if (onRemove != null) {
                IconButton(onClick = onRemove, enabled = enabled) { Icon(Icons.Rounded.Close, contentDescription = "Remove this aura", tint = Turu.Muted) }
            }
        }
        Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Toggle(listOf("≥", "≤"), if (aura.greater) 0 else 1, enabled, Modifier.width(96.dp), descriptions = listOf("At least", "At most")) { onSet(aura.copy(greater = it == 0)) }
            Spacer(Modifier.width(8.dp))
            StacksBox(aura.stacks, enabled, Modifier.width(96.dp)) { onSet(aura.copy(stacks = it)) }
            Spacer(Modifier.width(8.dp))
            Toggle(listOf("Self", "Target"), if (aura.target) 1 else 0, enabled, Modifier.weight(1f)) { onSet(aura.copy(target = it == 1)) }
        }
    }
}

/** A two-way choice as a segmented button row. */
@Composable
private fun Toggle(labels: List<String>, selected: Int, enabled: Boolean, modifier: Modifier, descriptions: List<String>? = null, onPick: (Int) -> Unit) {
    SingleChoiceSegmentedButtonRow(modifier) {
        labels.forEachIndexed { i, label ->
            SegmentedButton(
                selected = selected == i,
                onClick = { if (selected != i) onPick(i) },
                enabled = enabled,
                shape = SegmentedButtonDefaults.itemShape(i, labels.size),
                colors = segmentColors(),
                icon = {},
                modifier = Modifier.heightIn(min = 48.dp),
            ) {
                Text(
                    label,
                    maxLines = 1,
                    modifier = descriptions?.let { d -> Modifier.semantics { contentDescription = d[i] } } ?: Modifier,
                )
            }
        }
    }
}

/** A label with a line under it, and a control at the end. */
@Composable
private fun LabeledRow(label: String, detail: String, control: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(label)
            Text(detail, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
        }
        Spacer(Modifier.width(12.dp))
        Box(Modifier.width(132.dp)) { control() }
    }
}

/** A setting with its switch; the whole row toggles it. */
@Composable
private fun SwitchRow(label: String, detail: String, checked: Boolean, enabled: Boolean = true, onSet: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled) { onSet(!checked) }
            .heightIn(min = 56.dp)
            .padding(horizontal = 16.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label)
            Text(detail, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
        }
        Spacer(Modifier.width(12.dp))
        Switch(checked = checked, onCheckedChange = onSet, enabled = enabled)
    }
}

/** A whole number in `min`..`max`, sent as soon as it reads as one. */
@Composable
private fun NumberBox(value: Long, min: Long, max: Long, unit: String?, enabled: Boolean = true, modifier: Modifier = Modifier.fillMaxWidth(), onSet: (Long) -> Unit) {
    var text by remember(value) { mutableStateOf(value.toString()) }
    OutlinedTextField(
        value = text,
        onValueChange = { next ->
            text = next.filter { it.isDigit() }.take(10)
            text.toLongOrNull()?.coerceIn(min, max)?.takeIf { it != value }?.let(onSet)
        },
        suffix = unit?.let { { Text(it, color = Turu.Muted) } },
        singleLine = true,
        enabled = enabled,
        shape = RoundedCornerShape(12.dp),
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
        modifier = modifier,
    )
}

/** An aura's stacks: a number, decimals allowed. */
@Composable
private fun StacksBox(value: Double, enabled: Boolean, modifier: Modifier, onSet: (Double) -> Unit) {
    fun shown(v: Double) = if (v == Math.floor(v)) v.toLong().toString() else v.toString()
    var text by remember(value) { mutableStateOf(shown(value)) }
    OutlinedTextField(
        value = text,
        onValueChange = { next ->
            text = next.filter { it.isDigit() || it == '.' }.take(8)
            text.toDoubleOrNull()?.takeIf { it != value }?.let(onSet)
        },
        label = { Text("Stacks") },
        singleLine = true,
        enabled = enabled,
        shape = RoundedCornerShape(12.dp),
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
        modifier = modifier,
    )
}

/** Text, sent a moment after the typing stops. */
@Composable
private fun DebouncedText(
    value: String,
    label: String,
    placeholder: String,
    modifier: Modifier,
    enabled: Boolean = true,
    onTyping: (String) -> Unit = {},
    onSet: (String) -> Unit,
) {
    var text by remember { mutableStateOf(value) }
    // A change from elsewhere (a set opened, the egui window) shows unless this is mid-typing.
    var sent by remember { mutableStateOf(value) }
    LaunchedEffect(value) {
        if (value != sent) {
            text = value
            sent = value
        }
    }
    LaunchedEffect(text) {
        if (text != sent) {
            delay(400)
            sent = text
            onSet(text)
        }
    }
    // What the field holds, whenever it composes: Save sends it, so it never names a class the
    // field doesn't show. Leaving (another tab, the rules page) sends what wasn't sent yet.
    SideEffect { onTyping(text) }
    DisposableEffect(Unit) { onDispose { if (text != sent) onSet(text) } }
    OutlinedTextField(
        value = text,
        onValueChange = { text = it },
        label = { Text(label) },
        placeholder = { Text(placeholder) },
        singleLine = true,
        enabled = enabled,
        shape = RoundedCornerShape(12.dp),
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        modifier = modifier,
    )
}
