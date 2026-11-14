package com.muktiarafi.turu.shared

import android.app.Activity
import android.app.Application
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageInstaller
import android.os.Build
import android.os.Bundle
import android.util.Log
import java.io.File

/**
 * Installs a downloaded update's APK over the running app with Android's [PackageInstaller]
 * (both apps' update windows: `turu_frontend`'s `client_update.rs`). The engine downloads the APK
 * and checks it against the release's `SHA256SUMS.txt` first; this writes it into an install
 * session and commits it. Android then asks the user to confirm (and, the first time, to allow
 * installs from this app), installs, and stops the app; the user opens the new version.
 *
 * Android's confirmation is an activity, which an app can start only while it's on screen: when
 * the download ends with the app in the background, it is shown when one of the app's
 * activities comes back.
 */
object PackageUpdate {
    /** [done]'s statuses: installed (the process is usually stopped first), cancelled, failed. */
    const val INSTALLED = 0
    const val CANCELLED = 1
    const val FAILED = 2

    private const val ACTION = "com.muktiarafi.turu.shared.PACKAGE_INSTALL"

    /** What gets the outcome of the install in progress. */
    @Volatile
    private var done: ((Int, String?) -> Unit)? = null

    /** Android's confirmation, while it waits for an activity of the app to be on screen. */
    @Volatile
    private var confirm: Intent? = null

    /** The app's activity on screen, if one is. */
    @Volatile
    private var resumed: Activity? = null

    private var watching = false

    /**
     * Installs the APK at `path`, deleting it once it's in the session. `done` gets [INSTALLED],
     * [CANCELLED] or [FAILED] with Android's message, on the main thread. Null when the session
     * was committed, else why not. From any thread but the main one (it copies the file).
     * `context` is the activity on screen, which shows Android's confirmation.
     */
    fun install(context: Context, path: String, done: (Int, String?) -> Unit): String? = try {
        val app = context.applicationContext
        watch(app, context as? Activity)
        val file = File(path)
        val installer = app.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
        params.setAppPackageName(app.packageName)
        params.setSize(file.length())
        val id = installer.createSession(params)
        try {
            installer.openSession(id).use { session ->
                session.openWrite("update.apk", 0, file.length()).use { output ->
                    file.inputStream().use { it.copyTo(output) }
                    session.fsync(output)
                }
                file.delete()
                this.done = done
                val intent = Intent(ACTION).setPackage(app.packageName)
                // Mutable: the installer adds the status to it.
                val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                    (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
                session.commit(PendingIntent.getBroadcast(app, id, intent, flags).intentSender)
            }
        } catch (e: Exception) {
            installer.abandonSession(id)
            throw e
        }
        null
    } catch (e: Exception) {
        Log.w("turu", "Couldn't start installing the update: $e")
        e.toString()
    }

    /**
     * Registers the install's receiver and follows which activity is on screen, once: from
     * `current` (the one Download & install was tapped in, resumed before this could hear it).
     */
    @Synchronized
    private fun watch(app: Context, current: Activity?) {
        if (watching) return
        watching = true
        resumed = current
        val filter = IntentFilter(ACTION)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            app.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            app.registerReceiver(receiver, filter)
        }
        (app as Application).registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityResumed(a: Activity) {
                resumed = a
                confirm?.let { show(a, it) }
            }

            override fun onActivityPaused(a: Activity) {
                if (resumed === a) resumed = null
            }

            override fun onActivityCreated(a: Activity, savedInstanceState: Bundle?) {}
            override fun onActivityStarted(a: Activity) {}
            override fun onActivityStopped(a: Activity) {}
            override fun onActivitySaveInstanceState(a: Activity, outState: Bundle) {}
            override fun onActivityDestroyed(a: Activity) {}
        })
    }

    /** Shows Android's confirmation on `activity`; kept for the next activity if it can't. */
    private fun show(activity: Activity, intent: Intent) {
        confirm = try {
            activity.startActivity(intent)
            null
        } catch (e: Exception) {
            Log.w("turu", "Couldn't show the install confirmation: $e")
            intent
        }
    }

    /** The session's status, on the main thread. */
    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
            if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
                val ask = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableExtra(Intent.EXTRA_INTENT)
                } ?: return
                val activity = resumed
                if (activity != null) show(activity, ask) else confirm = ask
                return
            }
            confirm = null
            val outcome = when (status) {
                PackageInstaller.STATUS_SUCCESS -> INSTALLED
                PackageInstaller.STATUS_FAILURE_ABORTED -> CANCELLED
                else -> FAILED
            }
            val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
            Log.i("turu", "Update install: status $status ($message)")
            done?.invoke(outcome, message)
            done = null
        }
    }
}
