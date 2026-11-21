package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.RepoView
import com.muktiarafi.turu.models.RowView
import com.muktiarafi.turu.models.Tone
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * Search Scripts' copy of the tab on screen (`turu_frontend::models::repo`): its own client, read
 * only while the page shows (the view searches the whole repository each time it's read).
 */
class SearchPanel {
    var view by mutableStateOf<RepoView?>(null)
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
            api, token, models = listOf("repo"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<RepoView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/repo doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: String, body: JSONObject = JSONObject(), done: (String?) -> Unit = {}) {
        error = null
        val client = client ?: return done("The tab isn't up yet")
        client.act("repo", action, body) { failed ->
            error = failed
            done(failed)
        }
    }

    fun query(field: String, value: Any) = act("query", JSONObject().put(field, value))

    /**
     * Hands the script at `path` to the page that asked (`pick`), downloading it first if needed.
     * `done` gets the error (or null) and whether it's done now; false while the repository's task
     * downloads it (the launch script is set when that ends).
     */
    fun pick(path: String, pick: ScriptPick, done: (String?, Boolean) -> Unit) {
        error = null
        val client = client ?: return done("The tab isn't up yet", false)
        val (model, action, body) = when (pick) {
            is ScriptPick.Army -> Triple("army", "send", JSONObject().put("command", "script").put("path", path).put("keys", org.json.JSONArray(pick.keys)))
            ScriptPick.Launch -> Triple("repo", "launch", JSONObject().put("path", path))
        }
        var ready = true
        client.act(model, action, body, answer = { ready = it.optBoolean("ready", true) }) { failed ->
            error = failed
            done(failed, ready)
        }
    }
}


private fun toneColor(tone: Tone) = when (tone) {
    Tone.SUCCESS -> Turu.Success
    Tone.WARNING -> Turu.Warning
    Tone.MUTED -> Turu.Muted
}

/**
 * A file name run together ("DemandingApprovalFromNulgath") in words, so it can wrap; a name
 * with spaces stays as written.
 */
private fun shownName(name: String): String =
    if (' ' in name) name else name.replace(Regex("(?<=[a-z])(?=[A-Z])|(?<=[A-Za-z])(?=[0-9])"), " ")

/** The row's line under its name: its description, else its folder (an index's "null" is none). */
private fun about(row: RowView): String =
    row.description.trim().takeUnless { it.isEmpty() || it.equals("null", ignoreCase = true) }
        ?: row.path.substringBeforeLast('/', "").replace('/', ' ').ifEmpty { "Scripts folder" }

/**
 * The badge a row shows: only what sets it apart. Most scripts are downloaded and up to date, so
 * "Downloaded" goes; a script not yet downloaded says so (it has no badge of its own).
 */
private fun shownBadge(row: RowView): Pair<String, Tone>? = when {
    row.badge == null -> "Not downloaded" to Tone.MUTED
    row.badge.label == "Downloaded" -> null
    else -> row.badge.label to row.badge.tone
}

/** The first row is the best match only when its name starts with what's typed (the sort stays). */
private fun bestMatch(view: RepoView): String? {
    val typed = view.query.search.trim()
    val first = view.rows.firstOrNull() ?: return null
    if (typed.isEmpty()) return null
    val hit = listOf(first.name, shownName(first.name)).any { it.startsWith(typed, ignoreCase = true) }
    return first.path.takeIf { hit }
}

@Composable
private fun Badge(label: String, tone: Tone) {
    Text(
        label,
        style = MaterialTheme.typography.labelSmall,
        color = toneColor(tone),
        maxLines = 1,
        modifier = Modifier
            .background(toneColor(tone).copy(alpha = 0.14f), RoundedCornerShape(6.dp))
            .padding(horizontal = 6.dp, vertical = 1.dp),
    )
}

/**
 * Search Scripts (docs/UI.md, "Native shells"; VibeSkua's Script Repo): the search box, the
 * filters as chips (folder, Pinned, Local, the sort, what the search looks in), the rows with
 * Start and a menu (Load, Add to the queue, Download or Update, Pin), and the repository's task
 * and its buttons at the foot. Delete and Reset Scripts stay in the egui window.
 *
 * Opened to pick ([ShellState.scriptPick]: Army Control's clients, Accounts' launch script) it
 * picks instead: a bar says for what, each row offers its button ("Load in 2 clients", "Use"),
 * and a pick goes back to the page that asked, once a script that needs a download has it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SearchPage(state: ShellState, search: SearchPanel, fullWindow: () -> Unit) {
    // Leaving the page ends a pick (Back, Cancel, or the pick itself).
    DisposableEffect(Unit) { onDispose { search.follow(null); state.scriptPick = null } }
    val pick = state.scriptPick
    // A launch script being downloaded: the page goes back once the repository's task ends, or
    // stays to show why it failed.
    var downloading by remember { mutableStateOf(false) }
    LaunchedEffect(downloading, search.view?.busy) {
        val v = search.view ?: return@LaunchedEffect
        if (downloading && v.busy == null) {
            downloading = false
            if (v.message?.failed != true) state.back()
        }
    }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { search.follow(state.tabs.active) }
    val view = search.view
    Column(Modifier.fillMaxSize()) {
        PageBar("Search Scripts", onBack = { state.back() }) {
            view?.let {
                Text(
                    if (it.query.search.isBlank() && it.query.activeFilters == 0UL) "${it.total} scripts" else "${it.found} found",
                    style = MaterialTheme.typography.labelMedium,
                    color = Turu.Muted,
                )
                RepoMenu(it, search, fullWindow)
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        pick?.let { PickBar(it) { state.scriptPick = null; state.back() } }
        SearchBox(view, search)
        // While typing, the keyboard leaves little height: the filters step aside for results.
        if (!WindowInsets.isImeVisible) Filters(view, search)
        val top = bestMatch(view)
        LazyColumn(Modifier.weight(1f).padding(top = 4.dp)) {
            if (view.rows.isEmpty()) {
                item {
                    Column(Modifier.padding(20.dp)) {
                        Text(if (view.total == 0UL) "No scripts yet" else "No scripts found", style = MaterialTheme.typography.titleSmall)
                        Text(
                            if (view.total == 0UL) "Refresh gets the repository's list." else "Try other words, or clear the filters.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Turu.Muted,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                        if (view.query.activeFilters > 0UL) {
                            TextButton(onClick = { search.act("query", JSONObject().put("clearFilters", true)) }) { Text("Clear the filters") }
                        }
                    }
                }
            }
            items(view.rows, key = { it.path }) { row ->
                if (pick != null) {
                    PickRow(row, topHit = row.path == top, pick = pick) {
                        search.pick(row.path, pick) { failed, ready ->
                            when {
                                failed != null -> {}
                                ready -> state.back()
                                // Downloading first: back once the task ends (the foot shows it).
                                else -> downloading = true
                            }
                        }
                    }
                } else {
                    ScriptRow(row, topHit = row.path == top, search = search) { state.push(Page.Scripts) }
                }
            }
            if (view.found.toLong() > view.rows.size) {
                item {
                    Text(
                        "${view.found.toLong() - view.rows.size} more: narrow the search to see them",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                    )
                }
            }
        }
        // While typing, the foot's note would take what little height the keyboard leaves.
        if (!WindowInsets.isImeVisible) Footer(view, search)
    }
}

@Composable
private fun SearchBox(view: RepoView, search: SearchPanel) {
    // Typed here and sent a moment after the typing stops, so each key doesn't search again.
    var text by remember { mutableStateOf(view.query.search) }
    LaunchedEffect(text) {
        if (text != view.query.search) {
            delay(250)
            search.query("search", text)
        }
    }
    val focus = LocalFocusManager.current
    OutlinedTextField(
        value = text,
        onValueChange = { text = it },
        placeholder = { Text("Search scripts", maxLines = 1) },
        leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
        trailingIcon = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ScopeButton(view, search)
                if (text.isNotEmpty()) {
                    IconButton(onClick = { text = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") }
                }
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(12.dp),
        colors = fieldColors(),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        // Search shows the results: the keyboard hides them on a landscape phone.
        keyboardActions = KeyboardActions(onSearch = { search.query("search", text); focus.clearFocus() }),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
    )
}

/** What the search looks in, inside the box: "All fields", "Name", ... */
@Composable
private fun ScopeButton(view: RepoView, search: SearchPanel) {
    var open by remember { mutableStateOf(false) }
    val words = view.scopes.firstOrNull { it.first == view.query.scope }?.second ?: "All fields"
    Box {
        TextButton(
            onClick = { open = true },
            contentPadding = PaddingValues(start = 8.dp, end = 4.dp),
            colors = ButtonDefaults.textButtonColors(contentColor = if (view.query.scope != "all") Turu.Text else Turu.Muted),
            modifier = Modifier.semantics { contentDescription = "Search in: $words" },
        ) {
            Text(words, style = MaterialTheme.typography.labelLarge, maxLines = 1)
            Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            Text("Search in", style = MaterialTheme.typography.labelMedium, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
            for ((value, label) in view.scopes) {
                DropdownMenuItem(text = { Text(label) }, onClick = { open = false; search.query("scope", value) })
            }
        }
    }
}

/** VibeSkua's sorts in words: (sort, descending) to how it reads. */
private val SORTS = listOf(
    ("name" to false) to "Name A to Z",
    ("name" to true) to "Name Z to A",
    ("added" to true) to "Newest first",
    ("added" to false) to "Oldest first",
)

@Composable
private fun Filters(view: RepoView, search: SearchPanel) {
    val q = view.query
    LazyRow(
        Modifier.padding(top = 8.dp),
        contentPadding = PaddingValues(horizontal = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            MenuChip(if (q.category == "All") "All folders" else q.category, q.category != "All", view.categories.map { it to (if (it == "All") "All folders" else it) }) {
                search.query("category", it)
            }
        }
        item {
            // The chip says it short so the four fit a phone's panel; its menu in full.
            val now = when (q.sort to q.descending) {
                "name" to true -> "Z to A"
                "added" to true -> "Newest"
                "added" to false -> "Oldest"
                else -> "A to Z"
            }
            MenuChip(now, selected = false, SORTS.mapIndexed { i, it -> "$i" to it.second }) { picked ->
                val (sort, descending) = SORTS[picked.toInt()].first
                search.act("query", JSONObject().put("sort", sort).put("descending", descending))
            }
        }
        item { ToggleChip("Pinned", q.pinnedOnly) { search.query("pinnedOnly", !q.pinnedOnly) } }
        item { ToggleChip("On this device", q.localOnly) { search.query("localOnly", !q.localOnly) } }
    }
}

@Composable
private fun ToggleChip(label: String, selected: Boolean, menu: Boolean = false, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label, maxLines = 1) },
        trailingIcon = if (menu) {
            { Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp)) }
        } else {
            null
        },
        colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
        border = FilterChipDefaults.filterChipBorder(enabled = true, selected = selected, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
        modifier = Modifier.heightIn(min = 40.dp),
    )
}

