package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.DropView
import com.muktiarafi.turu.models.DropsAct
import com.muktiarafi.turu.models.DropsView
import kotlinx.serialization.json.Json

/**
 * Current Drops' copy of the tab on screen (`turu_frontend::models::drops`), read while the page
 * shows. The search, AC only and the selection are the page's own.
 */
class DropsPanel {
    var view by mutableStateOf<DropsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    var search by mutableStateOf("")
    var acOnly by mutableStateOf(false)

    /** The selected drops' ids. */
    val selected = mutableStateMapOf<Long, Unit>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) selected.clear()
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf(Api.drops), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<DropsView>(text) }
                    .onSuccess { v ->
                        view = v
                        // A drop picked up or gone from the stack can't stay selected.
                        selected.keys.retainAll(v.drops.map { it.id }.toSet())
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/drops doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    private fun act(action: DropsAct) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act(Api.drops, action) { error = it }
    }

    fun pickupAll() = act(DropsAct.pickup_all)

    fun pickupSelected() {
        act(DropsAct.pickup(selected.keys.toList()))
        selected.clear()
    }

    /** The drops this search and AC only show (as `models::drops::shows`). */
    fun shown(view: DropsView): List<DropView> {
        val needle = search.trim()
        return view.drops.filter { (!acOnly || it.coins) && (needle.isEmpty() || it.name.contains(needle, ignoreCase = true)) }
    }

    fun toggle(drop: DropView) {
        if (selected.remove(drop.id) == null) selected[drop.id] = Unit
    }
}

/**
 * Tools > Current Drops (docs/UI.md, "Native shells"): the tab's drop stack, searchable, with AC
 * only; a tap checks a drop. Pick up all, and Pick up N for the checked ones, at the foot.
 */
@Composable
fun DropsPage(state: ShellState, panel: DropsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    Column(Modifier.fillMaxSize()) {
        PageBar("Current Drops", onBack = { state.back() }) {
            view?.let { Text("${it.drops.size} in the stack", style = MaterialTheme.typography.labelMedium, color = Turu.Muted) }
            FullWindowMenu("The same drops, laid out as the desktop's", fullWindow)
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Its drops are back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        Row(Modifier.padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = panel.search,
                onValueChange = { panel.search = it },
                placeholder = { Text("Search drops") },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
                trailingIcon = if (panel.search.isNotEmpty()) {
                    { IconButton(onClick = { panel.search = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } }
                } else {
                    null
                },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            FilterChip(
                selected = panel.acOnly,
                onClick = { panel.acOnly = !panel.acOnly },
                label = { Text("AC only  ${view.acCount}") },
                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
                border = FilterChipDefaults.filterChipBorder(enabled = true, selected = panel.acOnly, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
                modifier = Modifier.heightIn(min = 40.dp),
            )
        }
        val shown = panel.shown(view)
        LazyColumn(Modifier.weight(1f).padding(top = 4.dp)) {
            if (shown.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(20.dp)) {
                        Text(if (view.drops.isEmpty()) "No drops right now" else "No drops match", style = MaterialTheme.typography.titleSmall)
                        Text(
                            if (view.drops.isEmpty()) "Items a monster or quest drops wait here until picked up." else "Clear the search or AC only.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Turu.Muted,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
            }
            items(shown, key = { it.id }) { drop ->
                val checked = drop.id in panel.selected
                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 1.dp)
                        .clip(MaterialTheme.shapes.medium)
                        .background(selectionTint(checked))
                        .clickable { panel.toggle(drop) }
                        .heightIn(min = 48.dp)
                        .padding(end = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = checked, onCheckedChange = { panel.toggle(drop) })
                    Text(drop.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    if (drop.quantity > 1) {
                        Text(" ×${drop.quantity}", style = MaterialTheme.typography.bodyMedium.copy(fontFeatureSettings = "tnum"), color = Turu.Muted)
                    }
                    Spacer(Modifier.weight(1f))
                    if (drop.coins) {
                        Text(
                            "AC",
                            style = MaterialTheme.typography.labelSmall,
                            color = Turu.Success,
                            modifier = Modifier
                                .background(Turu.Success.copy(alpha = 0.14f), MaterialTheme.shapes.small)
                                .padding(horizontal = 6.dp, vertical = 2.dp),
                        )
                    }
                }
            }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        panel.error?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val count = panel.selected.size
            FilledTonalButton(onClick = { panel.pickupSelected() }, enabled = count > 0, modifier = Modifier.heightIn(min = 48.dp)) {
                Text(if (count == 0) "Pick up selected" else "Pick up $count")
            }
            Button(onClick = { panel.pickupAll() }, enabled = view.drops.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Pick up all") }
        }
    }
}
