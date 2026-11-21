package com.muktiarafi.turu

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.GrabRowView
import com.muktiarafi.turu.models.GrabberView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * The Grabber's copy of the tab on screen (`turu_frontend::models::grabber`), read while the page
 * shows. The list on show, its search and page are the slot's (a list can be long: it sends a page);
 * the checked rows (their positions, in one grab) are the page's own.
 */
class GrabberPanel {
    var view by mutableStateOf<GrabberView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** The checked rows' positions, and the grab they belong to. */
    val checked = mutableStateMapOf<Long, Unit>()
    private var checkedIn: Pair<String, ULong>? = null

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
            api, token, models = listOf("grabber"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<GrabberView>(text) }
                    .onSuccess { v ->
                        // Another list or a fresh grab: the checks were of other rows.
                        val now = v.kind to v.revision
                        if (checkedIn != null && checkedIn != now) checked.clear()
                        checkedIn = now
                        view = v
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/grabber doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: String, body: JSONObject = JSONObject(), answer: (JSONObject) -> Unit = {}) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("grabber", "do", body.put("action", action), answer) { error = it }
    }

    private fun picked(): JSONObject {
        val v = view
        return JSONObject()
            .put("positions", JSONArray(checked.keys.toList()))
            .put("revision", v?.revision?.toLong() ?: 0L)
    }

    fun run(label: String) = act("run", picked().put("label", label))

    fun copy(context: Context) = act("copy", picked()) { answer ->
        val text = answer.optString("text").takeIf { !answer.isNull("text") && it.isNotEmpty() } ?: return@act
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Grabber", text))
    }

    fun toggle(row: GrabRowView, multiple: Boolean) {
        val at = row.position.toLong()
        if (checked.remove(at) != null) return
        if (!multiple) checked.clear()
        checked[at] = Unit
    }
}

/**
 * Tools & Helpers > Grabber (docs/UI.md, "Native shells"): the list (a drop-down of the ten) with
 * Grab, its search, then the rows (a tap checks one; the info button shows its fields), a page at a time. The
 * foot: what runs with Cancel, and the list's own actions. An action's quantity question and
 * message box are dialogs. ⋮: Copy (the checked rows, or all shown), Full window (Export).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun GrabberPage(state: ShellState, panel: GrabberPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    Column(Modifier.fillMaxSize()) {
        val context = LocalContext.current
        PageBar("Grabber", onBack = { state.back() }) {
            FullWindowMenu("The list beside its details, and Export to a file", fullWindow) { close ->
                val n = panel.checked.size
                val can = view != null && view.grabbed > 0u && !view.busy
                DropdownMenuItem(
                    text = { Text(if (n > 0) "Copy $n checked" else "Copy all shown") },
                    enabled = can,
                    onClick = { close(); panel.copy(context) },
                )
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. The Grabber is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        var search by remember(view.kind) { mutableStateOf(view.search) }
        LaunchedEffect(search) {
            if (search != view.search) {
                delay(300)
                panel.act("search", JSONObject().put("search", search))
            }
        }
        val list = rememberLazyListState()
        var shownPage by remember { mutableStateOf(view.kind to view.page) }
        LaunchedEffect(view.kind, view.page) {
            if ((view.kind to view.page) != shownPage) {
                shownPage = view.kind to view.page
                list.scrollToItem(0)
            }
        }
        // A landscape phone has no height to pin the fields: they scroll away with the rows.
        LazyColumn(Modifier.weight(1f), state = list) {
            item(key = "pick") {
                // Grab spans the field's box at any font scale: the row's height, less the half
                // line of the floated label (bodySmall) that the outlined field keeps above its box.
                val labelHalf = with(LocalDensity.current) { (MaterialTheme.typography.bodySmall.lineHeight.toPx() / 2).toDp() }
                Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min).padding(horizontal = 12.dp, vertical = 4.dp)) {
                    Column(Modifier.weight(1f)) {
                        Picker(
                            label = "List",
                            value = view.kind,
                            options = view.lists.map { it.title to if (it.count > 0u) "${it.title} - ${it.count} grabbed" else it.title },
                            loading = false,
                        ) { panel.act("select", JSONObject().put("kind", it)) }
                    }
                    Spacer(Modifier.width(8.dp))
                    Button(onClick = { panel.act("grab") }, enabled = !view.busy, modifier = Modifier.fillMaxHeight().padding(top = labelHalf)) { Text("Grab") }
                }
            }
            if (view.grabbed > 0u) {
                item(key = "search") {
                    OutlinedTextField(
                        value = search,
                        onValueChange = { search = it },
                        placeholder = { Text("Search ${view.kind.lowercase()}") },
                        leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
                        trailingIcon = if (search.isNotEmpty()) {
                            { IconButton(onClick = { search = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } }
                        } else {
                            null
                        },
                        singleLine = true,
                        shape = MaterialTheme.shapes.medium,
                        colors = fieldColors(),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
                    )
                }
            }
            if (view.rows.isEmpty()) {
                item(key = "empty") {
                    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                        val (title, about) = when {
                            view.busy && view.grabbed == 0u -> "Grabbing…" to "Reading ${view.kind.lowercase()} from the game."
                            view.grabbed == 0u -> "Nothing grabbed yet" to "Grab reads ${view.kind.lowercase()} from the game as it is now."
                            else -> "Nothing matches" to "Search by part of a name or an ID."
                        }
                        Text(title, style = MaterialTheme.typography.bodyLarge)
                        Text(about, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 2.dp))
                    }
                }
            }
            items(view.rows, key = { "r:${it.position}" }) { row -> GrabRow(row, row.position.toLong() in panel.checked, view, panel) }
            if (view.pages > 1u) {
                item(key = "pager") {
                    val first = view.page.toLong() * 100 + 1
                    val last = minOf(first + view.rows.size - 1, view.matches.toLong())
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
    view?.let { Dialogs(it, panel) }
}

@Composable
private fun GrabRow(row: GrabRowView, on: Boolean, view: GrabberView, panel: GrabberPanel) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(selectionTint(on))
            .clickable { panel.toggle(row, view.multiple) }
            .heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = on, onCheckedChange = { panel.toggle(row, view.multiple) })
        Text(row.text, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        Spacer(Modifier.width(8.dp))
        Text("${row.id}", style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"), color = Turu.Muted)
        IconButton(onClick = { panel.act("detail", JSONObject().put("position", row.position.toLong())) }) {
            Icon(Icons.Rounded.Info, contentDescription = "${row.text}: its fields", tint = Turu.Muted)
        }
    }
}

/**
 * What runs (with Cancel), the status or why the actions are off, then the list's actions in an
 * even grid on the page's edges.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Foot(view: GrabberView, panel: GrabberPanel) {
    val count = panel.checked.size
    // With nothing grabbed there's nothing to check: only what needs no rows (Unregister All).
    val shown = if (view.grabbed == 0u) view.actions.filter { it.label in WITHOUT_ROWS } else view.actions
    // Why buttons are off, as a disabled button can't say: a fight waits for the script, or
    // nothing is checked (VibeSkua answers "No quests found/selected." after the tap).
    val reason = shown.firstOrNull { !it.control.enabled && it.control.reason != "Busy" }?.control?.reason
        ?: if (count == 0 && view.grabbed > 0u && shown.any { it.label !in WITHOUT_ROWS }) "Check ${view.kind.lowercase()} in the list to use these" else null
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
    val status = panel.error ?: view.message ?: if (count > 0) "$count checked" else reason
    if (status != null || view.busy) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 2.dp).heightIn(min = 36.dp), verticalAlignment = Alignment.CenterVertically) {
            if (view.busy) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = Turu.Accent)
                Spacer(Modifier.width(10.dp))
            }
            Text(
                status ?: "Working…",
                style = MaterialTheme.typography.bodySmall,
                color = if (panel.error != null) Turu.Danger else Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (view.busy) {
                TextButton(onClick = { panel.act("cancel") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Cancel", color = Turu.Danger) }
            }
        }
    }
    if (shown.isEmpty()) return
    // As few rows as fit (up to four a row, 96 dp each), balanced: four in one row, five as 3 + 2.
    BoxWithConstraints(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 8.dp)) {
        val fit = (maxWidth / 96.dp).toInt().coerceIn(1, 4)
        val rows = (shown.size + fit - 1) / fit
        val columns = (shown.size + rows - 1) / rows
        FlowRow(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            maxItemsInEachRow = columns,
        ) {
            for (a in shown) {
                // Turu: VibeSkua's buttons stay on and answer "No ... found/selected." after the
                // tap; on a touch screen the page turns them off until a row is checked instead.
                val on = a.control.enabled && (count > 0 || a.label in WITHOUT_ROWS)
                OutlinedButton(
                    onClick = { panel.run(a.label) },
                    enabled = on,
                    colors = ButtonDefaults.outlinedButtonColors(
                        contentColor = if (a.destructive) Turu.Danger else Turu.Text,
                        disabledContentColor = Turu.Muted,
                    ),
                    // Off, a button keeps its outline: the row stays legible as the list's tools.
                    border = BorderStroke(1.dp, if (on) Turu.Edge else Turu.Edge.copy(alpha = 0.6f)),
                    contentPadding = PaddingValues(horizontal = 12.dp),
                    modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                ) { Text(a.label, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            }
        }
    }
}

/** The actions that run without checked rows (`turu_bot::grabber::prepare`). */
private val WITHOUT_ROWS = setOf("Unregister All")

