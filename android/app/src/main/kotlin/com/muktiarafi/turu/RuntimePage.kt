package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.RuntimeView
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/** Runtime's three parts, one on show at a time. */
enum class RuntimePart(val label: String) { Drops("Drops"), Quests("Quests"), Boosts("Boosts") }

/**
 * Runtime's copy of the tab on screen (`turu_frontend::models::runtime`), read while the page
 * shows. The part on show, the checked rows and the text typed are the page's own.
 */
class RuntimePanel {
    var view by mutableStateOf<RuntimeView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    var part by mutableStateOf(RuntimePart.Drops)
    var dropInput by mutableStateOf("")
    var questInput by mutableStateOf("")
    var rewardInput by mutableStateOf("")

    /** The checked rows: drops by their text, quests by their ID. */
    val drops = mutableStateMapOf<String, Unit>()
    val quests = mutableStateMapOf<Long, Unit>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) {
            drops.clear()
            quests.clear()
            error = null
        }
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("runtime"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<RuntimeView>(text) }
                    .onSuccess { v ->
                        view = v
                        // A row gone from the list (a script removed it) can't stay checked.
                        drops.keys.retainAll(v.drops.toSet())
                        quests.keys.retainAll(v.quests.map { it.id }.toSet())
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/runtime doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    private fun act(action: String, body: JSONObject = JSONObject(), done: () -> Unit = {}) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("runtime", "do", body.put("action", action)) { failed ->
            error = failed
            if (failed == null) done()
        }
    }

    fun set(action: String, on: Boolean) = act(action, JSONObject().put("on", on))

    fun boost(kind: String, on: Boolean) = act("boost", JSONObject().put("kind", kind).put("on", on))

    fun search(bank: Boolean) = act("search", JSONObject().put("bank", bank))

    fun interval(ms: Long) = act("interval", JSONObject().put("ms", ms))

    fun addDrops() {
        if (dropInput.isBlank()) return
        act("add-drops", JSONObject().put("input", dropInput))
        dropInput = ""
    }

    fun removeDrops() {
        act("remove-drops", JSONObject().put("items", JSONArray(drops.keys.toList())))
        drops.clear()
    }

    /** Clears the fields only once the IDs are taken: a typo stays to be fixed. */
    fun addQuests() {
        if (questInput.isBlank()) return
        val body = JSONObject().put("input", questInput)
        rewardInput.trim().toLongOrNull()?.let { body.put("reward", it) }
        act("add-quests", body) {
            questInput = ""
            rewardInput = ""
        }
    }

    fun removeQuests() {
        act("remove-quests", JSONObject().put("ids", JSONArray(quests.keys.toList())))
        quests.clear()
    }
}

