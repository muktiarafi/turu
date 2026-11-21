package com.muktiarafi.turu

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.CrashRow
import com.muktiarafi.turu.models.CrashesView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/** A crash report's text, as `models::crashes`' `text` answers it. */
class CrashText(val name: String, val text: String, val subject: String, val cut: Boolean)

/**
 * Logs > Crashes' copy of the reports (`turu_frontend::models::crashes`), read through the tab on
 * screen while Logs or a report shows. Both pages hold it, so it counts them: crossing from one
 * to the other doesn't drop the connection. It also keeps what a Delete took, for the list's
 * Undo after the report page is gone.
 */
class CrashesPanel {
    var view by mutableStateOf<CrashesView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** The report read last ([read]); null while it reads. */
    var report by mutableStateOf<CrashText?>(null)
        private set

    /** The list's snackbar: a deleted report, with Undo. */
    var notice by mutableStateOf<Notice?>(null)

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private var holders = 0
    private val json = Json { ignoreUnknownKeys = true }

    /** A page showing the reports for `tab`; [release] when it goes. */
    fun hold(tab: ShellTab?) {
        holders++
        follow(tab)
    }

    fun release() {
        holders = (holders - 1).coerceAtLeast(0)
        if (holders == 0) follow(null)
    }

    private fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && (client != null || tab == null)) return
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("crashes"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<CrashesView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/crashes doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    /** Reads the report `name` into [report]. */
    fun read(name: String) {
        if (report?.name == name) return
        report = null
        error = null
        val client = client ?: return
        client.act("crashes", "text", JSONObject().put("name", name), { a ->
            report = CrashText(name, a.optString("text"), a.optString("subject"), a.optBoolean("cut"))
        }) { e -> if (e != null) error = e }
    }

    /** Deletes the report `name`; `done` once it's gone, the list then offering Undo. */
    fun delete(row: CrashRow, done: () -> Unit) {
        error = null
        val client = client ?: return
        client.act("crashes", "delete", JSONObject().put("name", row.name)) { e ->
            if (e != null) {
                error = e
                return@act
            }
            if (report?.name == row.name) report = null
            notice = Notice("Deleted “${row.summary.ifBlank { row.name }}”") { restore(row.name) }
            done()
        }
    }

    private fun restore(name: String) {
        client?.act("crashes", "restore", JSONObject().put("name", name)) { e -> if (e != null) error = e }
    }
}

/** Holds `panel` for the tab on screen while the page shows. */
@Composable
private fun HoldCrashes(state: ShellState, panel: CrashesPanel) {
    val tab = state.tabs.active
    DisposableEffect(tab?.id, tab?.api) {
        panel.hold(tab)
        onDispose { panel.release() }
    }
}

/** The reports' count for Logs' segment, holding the panel while Logs shows. */
@Composable
fun crashCount(state: ShellState, panel: CrashesPanel): Int {
    HoldCrashes(state, panel)
    return panel.view?.reports?.size ?: 0
}

/**
 * Logs > Crashes: the reports earlier runs left, newest first, each with what happened and when;
 * a tap opens it. Most never crash, so Logs offers the segment only once there is one.
 */
