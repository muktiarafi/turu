package com.muktiarafi.turu.air

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager

/**
 * Keeps the AIR app's process running off screen: a foreground service (so Android neither
 * freezes the process nor cuts its network) holding a partial wake lock. As Turu's
 * TuruService does for the Ruffle build.
 */
class TuruAirService : Service() {
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification = notification(intent?.getStringExtra(EXTRA_TEXT) ?: "Running")
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(ID, notification)
        }
        if (wakeLock == null) {
            val power = getSystemService(POWER_SERVICE) as PowerManager
            wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Turu:air").apply {
                setReferenceCounted(false)
                acquire()
            }
        }
        return START_NOT_STICKY
    }

    private fun notification(text: String): Notification {
        val builder = if (Build.VERSION.SDK_INT >= 26) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(CHANNEL, appName(), NotificationManager.IMPORTANCE_LOW)
            )
            Notification.Builder(this, CHANNEL)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        packageManager.getLaunchIntentForPackage(packageName)?.let { open ->
            builder.setContentIntent(PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE))
        }
        return builder
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle(appName())
            .setContentText(text)
            .setOngoing(true)
            .build()
    }

    /** The app's name, as the launcher shows it ("Turu Mas"). */
    private fun appName(): String = applicationInfo.loadLabel(packageManager).toString()

    override fun onDestroy() {
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val EXTRA_TEXT = "text"
        private const val CHANNEL = "turu_air"
        private const val ID = 1
    }
}
