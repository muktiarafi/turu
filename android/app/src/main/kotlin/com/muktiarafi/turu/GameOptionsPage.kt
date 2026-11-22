package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextObfuscationMode
import androidx.compose.foundation.text.input.rememberTextFieldState
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
import androidx.compose.material3.OutlinedSecureTextField
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.GameOptionsAct
import com.muktiarafi.turu.models.GameOptionsView
import com.muktiarafi.turu.models.KindView
import com.muktiarafi.turu.models.OptionValue
import com.muktiarafi.turu.models.OptionView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json

/**
 * Game Options' copy of the tab on screen (`turu_frontend::models::game_options`), read only while
 * the page shows, and what was changed on this page ("Edited" against "Changed outside", most
 * often by a script): a window's own state, as the egui window keeps it.
 */
class GameOptionsPanel {
    var view by mutableStateOf<GameOptionsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set
    var saved by mutableStateOf(false)
        private set

    /** The options changed here since the last Save or Revert (by label). */
    val edited = mutableStateListOf<String>()

    /** The switch whose warning is up (Private Rooms going off): it stays on unless confirmed. */
    var askOff by mutableStateOf<OptionView?>(null)
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
        edited.clear()
        // A question about the tab that was on screen doesn't carry over to another.
        askOff = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("game-options"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<GameOptionsView>(text) }
                    .onSuccess { next ->
                        view = next
                        // Changed here, then back (or saved elsewhere): no longer an edit of ours.
                        edited.retainAll { label -> next.options.any { it.label == label && it.differs } }
                        if (next.unsaved.toLong() > 0) saved = false
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/game-options doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    private fun act(action: GameOptionsAct, done: (String?) -> Unit = {}) {
        error = null
        val client = client ?: return done("The tab isn't up yet")
        client.act("game-options", action, GameOptionsAct.JsonSerializer) { failed ->
            error = failed
            done(failed)
        }
    }

    fun set(option: OptionView, value: Any) {
        // Turu: asks before Private Rooms goes off, as the egui window does (PublicRoomsDialog).
        if (option.label == PUBLIC_ROOMS_OPTION && value == false) {
            askOff = option
            return
        }
        if (option.label !in edited) edited.add(option.label)
        act(GameOptionsAct.set(option.label, optionValue(value)))
    }

    /** The warning's answer: [confirmed] turns the option off, otherwise it stays on. */
    fun answer(confirmed: Boolean) {
        val option = askOff ?: return
        askOff = null
        if (confirmed) {
            if (option.label !in edited) edited.add(option.label)
            act(GameOptionsAct.set(option.label, OptionValue.switch(false)))
        }
    }

    fun reload(option: OptionView) = act(GameOptionsAct.set(option.label, OptionValue.none))

    fun save() = act(GameOptionsAct.save) { if (it == null) { edited.clear(); saved = true } }

    fun revert() = act(GameOptionsAct.revert) { if (it == null) edited.clear() }

    fun reset() {
        view?.options?.filter { it.differs }?.forEach { if (it.label !in edited) edited.add(it.label) }
        act(GameOptionsAct.reset)
    }
}

/** The option that asks before it goes off ([PublicRoomsDialog]). */
private const val PUBLIC_ROOMS_OPTION = "Private Rooms"

/**
 * The public room question, the same in Game Options (Private Rooms going off) and CoreBots
 * (Save with Private Rooms off), in `game_options::PUBLIC_ROOMS`'s words, as the egui windows
 * ask it (`warning_modal`). [onDismiss]: Back or a tap outside, no answer.
 */
@Composable
fun PublicRoomsDialog(onConfirm: () -> Unit, onKeep: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Bot in public rooms?") },
        text = {
            Text(
                "Other players in a public room can see the bot and report your account, and a report can get it " +
                    "banned. Private rooms keep strangers out.",
            )
        },
        confirmButton = { TextButton(onClick = onConfirm) { Text("Use public rooms", color = Turu.Danger) } },
        dismissButton = { TextButton(onClick = onKeep) { Text("Keep private") } },
        containerColor = Turu.Card,
    )
}

/** What an option's editor gives (a switch's Boolean, a number, a text or a choice's key), to set. */
internal fun optionValue(value: Any?): OptionValue = when (value) {
    is Boolean -> OptionValue.switch(value)
    is Number -> OptionValue.number(value.toLong())
    null -> OptionValue.none
    else -> OptionValue.text(value.toString())
}

