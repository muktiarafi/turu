package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.ClientUpdateView
import com.muktiarafi.turu.models.InstallWay
import com.muktiarafi.turu.models.UpdateStep
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * Update Turu's copy (`turu_frontend::models::client_update`), read from the tab on screen while
 * the page shows. The release and the install are its process's: on Android each tab's slot has
 * its own install, and the model holds the install while any tab runs.
 */
class UpdatePanel {
    var view by mutableStateOf<ClientUpdateView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** Which button was tapped (false: install, true: on quit), until the view says it works. */
    var starting by mutableStateOf<Boolean?>(null)
        private set

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        if (tab?.id != following?.id) {
            error = null
            starting = null
        }
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("client-update"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<ClientUpdateView>(text) }
                    .onSuccess { v ->
                        view = v
                        if (v.status?.step == UpdateStep.WORKING || v.status?.step == UpdateStep.FAILED) starting = null
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/client-update doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    fun install(onQuit: Boolean) {
        val client = client ?: run { error = "The tab isn't answering: it's starting again"; return }
        error = null
        starting = onQuit
        client.act("client-update", "install", JSONObject().put("onQuit", onQuit)) { e ->
            if (e != null) {
                error = e
                starting = null
            }
        }
    }
}

/**
 * Update Turu (docs/UI.md, "Native shells"), from the rail's Update and More's: the new version
 * leads, with the one installed and the release notes; then what the install is doing or why it
 * waits, the one filled button, and how the package is checked. ⋮: Full window.
 */
@Composable
fun UpdatePage(state: ShellState, panel: UpdatePanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    val context = LocalContext.current
    Column(Modifier.fillMaxSize()) {
        PageBar("Update ${view?.app ?: "Turu"}", onBack = { state.back() }) {
            FullWindowMenu("The same, laid out as the desktop's", fullWindow)
        }
        if (view == null) {
            val down = state.tabs.active?.down != null || state.tabs.active?.api == null
            Text(
                if (down) "This tab is starting again. The update is back with it." else "Reading the tab…",
                color = Turu.Muted,
                modifier = Modifier.padding(20.dp),
            )
            return@Column
        }
        val release = view.release
        if (release == null) {
            // Installed meanwhile, or a check found none: the egui window closes; here the page says so.
            Column(Modifier.padding(horizontal = 20.dp, vertical = 12.dp)) {
                Text("${view.app} ${view.current} is the newest", style = MaterialTheme.typography.titleMedium)
                Text(
                    "No update is waiting. ${view.app} checks again at launch and every 6 hours.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Turu.Muted,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            return@Column
        }
        Column(
            Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(top = 4.dp, bottom = 16.dp),
        ) {
            Text("${view.app} ${release.version}", style = MaterialTheme.typography.headlineSmall)
            Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (release.prerelease) {
                    Surface(color = Turu.Card, shape = MaterialTheme.shapes.small, modifier = Modifier.padding(end = 8.dp)) {
                        Text(
                            "Pre-release",
                            style = MaterialTheme.typography.labelMedium,
                            color = Turu.Warning,
                            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
                        )
                    }
                }
                Text("You have ${view.current}", style = MaterialTheme.typography.bodyMedium, color = Turu.Muted)
            }
            TextButton(
                onClick = { openLink(context, release.notesUrl) },
                // Its label lines up with the heading; the button's own padding stays for the touch.
                modifier = Modifier.padding(top = 4.dp).offset(x = (-12).dp).heightIn(min = 48.dp),
            ) { Text("Read the release notes") }

            Status(view, panel)

            val busy = view.status?.step == UpdateStep.WORKING || panel.starting != null
            when (view.way) {
                InstallWay.PACKAGE, InstallWay.INPLACE -> {
                    Button(
                        onClick = { panel.install(onQuit = false) },
                        enabled = view.install.enabled && !busy,
                        modifier = Modifier.fillMaxWidth().padding(top = 16.dp).heightIn(min = 48.dp),
                    ) {
                        if (panel.starting == false) {
                            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = Turu.Text)
                        } else {
                            Text(view.installLabel)
                        }
                    }
                    // The status says why it waits; a reason it doesn't already give goes here.
                    if (!view.install.enabled && view.status == null) Reason(view.install.reason)
                    if (view.way == InstallWay.INPLACE) {
                        OutlinedButton(
                            onClick = { panel.install(onQuit = true) },
                            enabled = view.onQuit.enabled && !busy,
                            modifier = Modifier.fillMaxWidth().padding(top = 8.dp).heightIn(min = 48.dp),
                        ) { Text("Update when I quit") }
                    }
                }
                InstallWay.LINK, InstallWay.NOPACKAGE -> if (view.linkUrl.isNotEmpty()) {
                    Button(
                        onClick = { openLink(context, view.linkUrl) },
                        modifier = Modifier.fillMaxWidth().padding(top = 16.dp).heightIn(min = 48.dp),
                    ) { Text(view.linkLabel) }
                }
            }
            panel.error?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, modifier = Modifier.padding(top = 8.dp))
            }
            if (view.footnote.isNotEmpty()) {
                Text(
                    view.footnote,
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    modifier = Modifier.padding(top = 16.dp),
                )
            }
        }
    }
}

/** What the install does (a wave while it works), what it waits for, or how it failed. */
@Composable
private fun Status(view: ClientUpdateView, panel: UpdatePanel) {
    val status = view.status
    val text = status?.text ?: if (panel.starting != null) "Starting the download…" else return
    val step = status?.step ?: UpdateStep.WORKING
    val color = when (step) {
        UpdateStep.WORKING -> Turu.Text
        UpdateStep.WAITING -> Turu.Success
        UpdateStep.FAILED -> Turu.Danger
        UpdateStep.HELD -> Turu.Muted
    }
    Column(
        Modifier
            .fillMaxWidth()
            .padding(top = 12.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // The dot sits on the first line, since a failure's text can run to several.
        Row(verticalAlignment = Alignment.Top) {
            if (step != UpdateStep.WORKING) {
                Surface(color = color, shape = CircleShape, modifier = Modifier.padding(top = 6.dp, end = 10.dp).size(8.dp)) {}
            }
            Text(text, style = MaterialTheme.typography.bodyMedium, color = color)
        }
        if (step == UpdateStep.WORKING) {
            LinearWavyProgressIndicator(Modifier.fillMaxWidth(), color = Turu.Accent, trackColor = Turu.Track)
        }
    }
}

@Composable
private fun Reason(text: String, color: Color = Turu.Muted) {
    if (text.isEmpty()) return
    Text(text, style = MaterialTheme.typography.bodySmall, color = color, modifier = Modifier.padding(top = 6.dp))
}