/**
 * Combat > Runtime (docs/UI.md, "Native shells"): Drops (the pickup list, its loop and options),
 * Quests (the registered quests) and Boosts, one at a time; whether drops are picked and boosts
 * used shows in the bar from any of them. Each part's add field sits at its foot, over the keyboard.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RuntimePage(state: ShellState, panel: RuntimePanel, notifyDrop: () -> Unit, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    Column(Modifier.fillMaxSize()) {
        PageBar("Runtime", onBack = { state.back() }) {
            view?.let {
                val running = listOfNotNull("Picking drops".takeIf { _ -> it.picking }, "Boosting".takeIf { _ -> it.boosting })
                if (running.isNotEmpty()) {
                    Text(running.joinToString(" · "), style = MaterialTheme.typography.labelMedium, color = Turu.Success)
                }
            }
            FullWindowMenu("Drops, quests and boosts side by side, as on the desktop", fullWindow)
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Its lists are back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        // With the keyboard up the panel has room for the field typed in: the parts step aside.
        if (!WindowInsets.isImeVisible) ConnectedChoice(RuntimePart.entries, panel.part, { panel.part = it }, Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) { p ->
            val count = when (p) {
                RuntimePart.Drops -> view.drops.size
                RuntimePart.Quests -> view.quests.size
                RuntimePart.Boosts -> view.boosts.count { it.uses }
            }
            Text(if (count > 0) "${p.label}  $count" else p.label, maxLines = 1)
        }
        LazyColumn(Modifier.weight(1f)) {
            when (panel.part) {
                RuntimePart.Drops -> drops(view, panel, notifyDrop)
                RuntimePart.Quests -> quests(view, panel)
                RuntimePart.Boosts -> boosts(view, panel)
            }
        }
        panel.error?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
        when (panel.part) {
            RuntimePart.Drops -> Foot {
                AddField(panel.dropInput, "Drop name or ID, | between several", Modifier.weight(1f), onDone = panel::addDrops) { panel.dropInput = it }
                Button(onClick = panel::addDrops, enabled = panel.dropInput.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Add") }
            }
            RuntimePart.Quests -> Foot {
                AddField(panel.questInput, "Quest IDs", Modifier.weight(1f), number = true, onDone = panel::addQuests) { panel.questInput = it }
                AddField(panel.rewardInput, "Reward", Modifier.width(120.dp), number = true, onDone = panel::addQuests) { panel.rewardInput = it }
                Button(onClick = panel::addQuests, enabled = panel.questInput.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Add") }
            }
            RuntimePart.Boosts -> {}
        }
    }
}

private fun LazyListScope.drops(view: RuntimeView, panel: RuntimePanel, notifyDrop: () -> Unit) {
    item {
        SwitchRow(
            "Pick drops",
            if (view.picking) "Picks up the list every second" else "Off: drops wait in the stack",
            view.picking,
            strong = true,
        ) { panel.set("picking", it) }
    }
    item {
        ListHead("Pickup list", view.drops.size, panel.drops.size, onRemove = panel::removeDrops)
    }
    if (view.drops.isEmpty()) {
        item { Empty("No drops in the list", "Add an item's name or ID below. A script adds its own as it runs.") }
    }
    items(view.drops, key = { "d:$it" }) { item ->
        CheckRow(item, item in panel.drops) { if (panel.drops.remove(item) == null) panel.drops[item] = Unit }
    }
    item {
        Text("Options", style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 2.dp))
    }
    item { SwitchRow("Reject else", "Turns down every drop not on the list", view.rejectElse) { panel.set("reject-else", it) } }
    item { SwitchRow("Accept AC drops", "Picks up items that cost AC, listed or not", view.acceptAc) { panel.set("accept-ac", it) } }
    item { IntervalRow(view.interval.toLong(), panel::interval) }
    item {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, top = 2.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Drop notifications", style = MaterialTheme.typography.bodyLarge)
                Text("A beep and a notification when a chosen item drops", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
            }
            Spacer(Modifier.width(12.dp))
            OutlinedButton(onClick = notifyDrop, modifier = Modifier.heightIn(min = 40.dp)) { Text("Open") }
        }
    }
}

private fun LazyListScope.quests(view: RuntimeView, panel: RuntimePanel) {
    item {
        Text(
            "Registered quests are accepted again and turned in every 2 s, a script running or not. Their rewards join the pickup list.",
            style = MaterialTheme.typography.bodySmall,
            color = Turu.Muted,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp),
        )
    }
    item { ListHead("Registered", view.quests.size, panel.quests.size, onRemove = panel::removeQuests) }
    if (view.quests.isEmpty()) {
        item { Empty("No registered quests", "Add quest IDs below (spaces, commas or | between several), with the reward to choose if it offers one.") }
    }
    items(view.quests, key = { "q:${it.id}" }) { quest ->
        CheckRow(quest.label, quest.id in panel.quests) { if (panel.quests.remove(quest.id) == null) panel.quests[quest.id] = Unit }
    }
}

private fun LazyListScope.boosts(view: RuntimeView, panel: RuntimePanel) {
    item {
        SwitchRow(
            "Use boosts",
            if (view.boosting) "Uses the boosts on below whenever theirs runs out" else "Off: no boost is used",
            view.boosting,
            strong = true,
        ) { panel.set("boosting", it) }
    }
    item { HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) }
    items(view.boosts, key = { "b:${it.kind}" }) { b ->
        SwitchRow(b.label, if (b.id > 0) "Item ${b.id}" else "No item found yet: search below", b.uses) { panel.boost(b.kind, it) }
    }
    item {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { panel.search(false) }, enabled = !view.searching, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                Text(if (view.searching) "Searching…" else "Search inventory", maxLines = 1)
            }
            OutlinedButton(onClick = { panel.search(true) }, enabled = !view.searching, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                Text(if (view.searching) "Searching…" else "Search bank", maxLines = 1)
            }
        }
    }
    item {
        Text(
            "A search finds the boost items you own and sets their IDs.",
            style = MaterialTheme.typography.bodySmall,
            color = Turu.Muted,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
    }
}

/** A setting with its switch; `strong` for a loop's on/off, which leads its part. */
@Composable
private fun SwitchRow(label: String, about: String, checked: Boolean, strong: Boolean = false, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(if (strong && checked) Turu.AccentTint else Color.Transparent)
            .clickable { onChange(!checked) }
            .heightIn(min = 56.dp)
            .padding(start = 8.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = if (strong) MaterialTheme.typography.titleSmall else MaterialTheme.typography.bodyLarge)
            Text(about, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
        }
        Spacer(Modifier.width(12.dp))
        Switch(checked = checked, onCheckedChange = onChange)
    }
}

