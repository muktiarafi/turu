package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TriStateCheckbox
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.JunkAct
import com.muktiarafi.turu.models.JunkRowView
import com.muktiarafi.turu.models.JunkView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json

/**
 * Junk Items' copy of the tab on screen (`turu_frontend::models::junk`), read while the page shows.
 * The search, filters, page and selection are the slot's; a check shows at once, before the slot's
 * next view says so.
 */
class JunkPanel {
    var view by mutableStateOf<JunkView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** Checks sent and not yet in a view, by "id:category". */
    val pending = mutableStateMapOf<String, Boolean>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        val opened = tab != null && tab.id != following?.id
        if (tab?.id != following?.id) error = null
        following = tab
        client?.stop()
        client = null
        view = null
        pending.clear()
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("junk"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<JunkView>(text) }
                    .onSuccess { view = it; pending.clear() }
                    .onFailure { Log.e(ModelClient.TAG, "/models/junk doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
        // VibeSkua refreshes the list when the window is shown.
        if (opened) act(JunkAct.refresh)
    }

    fun act(action: JunkAct, done: () -> Unit = {}) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("junk", action, JunkAct.JsonSerializer) { e ->
            error = e
            if (e == null) done()
        }
    }

    fun checked(row: JunkRowView) = pending[key(row)] ?: row.selected

    fun toggle(row: JunkRowView) {
        val on = !checked(row)
        pending[key(row)] = on
        act(JunkAct.select(row.id, row.category, on))
    }

    private fun key(row: JunkRowView) = "${row.id}:${row.category}"
}

/** A sale or a setting that asks first. */
private enum class JunkAsk { SellSelected, SellAll, SkipWarning, UnmarkAll }

