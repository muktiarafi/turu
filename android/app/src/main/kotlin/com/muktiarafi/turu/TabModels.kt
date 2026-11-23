package com.muktiarafi.turu

import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.AutoView
import com.muktiarafi.turu.models.BankView
import com.muktiarafi.turu.models.JumpView
import com.muktiarafi.turu.models.LogsAct
import com.muktiarafi.turu.models.ModelApi
import com.muktiarafi.turu.models.RailView
import com.muktiarafi.turu.models.ScriptsView
import kotlinx.serialization.json.Json

/**
 * The tab on screen's view models, read from its slot's control API (`turu_frontend::models`,
 * decoded into the generated types) for the native rail and its pages: one [ModelClient] for the
 * tab shown, made again when another tab shows or the slot comes back on a new port. Its logs are
 * line-fed, their last [KEEP] lines kept.
 */
class TabModels {
    var tab by mutableStateOf<ShellTab?>(null)
        private set
    var scripts by mutableStateOf<ScriptsView?>(null)
        private set
    var auto by mutableStateOf<AutoView?>(null)
        private set
    var jump by mutableStateOf<JumpView?>(null)
        private set
    var bank by mutableStateOf<BankView?>(null)
        private set
    var rail by mutableStateOf<RailView?>(null)
        private set

    /** The script log and the client's (debug) log. */
    val scriptLog = mutableStateListOf<Pair<String, String>>()
    val clientLog = mutableStateListOf<Pair<String, String>>()

    private var client: ModelClient? = null
    private val json = Json { ignoreUnknownKeys = true }

    /** Reads `next`'s models (the tab on screen), or nothing with null. */
    fun follow(next: ShellTab?) {
        val same = next?.id == tab?.id && next?.api == tab?.api
        tab = next
        if (same && client != null) return
        client?.stop()
        client = null
        scripts = null
        auto = null
        jump = null
        bank = null
        rail = null
        scriptLog.clear()
        clientLog.clear()
        val api = next?.api ?: return
        val token = next.token ?: return
        client = ModelClient(
            api, token,
            models = listOf(Api.scripts, Api.auto, Api.jump, Api.bank, Api.rail),
            logs = listOf("script", "debug"),
            onView = ::decode,
            onLog = { kind, time, text -> log(kind)?.let { append(it, time, text) } },
        ).also { client ->
            client.start()
            for (kind in listOf("script", "debug")) {
                client.readLog(kind, 0) { lines ->
                    val list = log(kind) ?: return@readLog
                    list.clear()
                    lines.takeLast(KEEP).forEach { list.add(it) }
                }
            }
        }
    }

    private fun log(kind: String) = when (kind) {
        "script" -> scriptLog
        "debug" -> clientLog
        else -> null
    }

    private fun append(list: MutableList<Pair<String, String>>, time: String, text: String) {
        list.add(time to text)
        while (list.size > KEEP) list.removeAt(0)
    }

    private fun decode(model: String, text: String) {
        // A record changed on one side only fails loudly here, with the route.
        runCatching {
            when (model) {
                Api.scripts.name -> scripts = json.decodeFromString<ScriptsView>(text)
                Api.auto.name -> auto = json.decodeFromString<AutoView>(text)
                Api.jump.name -> jump = json.decodeFromString<JumpView>(text)
                Api.bank.name -> bank = json.decodeFromString<BankView>(text)
                Api.rail.name -> rail = json.decodeFromString<RailView>(text)
            }
        }.onFailure { Log.e(ModelClient.TAG, "/models/$model doesn't decode: $it") }
    }

    /** Runs `action`, one of `model`'s (`Api.<model>`); `done` gets the error, or null. */
    fun <A> act(model: ModelApi<*, A>, action: A, done: (String?) -> Unit = {}) {
        val client = client ?: return done("The tab isn't up yet")
        client.act(model, action, done)
    }

    fun clearLog(kind: String) {
        act(Api.logs, LogsAct.clear(kind)) { if (it == null) log(kind)?.clear() }
    }

    companion object {
        const val KEEP = 500
    }
}
