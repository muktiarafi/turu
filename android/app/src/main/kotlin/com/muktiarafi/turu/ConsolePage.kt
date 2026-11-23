package com.muktiarafi.turu

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.ConsoleAct
import com.muktiarafi.turu.models.ConsoleView
import com.muktiarafi.turu.models.LineKind
import kotlinx.serialization.json.Json

/**
 * The Console's copy of the tab on screen (`turu_frontend::models::console`), read while the page
 * shows: the output, whether a snippet runs, the snippet last run. The editor is the page's own.
 */
class ConsolePanel {
    var view by mutableStateOf<ConsoleView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) error = null
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf(Api.console), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ConsoleView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/console doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun act(action: ConsoleAct) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        client.act(Api.console, action) { error = it }
    }

    fun copy(context: Context) {
        val text = view?.output?.joinToString("\n") { it.text }?.takeIf { it.isNotEmpty() } ?: return
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Console", text))
    }
}

/**
 * Diagnostics > Console (docs/UI.md, "Native shells"): a TypeScript snippet with `bot` in scope.
 * The editor, then Clear and Run (Stop while it runs), then the output, kept at its newest line.
 * With the keyboard up the editor takes the strip above it, Run beside it. ⋮: Copy output,
 * Clear output, Full window.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ConsolePage(state: ShellState, panel: ConsolePanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    val context = LocalContext.current
    // Typing on a landscape phone leaves a strip above the keyboard: the editor takes it, Run
    // beside it, and the bar, the note and the output step aside until the keyboard goes.
    val typing = WindowInsets.isImeVisible
    Column(Modifier.fillMaxSize()) {
        if (!typing) {
            PageBar("Console", onBack = { state.back() }) {
                FullWindowMenu("The same console in the egui window", fullWindow) { close ->
                    val any = view != null && view.output.isNotEmpty()
                    DropdownMenuItem(text = { Text("Copy output") }, enabled = any, onClick = { close(); panel.copy(context) })
                    DropdownMenuItem(text = { Text("Clear output") }, enabled = any, onClick = { close(); panel.act(ConsoleAct.clear) })
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                }
            }
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. The Console is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        // The editor starts from the snippet last run in this tab.
        var code by remember(state.tabs.active?.id) { mutableStateOf(view.code) }
        val editor = @Composable { m: Modifier ->
            OutlinedTextField(
                value = code,
                onValueChange = { code = it },
                placeholder = { Text("bot.player.gold", fontFamily = FontFamily.Monospace) },
                textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    autoCorrectEnabled = false,
                    keyboardType = KeyboardType.Ascii,
                ),
                modifier = m,
            )
        }
        val runOrStop = @Composable {
            if (view.running) {
                Button(
                    onClick = { panel.act(ConsoleAct.stop) },
                    colors = ButtonDefaults.buttonColors(containerColor = Turu.Danger, contentColor = Turu.Text),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Stop") }
            } else {
                Button(
                    onClick = { panel.act(ConsoleAct.run(code)) },
                    enabled = view.run.enabled && code.isNotBlank(),
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Run") }
            }
        }
        // One place for the editor in both layouts: moving it would drop its focus and the keyboard.
        Row(
            Modifier
                .then(if (typing) Modifier.weight(1f) else Modifier)
                .fillMaxWidth()
                .padding(start = 12.dp, end = 12.dp, top = if (typing) 8.dp else 0.dp, bottom = if (typing) 8.dp else 0.dp),
        ) {
            editor(Modifier.weight(1f).then(if (typing) Modifier.fillMaxHeight() else Modifier.heightIn(min = 96.dp, max = 160.dp)))
            if (typing) {
                Spacer(Modifier.width(8.dp))
                runOrStop()
            }
        }
        if (typing) return@Column
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 12.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { code = "" }, enabled = code.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Clear") }
            val note = panel.error ?: if (!view.running && !view.run.enabled) view.run.reason else null
            Text(
                note ?: if (view.running) "Running…" else "Shows the last expression's value",
                style = MaterialTheme.typography.bodySmall,
                color = if (panel.error != null) Turu.Danger else Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
            )
            if (view.running) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = Turu.Success)
                Spacer(Modifier.width(10.dp))
            }
            runOrStop()
        }
        Output(view, Modifier.weight(1f))
    }
}

/** The output, a monospaced line each in its colour, kept at the newest line; selectable to copy. */
@Composable
private fun Output(view: ConsoleView, modifier: Modifier) {
    val list = rememberLazyListState()
    val last = view.dropped + view.output.size.toULong()
    LaunchedEffect(last) { if (view.output.isNotEmpty()) list.scrollToItem(view.output.size - 1) }
    Column(
        modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, bottom = 8.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(Turu.Inset)
            .border(1.dp, Turu.Edge, MaterialTheme.shapes.medium),
    ) {
        if (view.output.isEmpty()) {
            Text("Run a snippet to see its output here", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(12.dp))
            return@Column
        }
        SelectionContainer {
            LazyColumn(Modifier.fillMaxSize().horizontalScroll(rememberScrollState()), state = list) {
                itemsIndexed(view.output, key = { i, _ -> view.dropped.toLong() + i }) { _, line ->
                    Text(
                        line.text,
                        style = MaterialTheme.typography.bodySmall,
                        fontFamily = FontFamily.Monospace,
                        color = when (line.kind) {
                            LineKind.RUN -> Turu.Muted
                            LineKind.LOG -> Turu.Text
                            LineKind.VALUE -> Turu.Success
                            LineKind.ERROR -> Turu.Danger
                            LineKind.STOPPED -> Turu.Warning
                        },
                        softWrap = false,
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 1.dp),
                    )
                }
            }
        }
    }
}
