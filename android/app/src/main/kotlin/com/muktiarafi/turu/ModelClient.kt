package com.muktiarafi.turu

import android.os.Handler
import android.os.Looper
import android.util.Log
import com.muktiarafi.turu.models.ModelApi
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.json.Json
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import kotlin.concurrent.thread

/**
 * One slot's view models, read over its control API on loopback (docs/NATIVE_UI_PROPOSAL.md, L2;
 * `turu_frontend::models`): `/events?models=...&logs=...` says when a view's revision moves and
 * brings log lines; `GET /models/<name>` reads a view; actions are `POST /models/<name>/<action>`.
 * The slot's address and token come from its hello (`slots::NativeTab`). A slot that restarts
 * says hello from a new port: the shell makes a new client for it. Callbacks run on the main
 * thread.
 *
 * Every client pauses while the app isn't on screen ([setShown]): no stream, no reads. The shell's
 * main thread applying a busy tab's views and log lines (5 a second) took 10-15% of a core in the
 * background, and Android closes a cached process over 2% (Sentry TURU-ANDROID-D, `process: main`
 * since process tabs). Back on screen, a client reads its views again and calls [onResume], where a
 * client of logs reads them again (the lines of the pause weren't streamed).
 */
class ModelClient(
    private val api: String,
    private val token: String,
    models: List<ModelApi<*, *>>,
    private val logs: List<String>,
    private val onView: (model: String, json: String) -> Unit,
    private val onLog: (kind: String, time: String, text: String) -> Unit,
    private val onResume: () -> Unit = {},
) {
    private val models = models.map { it.name }
    @Volatile private var stopped = false
    @Volatile private var events: HttpURLConnection? = null
    private val main = Handler(Looper.getMainLooper())
    private val actions = Executors.newSingleThreadExecutor { Thread(it, "Turu Model Actions") }

    fun start() {
        // Started off screen (a tab's hello in the background): its first pass is a return too.
        val startHides = LOCK.withLock {
            live.add(this)
            if (shown) hides else -1
        }
        thread(name = "Turu Models") {
            var seenHides = startHides
            while (awaitShown()) {
                // Off screen since the last pass (even briefly): what it missed is read again.
                if (hides != seenHides) {
                    seenHides = hides
                    main.post { if (!stopped) onResume() }
                }
                try {
                    for (model in models) fetch(model)
                    listen()
                } catch (e: Exception) {
                    if (!stopped && shown) Log.w(TAG, "Models at $api: $e")
                }
                if (!stopped && shown) Thread.sleep(1000)
            }
            LOCK.withLock { live.remove(this) }
        }
    }

    fun stop() {
        stopped = true
        LOCK.withLock { SHOWN.signalAll() }
        runCatching { events?.disconnect() }
        actions.shutdown()
    }

    /** Waits while the app is off screen; false once stopped. */
    private fun awaitShown(): Boolean {
        LOCK.withLock {
            while (!shown && !stopped) SHOWN.await()
        }
        return !stopped
    }

    private fun open(path: String, method: String = "GET"): HttpURLConnection =
        (URL("http://$api$path").openConnection() as HttpURLConnection).apply {
            requestMethod = method
            setRequestProperty("Authorization", "Bearer $token")
            connectTimeout = 2000
            readTimeout = 10_000
        }

    private fun listen() {
        val query = "models=${models.joinToString(",")}&logs=${logs.joinToString(",")}"
        val connection = open("/events?$query").apply { readTimeout = 30_000 }
        events = connection
        BufferedReader(InputStreamReader(connection.inputStream)).use { reader ->
            var event = ""
            while (!stopped && shown) {
                val line = reader.readLine() ?: return
                when {
                    line.startsWith("event: ") -> event = line.removePrefix("event: ")
                    line.startsWith("data: ") -> {
                        val data = JSONObject(line.removePrefix("data: "))
                        when (event) {
                            "model" -> fetch(data.getString("name"))
                            "log" -> {
                                val (kind, time, text) = Triple(data.getString("type"), data.getString("time"), data.getString("text"))
                                main.post { if (!stopped) onLog(kind, time, text) }
                            }
                        }
                    }
                }
            }
        }
    }

    private fun fetch(model: String) {
        val body = open("/models/$model").inputStream.bufferedReader().use { it.readText() }
        val view = JSONObject(body).getJSONObject("view").toString()
        main.post { if (!stopped) onView(model, view) }
    }

    /** A log's lines from `since` on, then [onLog] for each new one. */
    fun readLog(kind: String, since: Long, lines: (List<Pair<String, String>>) -> Unit) {
        actions.execute {
            runCatching {
                val body = open("/log?type=$kind&since=$since").inputStream.bufferedReader().use { it.readText() }
                val array = JSONObject(body).getJSONArray("lines")
                (0 until array.length()).map { array.getJSONObject(it).let { l -> l.getString("time") to l.getString("text") } }
            }.onSuccess { read -> main.post { if (!stopped) lines(read) } }
                .onFailure { Log.w(TAG, "Log $kind at $api: $it") }
        }
    }

    /**
     * Runs `action`, one of `model`'s actions (`Api.<model>`: the generated types say which); `done`
     * gets the error, or null.
     */
    fun <A> act(model: ModelApi<*, A>, action: A, done: (String?) -> Unit = {}) =
        post<A, Unit>(model, action, null, {}, done)

    /**
     * Runs `action`, `answer` getting what it answered as `reply`, a generated type. Which answer
     * goes with which action isn't checked by the compiler: one that doesn't decode is logged and
     * reaches `done` as the error.
     */
    fun <A, R> act(model: ModelApi<*, A>, action: A, reply: DeserializationStrategy<R>, answer: (R) -> Unit, done: (String?) -> Unit = {}) =
        post(model, action, reply, answer, done)

    private fun <A, R> post(model: ModelApi<*, A>, action: A, reply: DeserializationStrategy<R>?, answer: (R) -> Unit, done: (String?) -> Unit) {
        val body = JSON.encodeToString(model.act, action)
        actions.execute {
            var deliver: () -> Unit = {}
            val error = runCatching {
                val connection = open("/models/${model.name}/do", "POST").apply {
                    doOutput = true
                    setRequestProperty("Content-Type", "application/json")
                }
                connection.outputStream.use { it.write(body.toByteArray()) }
                if (connection.responseCode == 200) {
                    val text = connection.inputStream.bufferedReader().use { it.readText() }
                    if (reply != null) {
                        val value = runCatching { JSON.decodeFromString(reply, text) }
                            .onFailure { Log.e(TAG, "/models/${model.name}/do's answer doesn't decode: $it") }
                            .getOrElse { return@runCatching "The tab's answer didn't make sense: ${it.message}" }
                        deliver = { answer(value) }
                    }
                    null
                } else {
                    val text = connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
                    runCatching { JSONObject(text).getString("error") }.getOrDefault(text)
                }
            }.getOrElse { it.message ?: "the tab didn't answer" }
            main.post {
                if (stopped) return@post
                if (error == null) deliver()
                done(error)
            }
        }
    }

    /**
     * `POST <path>` on the control API (`/login`, ...), waiting up to `wait` ms for the answer;
     * `done` gets the error, or null.
     */
    fun call(path: String, wait: Int = 10_000, done: (String?) -> Unit = {}) {
        actions.execute {
            val error = runCatching {
                val connection = open(path, "POST").apply {
                    readTimeout = wait
                    doOutput = true
                }
                connection.outputStream.use { it.write(ByteArray(0)) }
                if (connection.responseCode == 200) {
                    null
                } else {
                    val text = connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
                    runCatching { JSONObject(text).getString("error") }.getOrDefault(text)
                }
            }.getOrElse { it.message ?: "the tab didn't answer" }
            main.post { if (!stopped) done(error) }
        }
    }

    companion object {
        const val TAG = "turu-models"

        private val LOCK = ReentrantLock()

        /** Signalled when the app comes on screen or a client stops. */
        private val SHOWN = LOCK.newCondition()

        /** Whether the app is on screen ([setShown]). */
        @Volatile private var shown = true

        /** How many times the app left the screen. */
        @Volatile private var hides = 0

        /** The clients started and not stopped (guarded by [LOCK]). */
        private val live = mutableSetOf<ModelClient>()

        /**
         * The app came on screen or left it (the activity started or stopped): off screen, every
         * client lets go of its stream and waits; back on, it reads its views again.
         */
        fun setShown(on: Boolean) {
            val clients = LOCK.withLock {
                if (!on && shown) hides++
                shown = on
                SHOWN.signalAll()
                live.toList()
            }
            // The streams block in a read: closing them ends it.
            if (!on) clients.forEach { runCatching { it.events?.disconnect() } }
        }

        /** Actions out and answers in; an answer may carry fields a newer slot added. */
        private val JSON = Json { ignoreUnknownKeys = true }
    }
}
