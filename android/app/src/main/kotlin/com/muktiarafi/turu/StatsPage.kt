package com.muktiarafi.turu

import android.content.ActivityNotFoundException
import android.content.Intent
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
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
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.CounterKind
import com.muktiarafi.turu.models.CounterView
import com.muktiarafi.turu.models.ProgressView
import com.muktiarafi.turu.models.SnapshotView
import com.muktiarafi.turu.models.StatsView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/** Stats' copy of the tab on screen (`turu_frontend::models::stats`), read while the page shows. */
class StatsPanel {
    var view by mutableStateOf<StatsView?>(null)
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
            api, token, models = listOf("stats"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<StatsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/stats doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: String, body: JSONObject = JSONObject(), answer: (JSONObject) -> Unit = {}) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act("stats", action, body, answer = answer) { error = it }
    }
}

/** Numbers that change in place keep their width (tabular figures). */
private fun TextStyle.tabular() = copy(fontFeatureSettings = "tnum")

/** A counter's colour, as egui's tiles: accepted and re-logins in egui's own blue and purple. */
private fun CounterKind.color(): Color = when (this) {
    CounterKind.KILLS, CounterKind.COMPLETED -> Turu.Success
    CounterKind.DEATHS -> Turu.Danger
    CounterKind.PICKUPS -> Turu.Warning
    CounterKind.ACCEPTED -> Color(0xFF60A5FA)
    CounterKind.RELOGINS -> Color(0xFFA78BFA)
}

/**
 * Combat > Stats (docs/UI.md, "Native shells"; VibeSkua's Script Stats): the script clock and the
 * counters; with Advanced, the session's rates (Level and Rank toward their next step, net gold,
 * items, kills, deaths, in the unit picked) and the rate history every client shares, with Save,
 * Export (Android's share sheet) and Clear. ⋮ has Reset (asked first) and Full window.
 */
