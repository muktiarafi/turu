package com.muktiarafi.turu.shared

import android.app.Application
import android.content.Context
import android.content.pm.ApplicationInfo
import android.os.Build
import androidx.core.content.pm.PackageInfoCompat
import android.os.Handler
import android.os.Looper
import android.util.Log
import io.sentry.Attachment
import io.sentry.Breadcrumb
import io.sentry.Sentry
import io.sentry.SentryEvent
import io.sentry.SentryLevel
import io.sentry.android.core.SentryAndroid
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.util.Date

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

    /**
     * An exit Sentry doesn't catch itself (low memory, a kill), with the app's report attached.
     * `tabs`: the tabs open at the end and the most this run, when the tabs log had them;
     * `process`: the process that ended (Turu's process tabs: `com.muktiarafi.turu:c1` for slot 1);
     * `windowAlone`: only the window's process ended, its tabs' went on ([CrashReports]).
     */
    fun exit(
        reason: String,
        summary: String,
        report: File,
        tabs: Pair<Int, Int>? = null,
        process: String? = null,
        windowAlone: Boolean = false,
    ) {
        if (!started) {
            return
        }
        Sentry.captureMessage(summary, SentryLevel.WARNING) { scope ->
            scope.setTag("exit_reason", reason)
            // The process that ended, not the one sending this (the scope's own "process" tag).
            process?.let { scope.setTag("process", processTag(it)) }
            tabs?.let { (open, most) ->
                scope.setTag("tabs", open.toString())
                scope.setTag("tabs_most", most.toString())
            }
            // One issue per exit reason: Sentry otherwise groups every exit by this call's stack,
            // and a ColorOS CPU kill, low-memory kills and a start timeout became one issue. The
            // window's process ending alone has its own (the tabs played on): what pausing its pages
            // off screen should shrink (TURU-ANDROID-D's CPU kills, `ModelClient`).
            if (windowAlone) scope.setTag("window_alone", "yes")
            scope.fingerprint = if (windowAlone) listOf("android-exit", reason, "window-alone") else listOf("android-exit", reason)
            scope.addAttachment(Attachment(report.absolutePath, report.name, "text/plain"))
        }
    }

    private fun init(context: Context) {
        val app = app ?: return
        val info = context.packageManager.getPackageInfo(context.packageName, 0)
        val code = PackageInfoCompat.getLongVersionCode(info)
        val debug = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        SentryAndroid.init(context) { options ->
            options.dsn = app.sentryDsn
            // As the engine's version (`turu::VERSION`) and the release workflow's symbols.
            options.release = app.release + "@" + info.versionName
            options.dist = code.toString()
            options.environment = if (debug) "debug" else "release"
            // No IP address and no user data: Sentry's `sendDefaultPii`, off by default, left so.
            options.isAttachScreenshot = false
            options.isAttachViewHierarchy = false
            // Native crashes: the NDK handler as they happen, merged with the tombstone where
            // Android hands it over (Android 12+), the raw tombstone attached.
            options.isEnableNdk = true
            options.isTombstoneEnabled = true
            options.isAttachRawTombstone = true
            // Each of the app's processes reads Android's exit record as it starts: one report per exit.
            options.setBeforeSend { event, _ -> if (firstToSend(context, event)) event else null }
        }
        // Turu's Sentry project takes both apps' reports: this tells them apart.
        app.client?.let { Sentry.setTag("client", it) }
        // Which process crashed: the app's ("main"), or a process tab's slot ("c1" ... "c4").
        Sentry.setTag("process", processTag(processName(context)))
        started = true
        Log.i(TAG, "Crash reports: on")
    }

    /**
     * Whether this process is the first to send `event`, when it's one of Sentry's reports read
     * from Android's exit record (a tombstone, an ANR); any other event is sent. Sentry reads the
     * record in every process as it starts, and remembers the last one sent in its cache folder,
     * which the app's processes share; but it writes that only once the report is stored, so
     * processes starting together (Turu's tabs, started again after a crash) each sent the same
     * tombstone: one crash came 14 times (TURU-ANDROID-7, a tab's process on 0.4.0-beta.3). Here
     * the exit's time is claimed under a lock every process takes, and only the first sends it.
     */
    @Synchronized
    private fun firstToSend(context: Context, event: SentryEvent): Boolean {
        val mechanism = event.exceptions?.firstNotNullOfOrNull { it.mechanism?.type } ?: return true
        val kind = EXIT_MECHANISMS[mechanism] ?: return true
        val at = event.timestamp?.time ?: return true
        val key = "$kind $at"
        return try {
            RandomAccessFile(File(context.filesDir, "sentry-exits.txt"), "rw").use { file ->
                file.channel.lock().use {
                    val sent = generateSequence { file.readLine() }.toMutableList()
                    if (key in sent) {
                        Log.i(TAG, "Crash reports: $kind of ${Date(at)} sent by another process already")
                        return false
                    }
                    sent.add(key)
                    file.setLength(0)
                    file.writeBytes(sent.takeLast(KEEP_EXITS).joinToString("\n", postfix = "\n"))
                    true
                }
            }
        } catch (e: IOException) {
            Log.w(TAG, "Couldn't check whether another process sent a report", e)
            true
        }
    }

    /** Sentry's mechanisms for the reports it reads from Android's exit record, by kind. */
    private val EXIT_MECHANISMS = mapOf(
        "Tombstone" to "tombstone",
        "TombstoneMerged" to "tombstone",
        "AppExitInfo" to "anr",
    )

    /** The exits remembered as sent (Android keeps a few per app). */
    private const val KEEP_EXITS = 64

    /** "main" for the app's own process, else the part after its ':' ("c1" for `…turu:c1`). */
    private fun processTag(name: String): String = name.substringAfter(':', "main")

    /** This process's name (`com.muktiarafi.turu`, `com.muktiarafi.turu:c1`). */
    private fun processName(context: Context): String =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            Application.getProcessName()
        } else {
            runCatching { File("/proc/self/cmdline").readText().trimEnd('\u0000') }
                .getOrDefault(context.packageName)
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
