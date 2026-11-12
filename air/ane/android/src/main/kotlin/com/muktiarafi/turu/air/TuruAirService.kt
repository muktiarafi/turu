package com.muktiarafi.turu.air

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager

/**
 * Keeps the AIR app's process running off screen: a foreground service, so Android neither
 * freezes the process nor cuts its network. As Turu's TuruService does for the Ruffle build, its
 * notification shows the loaded script's state (`notify.rs`, through [Bridge.status]) with Pause
 * and Stop while it runs and Start while it's loaded and stopped, and it holds a partial wake lock
 * only while the script runs.
 */
class TuruAirService : Service() {
    private lateinit var wakeLock: PowerManager.WakeLock

    override fun onCreate() {
        super.onCreate()
        instance = this
        CrashReports.collect(applicationContext)
        val power = getSystemService(POWER_SERVICE) as PowerManager
        wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Turu:air").apply {
            setReferenceCounted(false)
        }
        if (Build.VERSION.SDK_INT >= 26) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(CHANNEL, appName(), NotificationManager.IMPORTANCE_LOW),
            )
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> Bridge.nativeAction("start")
            ACTION_PAUSE -> Bridge.nativeAction("pause")
            ACTION_STOP -> Bridge.nativeAction("stop")
            else -> {
                intent?.getStringExtra(EXTRA_TEXT)?.let { if (shown.title == null) shown = shown.copy(text = it) }
                startForegroundWith(build())
            }
        }
        apply()
        return START_NOT_STICKY
    }

    private fun startForegroundWith(notification: Notification) {
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(ID, notification)
        }
    }

    /** Shows [shown], and holds the wake lock while a tab's script runs (the screen may go off). */
    private fun apply() {
        if (shown.awake && !wakeLock.isHeld) {
            wakeLock.acquire()
        } else if (!shown.awake && wakeLock.isHeld) {
            wakeLock.release()
        }
        getSystemService(NotificationManager::class.java).notify(ID, build())
    }

    private fun action(action: String, code: Int): PendingIntent {
        val intent = Intent(this, TuruAirService::class.java).setAction(action)
        return PendingIntent.getService(this, code, intent, PendingIntent.FLAG_IMMUTABLE)
    }

    private fun build(): Notification {
        val builder = if (Build.VERSION.SDK_INT >= 26) {
            Notification.Builder(this, CHANNEL)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        packageManager.getLaunchIntentForPackage(packageName)?.let { open ->
            builder.setContentIntent(PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE))
        }
        builder
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle(shown.title ?: appName())
            .setContentText(shown.text)
            // The expanded layout, as Turu's: with the plain one the shade cut the buttons off.
            .setStyle(Notification.BigTextStyle().bigText(shown.text))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
        if (shown.running) {
            builder.addAction(
                Notification.Action.Builder(null, if (shown.paused) "Resume" else "Pause", action(ACTION_PAUSE, 1)).build(),
            )
            builder.addAction(Notification.Action.Builder(null, "Stop", action(ACTION_STOP, 2)).build())
        } else if (shown.startable) {
            builder.addAction(Notification.Action.Builder(null, "Start", action(ACTION_START, 3)).build())
        }
        return builder.build()
    }

    /** The app's name, as the launcher shows it ("Turu Mas"). */
    private fun appName(): String = applicationInfo.loadLabel(packageManager).toString()

    override fun onDestroy() {
        instance = null
        if (wakeLock.isHeld) {
            wakeLock.release()
        }
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    /** What the notification shows; `title` null until the engine has sent a state (the app's name). */
    private data class Shown(
        val title: String? = null,
        val text: String = "Running",
        val running: Boolean = false,
        val paused: Boolean = false,
        val startable: Boolean = false,
        /** Some tab's script runs: the wake lock is held. */
        val awake: Boolean = false,
    )

    companion object {
        const val EXTRA_TEXT = "text"
        const val ACTION_START = "com.muktiarafi.turu.air.START"
        const val ACTION_PAUSE = "com.muktiarafi.turu.air.PAUSE"
        const val ACTION_STOP = "com.muktiarafi.turu.air.STOP"
        private const val CHANNEL = "turu_air"
        private const val ID = 1

        @Volatile
        private var instance: TuruAirService? = null

        /** The latest state, kept for a service that starts after it came. */
        @Volatile
        private var shown = Shown()
        private val main = Handler(Looper.getMainLooper())

        /** The engine's state (any thread): shown on the main thread. */
        fun status(title: String, text: String, running: Boolean, paused: Boolean, startable: Boolean, awake: Boolean) {
            shown = Shown(title, text, running, paused, startable, awake)
            main.post { instance?.apply() }
        }
    }
}
