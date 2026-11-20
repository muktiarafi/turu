package com.muktiarafi.turu

import android.content.ClipData
import android.content.ClipboardManager
import android.util.Log
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.LoaderQuest
import com.muktiarafi.turu.models.LoaderView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * The Loader's copy of the tab on screen (`turu_frontend::models::loader`), read while the page
 * shows. The search and page are the slot's (the cache is large: it sends one page); Shop or
 * Quests, the IDs typed and the checked quests are the page's own.
 */
class LoaderPanel {
    var view by mutableStateOf<LoaderView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    var quests by mutableStateOf(true)
    var ids by mutableStateOf("")

    /** The checked quests, ID to name (kept across pages, for Copy). */
    val checked = mutableStateMapOf<Long, String>()

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
            api, token, models = listOf("loader"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<LoaderView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/loader doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("loader", "do", JSONObject().put("action", "open"))
        }
    }

    fun act(action: String, body: JSONObject = JSONObject()) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("loader", "do", body.put("action", action)) { error = it }
    }

    fun load() {
        if (ids.isBlank()) return
        act(if (quests) "load-quests" else "load-shop", JSONObject().put("input", ids))
    }

    fun loadChecked() = act("load-quests", JSONObject().put("ids", JSONArray(checked.keys.toList())))

    fun toggle(q: LoaderQuest) {
        if (checked.remove(q.id) == null) checked[q.id] = q.name
    }

    /** The checked quests as `quest_cache::copy_ids`, `copy_names` or `copy_comments` give them. */
    fun copied(how: String): String {
        val list = checked.entries.toList()
        return when (how) {
            "ids" -> list.joinToString(",") { it.key.toString() }
            "names" -> list.joinToString(",") { it.value }
            else -> list.joinToString("\n") { "// ${it.value} | ${it.key}" }
        }
    }
}

/**
 * Tools & Helpers > Loader (docs/UI.md, "Native shells"): Shop or Quests with the IDs and Load,
 * then the quest cache, searchable, a page at a time (a tap checks a quest). The foot shows what
 * runs (with Cancel) or the last notice, and with quests checked: Copy, Fake complete (one) and
 * Load (Fake complete for one). ⋮: Download, Add missing, Rebuild all, Update range…, Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun LoaderPage(state: ShellState, panel: LoaderPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var ranging by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Loader", onBack = { state.back() }) {
            view?.let { Text("%,d cached".format(it.total.toLong()), style = MaterialTheme.typography.labelMedium, color = Turu.Muted) }
            FullWindowMenu("The list and its tools side by side", fullWindow) { close ->
                val idle = view?.status?.busy == false
                DropdownMenuItem(text = { Text("Download community cache") }, enabled = idle, onClick = { close(); panel.act("download") })
                DropdownMenuItem(text = { Text("Add missing quests") }, enabled = idle, onClick = { close(); panel.act("add-missing") })
                DropdownMenuItem(text = { Text("Rebuild all") }, enabled = idle, onClick = { close(); panel.act("rebuild-all") })
                DropdownMenuItem(text = { Text("Update range…") }, enabled = idle, onClick = { close(); ranging = true })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. The Loader is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        var search by remember { mutableStateOf(view.search) }
        LaunchedEffect(search) {
            if (search != view.search) {
                delay(300)
                panel.act("search", JSONObject().put("search", search))
            }
        }
        // A landscape phone has no height to pin the fields: they scroll away with the quests.
        val list = rememberLazyListState()
        // Another page starts at its first quest, the search just above it.
        var shownPage by remember { mutableStateOf(view.page) }
        LaunchedEffect(view.page) {
            if (view.page != shownPage) {
                shownPage = view.page
                list.scrollToItem(1)
            }
        }
        LazyColumn(Modifier.weight(1f), state = list) {
            item(key = "load") { LoadRow(panel) }
            item(key = "search") {
                OutlinedTextField(
                    value = search,
                    onValueChange = { search = it },
                    placeholder = { Text("Search the cache: name or ID") },
                    leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
                    trailingIcon = if (search.isNotEmpty()) {
                        { IconButton(onClick = { search = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } }
                    } else {
                        null
                    },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 12.dp, bottom = 4.dp),
                )
            }
            if (view.quests.isEmpty()) {
                item(key = "empty") {
                    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                        val (title, about) = when {
                            view.total == 0u && view.status.busy -> "Getting quests…" to "Reading the quest cache."
                            view.total == 0u -> "The quest cache is empty" to "Download the community cache from the menu, or log in and use Add missing quests."
                            else -> "No quests match" to "Search by part of a name or an ID."
                        }
                        Text(title, style = MaterialTheme.typography.bodyLarge)
                        Text(about, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 2.dp))
                    }
                }
            }
            items(view.quests, key = { "q:${it.id}" }) { q ->
                val on = q.id in panel.checked
                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 1.dp)
                        .clip(RoundedCornerShape(10.dp))
                        .background(if (on) Turu.AccentTint else Color.Transparent)
                        .clickable { panel.toggle(q) }
                        .heightIn(min = 48.dp)
                        .padding(end = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = on, onCheckedChange = { panel.toggle(q) })
                    Text(q.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    Spacer(Modifier.width(8.dp))
                    Text("${q.id}", style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"), color = Turu.Muted)
                }
            }
            if (view.matches > 0u) {
                item(key = "pager") {
                    val first = view.page.toLong() * 50 + 1
                    val last = minOf(first + view.quests.size - 1, view.matches.toLong())
                    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            "%,d to %,d of %,d".format(first, last, view.matches.toLong()),
                            style = MaterialTheme.typography.bodySmall,
                            color = Turu.Muted,
                            modifier = Modifier.weight(1f).padding(start = 8.dp),
                        )
                        IconButton(onClick = { panel.act("search", JSONObject().put("page", (view.page - 1u).toLong())) }, enabled = view.page > 0u) {
                            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowLeft, contentDescription = "Previous page")
                        }
                        IconButton(onClick = { panel.act("search", JSONObject().put("page", (view.page + 1u).toLong())) }, enabled = view.page + 1u < view.pages) {
                            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = "Next page")
                        }
                    }
                }
            }
        }
        if (!WindowInsets.isImeVisible) Foot(view, panel)
    }
    if (ranging) {
        NameDialog(
            title = "Update range",
            initial = "",
            confirm = "Update",
            note = "The first and last quest IDs, like 2566, 2599. Loads them in game into the cache.",
            onDismiss = { ranging = false },
        ) {
            ranging = false
            panel.act("update-range", JSONObject().put("input", it))
        }
    }
}

/** Shop or Quests, the IDs and Load. */
@Composable
private fun LoadRow(panel: LoaderPanel) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) {
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
            listOf("Quests" to true, "Shop" to false).forEachIndexed { i, (label, quests) ->
                SegmentedButton(
                    selected = panel.quests == quests,
                    onClick = { panel.quests = quests },
                    shape = SegmentedButtonDefaults.itemShape(i, 2),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = Turu.AccentTint,
                        activeContentColor = Turu.Text,
                        inactiveContainerColor = Color.Transparent,
                        inactiveContentColor = Turu.Muted,
                        activeBorderColor = Turu.Edge,
                        inactiveBorderColor = Turu.Edge,
                    ),
                    icon = {},
                ) { Text(label) }
            }
        }
        Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = panel.ids,
                onValueChange = { panel.ids = it },
                placeholder = { Text(if (panel.quests) "Quest IDs, like 2566, 2567" else "A shop's ID", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { panel.load() }),
                modifier = Modifier.weight(1f),
            )
            Button(onClick = panel::load, enabled = panel.ids.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Load") }
        }
    }
}

