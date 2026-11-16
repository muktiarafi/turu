package com.muktiarafi.turu

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.SystemClock
import android.util.Log
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * Process tabs (docs/TAB_PROCESSES_PROPOSAL.md, M3): the activity as the shell over the slots
 * ([SlotService]), while `turu-process-tabs.txt` is in the data folder. The tab list and what to do
 * are the Rust side's (`slot_shell.rs`): each call answers with actions (start, stop or show a slot,
 * send it a message), carried out here. Here: binding the slots, handing the surface to the slot on
 * screen (taken back first from the one before, once it says it let go), touches to it, and the
 * slots' messages passed on. Debugging: `adb shell am start -n com.muktiarafi.turu/.TuruActivity`
 * with `--ei slot N` (show), `--ei openTab 1` (a new tab at the login screen) or `--ei closeTab N`.
 */
class SlotShell(private val context: Context, private val view: SurfaceView, private val density: Float) {
    /** The slot on screen (0 before the first is shown). */
    private var current = 0

    private val main = Handler(Looper.getMainLooper())
    private val messengers = ConcurrentHashMap<Int, Messenger>()
    private val bound = mutableSetOf<Int>()
    private var surfaceValid = false
    private var attached: Int? = null
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
                main.post { if (!stopped) run(nativeFromSlot(slot, json)) }
                return
            }
            waiting.remove(msg.arg2)?.let { (latch, answer) ->
                answer[0] = msg.arg1
                latch.countDown()
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
            }

            override fun onServiceDisconnected(name: ComponentName) {
                messengers.remove(slot)
                Log.w(SlotService.TAG, "Shell: slot $slot disconnected (its process died)")
                if (attached == slot) {
                    attached = null
                    renewSurface()
                }
                if (!stopped) run(nativeSlotGone(slot))
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
        view.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                surfaceValid = true
                if (current != 0) attach(current)
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                if (attached == current) send(current, SlotService.MSG_RESIZE) else if (current != 0) attach(current)
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
            }
        }
    }

    private fun startSlot(slot: Int) {
        if (slot !in 1..SLOTS) return
        context.startForegroundService(intent(slot))
        if (bound.add(slot)) {
            context.bindService(intent(slot), connections.getValue(slot), Context.BIND_AUTO_CREATE)
        }
    }

    private fun stopSlot(slot: Int) {
        if (slot !in 1..SLOTS) return
        if (attached == slot) {
            detach()
        }
        // Unbound first: a bound service isn't destroyed by stopService.
        if (bound.remove(slot)) runCatching { context.unbindService(connections.getValue(slot)) }
        messengers.remove(slot)
        context.stopService(intent(slot))
        Log.i(SlotService.TAG, "Shell: slot $slot stopped")
    }

    /**
     * Moves the surface to `slot`: the slot before lets go of it first, or it stays there. A slot
     * still starting may not answer in time: tried again `tries` more times, a second apart, then
     * the Rust side is told the slot before is still on screen.
     */
    private fun show(slot: Int, tries: Int = 3) {
        if (slot !in 1..SLOTS) return
        if (slot == current) {
            if (surfaceValid && attached != slot) attach(slot)
            return
        }
        val started = SystemClock.elapsedRealtime()
        if (surfaceValid && attached != null && !detach()) {
            if (tries > 0) {
                Log.w(SlotService.TAG, "Shell: slot $attached didn't let go of the surface; trying again")
                main.postDelayed({ if (!stopped) show(slot, tries - 1) }, 1000)
            } else {
                Log.w(SlotService.TAG, "Shell: slot $attached didn't let go of the surface; staying on it")
                run(nativeStillShown(current))
            }
            return
        }
        if (current != 0) runCatching { context.unbindService(important.getValue(current)) }
        current = slot
        context.bindService(intent(slot), important.getValue(slot), Context.BIND_IMPORTANT)
        if (surfaceValid) attach(slot)
        Log.i(SlotService.TAG, "Shell: showing slot $slot (${SystemClock.elapsedRealtime() - started} ms)")
    }

    /**
     * The slot drawing in the surface died without letting it go: the window still counts it as
     * its producer, and a new slot's Vulkan surface on it is refused (`ERROR_NATIVE_WINDOW_IN_USE_KHR`).
     * Hiding and showing the view makes a new surface, which goes to the slot on screen once it's
     * back (`surfaceCreated`, or its hello).
     */
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

    /** Takes the surface back from the slot that has it; true once it let go (or none had it). */
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

    private fun sendJson(slot: Int, json: String) {
        send(slot, SlotService.MSG_TO_CLIENT, Bundle().apply { putString("json", json) })
    }

    private fun intent(slot: Int) = Intent(context, CLASSES[slot - 1])

    companion object {
        const val SLOTS = 4
        private val CLASSES = listOf(
            Slot1Service::class.java,
            Slot2Service::class.java,
            Slot3Service::class.java,
            Slot4Service::class.java,
        )

        init {
            System.loadLibrary("turu_android")
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
