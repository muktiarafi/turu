package com.muktiarafi.turu

import android.content.ClipData
import android.content.ClipboardManager
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.ArmyGearView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * Army Control's client pages (`turu_frontend::models::army_client`): a row's gear and logs, asked
 * by its key each time (a page that a slot restart left asks the next slot). One per process.
 */
class ArmyClientPanel {
    var error by mutableStateOf<String?>(null)
        private set

    private var users = 0
    private var client: ModelClient? = null
    private var following: ShellTab? = null

    fun open() { users++ }

    fun close() {
        users = maxOf(0, users - 1)
        if (users == 0) follow(null)
    }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) error = null
        following = tab
        client?.stop()
        client = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        // Only its actions are used: no view to read.
        client = ModelClient(api, token, models = emptyList(), logs = emptyList(), onView = { _, _ -> }, onLog = { _, _, _ -> })
    }

    val ready get() = client != null

    /** An action for the row `key`; `answer` gets its JSON. */
    fun act(key: String, action: String, body: JSONObject = JSONObject(), quiet: Boolean = false, answer: (JSONObject) -> Unit = {}) {
        val client = client ?: run { if (!quiet) error = "The tab isn't answering: it's starting again"; return }
        client.act("army-client", "do", body.put("action", action).put("key", key), answer) { e ->
            if (e != null || !quiet) error = e
        }
    }
}

@Composable
private fun FollowClient(state: ShellState, panel: ArmyClientPanel) {
    DisposableEffect(Unit) {
        panel.open()
        onDispose { panel.close() }
    }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
}

/**
 * Army Control > a row's ⋮ > Gear (docs/ARMY_EQUIPMENT_PROPOSAL.md): the account, online or not,
 * its bank's age and Refresh, then each slot with what's worn and what it could wear instead
 * (inventory first, then bank) with Equip. A running script is stopped first only when asked.
 */