@Composable
fun CrashList(state: ShellState, panel: CrashesPanel, models: TabModels) {
    val view = panel.view
    Box(Modifier.fillMaxSize()) {
        when {
            view == null -> Text(
                if (models.tab?.api == null) "The tab is starting…" else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            view.reports.isEmpty() -> Column(Modifier.padding(horizontal = 16.dp, vertical = 20.dp)) {
                Text("No crash reports", style = MaterialTheme.typography.titleSmall, color = Turu.Text)
                Text(
                    "When Turu closes unexpectedly, the next start writes what happened here.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Turu.Muted,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            else -> LazyColumn(Modifier.fillMaxSize().padding(top = 8.dp)) {
                item {
                    Text(
                        if (view.reports.size == 1) "1 report" else "${view.reports.size} reports, newest first",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    )
                }
                items(view.reports, key = { it.name }) { row ->
                    ListItem(
                        supportingContent = { Text(row.written, color = Turu.Muted) },
                        trailingContent = {
                            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Muted)
                        },
                        colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent, headlineColor = Turu.Text),
                        modifier = Modifier.clickable(onClickLabel = "Open the report") { state.push(Page.CrashReport(row.name)) },
                    ) {
                        Text(row.summary.ifBlank { "No summary" }, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
        NoticeHost(panel.notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { panel.notice = null }
    }
}

/**
 * A crash report: what happened and when, Share (the main action: it goes to whoever helps), Copy
 * and Delete, then the report's text as written, its long lines wrapped (most reports are sentences).
 */
@Composable
fun CrashReportPage(state: ShellState, panel: CrashesPanel, name: String) {
    HoldCrashes(state, panel)
    val row = panel.view?.reports?.firstOrNull { it.name == name }
    LaunchedEffect(name, panel.view != null) { if (panel.view != null) panel.read(name) }
    val report = panel.report?.takeIf { it.name == name }
    val context = LocalContext.current
    var notice by remember { mutableStateOf<Notice?>(null) }
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            PageBar("Crash report", onBack = { state.back() })
            Column(Modifier.padding(horizontal = 16.dp)) {
                Text(
                    row?.summary?.ifBlank { null } ?: name,
                    style = MaterialTheme.typography.titleSmall,
                    color = Turu.Text,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    listOfNotNull(row?.written, name).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Button(
                    onClick = { report?.let { share(context, it) } },
                    enabled = report != null,
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Share") }
                OutlinedButton(
                    onClick = {
                        report?.let { copy(context, it) }
                        // Android 13 and later confirm a copy themselves.
                        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) notice = Notice("Copied")
                    },
                    enabled = report != null,
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Copy") }
                Box(Modifier.weight(1f))
                TextButton(
                    onClick = { row?.let { panel.delete(it) { state.back() } } },
                    enabled = row != null,
                    colors = ButtonDefaults.textButtonColors(contentColor = Turu.Danger),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Delete") }
            }
            panel.error?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(horizontal = 16.dp))
            }
            if (report?.cut == true) {
                Text(
                    "Long report: the start is shown and shared, the rest stays in the file.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(top = 4.dp))
            Column(
                Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 8.dp)
                    .clip(MaterialTheme.shapes.medium)
                    .background(Turu.Inset)
                    .border(1.dp, Turu.Edge, MaterialTheme.shapes.medium),
            ) {
                when {
                    row == null && panel.view != null -> Text(
                        "This report is gone: it was deleted, or pushed out by newer ones.",
                        style = MaterialTheme.typography.bodySmall,
                        color = Turu.Muted,
                        modifier = Modifier.padding(12.dp),
                    )
                    report == null -> Text("Reading the report…", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(12.dp))
                    else -> SelectionContainer {
                        val lines = report.text.lines()
                        LazyColumn(Modifier.fillMaxSize()) {
                            itemsIndexed(lines) { _, line ->
                                Text(
                                    line,
                                    style = MaterialTheme.typography.bodySmall,
                                    fontFamily = FontFamily.Monospace,
                                    color = Turu.Text,
                                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 1.dp),
                                )
                            }
                        }
                    }
                }
            }
        }
        NoticeHost(notice, Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp)) { notice = null }
    }
}

private fun share(context: Context, report: CrashText) {
    val send = Intent(Intent.ACTION_SEND)
        .setType("text/plain")
        .putExtra(Intent.EXTRA_SUBJECT, report.subject)
        .putExtra(Intent.EXTRA_TEXT, report.text)
    try {
        context.startActivity(Intent.createChooser(send, null))
    } catch (e: ActivityNotFoundException) {
        Log.w("turu", "Nothing shares text: $e")
    }
}

private fun copy(context: Context, report: CrashText) {
    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(report.subject, report.text))
}
