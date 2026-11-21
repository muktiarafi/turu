package com.muktiarafi.turu

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
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
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.InterceptedRow
import com.muktiarafi.turu.models.InterceptorView
import com.muktiarafi.turu.models.PacketFilter
import com.muktiarafi.turu.models.PacketLoggerView
import com.muktiarafi.turu.models.SpammerView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * One packet tool's copy of the tab on screen (`turu_frontend::models::packets`): its model's
 * view, decoded to `V`, and its actions. The Logger also reads the `packet` log's lines.
 */
class PacketPanel<V : Any>(private val model: String, private val decode: (Json, String) -> V, private val readsLog: Boolean = false) {
    var view by mutableStateOf<V?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** The `packet` log's lines ("time text"), the newest last (the Logger). */
    val lines = mutableStateListOf<String>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) error = null
        following = tab
        client?.stop()
        client = null
        view = null
        lines.clear()
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf(model), logs = if (readsLog) listOf("packet") else emptyList(),
            onView = { _, text ->
                runCatching { decode(json, text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/$model doesn't decode: $it") }
            },
            onLog = { _, time, text -> append("$time $text") },
        ).also { c ->
            c.start()
            if (readsLog) c.readLog("packet", 0) { read -> lines.clear(); read.takeLast(KEEP).forEach { (t, x) -> lines.add("$t $x") } }
        }
    }

    /** Reads the whole `packet` log again (after an Undo puts lines back). */
    fun reloadLog() {
        client?.readLog("packet", 0) { read -> lines.clear(); read.takeLast(KEEP).forEach { (t, x) -> lines.add("$t $x") } }
    }

    private fun append(line: String) {
        lines.add(line)
        if (lines.size > KEEP) lines.removeRange(0, lines.size - KEEP)
    }

    fun act(action: String, body: JSONObject = JSONObject(), answer: (JSONObject) -> Unit = {}, done: () -> Unit = {}) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act(model, "do", body.put("action", action), answer) { e ->
            error = e
            if (e == null) done()
        }
    }

    companion object {
        /** The Logger's buffer holds 1,000 packets. */
        const val KEEP = 1000
    }
}

@Composable
private fun <V : Any> Follow(state: ShellState, panel: PacketPanel<V>) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
}

@Composable
private fun Waiting(state: ShellState, what: String) {
    val down = state.tabs.active?.down != null || state.tabs.active?.api == null
    Text(if (down) "This tab is starting again. The $what is back with it." else "Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
}

private fun copy(context: Context, label: String, text: String) {
    if (text.isEmpty()) return
    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(label, text))
}

/** "Show 10 / 12 ▾": opens the filters dialog. */
@Composable
private fun FiltersChip(filters: List<PacketFilter>, onClick: () -> Unit) {
    val shown = filters.count { it.shown }
    FilterChip(
        selected = shown < filters.size,
        onClick = onClick,
        label = { Text("Show $shown / ${filters.size}", maxLines = 1) },
        trailingIcon = { Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp)) },
        colors = FilterChipDefaults.filterChipColors(
            selectedContainerColor = Turu.AccentTint,
            selectedLabelColor = Turu.Text,
            selectedTrailingIconColor = Turu.Text,
            labelColor = Turu.Muted,
            iconColor = Turu.Muted,
        ),
        border = FilterChipDefaults.filterChipBorder(enabled = true, selected = shown < filters.size, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
    )
}

/** VibeSkua's "Show packets" flyout: a check box a kind, with All, None and Invert. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FiltersDialog(filters: List<PacketFilter>, note: String, close: () -> Unit, apply: (List<Boolean>) -> Unit) {
    var ticks by remember { mutableStateOf(filters.map { it.shown }) }
    AlertDialog(
        onDismissRequest = close,
        title = { Text("Show packets") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text(note, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.padding(vertical = 4.dp)) {
                    TextButton(onClick = { ticks = ticks.map { true } }) { Text("All") }
                    TextButton(onClick = { ticks = ticks.map { false } }) { Text("None") }
                    TextButton(onClick = { ticks = ticks.map { !it } }) { Text("Invert") }
                }
                FlowRow(maxItemsInEachRow = 2) {
                    filters.forEachIndexed { i, f ->
                        Row(
                            Modifier.weight(1f).heightIn(min = 48.dp).clickable { ticks = ticks.toMutableList().also { it[i] = !it[i] } },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Checkbox(checked = ticks[i], onCheckedChange = null, modifier = Modifier.padding(horizontal = 12.dp))
                            Text(f.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { apply(ticks); close() }) { Text("Apply") } },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}

/** A search field with its clear button. */
@Composable
private fun SearchField(value: String, hint: String, modifier: Modifier, change: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = change,
        placeholder = { Text(hint) },
        leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
        trailingIcon = if (value.isNotEmpty()) {
            { IconButton(onClick = { change("") }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } }
        } else {
            null
        },
        singleLine = true,
        shape = MaterialTheme.shapes.medium,
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
        modifier = modifier,
    )
}