/**
 * Tools & Helpers > Junk Items (docs/UI.md, "Native shells"): the search with Refresh, the
 * category, AC and highlight filters, then the inventory and bank items a page at a time (a tap
 * checks one; the box above checks every shown one). The foot: what the checked items can do (mark
 * or unmark as junk, a quantity goal, sell), else Sell all junk. ⋮: Sell all junk, Unmark all, Skip
 * sell warning, Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun JunkPage(state: ShellState, panel: JunkPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var ask by remember { mutableStateOf<JunkAsk?>(null) }
    var goal by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Junk Items", onBack = { state.back() }) {
            FullWindowMenu("The same list in the egui window", fullWindow) { close ->
                val canSell = view != null && view.junk > 0u && !view.busy
                DropdownMenuItem(
                    text = { Text("Sell all junk (${view?.junk ?: 0u})", color = if (canSell) Turu.Danger else Turu.Muted) },
                    enabled = canSell,
                    onClick = {
                        close()
                        if (view?.skipWarning == true) panel.act(JunkAct.sell_all) else ask = JunkAsk.SellAll
                    },
                )
                DropdownMenuItem(
                    text = { Text("Unmark all") },
                    enabled = view != null && view.junk > 0u,
                    onClick = { close(); ask = JunkAsk.UnmarkAll },
                )
                DropdownMenuItem(
                    text = { Text("Skip sell warning") },
                    trailingIcon = { Checkbox(checked = view?.skipWarning == true, onCheckedChange = null) },
                    enabled = view != null,
                    onClick = {
                        close()
                        // Turning it on asks first (VibeSkua `OnSkipSellWarningChanged`).
                        if (view?.skipWarning == true) panel.act(JunkAct.skip_warning(false)) else ask = JunkAsk.SkipWarning
                    },
                )
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Junk Items is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        var search by remember { mutableStateOf(view.search) }
        LaunchedEffect(search) {
            if (search != view.search) {
                delay(300)
                panel.act(JunkAct.filter(search = search))
            }
        }
        val list = rememberLazyListState()
        var shownPage by remember { mutableStateOf(view.page) }
        LaunchedEffect(view.page) {
            if (view.page != shownPage) {
                shownPage = view.page
                list.scrollToItem(0)
            }
        }
        // A landscape phone has no height to pin the filters: they scroll away with the items.
        LazyColumn(Modifier.weight(1f), state = list) {
            item(key = "search") {
                Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = search,
                        onValueChange = { search = it },
                        placeholder = { Text("Search by ID, name or category") },
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
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = { panel.act(JunkAct.refresh) }, enabled = !view.busy) {
                        Icon(Icons.Rounded.Refresh, contentDescription = "Read the inventory and bank again", tint = Turu.Muted)
                    }
                }
            }
            item(key = "filters") { JunkFilters(view, panel) }
            if (view.total > 0u) item(key = "all") { JunkSelectAll(view, panel) }
            if (view.rows.isEmpty()) {
                item(key = "empty") {
                    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                        val (title, about) = when {
                            view.busy && view.total == 0u -> "Reading items…" to "The inventory, then the bank (loaded first if it isn't)."
                            view.total == 0u -> "No items" to "Log in, then read the inventory and bank again."
                            else -> "Nothing matches" to "Search by part of a name, an ID or a category, or change a filter."
                        }
                        Text(title, style = MaterialTheme.typography.bodyLarge)
                        Text(about, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 2.dp))
                    }
                }
            }
            items(view.rows, key = { "r:${it.id}:${it.category}" }) { row -> JunkRow(row, panel.checked(row)) { panel.toggle(row) } }
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
                        IconButton(onClick = { panel.act(JunkAct.page(view.page - 1u)) }, enabled = view.page > 0u) {
                            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowLeft, contentDescription = "Previous page")
                        }
                        IconButton(onClick = { panel.act(JunkAct.page(view.page + 1u)) }, enabled = view.page + 1u < view.pages) {
                            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = "Next page")
                        }
                    }
                }
            }
        }
        if (goal && view.selected > 0u) {
            // Stays above the keyboard: a dialog's buttons would be under it on a landscape phone.
            JunkGoalBar(view, panel) { goal = false }
        } else if (!WindowInsets.isImeVisible) {
            JunkFoot(
                view,
                panel,
                goal = { goal = true },
                sell = { what -> if (view.skipWarning) panel.act(if (what == JunkAsk.SellAll) JunkAct.sell_all else JunkAct.sell_selected) else ask = what },
            )
        }
    }
    if (view != null) {
        ask?.let { a -> JunkConfirm(a, view, panel) { ask = null } }
    }
}

/** Category, AC and highlight as chips that open their options; a chip filtering is lit. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun JunkFilters(view: JunkView, panel: JunkPanel) {
    FlowRow(
        Modifier.fillMaxWidth().padding(horizontal = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        JunkChoice("Category", view.category, view.categories) { panel.act(JunkAct.filter(category = it)) }
        JunkChoice("AC", view.ac, view.acs) { panel.act(JunkAct.filter(ac = it)) }
        JunkChoice("Highlight", view.highlight, view.highlights) { panel.act(JunkAct.filter(highlight = it)) }
    }
}

/** A filter chip naming its choice ("All Categories"), a menu of `options` under it; the first is no filter. */
@Composable
private fun JunkChoice(label: String, value: String, options: List<String>, pick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val filtering = options.isNotEmpty() && value != options.first()
    Box {
        FilterChip(
            selected = filtering,
            onClick = { open = true },
            label = { Text(value, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            trailingIcon = { Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp)) },
            colors = FilterChipDefaults.filterChipColors(
                selectedContainerColor = Turu.AccentTint,
                selectedLabelColor = Turu.Text,
                selectedTrailingIconColor = Turu.Text,
                labelColor = Turu.Muted,
                iconColor = Turu.Muted,
            ),
            border = FilterChipDefaults.filterChipBorder(
                enabled = true,
                selected = filtering,
                borderColor = Turu.Edge,
                selectedBorderColor = Turu.Accent.copy(alpha = 0.6f),
            ),
            modifier = Modifier.semantics { contentDescription = "$label: $value" },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            for (option in options) {
                DropdownMenuItem(
                    text = { Text(option, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    trailingIcon = if (option == value) {
                        { Icon(Icons.Rounded.Check, contentDescription = "Chosen", tint = Turu.Accent) }
                    } else {
                        null
                    },
                    onClick = {
                        open = false
                        if (option != value) pick(option)
                    },
                )
            }
        }
    }
}

/** The box that checks every shown item (or clears every check), with the counts. */
@Composable
private fun JunkSelectAll(view: JunkView, panel: JunkPanel) {
    val box = when {
        view.selected == 0u -> ToggleableState.Off
        view.allShown -> ToggleableState.On
        else -> ToggleableState.Indeterminate
    }
    Row(Modifier.fillMaxWidth().padding(start = 8.dp, end = 16.dp), verticalAlignment = Alignment.CenterVertically) {
        TriStateCheckbox(
            state = box,
            enabled = view.matches > 0u,
            onClick = { panel.act(JunkAct.select_shown(box == ToggleableState.Off)) },
        )
        val shown = if (view.matches == view.total) "${view.total} items" else "${view.matches} of ${view.total} shown"
        Text(
            if (view.selected > 0u) "${view.selected} checked · $shown" else shown,
            style = MaterialTheme.typography.bodyMedium,
            color = if (view.selected > 0u) Turu.Text else Turu.Muted,
            modifier = Modifier.weight(1f),
        )
        if (view.junk > 0u) JunkChip("${view.junk} junk", JunkTone.Danger)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun JunkRow(row: JunkRowView, on: Boolean, toggle: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(
                when {
                    on -> Turu.AccentTint
                    // VibeSkua tints an equipped row.
                    row.equipped -> Turu.Warning.copy(alpha = 0.08f)
                    else -> Color.Transparent
                },
            )
            .clickable(onClick = toggle)
            .heightIn(min = 56.dp)
            .padding(end = 12.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = on, onCheckedChange = { toggle() })
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    row.name,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (row.equipped) FontWeight.SemiBold else null,
                    color = if (row.equipped) Turu.Warning else Turu.Text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Spacer(Modifier.width(8.dp))
                Text("${row.id}", style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"), color = Turu.Muted)
            }
            FlowRow(
                Modifier.padding(top = 2.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                Text(row.quantities, style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"), color = Turu.Muted)
                Text(row.category, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                if (row.ac) JunkChip("AC", JunkTone.Plain)
                if (row.equipped) JunkChip("Equipped", JunkTone.Plain)
                if (row.favorite) {
                    Icon(Icons.Rounded.Star, contentDescription = "Favourited in AQW", tint = Turu.Warning, modifier = Modifier.size(16.dp))
                }
                if (row.nearCap) JunkChip("Near cap", JunkTone.Warning)
                row.goal?.let { JunkChip("Goal $it", JunkTone.Plain) }
                if (row.junk) JunkChip("Junk", JunkTone.Danger)
            }
        }
    }
}

private enum class JunkTone { Plain, Warning, Danger }

/** VibeSkua's chips: `chip`, `chip warning`, `chip danger`. */
@Composable
private fun JunkChip(text: String, tone: JunkTone) {
    val color = when (tone) {
        JunkTone.Plain -> Turu.Text
        JunkTone.Warning -> Turu.Warning
        JunkTone.Danger -> Turu.Danger
    }
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = color,
        maxLines = 1,
        modifier = Modifier
            .clip(MaterialTheme.shapes.small)
            .background(if (tone == JunkTone.Plain) Turu.Track else color.copy(alpha = 0.16f))
            .border(1.dp, if (tone == JunkTone.Plain) Turu.Edge else color.copy(alpha = 0.5f), MaterialTheme.shapes.small)
            .padding(horizontal = 7.dp, vertical = 1.dp),
    )
}

/** What runs or failed, what the checked items can do, and Sell all junk. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun JunkFoot(view: JunkView, panel: JunkPanel, goal: () -> Unit, sell: (JunkAsk) -> Unit) {
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
    val failed = panel.error ?: view.error
    val status = failed ?: if (view.busy) "Working…" else null
    if (status != null) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 2.dp).heightIn(min = 36.dp), verticalAlignment = Alignment.CenterVertically) {
            if (view.busy) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = Turu.Accent)
                Spacer(Modifier.width(10.dp))
            }
            Text(
                status,
                style = MaterialTheme.typography.bodySmall,
                color = if (failed != null) Turu.Danger else Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
        }
    }
    if (view.selected > 0u) {
        // What checked items can do, on one row that scrolls sideways when the font is large (a
        // second row would leave the list no height); Sell all junk is in ⋮ meanwhile.
        val tight = PaddingValues(horizontal = 14.dp)
        BoxWithConstraints(Modifier.fillMaxWidth().padding(bottom = 6.dp)) {
            val width = maxWidth
            Row(Modifier.horizontalScroll(rememberScrollState())) {
                Row(
                    Modifier.widthIn(min = width).padding(horizontal = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                ) {
                    OutlinedButton(onClick = { panel.act(JunkAct.mark) }, contentPadding = tight, modifier = Modifier.heightIn(min = 48.dp)) { Text("Mark as junk", maxLines = 1) }
                    OutlinedButton(onClick = { panel.act(JunkAct.unmark) }, contentPadding = tight, modifier = Modifier.heightIn(min = 48.dp)) { Text("Unmark", maxLines = 1) }
                    OutlinedButton(onClick = goal, contentPadding = tight, modifier = Modifier.heightIn(min = 48.dp)) { Text("Goal…", maxLines = 1) }
                    OutlinedButton(
                        onClick = { sell(JunkAsk.SellSelected) },
                        enabled = !view.busy,
                        contentPadding = tight,
                        colors = ButtonDefaults.outlinedButtonColors(contentColor = Turu.Danger),
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text("Sell checked", maxLines = 1) }
                }
            }
        }
        return
    }
    Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            if (view.matches > 0u) "Check items to mark them as junk or give them a goal" else "",
            style = MaterialTheme.typography.bodySmall,
            color = Turu.Muted,
            modifier = Modifier.weight(1f).padding(start = 4.dp, end = 8.dp),
        )
        Button(
            onClick = { sell(JunkAsk.SellAll) },
            enabled = !view.busy && view.junk > 0u,
            colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Turu.Text),
            modifier = Modifier.heightIn(min = 48.dp),
        ) { Text("Sell all junk (${view.junk})", maxLines = 1) }
    }
}

/** VibeSkua's confirmations: the two sales (they can't be undone) and skipping them. */
@Composable
private fun JunkConfirm(ask: JunkAsk, view: JunkView, panel: JunkPanel, close: () -> Unit) {
    val (title, text, yes) = when (ask) {
        JunkAsk.SellSelected -> Triple(
            "Sell checked items?",
            "The ${view.selected} checked items become junk, then every junk item in your inventory is sold. Bank and equipped items stay. This can't be undone.",
            "Sell",
        )
        JunkAsk.SellAll -> Triple(
            "Sell all junk?",
            "Every item marked as junk in your inventory is sold. Bank and equipped items stay. This can't be undone.",
            "Sell",
        )
        JunkAsk.SkipWarning -> Triple("Skip the sell warning?", "Selling won't ask first any more.", "Skip it")
        JunkAsk.UnmarkAll -> Triple(
            "Unmark all ${view.junk} junk items?",
            "None of them is junk any more, and the junk list is saved empty. Mark them again to undo.",
            "Unmark all",
        )
    }
    AlertDialog(
        onDismissRequest = close,
        title = { Text(title) },
        text = { Text(text, color = Turu.Muted) },
        confirmButton = {
            TextButton(onClick = {
                close()
                when (ask) {
                    JunkAsk.SellSelected -> panel.act(JunkAct.sell_selected)
                    JunkAsk.SellAll -> panel.act(JunkAct.sell_all)
                    JunkAsk.SkipWarning -> panel.act(JunkAct.skip_warning(true))
                    JunkAsk.UnmarkAll -> panel.act(JunkAct.unmark_all)
                }
            }) { Text(yes, color = if (ask == JunkAsk.SkipWarning) Turu.Accent else Turu.Danger) }
        },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}

/** VibeSkua's Goal quantity row, for the checked items: a goal, or none. */
@Composable
private fun JunkGoalBar(view: JunkView, panel: JunkPanel, close: () -> Unit) {
    var text by remember { mutableStateOf("") }
    var wrong by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val set = {
        if (text.toIntOrNull()?.takeIf { it > 0 } == null) wrong = true
        else panel.act(JunkAct.goal(text)) { close() }
    }
    val items = if (view.selected == 1u) "1 item" else "${view.selected} items"
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
    Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it.filter { c -> c.isDigit() }.take(9); wrong = false },
            label = { Text(if (wrong) "A whole number above 0" else "Goal for $items, inventory + bank", maxLines = 1, overflow = TextOverflow.Ellipsis) },
            singleLine = true,
            isError = wrong,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { set() }),
            modifier = Modifier.weight(1f).focusRequester(focus),
        )
        Spacer(Modifier.width(8.dp))
        Button(onClick = { set() }, enabled = text.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Set") }
        TextButton(onClick = { panel.act(JunkAct.clear_goal) { close() } }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Clear", color = Turu.Danger) }
        IconButton(onClick = close) { Icon(Icons.Rounded.Close, contentDescription = "Close the goal", tint = Turu.Muted) }
    }
}
