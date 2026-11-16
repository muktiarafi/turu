package com.muktiarafi.turu

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.PowerManager
import android.util.Log
import android.view.Surface
import androidx.core.os.BundleCompat
import com.muktiarafi.turu.shared.Alerts
import com.muktiarafi.turu.shared.CrashReports
import com.muktiarafi.turu.shared.CrashUploads
import kotlin.concurrent.thread

/**
 * Process tabs (docs/TAB_PROCESSES_PROPOSAL.md, M3): one tab's engine in a process of its own
 * (`android:process=":c1"` ... `":c4"`), while `turu-process-tabs.txt` is in the data folder. A
 * foreground service with a notification of its own, started and bound by the activity (the shell,
 * [SlotShell]), which sends it its surface, touches and messages through a [Messenger]. The engine
 * is the Rust side's (`slots.rs`), set up from the slot's record (`slots/c<N>.json`). Android starts
 * a killed slot again by itself (`START_STICKY`), with or without the shell.
 */
abstract class SlotService(private val slot: Int) : Service() {
    private lateinit var wakeLock: PowerManager.WakeLock
    private var title = "Turu"
    private var text = "Starting"

    override fun onCreate() {
        super.onCreate()
        // Each process has its own crash reports: this slot's crashes, and how the app's
        // processes ended since (collected once across processes), as the app's service does.
        CrashUploads.start(applicationContext, CRASH_APP)
        CrashReports.collect(applicationContext, CRASH_APP)
        // Alerts (Notify Drop, a script stopped) from this slot: numbered apart from the other
        // slots', and posted while the shell says the app isn't on screen (no shell: it isn't).
        Alerts.init(applicationContext, R.drawable.ic_notification, ALERT_IDS * slot)
        Alerts.onScreen = { appOnScreen }
        Log.i(TAG, "Slot $slot: service created in process ${android.os.Process.myPid()}")
        instance = this
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Turu tabs", NotificationManager.IMPORTANCE_LOW),
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION + slot, build(), ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION + slot, build())
        }
        postSummary()
        // Held while the tab's script runs (`status`), as the app's service does.
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "turu:slot$slot")
        wakeLock.setReferenceCounted(false)
        Storage.init(applicationContext)
        val files = getExternalFilesDir(null) ?: filesDir
        nativeStartSlot(applicationContext, files.absolutePath, slot)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onDestroy() {
        Log.i(TAG, "Slot $slot: service destroyed")
        // Its notification goes with the service: the summary loses its line, or goes with the last.
        val manager = getSystemService(NotificationManager::class.java)
        manager.cancel(NOTIFICATION + slot)
        postSummary(without = NOTIFICATION + slot)
        if (wakeLock.isHeld) wakeLock.release()
        if (instance === this) instance = null
        super.onDestroy()
        // The tab was closed: the process is only its engine, which would otherwise keep playing
        // (logged in) until Android reclaims the process.
        android.os.Process.killProcess(android.os.Process.myPid())
    }

    override fun onBind(intent: Intent?): IBinder = messenger.binder

    /**
     * Android's memory levels for this slot's process (each process gets its own): the engine frees
     * what it can spare, as the app's service does.
     */
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        nativeTrimMemory(level)
    }

    /** The notification: the tab's script and state, grouped with the other tabs'; a tap opens Turu. */
    private fun build(): Notification {
        val open = PendingIntent.getActivity(
            this,
            slot,
            Intent(this, TuruActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setContentIntent(open)
            .setGroup(GROUP)
            .setOngoing(true)
            .build()
    }

    /**
     * The group's summary, one line per tab, which Android shows over the tabs' notifications
     * (collapsed, it's the one row). Every slot posts it again after its own changes, from the
     * notifications the app has up (all the slots', whichever process posted them): it needs no
     * state shared between the slots. `without`: a notification going away. With no tab left, the
     * summary goes too.
     */
    private fun postSummary(without: Int = 0) {
        val manager = getSystemService(NotificationManager::class.java)
        val mine = NOTIFICATION + slot
        // The other slots' from Android (this one's just posted may not be listed yet: its own
        // title and text instead).
        val tabs = manager.activeNotifications
            .filter { it.id != mine && it.id in NOTIFICATION + 1..NOTIFICATION + SLOTS }
            .associate { tab ->
                val extras = tab.notification.extras
                tab.id to (
                    extras.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty() to
                        extras.getCharSequence(Notification.EXTRA_TEXT)?.toString().orEmpty()
                    )
            }
            .toMutableMap()
        if (without != mine) tabs[mine] = title to text
        if (tabs.isEmpty()) {
            manager.cancel(NOTIFICATION)
            return
        }
        // The text names the tab ("dod is in game, ..."); the title is the app's ("Turu").
        val lines = tabs.toSortedMap().values.map { (title, text) ->
            text.lineSequence().firstOrNull().orEmpty().ifEmpty { title }
        }
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, TuruActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val count = if (tabs.size == 1) "1 tab" else "${tabs.size} tabs"
        val style = Notification.InboxStyle().setSummaryText(count)
        lines.forEach { style.addLine(it) }
        val summary = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Turu")
            .setContentText(lines.joinToString(" · "))
            .setStyle(style)
            .setContentIntent(open)
            .setGroup(GROUP)
            .setGroupSummary(true)
            // The tabs' own notifications say what changed; the summary never makes a sound.
            .setGroupAlertBehavior(Notification.GROUP_ALERT_CHILDREN)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .build()
        manager.notify(NOTIFICATION, summary)
    }

    private fun show(title: String, text: String, awake: Boolean) {
        this.title = title
        this.text = text
        if (awake && !wakeLock.isHeld) {
            wakeLock.acquire()
        } else if (!awake && wakeLock.isHeld) {
            wakeLock.release()
        }
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION + slot, build())
        postSummary()
    }

    /** The shell that last bound, watched so that its death lets the surface go. */
    private var shellBinder: IBinder? = null
    private val shellDied = IBinder.DeathRecipient {
        Log.w(TAG, "Slot $slot: the shell died; letting its surface go")
        shell = null
        appOnScreen = false
        thread(name = "Turu Detach") { nativeDetach() }
    }

    /** Keeps `replyTo` as the shell to send to, and watches it. */
    private fun watchShell(replyTo: Messenger?) {
        val binder = replyTo?.binder ?: return
        shell = replyTo
        if (binder == shellBinder) return
        shellBinder?.unlinkToDeath(shellDied, 0)
        try {
            binder.linkToDeath(shellDied, 0)
            shellBinder = binder
        } catch (e: android.os.RemoteException) {
            Log.w(TAG, "Slot $slot: the shell is gone already: $e")
            shellBinder = null
            shell = null
        }
    }

    /** Answers a shell's request with `answer`, when it waits for one (its `token`, from arg2). */
    private fun answer(what: Int, token: Int, replyTo: Messenger?, answer: Int) {
        if (token == 0) return
        try {
            replyTo?.send(Message.obtain(null, what, answer, token))
        } catch (e: Exception) {
            Log.w(TAG, "Slot $slot: couldn't answer the shell: $e")
        }
    }

    private val messenger = Messenger(object : Handler(Looper.getMainLooper()) {
        override fun handleMessage(msg: Message) {
            if (msg.what == MSG_DETACH) {
                // The engine lets go when it gets to it (later while it's starting up): waited for
                // on a thread of its own, answered then.
                val (token, replyTo) = msg.arg2 to msg.replyTo
                thread(name = "Turu Detach") { answer(MSG_DETACH, token, replyTo, nativeDetach()) }
                return
            }
            val answer = when (msg.what) {
                MSG_HELLO -> {
                    watchShell(msg.replyTo)
                    nativeShellBound()
                    1
                }
                MSG_TO_CLIENT -> {
                    msg.data.getString("json")?.let { nativeFromShell(it) }
                    1
                }
                MSG_ATTACH -> {
                    val data = msg.data
                    data.classLoader = Surface::class.java.classLoader
                    val surface = BundleCompat.getParcelable(data, "surface", Surface::class.java)
                    if (surface == null || !surface.isValid) {
                        0
                    } else {
                        watchShell(msg.replyTo)
                        nativeAttach(surface, data.getFloat("density"))
                    }
                }
                MSG_RESIZE -> {
                    nativeResize()
                    1
                }
                MSG_ON_SCREEN -> {
                    appOnScreen = msg.data.getBoolean("on")
                    1
                }
                MSG_TOUCH -> {
                    val data = msg.data
                    val kinds = data.getIntArray("kinds") ?: IntArray(0)
                    val ids = data.getIntArray("ids") ?: IntArray(0)
                    val xs = data.getFloatArray("xs") ?: FloatArray(0)
                    val ys = data.getFloatArray("ys") ?: FloatArray(0)
                    for (i in kinds.indices) nativeTouch(kinds[i], ids[i], xs[i], ys[i])
                    1
                }
                else -> -1
            }
            answer(msg.what, msg.arg2, msg.replyTo, answer)
        }
    })

    companion object {
        const val TAG = "turu-slots"
        private const val CHANNEL = "turu-slots"
        private const val GROUP = "turu-tabs"
        /** The group summary's notification ID; each slot's is this plus its slot. */
        private const val NOTIFICATION = 100

        /** The slots in the manifest (`Slot1Service` ... `Slot4Service`). */
        private const val SLOTS = 4
        const val MSG_ATTACH = 1
        const val MSG_DETACH = 2
        const val MSG_RESIZE = 3
        const val MSG_TOUCH = 4
        const val MSG_HELLO = 6
        const val MSG_TO_CLIENT = 7
        const val MSG_TO_SHELL = 8

        /** The app's activity came to the screen or left it (`on` in the data): for [Alerts]. */
        const val MSG_ON_SCREEN = 9

        /** Each slot's alerts are numbered from this times its slot, apart from the others'. */
        private const val ALERT_IDS = 100_000

        /** Whether the app is on screen, as the shell last said (false without a shell). */
        @Volatile
        private var appOnScreen = false

        /** This process's slot service (one per process). */
        @Volatile
        private var instance: SlotService? = null

        /** The shell's messenger, while one is bound. */
        @Volatile
        private var shell: Messenger? = null

        init {
            System.loadLibrary("turu_android")
        }

        /** A message for the shell (`turu_frontend::shell`), dropped while none is bound. Called from Rust. */
        @JvmStatic
        fun toShell(json: String) {
            val to = shell ?: return
            val slot = instance?.slot ?: return
            val message = Message.obtain(null, MSG_TO_SHELL, slot, 0).apply {
                data = Bundle().apply { putString("json", json) }
            }
            try {
                to.send(message)
            } catch (e: Exception) {
                Log.w(TAG, "Slot $slot: couldn't reach the shell: $e")
            }
        }

        /**
         * Puts `text` on the clipboard (egui's Copy, the game's Ctrl+C). Android lets any of the app's
         * processes use it while the app has the focus. Called from Rust.
         */
        @JvmStatic
        fun copyText(text: String) {
            val service = instance ?: return
            Handler(Looper.getMainLooper()).post {
                service.getSystemService(ClipboardManager::class.java)
                    .setPrimaryClip(ClipData.newPlainText("Turu", text))
            }
        }

        /** The clipboard's text, or null when it holds none (the game's Ctrl+V). Called from Rust. */
        @JvmStatic
        fun clipText(): String? {
            val service = instance ?: return null
            val clip = service.getSystemService(ClipboardManager::class.java).primaryClip ?: return null
            if (clip.itemCount < 1) return null
            return clip.getItemAt(0).coerceToText(service)?.toString()
        }

        /** The tab's notification and whether the CPU stays awake (its script runs). Called from Rust. */
        @JvmStatic
        fun status(title: String, text: String, awake: Boolean) {
            val service = instance ?: return
            Handler(Looper.getMainLooper()).post { service.show(title, text, awake) }
        }

        @JvmStatic
        private external fun nativeStartSlot(context: Context, filesDir: String, slot: Int)

        @JvmStatic
        private external fun nativeShellBound()

        @JvmStatic
        private external fun nativeFromShell(json: String)

        @JvmStatic
        private external fun nativeAttach(surface: Surface, density: Float): Int

        @JvmStatic
        private external fun nativeDetach(): Int

        @JvmStatic
        private external fun nativeResize()

        @JvmStatic
        private external fun nativeTouch(kind: Int, id: Int, x: Float, y: Float)

        @JvmStatic
        private external fun nativeTrimMemory(level: Int)
    }
}

class Slot1Service : SlotService(1)

class Slot2Service : SlotService(2)

class Slot3Service : SlotService(3)

class Slot4Service : SlotService(4)
