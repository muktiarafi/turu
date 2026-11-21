package com.muktiarafi.turu

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.ButlerAccount
import com.muktiarafi.turu.models.ButlerView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * Butler for the tab on screen (`turu_frontend::models::butler`): Butler's state is the slot's
 * bot's; this reads the view while the page shows and sends the actions.
 */
class ButlerPanel {
    var view by mutableStateOf<ButlerView?>(null)
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
            api, token, models = listOf("butler"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ButlerView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/butler doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: String, body: JSONObject = JSONObject(), answer: (JSONObject) -> Unit = {}) {
        error = null
        client?.act("butler", "do", body.put("action", action), answer) { error = it }
    }

    fun on(action: String, account: ButlerAccount) = act(action, JSONObject().put("name", account.username))
}

private enum class ButlerTab(val label: String) { Accounts("Accounts"), Settings("Settings"), Log("Log") }

/**
 * Plugins > Butler (docs/UI.md, "Native shells"; VibeSkua ButlerV5): this account and whom it
 * follows (Stop following, Release all), then Accounts (the other online accounts with Follow and
 * Summon, and why each rests; Summon all), Settings (Butler's options, saved as they change) and
 * Log. ⋮ has Send to player, the log's Copy, Save and Clear, and Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ButlerPage(state: ShellState, panel: ButlerPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    val context = LocalContext.current
    var tab by rememberSaveable { mutableStateOf(ButlerTab.Accounts) }
    var sending by remember { mutableStateOf(false) }

    Column(Modifier.fillMaxSize()) {
        PageBar("Butler", onBack = { state.back() }) {
            FullWindowMenu("Opens the folder of saved logs too", fullWindow) { close ->
                DropdownMenuItem(
                    text = { Text("Send to player…") },
                    enabled = view?.header?.enabled == true,
                    onClick = { close(); sending = true },
                )
                DropdownMenuItem(
                    text = { Text("Copy the log") },
                    enabled = (view?.logCount ?: 0u) > 0u,
                    onClick = {
                        close()
                        panel.act("copy-log") { answer ->
                            val text = answer.optString("text")
                            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                            clipboard.setPrimaryClip(ClipData.newPlainText("Butler log", text))
                        }
                    },
                )
                DropdownMenuItem(text = { Text("Save the log") }, enabled = (view?.logCount ?: 0u) > 0u, onClick = { close(); panel.act("save-log") })
                DropdownMenuItem(text = { Text("Clear the log") }, enabled = (view?.logCount ?: 0u) > 0u, onClick = { close(); panel.act("clear-log") })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        // With the keyboard up (a setting's number or text) the header and segments step aside,
        // so the field shows above it in landscape.
        val typing = WindowInsets.isImeVisible
        if (!typing) Header(view, panel)
        if (!typing) SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) {
            ButlerTab.entries.forEachIndexed { i, t ->
                SegmentedButton(
                    selected = tab == t,
                    onClick = { tab = t },
                    shape = SegmentedButtonDefaults.itemShape(i, ButlerTab.entries.size),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = Turu.AccentTint,
                        activeContentColor = Turu.Text,
                        inactiveContainerColor = Color.Transparent,
                        inactiveContentColor = Turu.Muted,
                        activeBorderColor = Turu.Edge,
                        inactiveBorderColor = Turu.Edge,
                    ),
                ) { Text(t.label) }
            }
        }
        when (tab) {
            ButlerTab.Accounts -> Accounts(view, panel, Modifier.weight(1f))
            ButlerTab.Settings -> Settings(view, panel, Modifier.weight(1f))
            ButlerTab.Log -> LogLines(view, Modifier.weight(1f))
        }
    }

    if (sending && view != null) SendDialog(view, panel) { sending = false }
}

/** This account and whom it follows; Stop following and Release all when they apply; the last outcome. */
@Composable
private fun Header(view: ButlerView, panel: ButlerPanel) {
    val h = view.header
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(h.me.ifEmpty { "Not logged in" }, style = MaterialTheme.typography.titleSmall)
            Text(h.state, style = MaterialTheme.typography.bodySmall, color = h.tint.color(), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (h.summoning) {
            TextButton(onClick = { panel.act("release-all") }, enabled = h.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Release all") }
        }
        if (h.following) {
            TextButton(onClick = { panel.act("stop-follow") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Stop following", color = Turu.Danger) }
        }
    }
    (panel.error?.let { it to Turu.Danger } ?: view.message?.let { it.text to it.tint.color() })?.let { (text, color) ->
        Text(text, style = MaterialTheme.typography.bodySmall, color = color, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 16.dp))
    }
}

/** The other online accounts, then the count and Summon all. */
@Composable
private fun Accounts(view: ButlerView, panel: ButlerPanel, modifier: Modifier) {
    Column(modifier) {
        LazyColumn(Modifier.weight(1f)) {
            if (view.accounts.isEmpty()) {
                item {
                    Text(
                        "No other accounts online. Accounts show up here once they log in to Turu, in another tab or without a window.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Turu.Muted,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
            items(view.accounts, key = { it.username }) { AccountRow(it, panel) }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            val n = view.accounts.size
            Text(
                when {
                    !view.header.enabled -> "Butler is off: turn on Enable Butler in Settings"
                    !view.summonAll.enabled -> view.summonAll.reason
                    else -> "$n other ${if (n == 1) "account" else "accounts"} online"
                },
                style = MaterialTheme.typography.bodySmall,
                color = if (view.header.enabled) Turu.Muted else Turu.Danger,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(12.dp))
            Button(onClick = { panel.act("summon-all") }, enabled = view.summonAll.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Summon all") }
        }
    }
}

/** An account: its name and chips, where it is, Follow (or Stop) and Summon (or Release), and why either rests. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AccountRow(a: ButlerAccount, panel: ButlerPanel) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(a.label, fontWeight = FontWeight.SemiBold, modifier = Modifier.align(Alignment.CenterVertically))
            for (c in a.chips) {
                val color = c.tint.color()
                Text(
                    c.text,
                    style = MaterialTheme.typography.labelSmall,
                    color = color,
                    modifier = Modifier
                        .align(Alignment.CenterVertically)
                        .clip(RoundedCornerShape(50))
                        .background(color.copy(alpha = 0.14f))
                        .padding(horizontal = 8.dp, vertical = 2.dp),
                )
            }
        }
        if (a.place.isNotEmpty()) {
            Text(a.place, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (a.followed) {
                OutlinedButton(onClick = { panel.act("stop-follow") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Stop", color = Turu.Danger) }
            } else {
                OutlinedButton(onClick = { panel.on("follow", a) }, enabled = a.follow.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Follow") }
            }
            if (a.summoned) {
                OutlinedButton(onClick = { panel.on("release", a) }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Release") }
            } else {
                FilledTonalButton(onClick = { panel.on("summon", a) }, enabled = a.summon.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Summon") }
            }
        }
        val reasons = listOfNotNull(
            a.follow.reason.takeIf { !a.follow.enabled && !a.followed && it.isNotEmpty() },
            a.summon.reason.takeIf { !a.summon.enabled && !a.summoned && it.isNotEmpty() },
        ).distinct()
        if (reasons.isNotEmpty()) {
            Text(reasons.joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 4.dp))
        }
    }
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.4f), modifier = Modifier.padding(horizontal = 16.dp))
}

/** Butler's options under their sections, each saved as it changes (shared by every client but This account's). */
@Composable
private fun Settings(view: ButlerView, panel: ButlerPanel, modifier: Modifier) {
    LazyColumn(modifier) {
        for (section in view.sections) {
            item(key = "h:${section.chip}") {
                Text(
                    section.title,
                    style = MaterialTheme.typography.labelLarge,
                    color = Turu.Muted,
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
                )
            }
            val rows = view.options.filter { it.section == section.chip }
            if (rows.isEmpty()) {
                item(key = "e:${section.chip}") {
                    Text("Log in to set this account's options.", style = MaterialTheme.typography.bodyMedium, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp))
                }
            }
            items(rows, key = { it.label }) { o ->
                OptionRow(o, edited = false, inGame = true, onSet = { value ->
                    panel.act("set", JSONObject().put("label", o.label).put("value", value))
                }, onAction = {})
            }
        }
    }
}

/** The newest log lines, oldest first, kept at the bottom as lines arrive. */
@Composable
private fun LogLines(view: ButlerView, modifier: Modifier) {
    val list = rememberLazyListState()
    LaunchedEffect(view.log.size, view.logCount) { if (view.log.isNotEmpty()) list.scrollToItem(view.log.size) }
    LazyColumn(modifier, state = list) {
        item {
            Text(
                when {
                    !view.logging -> "Debug logs is off in Settings: nothing is recorded."
                    view.logCount > view.log.size.toUInt() -> "The newest ${view.log.size} of ${view.logCount} lines · Copy or Save in ⋮ for all"
                    view.log.isEmpty() -> "Nothing logged yet."
                    else -> "${view.logCount} lines"
                },
                style = MaterialTheme.typography.bodySmall,
                color = if (view.logging) Turu.Muted else Turu.Warning,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }
        itemsIndexed(view.log) { _, line ->
            Text(
                line.text,
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                color = line.tint.color(),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 1.dp),
            )
        }
    }
}

/** Send to player: a name (the keyboard's Go sends) and the recent ones to pick from. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SendDialog(view: ButlerView, panel: ButlerPanel, onClose: () -> Unit) {
    var name by remember { mutableStateOf("") }
    val send = { if (name.isNotBlank()) { panel.act("send", JSONObject().put("player", name.trim())); onClose() } }
    AlertDialog(
        onDismissRequest = onClose,
        title = { Text("Send to player") },
        text = {
            Column {
                Text(
                    "This account and every available butler /goto the player.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                )
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    placeholder = { Text("Player name") },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { send() }),
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                if (view.recent.isNotEmpty()) {
                    Text("Recent", style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = Modifier.padding(top = 8.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        for (r in view.recent) {
                            TextButton(onClick = { name = r }, modifier = Modifier.heightIn(min = 48.dp)) { Text(r) }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = send, enabled = name.isNotBlank(), colors = ButtonDefaults.textButtonColors()) { Text("Send") }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}