/** What runs (with Cancel) or the notice, then the checked quests' actions; nothing when idle. */
@Composable
private fun Foot(view: LoaderView, panel: LoaderPanel) {
    val status = view.status
    val count = panel.checked.size
    val line = status.busy || status.notice != null || panel.error != null
    if (!line && count == 0) return
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
    if (line) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).heightIn(min = 40.dp), verticalAlignment = Alignment.CenterVertically) {
            if (status.busy) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = Turu.Accent)
                Spacer(Modifier.width(10.dp))
            }
            val (text, color) = when {
                panel.error != null -> panel.error!! to Turu.Danger
                status.busy -> status.progress.ifEmpty { "Working…" } to Turu.Muted
                else -> status.notice!!.text to if (status.notice.ok) Turu.Success else Turu.Warning
            }
            Text(text, style = MaterialTheme.typography.bodySmall, color = color, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            if (status.cancellable) {
                TextButton(onClick = { panel.act("cancel") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Cancel", color = Turu.Danger) }
            }
        }
    }
    if (count > 0) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Spacer(Modifier.weight(1f))
            CopyMenu(panel)
            // For one quest at a time (VibeSkua fakes the one clicked last).
            panel.checked.keys.singleOrNull()?.let { id ->
                OutlinedButton(
                    onClick = { panel.act("fake-complete", JSONObject().put("id", id)) },
                    enabled = !status.busy,
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Fake complete", maxLines = 1) }
            }
            Button(onClick = panel::loadChecked, modifier = Modifier.heightIn(min = 48.dp)) { Text("Load $count", maxLines = 1) }
        }
    }
}

@Composable
private fun CopyMenu(panel: LoaderPanel) {
    val context = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Box {
        FilledTonalButton(onClick = { open = true }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Copy") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            for ((label, how) in listOf("IDs" to "ids", "Names" to "names", "As // Name | ID" to "comments")) {
                DropdownMenuItem(text = { Text(label) }, onClick = {
                    open = false
                    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Quests", panel.copied(how)))
                })
            }
        }
    }
}
