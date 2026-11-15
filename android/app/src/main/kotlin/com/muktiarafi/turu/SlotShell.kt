package com.muktiarafi.turu

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Message
import android.os.Messenger
import android.os.SystemClock
import android.util.Log
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * M1 spike (docs/TAB_PROCESSES_PROPOSAL.md): the activity as the shell over two slot processes
 * ([SlotService]). It starts and binds both, hands the surface to the slot on screen (taking it
 * back first from the one before, and only once that one says it let go), and forwards touches to
 * it. Switching: `adb shell am start -n com.muktiarafi.turu/.TuruActivity --ei slot 2`.
 */
class SlotShell(private val context: Context, private val view: SurfaceView, private val density: Float) {
    /** The slot on screen. */
    @Volatile
    var current = 1
        private set

    private val messengers = ConcurrentHashMap<Int, Messenger>()
    private val connectedAt = ConcurrentHashMap<Int, Long>()
    private var surfaceValid = false
    private var attached: Int? = null

    // Answers come on a thread of their own: the main thread waits for them.
    private val replies = HandlerThread("Turu Shell").apply { start() }
    private val waiting = ConcurrentHashMap<Int, Pair<CountDownLatch, IntArray>>()
    private val tokens = AtomicInteger()
    private val replyTo = Messenger(object : Handler(replies.looper) {
        override fun handleMessage(msg: Message) {
            waiting.remove(msg.arg2)?.let { (latch, answer) ->
                answer[0] = msg.arg1
                latch.countDown()
            }
        }
    })

    private val connections = (1..SLOTS).associateWith { slot ->
        object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName, binder: IBinder) {
                messengers[slot] = Messenger(binder)
                connectedAt[slot] = SystemClock.elapsedRealtime()
                Log.i(SlotService.TAG, "Shell: slot $slot connected")
                if (slot == current && surfaceValid) attach(slot)
            }

            override fun onServiceDisconnected(name: ComponentName) {
                messengers.remove(slot)
                if (attached == slot) attached = null
                Log.w(SlotService.TAG, "Shell: slot $slot disconnected (its process died)")
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

    fun start() {
        for (slot in 1..SLOTS) {
            context.startForegroundService(intent(slot))
            context.bindService(intent(slot), connections.getValue(slot), Context.BIND_AUTO_CREATE)
        }
        context.bindService(intent(current), important.getValue(current), Context.BIND_IMPORTANT)
        view.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                surfaceValid = true
                attach(current)
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                if (attached == current) send(current, SlotService.MSG_RESIZE) else attach(current)
            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {
                surfaceValid = false
                detach()
            }
        })
    }

    fun stop() {
        for (slot in 1..SLOTS) {
            runCatching { context.unbindService(connections.getValue(slot)) }
        }
        runCatching { context.unbindService(important.getValue(current)) }
        replies.quitSafely()
    }

    /** Moves the surface to `slot`: the slot before lets go of it first, or it stays there. */
    fun switchTo(slot: Int) {
        if (slot == current || slot !in 1..SLOTS) return
        val started = SystemClock.elapsedRealtime()
        if (surfaceValid && attached != null && !detach()) {
            Log.w(SlotService.TAG, "Shell: slot $current didn't let go of the surface; staying on it")
            return
        }
        runCatching { context.unbindService(important.getValue(current)) }
        current = slot
        context.bindService(intent(slot), important.getValue(slot), Context.BIND_IMPORTANT)
        if (surfaceValid) attach(slot)
        Log.i(SlotService.TAG, "Shell: switched to slot $slot in ${SystemClock.elapsedRealtime() - started} ms")
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

    /** The file-lock probe in `slot` (1 take and hold, 2 let go): its answer, logged. */
    fun lockProbe(slot: Int, action: Int): Int {
        val answer = request(slot, SlotService.MSG_LOCK, arg = action)
        Log.i(SlotService.TAG, "Shell: lock probe in slot $slot, action $action: $answer (0 held, 1 busy, 2 let go)")
        return answer
    }

    /** Stops `slot`'s service now and starts it again after `delayMs` (the app may be in the background then). */
    fun restartLater(slot: Int, delayMs: Long) {
        if (slot == current) return
        // Unbound first: a bound service isn't destroyed by stopService.
        runCatching { context.unbindService(connections.getValue(slot)) }
        messengers.remove(slot)
        context.stopService(intent(slot))
        Log.i(SlotService.TAG, "Shell: slot $slot unbound and stopped; starting it again in $delayMs ms")
        Handler(replies.looper).postDelayed({
            try {
                context.startForegroundService(intent(slot))
                Log.i(SlotService.TAG, "Shell: started slot $slot from the background: allowed")
            } catch (e: Exception) {
                Log.w(SlotService.TAG, "Shell: starting slot $slot from the background refused: $e")
            }
            context.bindService(intent(slot), connections.getValue(slot), Context.BIND_AUTO_CREATE)
        }, delayMs)
    }

    private fun attach(slot: Int) {
        val surface = view.holder.surface
        if (!surface.isValid) return
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
    private fun request(slot: Int, what: Int, arg: Int = 0, data: Bundle? = null): Int {
        val messenger = messengers[slot] ?: return -2
        val token = tokens.incrementAndGet()
        val latch = CountDownLatch(1)
        val answer = intArrayOf(-2)
        waiting[token] = latch to answer
        val message = Message.obtain(null, what, arg, token).apply {
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

    private fun send(slot: Int, what: Int, data: Bundle? = null) {
        val message = Message.obtain(null, what).apply { data?.let { setData(it) } }
        runCatching { messengers[slot]?.send(message) }
    }

    private fun intent(slot: Int) = Intent(context, if (slot == 1) Slot1Service::class.java else Slot2Service::class.java)

    companion object {
        const val SLOTS = 2
    }
}
