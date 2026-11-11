package com.muktiarafi.turu.air

import android.Manifest
import android.app.Activity
import android.app.Application
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.util.Log

/**
 * The two questions Turu's own app asks once (its `TuruActivity.askOnce`): notifications (Android
 * 13+), without which [TuruAirService]'s notification is hidden, and leave from battery
 * optimisation, without which some phones stop the game off screen. One dialog at a time: the
 * battery question waits until the activity is back from the notifications one. AIR owns the
 * activity, so its permission result can't be had; its next resume stands in for it.
 */
object Prompts {
    private const val PREFS = "asked"

    fun askOnce(activity: Activity) {
        activity.runOnUiThread {
            try {
                if (askNotifications(activity)) {
                    afterNextResume(activity) { askBattery(it) }
                } else {
                    askBattery(activity)
                }
            } catch (e: Exception) {
                Log.w("TuruAir", "Couldn't ask for notifications or battery", e)
            }
        }
    }

    /** Asks for notifications once; whether it did (a dialog is up). */
    private fun askNotifications(activity: Activity): Boolean {
        val asked = activity.getSharedPreferences(PREFS, Activity.MODE_PRIVATE)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED ||
            asked.getBoolean("notifications", false)
        ) {
            return false
        }
        asked.edit().putBoolean("notifications", true).apply()
        activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        return true
    }

    private fun askBattery(activity: Activity) {
        val asked = activity.getSharedPreferences(PREFS, Activity.MODE_PRIVATE)
        val power = activity.getSystemService(PowerManager::class.java)
        if (power.isIgnoringBatteryOptimizations(activity.packageName) || asked.getBoolean("battery", false)) {
            return
        }
        asked.edit().putBoolean("battery", true).apply()
        activity.startActivity(
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${activity.packageName}")),
        )
    }

    /** Runs [then] once [activity] next comes back to the front (the permission dialog closed). */
    private fun afterNextResume(activity: Activity, then: (Activity) -> Unit) {
        val application = activity.application
        application.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            private var paused = false

            override fun onActivityPaused(a: Activity) {
                if (a === activity) paused = true
            }

            override fun onActivityResumed(a: Activity) {
                if (a !== activity || !paused) return
                application.unregisterActivityLifecycleCallbacks(this)
                then(a)
            }

            override fun onActivityCreated(a: Activity, savedInstanceState: Bundle?) {}
            override fun onActivityStarted(a: Activity) {}
            override fun onActivityStopped(a: Activity) {}
            override fun onActivitySaveInstanceState(a: Activity, outState: Bundle) {}
            override fun onActivityDestroyed(a: Activity) {
                if (a === activity) application.unregisterActivityLifecycleCallbacks(this)
            }
        })
    }
}