/** The quantity an action asks for, its message box, and a row's fields. */
@Composable
private fun Dialogs(view: GrabberView, panel: GrabberPanel) {
    view.ask?.let { ask ->
        var text by remember(ask) { mutableStateOf("") }
        val focus = remember { FocusRequester() }
        LaunchedEffect(ask) { focus.requestFocus() }
        AlertDialog(
            onDismissRequest = { panel.act("cancel-ask") },
            title = { Text(ask.title) },
            text = {
                Column {
                    Text(ask.text, color = Turu.Muted)
                    OutlinedTextField(
                        value = text,
                        onValueChange = { text = it.filter { c -> c.isDigit() } },
                        singleLine = true,
                        colors = fieldColors(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { panel.act("quantity", JSONObject().put("value", text)) }),
                        modifier = Modifier.padding(top = 8.dp).focusRequester(focus),
                    )
                }
            },
            confirmButton = { TextButton(onClick = { panel.act("quantity", JSONObject().put("value", text)) }, enabled = text.isNotEmpty()) { Text("OK") } },
            dismissButton = { TextButton(onClick = { panel.act("cancel-ask") }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    view.notice?.let { notice ->
        AlertDialog(
            onDismissRequest = { panel.act("dismiss") },
            title = { Text(notice.title) },
            text = { Text(notice.text) },
            confirmButton = { TextButton(onClick = { panel.act("dismiss") }) { Text("OK") } },
            containerColor = Turu.Card,
        )
    }
    view.details?.let { d ->
        val close = { panel.act("detail", JSONObject()) }
        AlertDialog(
            onDismissRequest = close,
            title = { Text(d.title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    for (f in d.fields) {
                        Text(f.key, style = MaterialTheme.typography.labelMedium, color = Turu.Muted, modifier = Modifier.padding(top = 6.dp))
                        Text(f.value, style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace))
                    }
                }
            },
            confirmButton = { TextButton(onClick = close) { Text("Close") } },
            containerColor = Turu.Card,
        )
    }
}
