package com.muktiarafi.turu.shared

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.util.Log
import java.util.concurrent.atomic.AtomicInteger

/**
 * Turu's alerts as Android notifications, heard with the screen off: a watched item dropped
 * (Notify Drop) and a tab's script stopped (`turu_frontend` decides when: `notify_drop.rs`,
 * `turu::watch_alerts`). Each
 * kind has a channel of its own, which the user can silence or turn off in Android's settings.
 * Posted only while none of the app's activities is on screen ([Foreground]): there, Notify
 * Drop beeps and the windows show the stop.
 */
object Alerts {
    /** [post]'s kinds, as the Rust side numbers them (`platform::Alert`). */
    const val DROP = 0
    const val SCRIPT_STOPPED = 1

    private const val DROP_CHANNEL = "turu_drops"
    private const val STOPPED_CHANNEL = "turu_script_stopped"

    /** The foreground service's notification has ID 1: these count up from 1000. */
    private val nextId = AtomicInteger(1000)

    @Volatile
    private var context: Context? = null

    private var icon = 0

    /**
     * Makes the channels and keeps the app's context and the notifications' small `icon`. From
     * the foreground service's `onCreate` (both apps).
     */
    fun init(context: Context, icon: Int) {
        val app = context.applicationContext
        this.icon = icon
        val manager = app.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(DROP_CHANNEL, "Drops", NotificationManager.IMPORTANCE_HIGH).apply {
                description = "An item Notify Drop watches dropped"
            },
        )
        manager.createNotificationChannel(
            NotificationChannel(STOPPED_CHANNEL, "Script stopped", NotificationManager.IMPORTANCE_HIGH).apply {
                description = "A tab's script finished, failed or was stopped"
            },
        )
        this.context = app
    }

    /**
     * Posts an alert of `kind` ([DROP], [SCRIPT_STOPPED]) unless the app is on screen; tapping it
     * opens the app. False when it wasn't posted. Any thread.
     */
    fun post(kind: Int, title: String, text: String): Boolean {
        val context = context ?: return false.also { Log.w("turu", "Alert before Alerts.init: $title") }
        if (Foreground.activity != null) {
            Log.i("turu", "Alert not posted, the app is on screen: $title")
            return false
        }
        val channel = if (kind == DROP) DROP_CHANNEL else STOPPED_CHANNEL
        val builder = Notification.Builder(context, channel)
            .setSmallIcon(icon)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setCategory(if (kind == DROP) Notification.CATEGORY_EVENT else Notification.CATEGORY_STATUS)
            .setGroup(channel)
            .setAutoCancel(true)
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.let { open ->
            builder.setContentIntent(PendingIntent.getActivity(context, 0, open, PendingIntent.FLAG_IMMUTABLE))
        }
        // Dropped by Android while the notification permission isn't granted.
        context.getSystemService(NotificationManager::class.java).notify(nextId.getAndIncrement(), builder.build())
        return true
    }
}