private fun OptionValue.bool() = (this as? OptionValue.switch)?.value ?: false
private fun OptionValue.long() = (this as? OptionValue.number)?.value ?: 0L
private fun OptionValue.string() = (this as? OptionValue.text)?.value ?: ""

/**
 * Options > Game (docs/UI.md, "Native shells"; VibeSkua's Game Options): the search and the
 * sections as chips (and Changed while anything differs from options.json), the options under
 * their section's heading with their control, "Edited" or "Changed outside" on the ones that
 * differ; Defaults, Revert and Save at the foot.
 */
@Composable
fun GameOptionsPage(state: ShellState, options: GameOptionsPanel, running: Boolean, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { options.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { options.follow(state.tabs.active) }
    val view = options.view
    var search by remember { mutableStateOf("") }
    var section by remember { mutableStateOf<String?>(null) }
    var changedOnly by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Game Options", onBack = { state.back() }) {
            FullWindowMenu("Revert mine, Relogin Server's list", fullWindow)
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        if (view.unsaved.toLong() == 0L) changedOnly = false
        OutlinedTextField(
            value = search,
            onValueChange = { search = it },
            placeholder = { Text("Search game options") },
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
            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
        )
        val needle = search.trim().lowercase()
        val found = view.options.filter { o ->
            (needle.isEmpty() || o.label.lowercase().contains(needle) || o.description.lowercase().contains(needle)) &&
                (!changedOnly || o.differs)
        }
        LazyRow(
            Modifier.padding(top = 8.dp),
            contentPadding = PaddingValues(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item { SectionChip("All ${found.size}", section == null) { section = null } }
            items(view.sections) { s ->
                val count = found.count { it.section == s.chip }
                SectionChip("${s.chip} $count", section == s.chip) { section = if (section == s.chip) null else s.chip }
            }
            if (view.unsaved.toLong() > 0) {
                item { SectionChip("Changed ${view.unsaved}", changedOnly) { changedOnly = !changedOnly } }
            }
        }
        val shown = found.filter { section == null || it.section == section }
        LazyColumn(Modifier.weight(1f).padding(top = 4.dp)) {
            for (w in view.warnings) {
                item { Text(w, style = MaterialTheme.typography.bodySmall, color = Turu.Warning, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) }
            }
            if (shown.isEmpty()) {
                item {
                    Column(Modifier.padding(20.dp)) {
                        Text("No game options match", style = MaterialTheme.typography.titleSmall)
                        TextButton(onClick = { search = ""; section = null; changedOnly = false }) { Text("Show them all") }
                    }
                }
            }
            for (s in view.sections) {
                val rows = shown.filter { it.section == s.chip }
                if (rows.isEmpty()) continue
                item(key = "title " + s.chip) {
                    Text(
                        s.title,
                        style = MaterialTheme.typography.labelLarge,
                        color = Turu.Accent,
                        modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 4.dp),
                    )
                }
                var group = ""
                for (o in rows) {
                    if (o.group != group && o.group.isNotEmpty()) {
                        val heading = o.group
                        item(key = "group " + s.chip + heading) {
                            Text(heading, style = MaterialTheme.typography.labelMedium, color = Turu.Muted, modifier = Modifier.padding(start = 16.dp, top = 8.dp))
                        }
                    }
                    group = o.group
                    item(key = o.label) {
                        OptionRow(
                            o,
                            edited = o.label in options.edited,
                            inGame = view.inGame,
                            onSet = { options.set(o, it) },
                            onAction = { options.reload(o) },
                        )
                    }
                }
            }
        }
        Footer(view, options, running)
    }
    if (options.askOff != null) {
        // A tap outside keeps the option as it is, as Keep private does.
        PublicRoomsDialog(
            onConfirm = { options.answer(true) },
            onKeep = { options.answer(false) },
            onDismiss = { options.answer(false) },
        )
    }
}

@Composable
private fun SectionChip(label: String, selected: Boolean, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label, maxLines = 1) },
        colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
        border = FilterChipDefaults.filterChipBorder(enabled = true, selected = selected, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
        modifier = Modifier.heightIn(min = 40.dp),
    )
}

