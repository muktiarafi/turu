package com.muktiarafi.turu.shared

import android.content.Context
import android.content.pm.ApplicationInfo
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import io.sentry.Attachment
import io.sentry.Breadcrumb
import io.sentry.Sentry
import io.sentry.SentryLevel
import io.sentry.android.core.SentryAndroid
import org.json.JSONObject
import java.io.File

/**
 * Crash reports sent to Turu's developers through Sentry, once the user said yes (the question
 * the app asks once, and Options > Application's switch: `sendCrashReports` in `settings.json`).
 * Sentry catches Java and native crashes as they happen, the engine's and (in Turu Mas) AIR's
 * runtime's alike (its NDK part, so a phone that keeps tombstones to itself still reports), reads
 * tombstones and ANRs from Android's exit records, and sends them at the next start. A Rust
 * panic's message goes with the native crash its abort makes ([panicked]); exits Sentry doesn't
 * catch (low memory, a battery saver's kill) go as [CrashReports]' reports ([exit]).
 *
 * Only builds made with a DSN have it ([CrashApp.sentryDsn]: Turu's `SENTRY_DSN`, Turu Mas's
 * `TURU_MAS_SENTRY_DSN`): without one, [available] is false, nothing starts and the question
 * isn't asked. Sentry sends no personal data (`sendDefaultPii` off: no IP address, no account),
 * no screenshots and no logs.
 */
object CrashUploads {
    private const val TAG = "turu"
    private val main = Handler(Looper.getMainLooper())

    private var context: Context? = null

    /** The app ([start]). */
    @Volatile
    private var app: CrashApp? = null

    @Volatile
    private var started = false

    /** Whether this build can send crash reports (known once [start] ran). */
    fun available(): Boolean = app?.sentryDsn?.isNotEmpty() == true

    /** Starts Sentry for `app` if the user said yes. Once per process, before the engine starts. */
    @Synchronized
    fun start(appContext: Context, app: CrashApp) {
        if (context != null) {
            return
        }
        this.app = app
        val context = appContext.applicationContext
        this.context = context
        if (available() && consented(context)) {
            init(context)
        }
    }

    /** The user's answer, from the app's settings (any thread). */
    fun setEnabled(on: Boolean) {
        main.post {
            synchronized(this) {
                val context = context
                if (!available() || context == null) {
                    return@post
                }
                if (on && !started) {
                    init(context)
                } else if (!on && started) {
                    Sentry.close()
                    started = false
                    Log.i(TAG, "Crash reports: off")
                }
            }
        }
    }

    /**
     * A Rust panic, from the panic hook just before the abort: its message goes with the native
     * crash Sentry records next (the tombstone, when the phone has one, also holds it).
     */
    fun panicked(message: String) {
        if (!started) {
            return
        }
        try {
            val crumb = Breadcrumb()
            crumb.category = "panic"
            crumb.level = SentryLevel.FATAL
            crumb.message = message
            Sentry.addBreadcrumb(crumb)
            Sentry.setExtra("panic", message)
            Sentry.setTag("panic", "yes")
        } catch (_: Throwable) {
            // About to abort anyway.
        }
    }

    /** A warning from the Rust side (ruffle-aqw's GPU warnings), kept for the next crash report. */
    fun breadcrumb(category: String, message: String) {
        if (!started) {
            return
        }
        try {
            val crumb = Breadcrumb()
            crumb.category = category
            crumb.level = SentryLevel.WARNING
            crumb.message = message
            Sentry.addBreadcrumb(crumb)
        } catch (_: Throwable) {
            // Only a breadcrumb.
        }
    }

    /**
     * The GPU dropped the device (out of memory, a driver reset): the app keeps running without a
     * picture. A warning of its own, grouped as one issue whatever the driver's message.
     */
    fun gpuLost(why: String) {
        if (!started) {
            return
        }
        Sentry.captureMessage("The GPU stopped working: $why", SentryLevel.WARNING) { scope ->
            scope.setTag("gpu", "lost")
            scope.fingerprint = listOf("gpu-lost")
        }
    }

    /** An exit Sentry doesn't catch itself (low memory, a kill), with the app's report attached. */
    fun exit(reason: String, summary: String, report: File) {
        if (!started) {
            return
        }
        Sentry.captureMessage(summary, SentryLevel.WARNING) { scope ->
            scope.setTag("exit_reason", reason)
            // One issue per exit reason: Sentry otherwise groups every exit by this call's stack,
            // and a ColorOS CPU kill, low-memory kills and a start timeout became one issue.
            scope.fingerprint = listOf("android-exit", reason)
            scope.addAttachment(Attachment(report.absolutePath, report.name, "text/plain"))
        }
    }

    private fun init(context: Context) {
        val app = app ?: return
        val info = context.packageManager.getPackageInfo(context.packageName, 0)
        val code = if (Build.VERSION.SDK_INT >= 28) info.longVersionCode else {
            @Suppress("DEPRECATION")
            info.versionCode.toLong()
        }
        val debug = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        SentryAndroid.init(context) { options ->
            options.dsn = app.sentryDsn
            // As the engine's version (`turu::VERSION`) and the release workflow's symbols.
            options.release = app.release + "@" + info.versionName
            options.dist = code.toString()
            options.environment = if (debug) "debug" else "release"
            @Suppress("DEPRECATION") // Deprecated in Sentry 8 but still read; off keeps the IP address out.
            options.isSendDefaultPii = false
            options.isAttachScreenshot = false
            options.isAttachViewHierarchy = false
            // Native crashes: the NDK handler as they happen, merged with the tombstone where
            // Android hands it over (Android 12+), the raw tombstone attached.
            options.isEnableNdk = true
            options.isTombstoneEnabled = true
            options.isAttachRawTombstone = true
        }
        // Turu's Sentry project takes both apps' reports: this tells them apart.
        app.client?.let { Sentry.setTag("client", it) }
        started = true
        Log.i(TAG, "Crash reports: on")
    }

    /** `sendCrashReports` in the data folder's settings.json (Turu's `AppSettings`). */
    private fun consented(context: Context): Boolean {
        val settings = File(context.getExternalFilesDir(null) ?: context.filesDir, "settings.json")
        return try {
            JSONObject(settings.readText()).optBoolean("sendCrashReports", false)
        } catch (_: Exception) {
            // No settings yet: not asked yet.
            false
        }
    }
}