/** The inset well packets show in, monospaced. */
@Composable
private fun Well(modifier: Modifier, content: @Composable () -> Unit) {
    Box(
        modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 8.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(Turu.Inset)
            .border(1.dp, Turu.Edge, MaterialTheme.shapes.medium),
    ) { content() }
}

/** A state with its dot: "Logging", "Connected", "Sending 2 of 5". */
@Composable
private fun StateDot(text: String, on: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(if (on) Turu.Success else Turu.Muted))
        Spacer(Modifier.width(6.dp))
        Text(text, style = MaterialTheme.typography.bodySmall, color = if (on) Turu.Success else Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

// --- Packet Logger ---------------------------------------------------------------------------

fun packetLoggerPanel() = PacketPanel("packet-logger", { json, text -> json.decodeFromString<PacketLoggerView>(text) }, readsLog = true)

/**
 * Diagnostics > Packet Logger (VibeSkua "Logger"): Start or Stop logging with its state, the
 * search and the "Show n / m" filters (a kind turned off isn't logged from then on), then the
 * outgoing packets, the newest last, selectable. ⋮: Copy shown, Clear, Full window (Save).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PacketLoggerPage(state: ShellState, panel: PacketPanel<PacketLoggerView>, fullWindow: () -> Unit) {
    Follow(state, panel)
    val view = panel.view
    val context = LocalContext.current
    var search by remember { mutableStateOf("") }
    var filtering by remember { mutableStateOf(false) }
    val shown = remember(search, panel.lines.size, panel.lines.lastOrNull()) {
        val needle = search.lowercase()
        if (needle.isEmpty()) panel.lines.toList() else panel.lines.filter { it.lowercase().contains(needle) }
    }
    var notice by remember { mutableStateOf<Notice?>(null) }
    Box(Modifier.fillMaxSize()) {
    Column(Modifier.fillMaxSize()) {
            PageBar("Packet Logger", onBack = { state.back() }) {
                FullWindowMenu("Save the packets to a file", fullWindow) { close ->
                    DropdownMenuItem(text = { Text("Copy shown") }, enabled = shown.isNotEmpty(), onClick = { close(); copy(context, "Packets", shown.joinToString("\n")) })
                    DropdownMenuItem(text = { Text("Clear") }, enabled = panel.lines.isNotEmpty(), onClick = {
                    close()
                    val n = panel.lines.size
                    panel.act("clear") {
                        panel.lines.clear()
                        notice = Notice(if (n == 1) "Cleared 1 packet" else "Cleared $n packets") { panel.act("undo-clear") { panel.reloadLog() } }
                    }
                })
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                }
            }
            if (view == null) {
                Waiting(state, "Packet Logger")
                return@Column
            }
            if (!WindowInsets.isImeVisible) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (view.logging) {
                        Button(
                            onClick = { panel.act("logging", JSONObject().put("on", false)) },
                            colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Turu.Text),
                            modifier = Modifier.heightIn(min = 48.dp),
                        ) { Text("Stop logging") }
                    } else {
                        Button(onClick = { panel.act("logging", JSONObject().put("on", true)) }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Start logging") }
                    }
                    Spacer(Modifier.width(12.dp))
                    Box(Modifier.weight(1f)) { StateDot(if (view.logging) "Logging" else "Stopped", view.logging) }
                    FiltersChip(view.filters) { filtering = true }
                }
            }
            Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                SearchField(search, "Search packets", Modifier.weight(1f)) { search = it }
                Text("${shown.size}", style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"), color = Turu.Muted, modifier = Modifier.padding(start = 12.dp))
            }
            panel.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp)) }
            Lines(shown, if (view.logging) "Nothing logged yet: play a moment" else "Start logging to see the packets the game sends", Modifier.weight(1f))
        }
        NoticeHost(notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { notice = null }
    }
    if (filtering && view != null) {
        FiltersDialog(view.filters, "A kind turned off isn't logged from then on; what's logged stays.", { filtering = false }) { ticks ->
            panel.act("filters", JSONObject().put("shown", JSONArray(ticks)))
        }
    }
}

/** Lines in the well, kept at the newest, selectable, scrolling sideways. */
@Composable
private fun Lines(lines: List<String>, empty: String, modifier: Modifier) {
    val list = rememberLazyListState()
    LaunchedEffect(lines.size, lines.lastOrNull()) { if (lines.isNotEmpty()) list.scrollToItem(lines.size - 1) }
    Well(modifier) {
        if (lines.isEmpty()) {
            Text(empty, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(12.dp))
            return@Well
        }
        SelectionContainer {
            LazyColumn(Modifier.fillMaxSize().horizontalScroll(rememberScrollState()), state = list) {
                itemsIndexed(lines) { _, line ->
                    Text(line, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, softWrap = false, modifier = Modifier.padding(horizontal = 10.dp, vertical = 1.dp))
                }
            }
        }
    }
}

// --- Spammer ---------------------------------------------------------------------------------

fun spammerPanel() = PacketPanel("spammer", { json, text -> json.decodeFromString<SpammerView>(text) })

/**
 * Diagnostics > Spammer (VibeSkua `PacketSpammerView`): Start or Stop with what it sends, the
 * delay between packets and Send to client; the queue (a row's ✕ removes it); the composer with
 * Add and Send now. ⋮: Clear the queue, Full window (Load and Save a list).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SpammerPage(state: ShellState, panel: PacketPanel<SpammerView>, fullWindow: () -> Unit) {
    Follow(state, panel)
    val view = panel.view
    var packet by remember { mutableStateOf("") }
    var notice by remember { mutableStateOf<Notice?>(null) }
    Box(Modifier.fillMaxSize()) {
    Column(Modifier.fillMaxSize()) {
            PageBar("Spammer", onBack = { state.back() }) {
                FullWindowMenu("Load and save a list of packets", fullWindow) { close ->
                    DropdownMenuItem(
                    text = { Text("Clear the queue") },
                    enabled = view?.packets?.isNotEmpty() == true,
                    onClick = {
                        close()
                        val n = view?.packets?.size ?: 0
                        panel.act("clear") { notice = Notice(if (n == 1) "Cleared 1 packet from the queue" else "Cleared $n packets from the queue") { panel.act("undo-clear") } }
                    },
                )
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                }
            }
            if (view == null) {
                Waiting(state, "Spammer")
                return@Column
            }
            val typing = WindowInsets.isImeVisible
            if (!typing) {
                FlowRow(
                    Modifier.fillMaxWidth().padding(horizontal = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                    itemVerticalAlignment = Alignment.CenterVertically,
                ) {
                    if (view.running) {
                        Button(onClick = { panel.act("stop") }, colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Turu.Text), modifier = Modifier.heightIn(min = 48.dp)) { Text("Stop") }
                    } else {
                        Button(onClick = { panel.act("start") }, enabled = view.start.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Start") }
                    }
                    DelayBox(view.delayMs.toLong()) { panel.act("delay", JSONObject().put("ms", it)) }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Switch(checked = view.toClient, onCheckedChange = { panel.act("to-client", JSONObject().put("on", it)) })
                        Spacer(Modifier.width(8.dp))
                        Text("Send to client")
                    }
                }
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Queue · ${view.packets.size}", style = MaterialTheme.typography.titleSmall, color = Turu.Muted, modifier = Modifier.weight(1f))
                    when {
                        view.running -> StateDot(view.status, true)
                        !view.start.enabled -> Text(view.start.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                    }
                }
            }
            // Typing on a landscape phone leaves a strip: the composer takes it, the queue steps aside.
            if (!typing) Well(Modifier.weight(1f)) {
                if (view.packets.isEmpty()) {
                    Text("The queue is empty: type a packet below and Add it", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(12.dp))
                    return@Well
                }
                LazyColumn(Modifier.fillMaxSize()) {
                    itemsIndexed(view.packets) { i, p ->
                        val sending = view.sending == i.toLong()
                        Row(
                            Modifier.fillMaxWidth().background(selectionTint(sending)).heightIn(min = 48.dp).padding(start = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text("${i + 1}", style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"), color = Turu.Muted, modifier = Modifier.width(28.dp))
                            Text(p, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                            IconButton(onClick = { panel.act("remove", JSONObject().put("index", i).put("packet", p)) }) {
                                Icon(Icons.Rounded.Close, contentDescription = "Remove packet ${i + 1}", tint = Turu.Muted)
                            }
                        }
                    }
                }
            }
            panel.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp)) }
            Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = packet,
                    onValueChange = { packet = it },
                    placeholder = { Text("%xt%zm%…", fontFamily = FontFamily.Monospace) },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                    singleLine = true,
                    shape = MaterialTheme.shapes.medium,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, autoCorrectEnabled = false),
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                OutlinedButton(onClick = { panel.act("add", JSONObject().put("packet", packet)) { packet = "" } }, enabled = packet.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Add") }
                Spacer(Modifier.width(8.dp))
                Button(onClick = { panel.act("send", JSONObject().put("packet", packet)) { notice = Notice(if (view.toClient) "Sent to the client" else "Sent to the server") } }, enabled = packet.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Send now") }
            }
        }
        NoticeHost(notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { notice = null }
    }
}

/** "Every [800] ms": the delay between packets, saved as it's typed. */
@Composable
private fun DelayBox(value: Long, set: (Long) -> Unit) {
    var text by remember { mutableStateOf(value.toString()) }
    var editing by remember { mutableStateOf(false) }
    LaunchedEffect(value, editing) { if (!editing) text = value.toString() }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text("Every", color = Turu.Muted)
        Spacer(Modifier.width(8.dp))
        OutlinedTextField(
            value = text,
            onValueChange = { next ->
                editing = true
                text = next.filter { it.isDigit() }.take(6)
                text.toLongOrNull()?.takeIf { it <= 600_000 && it != value }?.let(set)
            },
            suffix = { Text("ms", color = Turu.Muted) },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.width(128.dp),
        )
    }
}

