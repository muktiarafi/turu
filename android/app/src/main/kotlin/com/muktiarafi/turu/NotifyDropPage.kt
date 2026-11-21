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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import com.muktiarafi.turu.models.NotifyDropView
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * Notify Drop's copy of the tab on screen (`turu_frontend::models::notify_drop`), read while the
 * page shows. The text typed and the checked rows are the page's own.
 */
class NotifyDropPanel {
    var view by mutableStateOf<NotifyDropView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    var input by mutableStateOf("")

    /** The checked rows, by name. */
    val checked = mutableStateMapOf<String, Unit>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) {
            checked.clear()
            error = null
        }
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("notify-drop"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<NotifyDropView>(text) }
                    .onSuccess { v ->
                        view = v
                        checked.keys.retainAll(v.items.toSet())
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/notify-drop doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    private fun act(action: String, body: JSONObject = JSONObject()) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("notify-drop", "do", body.put("action", action)) { error = it }
    }

    fun add() {
        if (input.isBlank()) return
        act("add", JSONObject().put("input", input))
        input = ""
    }

    fun remove() {
        act("remove", JSONObject().put("items", JSONArray(checked.keys.toList())))
        checked.clear()
    }

    fun clear() {
        act("clear")
        checked.clear()
    }

    fun count(n: Long) = act("count", JSONObject().put("count", n))

    fun delay(ms: Long) = act("delay", JSONObject().put("delay", ms))

    fun test() = act("test")

    fun toggle(name: String) {
        if (checked.remove(name) == null) checked[name] = Unit
    }
}

/**
 * Combat > Runtime > Notifications (docs/UI.md, "Native shells"): the beep (how many, how far
 * apart, Test beep), then the watched names, a tap checking one for Remove; the add field at the
 * foot, over the keyboard. ⋮: Clear all, Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun NotifyDropPage(state: ShellState, panel: NotifyDropPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    Column(Modifier.fillMaxSize()) {
        PageBar("Notify Drop", onBack = { state.back() }) {
            FullWindowMenu("The same list, laid out as the desktop's", fullWindow) { close ->
                DropdownMenuItem(
                    text = { Text("Clear all", color = if (view?.items?.isNotEmpty() == true) Turu.Danger else Turu.Muted) },
                    enabled = view?.items?.isNotEmpty() == true,
                    onClick = { close(); panel.clear() },
                )
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Its list is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        // With the keyboard up the panel has room for the field typed in: the beep steps aside.
        val typing = WindowInsets.isImeVisible
        var beepFocused by remember { mutableStateOf(false) }
        LazyColumn(Modifier.weight(1f)) {
            if (!typing || beepFocused) item { Beep(view, panel) { beepFocused = it } }
            item {
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Watching", style = MaterialTheme.typography.titleSmall)
                    if (view.items.isNotEmpty()) Text("  ${view.items.size}", style = MaterialTheme.typography.labelMedium, color = Turu.Muted)
                    Spacer(Modifier.weight(1f))
                    val n = panel.checked.size
                    TextButton(onClick = panel::remove, enabled = n > 0, modifier = Modifier.heightIn(min = 48.dp)) {
                        Text(if (n > 0) "Remove $n" else "Remove", color = if (n > 0) Turu.Danger else Turu.Muted)
                    }
                }
            }
            if (view.items.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                        Text("Nothing watched yet", style = MaterialTheme.typography.bodyLarge)
                        Text(
                            if (view.alerts) {
                                "Add exact item names below. When one drops, Turu beeps and posts a notification, heard with the screen off."
                            } else {
                                "Add exact item names below. When one drops, Turu beeps."
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = Turu.Muted,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    }
                }
            }
            items(view.items, key = { it }) { name ->
                val on = name in panel.checked
                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 1.dp)
                        .clip(MaterialTheme.shapes.medium)
                        .background(selectionTint(on))
                        .clickable { panel.toggle(name) }
                        .heightIn(min = 48.dp)
                        .padding(end = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = on, onCheckedChange = { panel.toggle(name) })
                    Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        panel.error?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
        if (!typing || !beepFocused) {
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                OutlinedTextField(
                    value = panel.input,
                    onValueChange = { panel.input = it },
                    placeholder = { Text("Item name, | between several", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    singleLine = true,
                    shape = MaterialTheme.shapes.medium,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { panel.add() }),
                    modifier = Modifier.weight(1f),
                )
                Button(onClick = panel::add, enabled = panel.input.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Add") }
            }
        }
    }
}

/** Test beep, then "[5] beeps, [200 ms] apart". */
@Composable
private fun Beep(view: NotifyDropView, panel: NotifyDropPanel, focused: (Boolean) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Beep on a drop", style = MaterialTheme.typography.titleSmall, modifier = Modifier.weight(1f))
            OutlinedButton(onClick = panel::test, modifier = Modifier.heightIn(min = 48.dp)) { Text("Test beep") }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            NumberBox(view.count.toLong(), view.countMin.toLong(), view.countMax.toLong(), null, 72, panel::count, focused)
            Spacer(Modifier.width(8.dp))
            Text(if (view.count == 1u) "beep," else "beeps,", color = Turu.Muted)
            Spacer(Modifier.width(16.dp))
            NumberBox(view.delay.toLong(), view.delayMin.toLong(), view.delayMax.toLong(), "ms", 120, panel::delay, focused)
            Spacer(Modifier.width(8.dp))
            Text("apart", color = Turu.Muted)
        }
    }
}

/** A number: sent once in range (`min`..`max`), set back to the tab's when left out of range. */
@Composable
private fun NumberBox(value: Long, min: Long, max: Long, unit: String?, width: Int, onSet: (Long) -> Unit, focused: (Boolean) -> Unit) {
    var text by remember { mutableStateOf(value.toString()) }
    var editing by remember { mutableStateOf(false) }
    LaunchedEffect(value, editing) { if (!editing) text = value.toString() }
    OutlinedTextField(
        value = text,
        onValueChange = { next ->
            text = next.filter { it.isDigit() }.take(5)
            text.toLongOrNull()?.takeIf { it in min..max && it != value }?.let(onSet)
        },
        suffix = unit?.let { { Text(it, color = Turu.Muted) } },
        singleLine = true,
        shape = MaterialTheme.shapes.medium,
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
        modifier = Modifier.width(width.dp).onFocusChanged {
            editing = it.isFocused
            focused(it.isFocused)
        },
    )
}