/** One option: its label (and why it differs), its description, its control. */
/**
 * One option (both options pages): its label (why it differs, "Restart required"), description
 * and control. `edited`: changed on this page; `onAction` for a button's press. `enabled` off:
 * dimmed and untouchable for now (logged out, say), unlike `unused`, which Turu doesn't read at all.
 */
@Composable
fun OptionRow(
    o: OptionView,
    edited: Boolean,
    inGame: Boolean,
    onSet: (Any) -> Unit,
    onAction: () -> Unit,
    fullWindow: () -> Unit = {},
    enabled: Boolean = true,
) {
    val dim = !o.applies || o.unused || !enabled
    val live = enabled && !o.unused
    Column(
        Modifier
            .fillMaxWidth()
            .padding(start = if (o.applies) 16.dp else 32.dp, end = 12.dp, top = 6.dp, bottom = 6.dp)
            .alpha(if (dim) 0.55f else 1f),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(o.label, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    if (o.differs) {
                        val mine = edited
                        val color = if (mine) Turu.Accent else Turu.Warning
                        Spacer(Modifier.width(6.dp))
                        // Outlined, not filled: on a tinted pill the accent text would be under 4.5:1.
                        Text(
                            if (mine) "Edited" else "Changed outside",
                            style = MaterialTheme.typography.labelSmall,
                            color = color,
                            modifier = Modifier
                                .border(1.dp, color.copy(alpha = 0.6f), MaterialTheme.shapes.small)
                                .padding(horizontal = 6.dp, vertical = 1.dp),
                        )
                    }
                }
                if (o.restart) {
                    Text("Restart required", style = MaterialTheme.typography.labelSmall, color = Turu.Muted)
                }
                val about = when {
                    o.kind is KindView.elsewhere -> "In the full window. ${o.description}".trim()
                    o.unused && o.kind is KindView.switch -> "Coming soon: Turu doesn't use this yet"
                    else -> o.description
                }
                if (about.isNotEmpty()) Text(
                    when {
                        o.kind is KindView.elsewhere -> "In the full window. ${o.description}".trim()
                        o.unused && o.kind is KindView.switch -> "Coming soon: Turu doesn't use this yet"
                        else -> o.description
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                )
            }
            val kind = o.kind
            // A gap between a long description and its control.
            if (kind !is KindView.text && kind !is KindView.secret && kind !is KindView.server) Spacer(Modifier.width(12.dp))
            when (kind) {
                is KindView.switch -> Switch(checked = o.value.bool(), enabled = live, onCheckedChange = { onSet(it) })
                is KindView.action -> OutlinedButton(
                    onClick = onAction,
                    enabled = (inGame || !kind.login) && live && o.applies,
                    modifier = Modifier.heightIn(min = 40.dp),
                ) { Text(kind.label) }
                is KindView.elsewhere -> TextButton(onClick = fullWindow, modifier = Modifier.heightIn(min = 40.dp)) { Text("Open") }
                is KindView.choice -> Box(Modifier.width(150.dp)) {
                    Picker(label = "", value = o.value.string(), options = kind.choices.map { it.first to it.second }, loading = false, enabled = live) { onSet(it) }
                }
                is KindView.number -> NumberField(o.value.long(), kind.min.toLong(), kind.max?.toLong() ?: Long.MAX_VALUE, kind.unit, enabled = live) { onSet(it) }
                is KindView.fps -> Row(verticalAlignment = Alignment.CenterVertically) {
                    NumberField(o.value.long(), 0, kind.max.toLong(), if (o.value.long() == 0L) "Off" else "FPS", enabled = enabled) { onSet(it) }
                    FpsPresets(kind.presets) { onSet(it) }
                }
                else -> {}
            }
        }
        when (val kind = o.kind) {
            is KindView.text -> TextOption(o.value.string(), kind.hint, secret = false, enabled = enabled) { onSet(it) }
            is KindView.secret -> TextOption(o.value.string(), "", secret = true, enabled = enabled) { onSet(it) }
            is KindView.server -> TextOption(o.value.string(), "A server's name", secret = false, enabled = enabled) { onSet(it) }
            else -> {}
        }
    }
}

