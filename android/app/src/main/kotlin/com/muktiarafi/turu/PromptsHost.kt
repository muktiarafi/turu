package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import com.muktiarafi.turu.models.PromptView
import com.muktiarafi.turu.models.PromptsView
import kotlinx.serialization.json.Json
import org.json.JSONObject

/**
 * What the tab on screen asks of the user (`turu_frontend::models::prompts`): a script's message
 * box, Butler's notice, the crash notice, the Tab Limit, a graphics tip; one at a time. Followed
 * whenever a tab shows, the panel open or not; while it's read, the slot's egui leaves them to it.
 */
class PromptsPanel {
    var prompt by mutableStateOf<PromptView?>(null)
        private set

    /** The prompt answered and not yet gone from the view: not shown again meanwhile. */
    private var answered: String? = null

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        following = tab
        client?.stop()
        client = null
        prompt = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("prompts"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<PromptsView>(text) }
                    .onSuccess { v ->
                        if (v.prompt?.id != answered) answered = null
                        prompt = v.prompt?.takeIf { it.id != answered }
                    }
                    .onFailure { Log.e(ModelClient.TAG, "/models/prompts doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    /**
     * Answers the prompt with its button `button` (-1: closed without one). `window` gets an egui
     * window to open ("logs"), `open` a native screen ("application_options").
     */
    fun answer(p: PromptView, button: Int, window: (String) -> Unit, open: (String) -> Unit) {
        answered = p.id
        prompt = null
        client?.act("prompts", "answer", JSONObject().put("id", p.id).put("button", button), { reply ->
            reply.optString("window").takeIf { it.isNotEmpty() }?.let(window)
            reply.optString("open").takeIf { it.isNotEmpty() }?.let(open)
        }) { e -> if (e != null) Log.w(ModelClient.TAG, "prompt answer: $e") }
    }
}

/**
 * The prompt as a Material dialog: its title, text (a script's or a report's scrolls when long),
 * a smaller note, and its buttons left to right with the last as the main one. A script's box
 * isn't closed by a tap beside it (its answer would be "closed"); Back still closes it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PromptsHost(state: ShellState, panel: PromptsPanel, window: (String) -> Unit, open: (String) -> Unit) {
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val p = panel.prompt ?: return
    val script = p.id.startsWith("box:")
    val answer = { button: Int -> panel.answer(p, button, window, open) }
    AlertDialog(
        onDismissRequest = { answer(-1) },
        properties = DialogProperties(dismissOnClickOutside = !script),
        title = { Text(p.title) },
        text = {
            Column(Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState())) {
                Text(p.text, style = MaterialTheme.typography.bodyMedium, color = Turu.Text)
                if (p.note.isNotEmpty()) {
                    Text(p.note, style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.padding(top = 12.dp))
                }
            }
        },
        confirmButton = {
            // The last button is the main one; the others sit before it and wrap when long.
            FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp, androidx.compose.ui.Alignment.End)) {
                p.buttons.forEachIndexed { i, label ->
                    val main = i == p.buttons.lastIndex
                    TextButton(onClick = { answer(i) }, modifier = Modifier.heightIn(min = 48.dp)) {
                        Text(label, color = if (main) Turu.Accent else Turu.Text, fontWeight = if (main) FontWeight.SemiBold else null)
                    }
                }
            }
        },
        containerColor = Turu.Card,
        titleContentColor = Turu.Text,
    )
}