@Composable
fun StatsPage(state: ShellState, panel: StatsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    val context = LocalContext.current
    var resetting by remember { mutableStateOf(false) }
    var clearing by remember { mutableStateOf(false) }
    fun export() = panel.act("export") { answer ->
        val csv = answer.optString("csv").takeIf { !answer.isNull("csv") && it.isNotEmpty() } ?: return@act
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/csv")
            .putExtra(Intent.EXTRA_SUBJECT, "Turu rate history")
            .putExtra(Intent.EXTRA_TEXT, csv)
        try {
            context.startActivity(Intent.createChooser(send, null))
        } catch (e: ActivityNotFoundException) {
            Log.w("turu", "Nothing shares text: $e")
        }
    }
    Column(Modifier.fillMaxSize()) {
        PageBar("Stats", onBack = { state.back() }) {
            FullWindowMenu("The history as a table, sorted by any column", fullWindow) { close ->
                DropdownMenuItem(text = { Text("Reset…") }, onClick = { close(); resetting = true })
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Its stats are back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp)) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)) {
                    Column(Modifier.weight(1f)) {
                        Text(view.scriptTime, style = MaterialTheme.typography.headlineSmall.tabular())
                        Text("script time", style = MaterialTheme.typography.labelMedium, color = Turu.Muted)
                    }
                    // The label toggles too: one control, as Android's settings rows.
                    Row(
                        Modifier
                            .clip(RoundedCornerShape(12.dp))
                            .toggleable(value = view.advanced, role = Role.Switch) { panel.act("advanced", JSONObject().put("on", it)) }
                            .heightIn(min = 48.dp)
                            .padding(start = 12.dp, end = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text("Advanced", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(end = 8.dp))
                        Switch(checked = view.advanced, onCheckedChange = null)
                    }
                }
            }
            item { Counters(view.counters) }
            view.gameplay?.let { g ->
                item {
                    Section("Gameplay") {
                        Text(g.session, style = MaterialTheme.typography.labelLarge.tabular())
                        Text(" session", style = MaterialTheme.typography.labelMedium, color = Turu.Muted)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(bottom = 8.dp)) {
                        view.units.forEach { unit ->
                            val on = unit == view.unit
                            FilterChip(
                                selected = on,
                                onClick = { if (!on) panel.act("unit", JSONObject().put("unit", unit)) },
                                label = { Text(unit) },
                                colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
                                border = FilterChipDefaults.filterChipBorder(enabled = true, selected = on, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
                                modifier = Modifier.heightIn(min = 40.dp),
                            )
                        }
                    }
                    Progress(g.level, Turu.Success)
                    Spacer(Modifier.size(10.dp))
                    Progress(g.rank, Turu.Warning)
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(vertical = 10.dp))
                    g.rates.forEach { rate ->
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.heightIn(min = 32.dp)) {
                            Box(Modifier.size(7.dp).clip(CircleShape).background(rate.tint.color()))
                            Spacer(Modifier.width(8.dp))
                            Column(Modifier.weight(1f)) {
                                Text(rate.label, style = MaterialTheme.typography.bodyMedium)
                                if (rate.note.isNotEmpty()) Text(rate.note, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                            }
                            Text(rate.value, style = MaterialTheme.typography.titleSmall.tabular())
                        }
                    }
                }
                item {
                    Section("Rate history") {
                        Text(" ${view.history.size}", style = MaterialTheme.typography.labelLarge.tabular(), color = Turu.Muted)
                    }
                    Text(
                        "Save a checkpoint of the current rates to compare runs. Rates are in the unit shown when saved.",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 8.dp)) {
                        FilledTonalButton(onClick = { panel.act("save") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Save") }
                        TextButton(onClick = { export() }, enabled = view.history.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Export") }
                        Spacer(Modifier.weight(1f))
                        TextButton(onClick = { clearing = true }, enabled = view.history.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) {
                            Text("Clear…", color = if (view.history.isNotEmpty()) Turu.Danger else Turu.Muted)
                        }
                    }
                    if (view.history.isEmpty()) {
                        Text("No snapshots yet. Save one to start a history.", style = MaterialTheme.typography.bodyMedium, color = Turu.Muted, modifier = Modifier.padding(vertical = 8.dp))
                    }
                }
                items(view.history) { Snapshot(it) }
            }
        }
        (panel.error ?: view.note)?.let {
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 16.dp, end = 4.dp)) {
                Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.weight(1f))
                if (panel.error == null) TextButton(onClick = { panel.act("dismiss") }) { Text("OK") }
            }
        }
    }
    if (resetting) {
        AlertDialog(
            onDismissRequest = { resetting = false },
            title = { Text("Reset the stats?") },
            text = { Text("The counters and the session's rates start over. The rate history stays.", color = Turu.Muted) },
            confirmButton = { TextButton(onClick = { resetting = false; panel.act("reset") }) { Text("Reset") } },
            dismissButton = { TextButton(onClick = { resetting = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    if (clearing) {
        AlertDialog(
            onDismissRequest = { clearing = false },
            title = { Text("Clear the rate history?") },
            text = { Text("Every saved snapshot goes, for every client. It can't be undone.", color = Turu.Muted) },
            confirmButton = { TextButton(onClick = { clearing = false; panel.act("clear") }) { Text("Clear", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { clearing = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** The six counters, three to a row, on flat tonal cells. */
@Composable
private fun Counters(counters: List<CounterView>) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        counters.chunked(3).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { c ->
                    Column(
                        Modifier
                            .weight(1f)
                            .clip(RoundedCornerShape(10.dp))
                            .background(Turu.Card)
                            .padding(horizontal = 12.dp, vertical = 10.dp),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(Modifier.size(7.dp).clip(CircleShape).background(c.kind.color()))
                            Spacer(Modifier.width(6.dp))
                            Text(c.label, style = MaterialTheme.typography.labelSmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Text(c.value, style = MaterialTheme.typography.titleLarge.tabular(), maxLines = 1)
                    }
                }
            }
        }
    }
}

/** A heading with what sits beside it. */
@Composable
private fun Section(title: String, beside: @Composable () -> Unit = {}) {
    Row(verticalAlignment = Alignment.Bottom, modifier = Modifier.padding(top = 20.dp, bottom = 8.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall, modifier = Modifier.weight(1f))
        beside()
    }
}

/** A bar toward the next step, its value, and the rate and time left under it. */
@Composable
private fun Progress(p: ProgressView, color: Color) {
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(p.label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
            Text(p.value, style = MaterialTheme.typography.labelLarge.tabular(), color = Turu.Muted)
        }
        LinearProgressIndicator(
            progress = { (p.percent / 100.0).toFloat().coerceIn(0f, 1f) },
            color = color,
            trackColor = Turu.Track,
            strokeCap = StrokeCap.Round,
            drawStopIndicator = {},
            modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        )
        Text(p.hint, style = MaterialTheme.typography.bodySmall.tabular(), color = Turu.Muted)
    }
}

/** A saved snapshot: when and who, then its rates in the unit it was saved in. */
@Composable
private fun Snapshot(s: SnapshotView) {
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.35f), modifier = Modifier.padding(bottom = 8.dp))
        Row {
            Text(s.character, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${s.saved} · ${s.session}", style = MaterialTheme.typography.labelMedium.tabular(), color = Turu.Muted)
        }
        Text(
            "EXP ${s.exp} · Gold ${s.gold} · CP ${s.classPoints} · Rank ${s.rank}\nKills ${s.kills} · Deaths ${s.deaths} · Items ${s.items}  ${s.unit}",
            style = MaterialTheme.typography.bodySmall.tabular(),
            color = Turu.Muted,
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}