@Composable
fun ArmyGearPage(state: ShellState, panel: ArmyClientPanel, key: String, label: String) {
    FollowClient(state, panel)
    val json = remember { Json { ignoreUnknownKeys = true } }
    var view by remember { mutableStateOf<ArmyGearView?>(null) }
    var stop by remember { mutableStateOf(false) }
    val picked = remember { mutableStateMapOf<String, Long>() }
    // The gear file is read again each second, as the egui window does.
    LaunchedEffect(key, state.tabs.active?.api) {
        while (true) {
            if (panel.ready) {
                panel.act(key, "gear", quiet = true) { answer ->
                    runCatching { json.decodeFromString<ArmyGearView>(answer.toString()) }
                        .onSuccess { view = it }
                        .onFailure { Log.e(ModelClient.TAG, "army-client gear doesn't decode: $it") }
                }
            }
            delay(1000)
        }
    }
    Column(Modifier.fillMaxSize()) {
        PageBar("Gear: $label", onBack = { state.back() })
        val v = view ?: run {
            Text(panel.error ?: "Reading the gear…", color = if (panel.error != null) Turu.Danger else Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        val busy = v.busy.isNotEmpty()
        val canEquip = v.online && !busy && (v.script.isEmpty() || stop)
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(8.dp).clip(CircleShape).background(if (v.online) Turu.Success else Turu.Muted))
                Spacer(Modifier.width(8.dp))
                Column(Modifier.weight(1f)) {
                    Text(if (v.online) "Online" else "Offline", style = MaterialTheme.typography.bodyMedium)
                    Text(v.bank, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
                TextButton(onClick = { panel.act(key, "refresh") }, enabled = v.online && !busy, modifier = Modifier.heightIn(min = 48.dp)) { Text("Refresh") }
            }
            if (v.script.isNotEmpty()) {
                Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Stop the script first")
                        Text("${v.script} is running: gear isn't changed under it. It stays stopped.", style = MaterialTheme.typography.bodySmall, color = Turu.Warning)
                    }
                    Switch(checked = stop, onCheckedChange = { stop = it })
                }
            }
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(vertical = 4.dp))
            if (!v.known) {
                Text("Nothing to show yet: the gear file is written once the account logs in.", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(16.dp))
            }
            v.slots.forEach { slot ->
                val worn = slot.choices.firstOrNull { it.worn }
                val choice = picked[slot.slot]?.let { id -> slot.choices.firstOrNull { it.id == id } }
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Picker(
                            label = slot.slot,
                            value = (choice ?: worn)?.id?.toString().orEmpty(),
                            options = slot.choices.map { it.id.toString() to it.label },
                            loading = false,
                            empty = "Nothing for this slot",
                            none = slot.worn,
                        ) { id ->
                            val item = id.toLong()
                            if (item == worn?.id) picked.remove(slot.slot) else picked[slot.slot] = item
                        }
                    }
                    Spacer(Modifier.width(8.dp))
                    FilledTonalButton(
                        onClick = {
                            val c = choice ?: return@FilledTonalButton
                            picked.remove(slot.slot)
                            panel.act(key, "equip", JSONObject().put("item", c.id).put("name", c.label.substringBefore(" (")).put("stop", stop))
                        },
                        enabled = canEquip && choice != null && choice.id != worn?.id,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("Equip") }
                }
            }
            Spacer(Modifier.size(8.dp))
        }
        val line = panel.error ?: if (busy) "${v.busy}…" else v.last.ifEmpty { null }
        if (line != null) {
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (busy) {
                    CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = Turu.Accent)
                    Spacer(Modifier.width(10.dp))
                }
                Text(
                    line,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (panel.error != null || (!busy && !v.lastOk)) Turu.Danger else Turu.Muted,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** Most lines a log keeps on the page. */
private const val LOG_LINES = 2000

/**
 * Army Control > a row's ⋮ > Logs: the client's script or debug log, read every two seconds from
 * where it left off, kept at the newest line; selectable. ⋮: Copy. The full window saves to a file.
 */
@Composable
fun ArmyLogsPage(state: ShellState, panel: ArmyClientPanel, key: String, label: String, fullWindow: () -> Unit) {
    FollowClient(state, panel)
    var kind by remember { mutableStateOf("script") }
    val lines = remember(kind) { mutableStateListOf<String>() }
    var dropped by remember(kind) { mutableStateOf(0L) }
    LaunchedEffect(key, kind, state.tabs.active?.api) {
        var next = 0L
        while (true) {
            if (panel.ready) {
                panel.act(key, "logs", JSONObject().put("kind", kind).put("since", next), quiet = true) { answer ->
                    val got = answer.optJSONArray("lines") ?: JSONArray()
                    for (i in 0 until got.length()) lines.add(got.optString(i))
                    val over = lines.size - LOG_LINES
                    if (over > 0) {
                        lines.removeRange(0, over)
                        dropped += over
                    }
                    next = answer.optLong("next", next)
                }
            }
            delay(2000)
        }
    }
    val context = LocalContext.current
    Column(Modifier.fillMaxSize()) {
        PageBar("Logs: $label", onBack = { state.back() }) {
            FullWindowMenu("Army Control, where a client's logs save to a file", fullWindow) { close ->
                DropdownMenuItem(
                    text = { Text("Copy") },
                    enabled = lines.isNotEmpty(),
                    onClick = {
                        close()
                        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Logs", lines.joinToString("\n")))
                    },
                )
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            for ((name, title) in listOf("script" to "Script", "debug" to "Debug")) {
                FilterChip(
                    selected = kind == name,
                    onClick = { kind = name },
                    label = { Text(title) },
                    colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
                )
            }
            Text("${lines.size} lines", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.weight(1f))
        }
        panel.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp)) }
        val list = rememberLazyListState()
        LaunchedEffect(dropped + lines.size) { if (lines.isNotEmpty()) list.scrollToItem(lines.size - 1) }
        Column(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 8.dp)
                .clip(MaterialTheme.shapes.medium)
                .background(Turu.Inset)
                .border(1.dp, Turu.Edge, MaterialTheme.shapes.medium),
        ) {
            if (lines.isEmpty()) {
                Text("Nothing logged yet", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(12.dp))
                return@Column
            }
            SelectionContainer {
                LazyColumn(Modifier.fillMaxSize().horizontalScroll(rememberScrollState()), state = list) {
                    itemsIndexed(lines, key = { i, _ -> dropped + i }) { _, line ->
                        Text(line, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, softWrap = false, modifier = Modifier.padding(horizontal = 10.dp, vertical = 1.dp))
                    }
                }
            }
        }
    }
}
