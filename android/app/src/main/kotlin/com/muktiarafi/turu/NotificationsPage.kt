package com.muktiarafi.turu

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material.icons.rounded.Notifications
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.NoticeKind
import com.muktiarafi.turu.models.NoticeView
import com.muktiarafi.turu.models.NotificationsView
import kotlinx.serialization.json.Json

/**
 * Notification Center's copy of the tab on screen (`turu_frontend::models::notifications`), read
 * while the page shows. The filter and the selection are the page's own, as the egui window's.
 */
class NotificationsPanel {
    var view by mutableStateOf<NotificationsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** The filter on show (null: All). */
    var filter by mutableStateOf<NoticeKind?>(null)

    /** The selected events' ids. */
    val selected = mutableStateMapOf<ULong, Unit>()

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        // Another tab's feed: its events have other ids.
        if (tab?.id != following?.id) selected.clear()
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("notifications"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<NotificationsView>(text) }
                    .onSuccess { v ->
                        view = v
                        // An event the feed dropped (past 500, or Clear) can't stay selected.
                        val ids = v.entries.map { it.id }.toSet()
                        selected.keys.retainAll(ids)
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/notifications doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun clear() {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        selected.clear()
        client.act("notifications", "clear") { error = it }
    }

    /** The events the filter shows, newest first. */
    fun shown(view: NotificationsView): List<NoticeView> = view.entries.filter { filter == null || it.kind == filter }

    /** The selected events the filter shows, else every one it shows, as lines oldest first. */
    fun text(view: NotificationsView): Pair<String, Int> {
        val shown = shown(view)
        val picked = shown.filter { it.id in selected }.ifEmpty { shown }
        return picked.reversed().joinToString("\n") { it.line } to picked.size
    }

    fun toggle(entry: NoticeView) {
        if (selected.remove(entry.id) == null) selected[entry.id] = Unit
    }
}

/** A kind's icon and colour: items green, warnings amber, errors red, maps the accent, activity muted (as egui). */
private fun NoticeKind.icon(): Pair<ImageVector, Color> = when (this) {
    NoticeKind.ITEM -> RailIcons.Gift to Turu.Success
    NoticeKind.ACTIVITY -> RailIcons.Activity to Turu.Muted
    NoticeKind.WARNING -> Icons.Rounded.Warning to Turu.Warning
    NoticeKind.ERROR -> RailIcons.Alert to Turu.Danger
    NoticeKind.MAP -> Icons.Rounded.LocationOn to Turu.Accent
}

private fun events(n: Int) = if (n == 1) "1 event" else "$n events"

/**
 * Diagnostics > Notification Center (docs/UI.md, "Native shells"; VibeSkua's): the tab's feed of
 * drops, deaths, relogins, maps, quests and script starts and stops, newest first, under filter
 * chips with their counts. A tap selects an event; while some are, the bar offers Copy and Share
 * for them (else for every event shown, from ⋮), and ✕ clears the selection. Clear asks first.
 */
@Composable
fun NotificationsPage(state: ShellState, panel: NotificationsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    val context = LocalContext.current
    var clearing by remember { mutableStateOf(false) }
    // "Copied 3 events", in place of the count until the feed or the filter changes.
    var status by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(view?.entries?.size, panel.filter) { status = null }
    fun copy(view: NotificationsView) {
        val (text, count) = panel.text(view)
        if (count == 0) return
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Turu notifications", text))
        // Done with the selection, as Android's own lists end it after Copy.
        panel.selected.clear()
        status = "Copied ${events(count)}"
    }
    fun share(view: NotificationsView) {
        val (text, count) = panel.text(view)
        if (count == 0) return
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, "Turu notifications")
            .putExtra(Intent.EXTRA_TEXT, text)
        try {
            context.startActivity(Intent.createChooser(send, null))
        } catch (e: ActivityNotFoundException) {
            Log.w("turu", "Nothing shares text: $e")
        }
    }
    Column(Modifier.fillMaxSize()) {
        val picked = view?.let { v -> panel.shown(v).count { it.id in panel.selected } } ?: 0
        if (view != null && picked > 0) {
            // The selection's own bar, as Android's: how many, Copy, Share, and ✕ to end it.
            Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).background(Turu.AccentTint).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = { panel.selected.clear() }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the selection") }
                Text("$picked selected", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f).padding(start = 4.dp))
                TextButton(onClick = { copy(view) }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Copy") }
                IconButton(onClick = { share(view) }) { Icon(Icons.Rounded.Share, contentDescription = "Share the selected events") }
            }
        } else {
            PageBar("Notification Center", onBack = { state.back() }) {
                view?.let { v ->
                    Text(status ?: events(panel.shown(v).size), style = MaterialTheme.typography.labelMedium, color = Turu.Muted)
                    FullWindowMenu("The same feed, laid out as VibeSkua's", fullWindow) { close ->
                        val any = panel.shown(v).isNotEmpty()
                        DropdownMenuItem(text = { Text("Copy all shown") }, enabled = any, onClick = { close(); copy(v) })
                        DropdownMenuItem(text = { Text("Share all shown") }, enabled = any, onClick = { close(); share(v) })
                        DropdownMenuItem(
                            text = { Text("Clear…", color = if (v.entries.isEmpty()) Turu.Muted else Turu.Danger) },
                            enabled = v.entries.isNotEmpty(),
                            onClick = { close(); clearing = true },
                        )
                        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                    }
                }
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. Its events are back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        LazyRow(
            Modifier.padding(top = 4.dp, bottom = 4.dp),
            contentPadding = PaddingValues(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(view.filters, key = { it.label }) { f ->
                val on = panel.filter == f.kind
                FilterChip(
                    selected = on,
                    onClick = { panel.filter = f.kind },
                    label = { Text("${f.label}  ${f.count}", maxLines = 1) },
                    colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
                    border = FilterChipDefaults.filterChipBorder(enabled = true, selected = on, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
                    modifier = Modifier.heightIn(min = 40.dp),
                )
            }
        }
        val shown = panel.shown(view)
        if (shown.isEmpty()) {
            val empty = view.filters.firstOrNull { it.kind == panel.filter }?.empty.orEmpty()
            Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(Icons.Rounded.Notifications, contentDescription = null, tint = Turu.Muted, modifier = Modifier.size(28.dp))
                Text(
                    empty,
                    style = MaterialTheme.typography.bodyMedium,
                    color = Turu.Muted,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp).widthIn(max = 300.dp),
                )
            }
        } else {
            LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = 8.dp)) {
                items(shown, key = { it.id.toString() }) { entry ->
                    NoticeRow(entry, selected = entry.id in panel.selected) { panel.toggle(entry) }
                }
            }
        }
        panel.error?.let {
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        }
    }
    if (clearing) {
        AlertDialog(
            onDismissRequest = { clearing = false },
            title = { Text("Clear every event?") },
            text = { Text("The tab's whole feed goes, under every filter. It can't be brought back.", color = Turu.Muted) },
            confirmButton = { TextButton(onClick = { clearing = false; panel.clear() }) { Text("Clear", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { clearing = false }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** One event: its kind's icon, the time, the message (wrapping). A tap selects it. */
@Composable
private fun NoticeRow(entry: NoticeView, selected: Boolean, onClick: () -> Unit) {
    val (icon, tint) = entry.kind.icon()
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) Turu.AccentTint else Color.Transparent)
            .clickable(onClick = onClick)
            .heightIn(min = 48.dp)
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Box(Modifier.padding(top = 1.dp)) {
            Icon(icon, contentDescription = entry.kindName, tint = tint, modifier = Modifier.size(18.dp))
        }
        Spacer(Modifier.width(10.dp))
        Text(
            entry.time,
            style = MaterialTheme.typography.labelMedium,
            fontFamily = FontFamily.Monospace,
            color = Turu.Muted,
            modifier = Modifier.padding(top = 1.dp),
        )
        Spacer(Modifier.width(10.dp))
        Text(entry.message, style = MaterialTheme.typography.bodyMedium, color = Turu.Text, modifier = Modifier.weight(1f))
    }
}
