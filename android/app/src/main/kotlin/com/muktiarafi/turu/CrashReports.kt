package com.muktiarafi.turu

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import android.os.PowerManager
import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.IOException
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * How the last runs ended, from Android's own record (ActivityManager's historical exit
 * reasons, Android 11 and later): a report in the data folder's `crash/` for each exit Turu
 * didn't ask for, so a crash, a low-memory kill or a battery saver's kill is known after the fact
 * without logcat. Native crashes come with their tombstone (Android 12 and later), read by
 * [Tombstone], plus the raw file next to the report. The engine's memory log of that process
 * (`crash/memory-<pid>.log`, `android/src/memory.rs`) is added to its report, and the logs of
 * older processes are then deleted. The Rust side lists the reports
 * (Diagnostics > Logs > Crashes, `turu_frontend::crash_reports`) and shows a notice for new ones.
 * Runs once per process, before the engine starts.
 */
internal object CrashReports {
    private const val TAG = "turu"

    /** The most exits Android is asked for; it keeps a few per app. */
    private const val MAX_EXITS = 16

    /** An ANR's trace can be several MB; the start has the main thread. */
    private const val MAX_TRACE_BYTES = 512 * 1024

    private var collected = false

    /** Writes reports for the exits since the last call (in an earlier process). */
    @Synchronized
    fun collect(context: Context) {
        if (collected) {
            return
        }
        collected = true
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            return
        }
        try {
            collectNow(context)
        } catch (e: Throwable) {
            Log.w(TAG, "Couldn't read how the last runs ended", e)
        }
    }

    private fun collectNow(context: Context) {
        val dir = File(context.getExternalFilesDir(null) ?: context.filesDir, "crash")
        val prefs = context.getSharedPreferences("crash", Context.MODE_PRIVATE)
        val last = prefs.getLong("lastExit", 0)
        val activities = context.getSystemService(ActivityManager::class.java)
        val exits = activities.getHistoricalProcessExitReasons(context.packageName, 0, MAX_EXITS)
        var newest = last
        // Newest first; the reports' times sort them anyway.
        for (exit in exits) {
            if (exit.timestamp <= last) {
                continue
            }
            newest = maxOf(newest, exit.timestamp)
            if (!unexpected(exit)) {
                continue
            }
            try {
                write(context, activities, dir, exit)
            } catch (e: Throwable) {
                Log.w(TAG, "Couldn't write the report of an exit", e)
            }
        }
        prefs.edit().putLong("lastExit", newest).apply()
        deleteMemoryLogs(dir)
    }

    /** The memory logs of earlier processes, once their reports have them (or they ended well). */
    private fun deleteMemoryLogs(dir: File) {
        val mine = "memory-${android.os.Process.myPid()}.log"
        dir.listFiles { file -> file.name.startsWith("memory-") && file.name.endsWith(".log") && file.name != mine }
            ?.forEach { it.delete() }
    }

    /** Not a stop someone asked for (Force stop, an update, Turu's own clean exit). */
    private fun unexpected(exit: ApplicationExitInfo): Boolean = when (exit.reason) {
        ApplicationExitInfo.REASON_USER_REQUESTED,
        ApplicationExitInfo.REASON_USER_STOPPED,
        ApplicationExitInfo.REASON_PACKAGE_UPDATED,
        ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE,
        -> false
        ApplicationExitInfo.REASON_EXIT_SELF -> exit.status != 0
        else -> true
    }

    private fun write(context: Context, activities: ActivityManager, dir: File, exit: ApplicationExitInfo) {
        if (!dir.isDirectory && !dir.mkdirs()) {
            throw IOException("Couldn't make $dir")
        }
        val reason = exit.reason
        val `when` = exit.timestamp
        val name = "exit-" + SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.US).format(Date(`when`)) +
            "-" + reasonName(reason).lowercase(Locale.US)
        val trace = readTrace(exit)
        val tombstone = trace != null && reason == ApplicationExitInfo.REASON_CRASH_NATIVE
        val report = StringBuilder()
        report.append(summary(exit)).append("\n\n")
        hint(exit)?.let { report.append(it).append("\n\n") }
        report.append("When: ").append(SimpleDateFormat("yyyy-MM-dd HH:mm:ss Z", Locale.US).format(Date(`when`)))
            .append('\n')
        report.append("Reason: ").append(reasonName(reason)).append(" (").append(reason)
            .append("), status ").append(exit.status).append('\n')
        exit.description?.let { report.append("Description: ").append(it).append('\n') }
        report.append("Was: ").append(importance(exit.importance)).append('\n')
        // Android often leaves PSS at 0.
        report.append("Memory at the end: RSS ").append(exit.rss / 1024).append(" MB")
        if (exit.pss > 0) {
            report.append(", PSS ").append(exit.pss / 1024).append(" MB")
        }
        report.append('\n')
        appendDevice(context, activities, report)
        val memory = File(dir, "memory-${exit.pid}.log")
        if (memory.isFile) {
            report.append("\nMemory over the run (").append(memory.name).append(", a reading a minute)\n")
                .append(memory.readText())
        }
        if (tombstone) {
            val formatted = Tombstone.format(trace)
            val raw = File(dir, "$name-tombstone.pb")
            raw.writeBytes(trace)
            report.append("\nTombstone (raw: ").append(raw.name).append(")\n")
            report.append(formatted ?: "Couldn't read it; the raw file has it.\n")
        } else if (trace != null) {
            report.append("\nTrace\n").append(String(trace, Charsets.UTF_8))
            if (trace.size >= MAX_TRACE_BYTES) {
                report.append("\n... (cut)\n")
            }
        }
        val file = File(dir, "$name.txt")
        file.writeText(report.toString())
        // The list sorts by this: the exit's time, not the report's.
        file.setLastModified(`when`)
        // Sentry reports crashes and ANRs itself (with more detail); the rest goes from here.
        if (reason != ApplicationExitInfo.REASON_CRASH && reason != ApplicationExitInfo.REASON_CRASH_NATIVE &&
            reason != ApplicationExitInfo.REASON_ANR
        ) {
            CrashUploads.exit(reasonName(reason), summary(exit), file)
        }
        Log.w(TAG, "The last run ended: ${summary(exit)} ($file)")
    }

    private fun summary(exit: ApplicationExitInfo): String {
        val description = exit.description
        val detail = if (!description.isNullOrEmpty()) " ($description)" else ""
        return when (exit.reason) {
            ApplicationExitInfo.REASON_LOW_MEMORY -> "Android closed Turu to free memory: the phone ran low"
            ApplicationExitInfo.REASON_CRASH_NATIVE ->
                if (exit.status == 6) {
                    "Turu crashed: native code aborted (signal 6)"
                } else {
                    "Turu crashed in native code (signal ${exit.status})"
                }
            ApplicationExitInfo.REASON_CRASH -> "Turu crashed: a Java exception$detail"
            ApplicationExitInfo.REASON_ANR -> "Turu stopped responding and Android closed it"
            ApplicationExitInfo.REASON_SIGNALED -> "The system killed Turu (signal ${exit.status})$detail"
            ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE ->
                "Android closed Turu for using too much CPU in the background$detail"
            ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "Turu failed to start$detail"
            ApplicationExitInfo.REASON_PERMISSION_CHANGE -> "Android closed Turu after a permission changed"
            ApplicationExitInfo.REASON_DEPENDENCY_DIED -> "Android closed Turu after a process it depends on died"
            ApplicationExitInfo.REASON_FREEZER -> "Android closed Turu while it was frozen in the background"
            ApplicationExitInfo.REASON_EXIT_SELF -> "Turu exited with status ${exit.status}"
            else -> "The system closed Turu$detail"
        }
    }

    /** What the person can do about it, when there is something. */
    private fun hint(exit: ApplicationExitInfo): String? = when (exit.reason) {
        ApplicationExitInfo.REASON_LOW_MEMORY ->
            "One game client uses about 400 MB. Closing other apps while Turu runs helps."
        ApplicationExitInfo.REASON_SIGNALED,
        ApplicationExitInfo.REASON_OTHER,
        ApplicationExitInfo.REASON_UNKNOWN,
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE,
        ApplicationExitInfo.REASON_FREEZER,
        ->
            "If nobody stopped Turu: some phones' battery savers (Xiaomi's MIUI and HyperOS, Huawei, " +
                "Samsung's sleeping apps) close apps in the background. Allow Turu to start on its own " +
                "(Autostart) and set its battery saver to No restrictions."
        else -> null
    }

    private fun appendDevice(context: Context, activities: ActivityManager, report: StringBuilder) {
        report.append("Phone: ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append(" (")
            .append(Build.DEVICE).append("), Android ").append(Build.VERSION.RELEASE).append(" (API ")
            .append(Build.VERSION.SDK_INT).append("), build ").append(Build.DISPLAY).append('\n')
        val memory = ActivityManager.MemoryInfo()
        activities.getMemoryInfo(memory)
        report.append("Phone memory: ").append(memory.totalMem / (1024 * 1024)).append(" MB, ")
            .append(memory.availMem / (1024 * 1024)).append(" MB free now\n")
        try {
            val info = context.packageManager.getPackageInfo(context.packageName, 0)
            report.append("Turu: ").append(info.versionName).append(" (").append(info.longVersionCode).append(")\n")
        } catch (_: Exception) {
            // The report is still useful without it.
        }
        val power = context.getSystemService(PowerManager::class.java)
        report.append("Battery optimisation: ")
            .append(if (power.isIgnoringBatteryOptimizations(context.packageName)) "off for Turu" else "on")
            .append(", background restricted: ").append(if (activities.isBackgroundRestricted) "yes" else "no")
            .append(" (now)\n")
    }

    private fun readTrace(exit: ApplicationExitInfo): ByteArray? = try {
        exit.traceInputStream?.use { input ->
            val out = ByteArrayOutputStream()
            val buffer = ByteArray(16 * 1024)
            val limit = if (exit.reason == ApplicationExitInfo.REASON_CRASH_NATIVE) Int.MAX_VALUE else MAX_TRACE_BYTES
            while (out.size() < limit) {
                val n = input.read(buffer)
                if (n <= 0) {
                    break
                }
                out.write(buffer, 0, minOf(n, limit - out.size()))
            }
            out.toByteArray()
        }
    } catch (e: IOException) {
        Log.w(TAG, "Couldn't read an exit's trace", e)
        null
    }

    private fun importance(importance: Int): String = when (importance) {
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND -> "on screen"
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND_SERVICE ->
            "in the background with its notification (foreground service)"
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_VISIBLE -> "visible"
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_PERCEPTIBLE -> "perceptible"
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_SERVICE -> "a background service"
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_CACHED -> "cached in the background"
        ActivityManager.RunningAppProcessInfo.IMPORTANCE_GONE -> "gone"
        else -> "importance $importance"
    }

    private fun reasonName(reason: Int): String = when (reason) {
        ApplicationExitInfo.REASON_EXIT_SELF -> "EXIT_SELF"
        ApplicationExitInfo.REASON_SIGNALED -> "SIGNALED"
        ApplicationExitInfo.REASON_LOW_MEMORY -> "LOW_MEMORY"
        ApplicationExitInfo.REASON_CRASH -> "CRASH"
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "CRASH_NATIVE"
        ApplicationExitInfo.REASON_ANR -> "ANR"
        ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "INITIALIZATION_FAILURE"
        ApplicationExitInfo.REASON_PERMISSION_CHANGE -> "PERMISSION_CHANGE"
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "EXCESSIVE_RESOURCE_USAGE"
        ApplicationExitInfo.REASON_USER_REQUESTED -> "USER_REQUESTED"
        ApplicationExitInfo.REASON_USER_STOPPED -> "USER_STOPPED"
        ApplicationExitInfo.REASON_DEPENDENCY_DIED -> "DEPENDENCY_DIED"
        ApplicationExitInfo.REASON_OTHER -> "OTHER"
        ApplicationExitInfo.REASON_FREEZER -> "FREEZER"
        ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE -> "PACKAGE_STATE_CHANGE"
        ApplicationExitInfo.REASON_PACKAGE_UPDATED -> "PACKAGE_UPDATED"
        else -> "UNKNOWN"
    }
}
