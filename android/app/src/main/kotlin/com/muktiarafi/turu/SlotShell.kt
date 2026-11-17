package com.muktiarafi.turu

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.SystemClock
import android.util.Log
import android.view.Gravity
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import com.muktiarafi.turu.shared.PackageUpdate
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.concurrent.thread

/**
 * Process tabs (docs/TAB_PROCESSES_PROPOSAL.md, M3): the activity as the shell over the slots
 * ([SlotService]), while process tabs are on ([switchedOn]). The tab list and what to do
 * are the Rust side's (`slot_shell.rs`): each call answers with actions (start, stop or show a slot,
 * send it a message), carried out here. Here: binding the slots, handing the surface to the slot on
 * screen (taken back first from the one before, once it says it let go), touches to it, and the
 * slots' messages passed on. Debugging: `adb shell am start -n com.muktiarafi.turu/.TuruActivity`
 * with `--ei slot N` (show), `--ei openTab 1` (a new tab at the login screen) or `--ei closeTab N`.
 */
class SlotShell(private val activity: TuruActivity, private val view: SurfaceView, private val density: Float) {
    private val context: Context = activity
    /** The slot on screen (0 before the first is shown). */
    private var current = 0

    /** The slot to show: [current] once the surface has moved there. */
    private var target = 0

    /** The let-go under way (its token, 0 when none) and the slot asked. */
    private var handingToken = 0
    private var handingSlot = 0
    private var handingStarted = 0L

    private val main = Handler(Looper.getMainLooper())
    private val messengers = ConcurrentHashMap<Int, Messenger>()
    private val bound = mutableSetOf<Int>()
    private var surfaceValid = false

    /** The slot drawing in the surface; the "starting" overlay shows while it's none (or not up yet). */
    private var attached: Int? = null
        set(value) {
            field = value
            updateOverlay()
        }

    /** The slots whose engine is up (they said hello with their control API). */
    private val ready = mutableSetOf<Int>()

    /** The text fields above the keyboard shown for the slot on screen ([GAME], [UI]). */
    private val fields = mutableSetOf<String>()

    /** The window's keyboard height and cutout sides, for the slots' layout (`insets`). */
    private var insets = Triple(0, 0, 0)

