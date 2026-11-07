package com.muktiarafi.turu

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager

/**
 * Keeps Turu running in the background: without a foreground service Android cuts a background
 * app's network after a few seconds, which drops the game. It starts the engine (the Rust side,
 * also when Android restarts the service after killing the process, with no activity), shows
 * the script's state with Pause and Stop (Start while one is loaded and stopped), and holds a wake lock while a script runs.
 *
 * The Rust side reaches only this class (its `@JvmStatic` methods, by name and JNI signature:
 * `notify.rs`, `platform.rs`), so keep those static, public and with the same parameter types.
 */
class TuruService : Service() {
    private lateinit var wakeLock: PowerManager.WakeLock
    private var title = "Turu"
    private var text = "Starting"
    private var running = false
    private var awake = false
    private var paused = false
    private var startable = false

    override fun onCreate() {
        super.onCreate()
        instance = this
        val power = getSystemService(PowerManager::class.java)
        wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "turu:script")
        wakeLock.setReferenceCounted(false)
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Turu running", NotificationManager.IMPORTANCE_LOW),
        )
        startForegroundWith(build())
        CrashUploads.start(applicationContext)
        CrashReports.collect(applicationContext)
        // Passwords and documents (`storage.rs`), before the engine reads a password.
        Storage.init(applicationContext)
        // No activity in this process: Android restarted the service after killing it, and the
        // engine runs again the script that was running (`restart.rs`).
        if (!TuruActivity.started) {
            nativeRestarted()
        }
        val files = getExternalFilesDir(null) ?: filesDir
        nativeStart(applicationContext, files.absolutePath)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> nativeAction("start")
            ACTION_STOP -> nativeAction("stop")
            ACTION_PAUSE -> nativeAction("pause")
        }
        // Started again after Android killed the process: the engine logs back in and runs
        // turu-args.txt's script.
        return START_STICKY
    }

    override fun onDestroy() {
        instance = null
        if (wakeLock.isHeld) {
            wakeLock.release()
        }
        super.onDestroy()
    }

    /** The phone is low on memory, or the UI went away: Turu frees what it can spare (`memory.rs`). */
    @Suppress("DEPRECATION")
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level >= TRIM_MEMORY_RUNNING_LOW) {
            nativeTrimMemory(level)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun show(
        title: String,
        text: String,
        running: Boolean,
        paused: Boolean,
        startable: Boolean,
        awake: Boolean,
    ) {
        this.title = title
        this.text = text
        this.running = running
        this.paused = paused
        this.startable = startable
        this.awake = awake
        // The CPU stays awake while any tab's script runs (the screen may go off).
        if (awake && !wakeLock.isHeld) {
            wakeLock.acquire()
        } else if (!awake && wakeLock.isHeld) {
            wakeLock.release()
        }
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION, build())
    }

    private fun startForegroundWith(notification: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION, notification)
        }
    }

    private fun action(action: String, code: Int): PendingIntent {
        val intent = Intent(this, TuruService::class.java).setAction(action)
        return PendingIntent.getService(this, code, intent, PendingIntent.FLAG_IMMUTABLE)
    }

    private fun build(): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, TuruActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(text)
            // With several tabs, a line per tab.
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
        if (running) {
            builder.addAction(
                Notification.Action.Builder(null, if (paused) "Resume" else "Pause", action(ACTION_PAUSE, 1)).build(),
            )
            builder.addAction(Notification.Action.Builder(null, "Stop", action(ACTION_STOP, 2)).build())
        } else if (startable) {
            builder.addAction(Notification.Action.Builder(null, "Start", action(ACTION_START, 3)).build())
        }
        return builder.build()
    }

    companion object {
        private const val CHANNEL = "turu"
        private const val NOTIFICATION = 1
        const val ACTION_START = "com.muktiarafi.turu.START"
        const val ACTION_STOP = "com.muktiarafi.turu.STOP"
        const val ACTION_PAUSE = "com.muktiarafi.turu.PAUSE"

        private var instance: TuruService? = null
        private val main = Handler(Looper.getMainLooper())

        init {
            System.loadLibrary("turu_android")
        }

        @JvmStatic
        private external fun nativeStart(context: Context, filesDir: String)

        @JvmStatic
        private external fun nativeAction(action: String)

        @JvmStatic
        private external fun nativeRestarted()

        @JvmStatic
        private external fun nativeTrimMemory(level: Int)

        /** For the Rust side, which can only reach this class: see [CrashUploads]. */
        @JvmStatic
        fun crashUploadsAvailable(): Boolean = CrashUploads.available()

        @JvmStatic
        fun setCrashUploads(on: Boolean) = CrashUploads.setEnabled(on)

        @JvmStatic
        fun panicked(message: String) = CrashUploads.panicked(message)

        /** Called by the Rust side (any thread): a warning that goes with the next crash report. */
        @JvmStatic
        fun breadcrumb(category: String, message: String) = CrashUploads.breadcrumb(category, message)

        /** Called by the Rust side (any thread): the GPU dropped the device. */
        @JvmStatic
        fun gpuLost(why: String) = CrashUploads.gpuLost(why)

        fun start(context: Context) {
            context.startForegroundService(Intent(context, TuruService::class.java))
        }

        /**
         * Called by the Rust side (any thread) when the bots' state changes: [running] and
         * [paused] are the active tab's script's (Pause and Stop act on it), [startable] says its
         * loaded script can start (Start), [awake] says whether any tab's script runs (the wake
         * lock).
         */
        @JvmStatic
        fun status(title: String, text: String, running: Boolean, paused: Boolean, startable: Boolean, awake: Boolean) {
            main.post { instance?.show(title, text, running, paused, startable, awake) }
        }

        /** Posts the notification again: it's dropped while the permission isn't granted yet. */
        fun refresh() {
            main.post { instance?.let { it.show(it.title, it.text, it.running, it.paused, it.startable, it.awake) } }
        }
    }
}