// --- Interceptor -----------------------------------------------------------------------------

fun interceptorPanel() = PacketPanel("interceptor", { json, text -> json.decodeFromString<InterceptorView>(text) })

/**
 * Diagnostics > Interceptor (VibeSkua `PacketInterceptorView`): the server Connect relogs to
 * (Current server stays), Connect or Disconnect, Log packets; the search and "Show n / m"; the
 * captured packets (OUT, IN or BLOCK; a tap reads one in full, a long press selects; while some
 * are selected a tap selects too), the newest 300; the count. ⋮: Copy
 * (the selected, or all shown), Clear, Full window (Export).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InterceptorPage(state: ShellState, panel: PacketPanel<InterceptorView>, fullWindow: () -> Unit) {
    Follow(state, panel)
    val view = panel.view
    val context = LocalContext.current
    var filtering by remember { mutableStateOf(false) }
    // Selections show at once, before the tab's next view says so; a tap reads a packet.
    val picks = remember { mutableStateMapOf<ULong, Boolean>() }
    LaunchedEffect(view) { picks.clear() }
    var reading by remember { mutableStateOf<InterceptedRow?>(null) }
    var notice by remember { mutableStateOf<Notice?>(null) }
    Box(Modifier.fillMaxSize()) {
    Column(Modifier.fillMaxSize()) {
            PageBar("Interceptor", onBack = { state.back() }) {
                FullWindowMenu("Export the packets to a file", fullWindow) { close ->
                    val n = view?.selected ?: 0uL
                    DropdownMenuItem(
                        text = { Text(if (n > 0uL) "Copy $n selected" else "Copy all shown") },
                        enabled = view != null && view.matches > 0uL,
                        onClick = { close(); panel.act("copy", answer = { copy(context, "Packets", it.optString("text")) }) },
                    )
                    DropdownMenuItem(
                    text = { Text("Clear") },
                    enabled = view != null && view.matches > 0uL,
                    onClick = {
                        close()
                        panel.act("clear") { notice = Notice("Cleared the captured packets") { panel.act("undo-clear") } }
                    },
                )
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                }
            }
            if (view == null) {
                Waiting(state, "Interceptor")
                return@Column
            }
            var search by remember { mutableStateOf(view.search) }
            LaunchedEffect(search) {
                if (search != view.search) {
                    delay(300)
                    panel.act("search", JSONObject().put("text", search))
                }
            }
            if (!WindowInsets.isImeVisible) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Picker(
                            label = "Server",
                            value = view.server,
                            options = listOf("" to "Current server") + view.servers.map { it to it },
                            loading = view.refreshing,
                            none = "Current server",
                            onOpen = { panel.act("refresh-servers") },
                        ) { panel.act("server", JSONObject().put("name", it)) }
                    }
                    IconButton(onClick = { panel.act("refresh-servers") }, enabled = !view.refreshing) {
                        Icon(Icons.Rounded.Refresh, contentDescription = "Refresh the server list", tint = Turu.Muted)
                    }
                }
                FlowRow(
                    Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    itemVerticalAlignment = Alignment.CenterVertically,
                ) {
                    if (view.running) {
                        Button(onClick = { panel.act("disconnect") }, colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Turu.Text), modifier = Modifier.heightIn(min = 48.dp)) { Text("Disconnect") }
                    } else {
                        Button(onClick = { panel.act("connect") }, modifier = Modifier.heightIn(min = 48.dp)) { Text(if (view.server.isEmpty()) "Connect" else "Connect and relog") }
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Switch(checked = view.logging, onCheckedChange = { panel.act("logging", JSONObject().put("on", it)) })
                        Spacer(Modifier.width(8.dp))
                        Text("Log packets")
                    }
                    StateDot(if (view.running) "Connected" else "Not capturing", view.running)
                }
            }
            Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                SearchField(search, "Search packets", Modifier.weight(1f)) { search = it }
                Spacer(Modifier.width(8.dp))
                FiltersChip(view.filters) { filtering = true }
            }
            panel.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp)) }
            val list = rememberLazyListState()
            val newest = view.rows.lastOrNull()?.number
            LaunchedEffect(newest) { if (view.rows.isNotEmpty()) list.scrollToItem(view.rows.size - 1) }
            Well(Modifier.weight(1f)) {
                if (view.rows.isEmpty()) {
                    Text(
                        if (view.running) "Nothing captured yet: play a moment" else "No packets yet: Connect to capture the game's packets",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(12.dp),
                    )
                    return@Well
                }
                LazyColumn(Modifier.fillMaxSize(), state = list) {
                    itemsIndexed(view.rows, key = { _, r -> r.number.toLong() }) { _, r ->
                        val selected = picks[r.number] ?: r.selected
                        val toggle = {
                            picks[r.number] = !selected
                            panel.act("select", JSONObject().put("number", r.number.toLong()).put("on", !selected))
                        }
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .background(selectionTint(selected))
                                .combinedClickable(
                                    onClickLabel = "Read the packet",
                                    onLongClickLabel = if (selected) "Unselect" else "Select",
                                    onLongClick = toggle,
                                    onClick = { if (picks.any { it.value } || view.selected > 0uL) toggle() else reading = r },
                                )
                                .heightIn(min = 48.dp)
                                .padding(horizontal = 10.dp, vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Direction(r.direction)
                            Spacer(Modifier.width(8.dp))
                            Text(
                                r.packet,
                                style = MaterialTheme.typography.bodySmall,
                                fontFamily = FontFamily.Monospace,
                                color = if (r.direction == "BLOCK") Turu.Muted else Turu.Text,
                                textDecoration = if (r.direction == "BLOCK") TextDecoration.LineThrough else null,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                val count = if (view.matches == 1uL) "1 packet" else "${view.matches} packets"
                val more = if (view.matches > view.rows.size.toULong()) " · the newest ${view.rows.size} shown" else ""
                Text(count + more + if (view.selected > 0uL) " · ${view.selected} selected" else "", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.weight(1f))
                if (view.selected > 0uL) TextButton(onClick = { panel.act("unselect") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Unselect") }
            }
        }
        NoticeHost(notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { notice = null }
    }
    if (filtering && view != null) {
        FiltersDialog(view.filters, "Only the kinds checked show; the capture keeps them all.", { filtering = false }) { ticks ->
            panel.act("filters", JSONObject().put("shown", JSONArray(ticks)))
        }
    }
    reading?.let { r ->
        AlertDialog(
            onDismissRequest = { reading = null },
            title = { Text(when (r.direction) { "OUT" -> "Sent to the server"; "IN" -> "From the server"; else -> "Blocked by an interceptor" }) },
            text = {
                SelectionContainer {
                    Text(
                        r.packet,
                        style = MaterialTheme.typography.bodyMedium,
                        fontFamily = FontFamily.Monospace,
                        modifier = Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState()),
                    )
                }
            },
            confirmButton = { TextButton(onClick = { copy(context, "Packet", r.packet); reading = null; notice = Notice("Copied the packet") }) { Text("Copy") } },
            dismissButton = {
                Row {
                    TextButton(onClick = { panel.act("select", JSONObject().put("number", r.number.toLong()).put("on", true)); reading = null }) { Text("Select") }
                    TextButton(onClick = { reading = null }) { Text("Close") }
                }
            },
            containerColor = Turu.Card,
        )
    }
}

/** The OUT / IN / BLOCK tag before a packet. */
@Composable
private fun Direction(direction: String) {
    val color = when (direction) {
        "OUT" -> Turu.Accent
        "IN" -> Turu.Success
        else -> Turu.Danger
    }
    Text(
        direction,
        style = MaterialTheme.typography.labelMedium,
        color = color,
        maxLines = 1,
        modifier = Modifier
            .width(52.dp)
            .clip(MaterialTheme.shapes.small)
            .background(color.copy(alpha = 0.18f))
            .padding(vertical = 2.dp),
        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
    )
}