/** A number, sent once it reads as one in range (`min`..`max`). */
@Composable
private fun NumberField(value: Long, min: Long, max: Long, unit: String?, enabled: Boolean, onSet: (Long) -> Unit) {
    var text by remember(value) { mutableStateOf(value.toString()) }
    OutlinedTextField(
        value = text,
        onValueChange = { next ->
            text = next.filter { it.isDigit() }
            text.toLongOrNull()?.takeIf { it in min..max && it != value }?.let(onSet)
        },
        suffix = unit?.let { { Text(it, color = Turu.Muted) } },
        singleLine = true,
        enabled = enabled,
        shape = MaterialTheme.shapes.medium,
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
        modifier = Modifier.width(120.dp),
    )
}

@Composable
private fun FpsPresets(presets: List<Int>, onSet: (Long) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(Icons.Rounded.ArrowDropDown, contentDescription = "The usual frame rates") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            for (fps in presets) {
                DropdownMenuItem(text = { Text(if (fps == 0) "Off" else "$fps FPS") }, onClick = { open = false; onSet(fps.toLong()) })
            }
        }
    }
}

/** Text, sent a moment after the typing stops. */
@Composable
private fun TextOption(value: String, hint: String, secret: Boolean, enabled: Boolean = true, onSet: (String) -> Unit) {
    val field = rememberTextFieldState(value)
    val text = field.text.toString()
    LaunchedEffect(text) {
        if (text != value) {
            delay(400)
            onSet(text)
        }
    }
    val modifier = Modifier.fillMaxWidth().padding(top = 6.dp)
    if (secret) {
        OutlinedSecureTextField(
            state = field,
            enabled = enabled,
            placeholder = { Text(hint) },
            textObfuscationMode = TextObfuscationMode.Hidden,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            modifier = modifier,
        )
    } else {
        OutlinedTextField(
            state = field,
            enabled = enabled,
            placeholder = { Text(hint) },
            lineLimits = TextFieldLineLimits.SingleLine,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            modifier = modifier,
        )
    }
}

/** The status line, then Defaults (asks first), Revert (asks first while a script runs) and Save. */
@Composable
private fun Footer(view: GameOptionsView, options: GameOptionsPanel, running: Boolean) {
    var ask by remember { mutableStateOf<String?>(null) }
    val unsaved = view.unsaved.toLong()
    val mine = options.edited.size
    val outside = (unsaved - mine).coerceAtLeast(0)
    val (status, color) = when {
        options.error != null -> "Couldn't do that: ${options.error}" to Turu.Danger
        unsaved > 0 && outside > 0 -> "$outside changed outside this page (a script?): Save keeps them too" to Turu.Warning
        unsaved > 0 -> "$unsaved changed here: they apply now, Save keeps them" to Turu.Warning
        options.saved -> "Saved to options.json" to Turu.Success
        else -> "Changes apply now; Save keeps them for next time" to Turu.Muted
    }
    Column(Modifier.fillMaxWidth()) {
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Text(status, style = MaterialTheme.typography.bodySmall, color = color, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 6.dp))
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { ask = "defaults" }, modifier = Modifier.heightIn(min = 44.dp)) { Text("Defaults") }
            TextButton(
                onClick = { if (running && outside > 0) ask = "revert" else options.revert() },
                enabled = unsaved > 0,
                modifier = Modifier.heightIn(min = 44.dp),
            ) { Text("Revert") }
            Spacer(Modifier.weight(1f))
            Button(onClick = { options.save() }, enabled = unsaved > 0, modifier = Modifier.heightIn(min = 44.dp)) { Text("Save") }
        }
    }
    when (ask) {
        "defaults" -> AlertDialog(
            onDismissRequest = { ask = null },
            title = { Text("Back to the defaults?") },
            text = { Text("Every game option goes back to its default now, also under a running script. Save keeps it.") },
            confirmButton = { TextButton(onClick = { ask = null; options.reset() }) { Text("Reset", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { ask = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
        "revert" -> AlertDialog(
            onDismissRequest = { ask = null },
            title = { Text("Revert all?") },
            text = { Text("A script is running: Revert also undoes the $outside changed outside this page, most likely by it.") },
            confirmButton = { TextButton(onClick = { ask = null; options.revert() }) { Text("Revert all", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { ask = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}