/** The pickup loop's wait: sent as typed (empty is 0), never overwritten while it's being typed. */
@Composable
private fun IntervalRow(ms: Long, onSet: (Long) -> Unit) {
    var text by remember { mutableStateOf(ms.toString()) }
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(ms, focused) { if (!focused) text = ms.toString() }
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, top = 2.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text("Interval", style = MaterialTheme.typography.bodyLarge)
            Text("Waited on top of the loop's 1 s tick", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
        }
        Spacer(Modifier.width(12.dp))
        OutlinedTextField(
            value = text,
            onValueChange = { next ->
                text = next.filter { it.isDigit() }.take(9)
                val value = if (text.isEmpty()) 0L else text.toLongOrNull()
                if (value != null && value != ms) onSet(value)
            },
            suffix = { Text("ms", color = Turu.Muted) },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
            modifier = Modifier.width(130.dp).onFocusChanged { focused = it.isFocused },
        )
    }
}

/** A list's heading, its count, and Remove for the checked rows. */
@Composable
private fun ListHead(title: String, count: Int, checked: Int, onRemove: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = MaterialTheme.typography.titleSmall)
        if (count > 0) Text("  $count", style = MaterialTheme.typography.labelMedium, color = Turu.Muted)
        Spacer(Modifier.weight(1f))
        TextButton(onClick = onRemove, enabled = checked > 0, modifier = Modifier.heightIn(min = 48.dp)) {
            Text(if (checked > 0) "Remove $checked" else "Remove", color = if (checked > 0) Turu.Danger else Turu.Muted)
        }
    }
}

@Composable
private fun CheckRow(label: String, checked: Boolean, onToggle: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(if (checked) Turu.AccentTint else Color.Transparent)
            .clickable(onClick = onToggle)
            .heightIn(min = 48.dp)
            .padding(end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = checked, onCheckedChange = { onToggle() })
        Text(label, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun Empty(title: String, about: String) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge)
        Text(about, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 2.dp))
    }
}

@Composable
private fun Foot(content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}

@Composable
private fun AddField(value: String, hint: String, modifier: Modifier, number: Boolean = false, onDone: () -> Unit, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        placeholder = { Text(hint, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        singleLine = true,
        shape = MaterialTheme.shapes.medium,
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.Ascii else KeyboardType.Text, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onDone() }),
        modifier = modifier,
    )
}