/** A chip that opens a menu of `options` (value, words). */
@Composable
private fun MenuChip(label: String, selected: Boolean, options: List<Pair<String, String>>, onPick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        ToggleChip(label, selected, menu = true) { open = true }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            for ((value, words) in options) {
                DropdownMenuItem(text = { Text(words) }, onClick = { open = false; onPick(value) })
            }
        }
    }
}

/** What Search Scripts picks for, and Cancel (back to the page that asked, nothing picked). */
@Composable
private fun PickBar(pick: ScriptPick, cancel: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, bottom = 8.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(Turu.AccentTint)
            .padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            pick.bar,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        TextButton(onClick = cancel, modifier = Modifier.heightIn(min = 48.dp)) { Text("Cancel") }
    }
}

/** A script while picking: name, badge and description; a tap (or its button) picks it. */
@Composable
private fun PickRow(row: RowView, topHit: Boolean, pick: ScriptPick, onPick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .clickable(enabled = !row.working, onClick = onPick)
            .padding(start = 12.dp, top = 8.dp, bottom = 8.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RowText(row)
        if (row.working) {
            CircularProgressIndicator(Modifier.padding(horizontal = 16.dp).size(20.dp), strokeWidth = 2.dp)
        } else {
            val label = pick.button
            // A long account name stops at the button's width.
            val modifier = Modifier.heightIn(min = 48.dp).widthIn(max = 200.dp)
            val text: @Composable () -> Unit = { Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            if (topHit) {
                Button(onClick = onPick, modifier = modifier) { text() }
            } else {
                FilledTonalButton(onClick = onPick, modifier = modifier) { text() }
            }
        }
    }
}

/** A row's name (a star when pinned) and, under it, what sets it apart and what it does. */
@Composable
private fun RowScope.RowText(row: RowView) {
    Column(Modifier.weight(1f)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (row.pinned) {
                Icon(Icons.Rounded.Star, contentDescription = "Pinned", tint = Turu.Warning, modifier = Modifier.size(14.dp))
                Spacer(Modifier.width(4.dp))
            }
            Text(shownName(row.name), style = MaterialTheme.typography.bodyLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
            shownBadge(row)?.let { (label, tone) ->
                Badge(label, tone)
                Spacer(Modifier.width(6.dp))
            }
            Text(about(row), style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/**
 * One script: its name and line; Start (filled for the best match, else a text button, so the
 * list doesn't read as a column of buttons) and its menu, Start first.
 */
@Composable
private fun ScriptRow(row: RowView, topHit: Boolean, search: SearchPanel, started: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    val body = JSONObject().put("path", row.path)
    val start = { search.act("start", body) { if (it == null) started() } }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClickLabel = "more for ${row.name}") { menu = true }
            .padding(start = 12.dp, top = 6.dp, bottom = 6.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RowText(row)
        if (row.working) {
            CircularProgressIndicator(Modifier.padding(horizontal = 16.dp).size(20.dp), strokeWidth = 2.dp)
        } else if (topHit) {
            Button(onClick = start, enabled = row.start.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Start") }
        } else {
            TextButton(onClick = start, enabled = row.start.enabled, modifier = Modifier.heightIn(min = 48.dp)) { Text("Start") }
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "More for ${row.name}", tint = Turu.Muted) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                Text(row.path, style = MaterialTheme.typography.labelSmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                row.badge?.let {
                    Text("${it.label}: ${it.meaning}", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                }
                // A touch screen has no tooltips: why an item rests, under it.
                MenuAction("Start", row.start) { menu = false; start() }
                MenuAction("Load", row.load) { menu = false; search.act("load", body) }
                MenuAction("Add to the queue", row.queue) { menu = false; search.act("queue", body) }
                row.download?.let { label ->
                    MenuAction(label, com.muktiarafi.turu.models.Control(true, "")) {
                        menu = false
                        search.act(if (label.startsWith("Replace")) "replace" else "download", body)
                    }
                }
                MenuAction(if (row.pinned) "Unpin" else "Pin", com.muktiarafi.turu.models.Control(true, "")) {
                    menu = false
                    search.act("pin", JSONObject().put("path", row.path).put("pinned", !row.pinned))
                }
            }
        }
    }
}

@Composable
private fun MenuAction(label: String, control: com.muktiarafi.turu.models.Control, onClick: () -> Unit) {
    DropdownMenuItem(
        text = {
            Column {
                Text(label, color = if (control.enabled) Turu.Text else Turu.Muted)
                if (!control.enabled && control.reason.isNotEmpty()) {
                    Text(control.reason, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
            }
        },
        enabled = control.enabled,
        onClick = onClick,
    )
}

/** The repository's task (its progress and Cancel) or its last outcome, on one line at the foot. */
@Composable
private fun Footer(view: RepoView, search: SearchPanel) {
    val busy = view.busy
    val note = search.error?.let { it to true } ?: view.message?.let { it.text to it.failed }
    // A done task's note ("Started ...", "Another tab updated ...") goes after a while; a failure stays.
    var seen by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(note) {
        if (note != null && !note.second) {
            delay(8_000)
            seen = note.first
        }
    }
    val shown = note?.takeUnless { !it.second && it.first == seen }
    if (busy == null && shown == null) return
    Column(Modifier.fillMaxWidth()) {
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        if (busy != null) {
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("${busy.task}: ${busy.message}", style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (busy.total.toLong() > 0) {
                        LinearProgressIndicator(
                            progress = { busy.done.toFloat() / busy.total.toFloat() },
                            modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                            color = Turu.Accent,
                            trackColor = Turu.Track,
                        )
                    }
                }
                TextButton(onClick = { search.act("cancel") }) { Text("Cancel") }
            }
        } else if (shown != null) {
            Text(
                shown.first,
                style = MaterialTheme.typography.bodySmall,
                color = if (shown.second) Turu.Danger else Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
    }
}

/** The page bar's ⋮: Update all (with how many), Download all, Refresh. */
@Composable
private fun RepoMenu(view: RepoView, search: SearchPanel, fullWindow: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Rounded.MoreVert, contentDescription = "Update, download, refresh", tint = Turu.Muted)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            val update = if (view.outdated.toLong() > 0) "Update all (${view.outdated})" else "Update all"
            MenuAction(update, view.updateAll) { open = false; search.act("update-all") }
            MenuAction("Download all", view.downloadAll) { open = false; search.act("download-all") }
            MenuAction("Refresh the list", view.refresh) { open = false; search.act("refresh") }
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            DropdownMenuItem(
                text = {
                    Column {
                        Text("Full window")
                        Text("Delete a script, Reset Scripts", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                    }
                },
                onClick = { open = false; fullWindow() },
            )
        }
    }
}