    /**
     * What shows where the game goes while no slot draws there: a tab starting, coming back after
     * Android stopped it, or a new surface on its way (the activity's process has no GPU of its own
     * to draw anything). A plain view over the surface.
     */
    private val overlay = FrameLayout(context).apply {
        setBackgroundColor(Color.rgb(14, 17, 22))
        val column = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            addView(ProgressBar(context).apply { isIndeterminate = true })
            addView(
                TextView(context).apply {
                    text = "Starting the tab…"
                    setTextColor(Color.rgb(200, 205, 214))
                    textSize = 15f
                    gravity = Gravity.CENTER
                    setPadding(0, (12 * density).toInt(), 0, 0)
                },
            )
        }
        addView(
            column,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER,
            ),
        )
    }
    private var stopped = false

    // Answers and the slots' messages come on a thread of their own: the main thread waits for answers.
    private val replies = HandlerThread("Turu Shell").apply { start() }
    private val waiting = ConcurrentHashMap<Int, Pair<CountDownLatch, IntArray>>()
    private val tokens = AtomicInteger()
    private val replyTo = Messenger(object : Handler(replies.looper) {
        override fun handleMessage(msg: Message) {
            if (msg.what == SlotService.MSG_TO_SHELL) {
                val slot = msg.arg1
                val json = msg.data.getString("json") ?: return
                main.post {
                    if (stopped) return@post
                    noteReady(slot, json)
                    run(nativeFromSlot(slot, json))
                }
                return
            }
            val waited = waiting.remove(msg.arg2)
            if (waited != null) {
                waited.second[0] = msg.arg1
                waited.first.countDown()
            } else if (msg.what == SlotService.MSG_DETACH) {
                val (token, answer) = msg.arg2 to msg.arg1
                main.post { if (!stopped) detached(token, answer) }
            }
        }
    })

    private val connections = (1..SLOTS).associateWith { slot ->
        object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName, binder: IBinder) {
                val messenger = Messenger(binder)
                messengers[slot] = messenger
                Log.i(SlotService.TAG, "Shell: slot $slot bound")
                // It answers with its hello (a message to the shell).
                send(slot, SlotService.MSG_HELLO)
                sendOnScreen(slot)
            }

            override fun onServiceDisconnected(name: ComponentName) {
                messengers.remove(slot)
                ready.remove(slot)
                Log.w(SlotService.TAG, "Shell: slot $slot disconnected (its process died)")
                if (attached == slot) {
                    attached = null
                    renewSurface()
                }
                // A slot asked to let go that died instead: let go all the same.
                if (handingToken != 0 && handingSlot == slot) handingToken = 0
                if (!stopped) {
                    run(nativeSlotGone(slot))
                    handOver()
                }
            }
        }
    }

    /** The binding that marks the slot on screen as important to the shell. */
    private val important = (1..SLOTS).associateWith {
        object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName, binder: IBinder) {}
            override fun onServiceDisconnected(name: ComponentName) {}
        }
    }

    fun start(filesDir: String) {
        (view.parent as? ViewGroup)?.addView(
            overlay,
            ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        updateOverlay()
        view.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                surfaceValid = true
                handOver()
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                if (attached != null && attached == current) send(current, SlotService.MSG_RESIZE) else handOver()
            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {
                surfaceValid = false
                detach()
            }
        })
        run(nativeShellStart(filesDir))
    }

    fun stop() {
        stopped = true
        for (slot in bound.toList()) {
            runCatching { context.unbindService(connections.getValue(slot)) }
        }
        if (current != 0) runCatching { context.unbindService(important.getValue(current)) }
        replies.quitSafely()
    }

    /** A message as if from the slot on screen (the adb controls): `{"type": "select_tab", ...}`. */
    fun asShown(json: String) {
        run(nativeFromSlot(current, json))
    }

    /** Actions the Rust side sent of its own (keys for the slot on screen), on the main thread. */
    fun actions(actions: String) {
        if (!stopped) run(actions)
    }

    /** Carries out the Rust side's actions, in order. */
    private fun run(actions: String) {
        val list = JSONArray(actions)
        for (i in 0 until list.length()) {
            val action: JSONObject = list.getJSONObject(i)
            val slot = action.getInt("slot")
            when (action.getString("kind")) {
                "start" -> startSlot(slot)
                "stop" -> stopSlot(slot)
                "show" -> show(slot)
                "send" -> sendJson(slot, action.getString("json"))
                "activity" -> forActivity(slot, JSONObject(action.getString("json")))
            }
        }
    }

    private fun startSlot(slot: Int) {
        if (slot !in 1..SLOTS) return
        context.startForegroundService(intent(slot))
        // Waiving priority: this binding doesn't lift the slot to the activity's importance, so a
        // hidden slot stays at its foreground service's and Android stops it before the one on
        // screen, which `important` lifts.
        if (bound.add(slot)) {
            context.bindService(
                intent(slot),
                connections.getValue(slot),
                Context.BIND_AUTO_CREATE or Context.BIND_WAIVE_PRIORITY,
            )
        }
    }

    private fun stopSlot(slot: Int) {
        if (slot !in 1..SLOTS) return
        if (attached == slot && !detach()) {
            // Stopped while it still had the surface: the next slot needs a new one.
            attached = null
            renewSurface()
        }
        if (handingToken != 0 && handingSlot == slot) handingToken = 0
        // Unbound first: a bound service isn't destroyed by stopService.
        if (bound.remove(slot)) runCatching { context.unbindService(connections.getValue(slot)) }
        messengers.remove(slot)
        context.stopService(intent(slot))
        Log.i(SlotService.TAG, "Shell: slot $slot stopped")
    }

    /** Shows `slot`: the surface moves there once the slot on screen has let go of it ([handOver]). */
    private fun show(slot: Int) {
        if (slot !in 1..SLOTS) return
        if (slot != target) closeFields()
        target = slot
        handOver()
    }

    /** The slot waiting for a file picker's answer, or 0. */
    private var picking = 0

    /** What only the activity does, for `slot`: its text field, a link, sharing, a picker, an install. */
    private fun forActivity(slot: Int, message: JSONObject) {
        when (message.optString("type")) {
            "field_show", "field_update", "field_hide" -> field(message)
            "open_url" -> activity.openUrl(message.optString("url"))
            "share" -> activity.shareText(message.optString("subject"), message.optString("text"))
            "pick_file" -> {
                picking = slot
                activity.openDocument(message.optString("mime_types"), message.optString("folder"))
            }
            "save_file" -> {
                picking = slot
                activity.createDocument(message.optString("mime_type"), message.optString("file_name"))
            }
            "install" -> install(slot, message.optString("path"))
        }
    }

    /** The slot that asked for the picker, now answered. */
    private fun takePicking(): Int = picking.also { picking = 0 }

    /** The "open" picker's answer: to the slot that asked. */
    fun documentPicked(path: String?) {
        val message = JSONObject().put("type", "document_picked").put("path", path ?: JSONObject.NULL)
        main.post { sendJson(takePicking(), message.toString()) }
    }

    /** The "save" picker's answer: to the slot that asked, which writes the file and copies it in. */
    fun documentCreated(uri: String?, path: String?) {
        val message = JSONObject().put("type", "document_created")
            .put("uri", uri ?: JSONObject.NULL).put("path", path ?: JSONObject.NULL)
        main.post { sendJson(takePicking(), message.toString()) }
    }

    /** An update's APK to Android's installer, off the main thread; the outcome goes back to `slot`. */
    private fun install(slot: Int, path: String) {
        val installed = { status: Int, message: String? ->
            val answer = JSONObject().put("type", "installed").put("status", status)
                .put("message", message ?: JSONObject.NULL)
            main.post { sendJson(slot, answer.toString()) }
        }
        thread(name = "Turu Install") {
            val error = PackageUpdate.install(activity, path) { status, message -> installed(status, message) }
            if (error != null) installed(2, error)
        }
    }

    /** The slot on screen's text field above the keyboard: shown, updated or hidden. */
    private fun field(message: JSONObject) {
        val game = message.optString("field") == GAME
        val text = message.optString("text")
        val (start, end) = message.optInt("start") to message.optInt("end")
        when (message.optString("type")) {
            "field_show" -> {
                fields.add(if (game) GAME else UI)
                if (game) {
                    activity.showGameField(text, start, end, message.optBoolean("password"), message.optInt("max"))
                } else {
                    val kind = when {
                        message.optBoolean("password") -> 1
                        message.optBoolean("lines") -> 2
                        message.optBoolean("number") -> 3
                        else -> 0
                    }
                    activity.showUiField(text, start, end, kind, message.optString("hint"))
                }
            }
            "field_update" -> if (game) activity.updateGameField(text, start, end) else activity.updateUiField(text, start, end)
            "field_hide" -> {
                fields.remove(if (game) GAME else UI)
                if (game) activity.hideGameField() else activity.hideUiField()
            }
        }
    }

    /** Another tab is chosen: the fields shown for this one go, and its text boxes lose the focus. */
    private fun closeFields() {
        for (field in fields.toList()) {
            if (field == GAME) activity.hideGameField() else activity.hideUiField()
            fieldClosed(field)
        }
        fields.clear()
    }

    /** The field above the keyboard was edited: to the slot on screen. */
    fun fieldEdited(field: String, text: String, start: Int, end: Int) {
        val message = JSONObject().put("type", "field_edited").put("field", field)
            .put("text", text).put("start", start).put("end", end)
        sendJson(current, message.toString())
    }

    /** The field's Send. */
    fun fieldEnter(field: String) {
        sendJson(current, JSONObject().put("type", "field_enter").put("field", field).toString())
    }

    /** The field went (Back, a touch outside it). */
    fun fieldClosed(field: String) {
        fields.remove(field)
        sendJson(current, JSONObject().put("type", "field_closed").put("field", field).toString())
    }

    /** The window's insets changed: every slot lays Turu's UI out around them. */
    fun insets(keyboard: Int, cutoutLeft: Int, cutoutRight: Int) {
        insets = Triple(keyboard, cutoutLeft, cutoutRight)
        for (slot in messengers.keys) sendInsets(slot)
    }

    private fun sendInsets(slot: Int) {
        val (keyboard, left, right) = insets
        val message = JSONObject().put("type", "insets").put("keyboard", keyboard)
            .put("cutout_left", left).put("cutout_right", right)
        sendJson(slot, message.toString())
    }

    /**
     * Moves the surface toward [target], without waiting on the main thread: the slot that has it is
     * asked to let go, and its answer ([detached]) carries on. A slot still starting answers once its
     * engine gets to it; meanwhile it keeps the screen, and a newer choice replaces the target.
     */
    private fun handOver() {
        if (handingToken != 0 || target == 0) return
        val from = attached
        if (from != null && from != target && surfaceValid) {
            val messenger = messengers[from]
            if (messenger != null) {
                handingToken = tokens.incrementAndGet()
                handingSlot = from
                handingStarted = SystemClock.elapsedRealtime()
                val message = Message.obtain(null, SlotService.MSG_DETACH, 0, handingToken).apply {
                    replyTo = this@SlotShell.replyTo
                }
                try {
                    messenger.send(message)
                    return
                } catch (e: Exception) {
                    Log.w(SlotService.TAG, "Shell: couldn't reach slot $from: $e")
                    handingToken = 0
                }
            }
            attached = null
        }
        if (current != target) {
            if (current != 0) runCatching { context.unbindService(important.getValue(current)) }
            current = target
            context.bindService(
                intent(current),
                important.getValue(current),
                Context.BIND_AUTO_CREATE or Context.BIND_IMPORTANT,
            )
            Log.i(SlotService.TAG, "Shell: showing slot $current")
        }
        if (surfaceValid && attached != current) attach(current)
    }

    /** The answer to the let-go [handOver] asked for: on to the target, or (it didn't let go) stay. */
    private fun detached(token: Int, answer: Int) {
        if (token != handingToken) return
        val from = handingSlot
        handingToken = 0
        val waited = SystemClock.elapsedRealtime() - handingStarted
        if (answer == 1 || messengers[from] == null) {
            Log.i(SlotService.TAG, "Shell: slot $from let go ($waited ms)")
            if (attached == from) attached = null
            handOver()
        } else {
            Log.w(SlotService.TAG, "Shell: slot $from didn't let go of the surface ($waited ms); staying on it")
            target = from
            run(nativeStillShown(from))
        }
    }

    /**
     * The slot drawing in the surface died without letting it go: the window still counts it as
     * its producer, and a new slot's Vulkan surface on it is refused (`ERROR_NATIVE_WINDOW_IN_USE_KHR`).
     * Hiding and showing the view makes a new surface, which goes to the slot on screen once it's
     * back (`surfaceCreated`, or its hello).
     */
    /** A slot's hello with its control API: its engine is up, and draws as soon as it has the surface. */
    private fun noteReady(slot: Int, json: String) {
        val message = runCatching { JSONObject(json) }.getOrNull() ?: return
        if (message.optString("type") != "hello") return
        // A slot that's (back) up learns the window's insets.
        sendInsets(slot)
        if (!message.isNull("api")) {
            ready.add(slot)
            updateOverlay()
        }
    }

    /** The overlay shows while no slot whose engine is up has the surface. */
    private fun updateOverlay() {
        val drawing = attached?.let { it in ready } == true
        overlay.visibility = if (drawing) View.GONE else View.VISIBLE
    }

    private fun renewSurface() {
        main.post {
            Log.i(SlotService.TAG, "Shell: a new surface for the next slot (the last one's died with it)")
            view.visibility = View.INVISIBLE
            // In a later frame: both in one would change nothing.
            main.postDelayed({ view.visibility = View.VISIBLE }, 100)
        }
    }

    /** A touch on the activity: to the slot on screen, in window pixels. */
    fun touch(event: MotionEvent) {
        val kind = when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> 0
            MotionEvent.ACTION_MOVE -> 1
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> 2
            MotionEvent.ACTION_CANCEL -> 3
            else -> return
        }
        val indices = if (kind == 1) (0 until event.pointerCount).toList() else listOf(event.actionIndex)
        val data = Bundle().apply {
            putIntArray("kinds", IntArray(indices.size) { kind })
            putIntArray("ids", indices.map { event.getPointerId(it) }.toIntArray())
            putFloatArray("xs", indices.map { event.getX(it) }.toFloatArray())
            putFloatArray("ys", indices.map { event.getY(it) }.toFloatArray())
        }
        send(current, SlotService.MSG_TOUCH, data = data)
    }

    private fun attach(slot: Int) {
        val surface = view.holder.surface
        if (!surface.isValid || messengers[slot] == null) return
        val data = Bundle().apply {
            putParcelable("surface", surface)
            putFloat("density", density)
        }
        val answer = request(slot, SlotService.MSG_ATTACH, data = data)
        if (answer == 1) attached = slot
        Log.i(SlotService.TAG, "Shell: surface to slot $slot: ${if (answer == 1) "taken" else "not taken ($answer)"}")
    }

    /**
     * Takes the surface back from the slot that has it, waiting (at most 3 s): only where it must be
     * gone before returning (the surface destroyed, the slot stopped). True once it let go (or none
     * had it).
     */
    private fun detach(): Boolean {
        val slot = attached ?: return true
        val started = SystemClock.elapsedRealtime()
        val answer = request(slot, SlotService.MSG_DETACH)
        Log.i(SlotService.TAG, "Shell: slot $slot let go: ${answer == 1} (${SystemClock.elapsedRealtime() - started} ms)")
        if (answer == 1 || messengers[slot] == null) attached = null
        return attached == null
    }

    /** Sends and waits for the answer (at most 3 s); -2 when there's none. */
    private fun request(slot: Int, what: Int, data: Bundle? = null): Int {
        val messenger = messengers[slot] ?: return -2
        val token = tokens.incrementAndGet()
        val latch = CountDownLatch(1)
        val answer = intArrayOf(-2)
        waiting[token] = latch to answer
        val message = Message.obtain(null, what, 0, token).apply {
            this.replyTo = this@SlotShell.replyTo
            data?.let { setData(it) }
        }
        try {
            messenger.send(message)
        } catch (e: Exception) {
            waiting.remove(token)
            Log.w(SlotService.TAG, "Shell: couldn't reach slot $slot: $e")
            return -2
        }
        latch.await(3, TimeUnit.SECONDS)
        waiting.remove(token)
        return answer[0]
    }

    /** Sends without waiting; the slot learns who the shell is from `replyTo`. */
    private fun send(slot: Int, what: Int, data: Bundle? = null) {
        val message = Message.obtain(null, what).apply {
            replyTo = this@SlotShell.replyTo
            data?.let { setData(it) }
        }
        runCatching { messengers[slot]?.send(message) }
    }

    /** Whether the activity is on screen, for the slots' alerts ([onScreen]). */
    private var onScreen = false

    /** The activity came to the screen or left it: every slot posts its alerts only while it isn't. */
    fun onScreen(on: Boolean) {
        onScreen = on
        for (slot in messengers.keys) sendOnScreen(slot)
    }

    private fun sendOnScreen(slot: Int) {
        send(slot, SlotService.MSG_ON_SCREEN, Bundle().apply { putBoolean("on", onScreen) })
    }

    private fun sendJson(slot: Int, json: String) {
        send(slot, SlotService.MSG_TO_CLIENT, Bundle().apply { putString("json", json) })
    }

    private fun intent(slot: Int) = Intent(context, CLASSES[slot - 1])

    companion object {
        const val SLOTS = 4

        /** The text fields above the keyboard, as the messages name them. */
        const val GAME = "game"
        const val UI = "ui"
        private val CLASSES = listOf(
            Slot1Service::class.java,
            Slot2Service::class.java,
            Slot3Service::class.java,
            Slot4Service::class.java,
        )

        init {
            System.loadLibrary("turu_android")
        }

        /** The switch file in the data folder (Options > Application > Process Tabs). */
        private const val SWITCH_FILE = "turu-process-tabs.txt"

        /**
         * Whether process tabs are on: unless the switch file in [files] (the data folder) says
         * `off`, as `turu_frontend::slots::switched_on` reads it. On by default since 2026-11-17.
         */
        fun switchedOn(files: File): Boolean =
            runCatching { File(files, SWITCH_FILE).readText().trim() != "off" }.getOrDefault(true)

        /**
         * Stops every slot's service (each one's process ends with it): process tabs were turned off.
         * Their records stay, so their tabs come back if they're turned on again.
         */
        fun stopAll(context: Context) {
            for (slot in CLASSES) context.stopService(Intent(context, slot))
        }

        @JvmStatic
        private external fun nativeShellStart(filesDir: String): String

        @JvmStatic
        private external fun nativeFromSlot(slot: Int, json: String): String

        @JvmStatic
        private external fun nativeSlotGone(slot: Int): String

        @JvmStatic
        private external fun nativeStillShown(slot: Int): String
    }
}
