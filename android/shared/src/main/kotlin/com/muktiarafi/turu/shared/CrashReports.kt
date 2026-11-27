package com.muktiarafi.turu.shared

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import android.os.PowerManager
import android.os.SystemClock
import android.system.Os
import android.system.OsConstants
import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * How the last runs ended, from Android's own record (ActivityManager's historical exit
 * reasons, Android 11 and later): a report in the data folder's `crash/` for each exit the app
 * didn't ask for, so a crash (the engine's, or AIR's runtime's in Turu Mas), a hang, a low-memory
 * kill or a battery saver's kill is known after the fact without logcat. Native crashes come with
 * their tombstone (Android 12 and later), read by [Tombstone], plus the raw file next to the
 * report. The engine's memory log of that process (`crash/memory-<pid>.log`, Turu's
 * `android/src/memory.rs`; Turu Mas keeps none) is added to its report, and the logs of older
 * processes are then deleted. The Rust side lists the reports (Diagnostics > Logs > Crashes,
 * `turu_frontend::crash_reports`) and shows a notice for new ones. Runs once per process, before
 * the engine starts, after [CrashUploads.start]: exits Sentry doesn't catch itself go to it
 * ([CrashUploads.exit]). Turu's process tabs run it in each slot's process too, which can start at
 * once: a file lock makes one process at a time read and mark the exits, so each is reported once.
 */
object CrashReports {
    private const val TAG = "turu"

    /** The most exits Android is asked for; it keeps a few per app. */
    private const val MAX_EXITS = 16

    /** An ANR's trace can be several MB; the start has the main thread. */
    private const val MAX_TRACE_BYTES = 512 * 1024

    private var collected = false

    /** The app the reports are about ([collect]). */
    private var app = CrashApp("Turu", "", "turu", null, "")

    /** Writes reports for the exits of `app` since the last call (in an earlier process). */
    @Synchronized
    fun collect(context: Context, app: CrashApp) {
        if (collected) {
            return
        }
        collected = true
        this.app = app
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
        if (!dir.isDirectory && !dir.mkdirs()) {
            throw IOException("Couldn't make $dir")
        }
        // The app's other processes (process tabs' slots) may be collecting at the same moment.
        RandomAccessFile(File(dir, "collect.lock"), "rw").use { file ->
            file.channel.lock().use { collectLocked(context, dir) }
        }
    }

    private fun collectLocked(context: Context, dir: File) {
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
        // Written before the lock is let go (not `apply`, which writes later), for the next process.
        prefs.edit().putLong("lastExit", newest).commit()
        deleteMemoryLogs(dir, activities)
    }

    /**
     * The memory and tabs logs of earlier processes, once their reports have them (or they ended
     * well). Not those of the app's processes still running (other process tabs).
     */
    private fun deleteMemoryLogs(dir: File, activities: ActivityManager) {
        val running = activities.runningAppProcesses.orEmpty().map { it.pid }.toSet() +
            android.os.Process.myPid()
        dir.listFiles { file ->
            val pid = PROCESS_LOG.matchEntire(file.name)?.groupValues?.get(2)?.toIntOrNull()
            pid != null && pid !in running
        }?.forEach { it.delete() }
    }

    private val PROCESS_LOG = Regex("""(memory|tabs)-(\d+)\.log""")

    /**
     * The tabs open at the end and the most this run, from the tabs log's last line
     * ("12:00:00 2 tabs, at most 3 this run: ...").
     */
    internal fun tabCounts(log: String): Pair<Int, Int>? {
        val line = log.lineSequence().lastOrNull { TAB_COUNTS.containsMatchIn(it) } ?: return null
        val match = TAB_COUNTS.find(line) ?: return null
        return match.groupValues[1].toInt() to match.groupValues[2].toInt()
    }

    private val TAB_COUNTS = Regex("""(\d+) tabs?, at most (\d+)""")

    /** Not a stop someone asked for (Force stop, an update, the app's own clean exit). */
    private fun unexpected(exit: ApplicationExitInfo): Boolean = when (exit.reason) {
        ApplicationExitInfo.REASON_USER_REQUESTED,
        ApplicationExitInfo.REASON_USER_STOPPED,
        ApplicationExitInfo.REASON_PACKAGE_UPDATED,
        ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE,
        -> false
        // A slot that lost its GPU ends itself to start again (`memory.rs`, `EXIT_GPU_LOST`): that
        // went to Sentry as a warning already.
        // Nor one started again by hand after kills for memory (`memory_kills.rs`, `EXIT_START_AGAIN`).
        ApplicationExitInfo.REASON_EXIT_SELF ->
            exit.status != 0 && exit.status != EXIT_GPU_LOST && exit.status != EXIT_START_AGAIN
        else -> true
    }

    /** The exit status of a Turu slot that lost its GPU (`android/src/memory.rs`). */
    private const val EXIT_GPU_LOST = 86

    /** The exit status of a Turu slot started again by hand (`android/src/memory_kills.rs`). */
    private const val EXIT_START_AGAIN = 87

    /** How a process ended: Android's `ApplicationExitInfo` timestamp (ms), reason and status. */
    data class ProcessExit(val timestamp: Long, val reason: Int, val status: Int)

    /**
     * How process `processName` of this app last ended, from Android's record (Android 11 and
     * later; null before, or with no record): Turu's process tabs read it as a slot starts, to tell
     * a kill for memory from other restarts. Its own query, over every record Android keeps (the
     * other processes' exits may be newer): [collect] may have marked this one read already.
     */
    fun lastExit(context: Context, processName: String): ProcessExit? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return null
        return try {
            context.getSystemService(ActivityManager::class.java)
                .getHistoricalProcessExitReasons(context.packageName, 0, 0)
                .filter { it.processName == processName }
                .maxByOrNull { it.timestamp }
                ?.let { ProcessExit(it.timestamp, it.reason, it.status) }
        } catch (e: Throwable) {
            Log.w(TAG, "Couldn't read how $processName last ended", e)
            null
        }
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
        val windowAlone = windowEndedAlone(context, activities, exit)
        val summary = summary(exit, windowAlone)
        val report = StringBuilder()
        report.append(summary).append("\n\n")
        (if (windowAlone) windowHint() else hint(exit))?.let { report.append(it).append("\n\n") }
        report.append("When: ").append(SimpleDateFormat("yyyy-MM-dd HH:mm:ss Z", Locale.US).format(Date(`when`)))
            .append('\n')
        report.append("Reason: ").append(reasonName(reason)).append(" (").append(reason)
            .append("), status ").append(exit.status).append('\n')
        exit.description?.let { report.append("Description: ").append(it).append('\n') }
        report.append("Was: ").append(importance(exit.importance)).append('\n')
        // Android often leaves RSS and PSS at 0 (ColorOS's kills: "RSS 0 MB"), and some phones
        // fill in nonsense (an OPPO: 20 million MB): only sizes the phone's memory could hold are
        // shown, and the line is left out without any (Turu's memory log below has the readings).
        val phoneMemory = ActivityManager.MemoryInfo().also { activities.getMemoryInfo(it) }.totalMem
        val sizes = listOf("RSS" to exit.rss, "PSS" to exit.pss)
            .filter { (_, kb) -> kb > 0 && kb * 1024 <= phoneMemory }
            .joinToString(", ") { (name, kb) -> "$name ${kb / 1024} MB" }
        if (sizes.isNotEmpty()) {
            report.append("Memory at the end: ").append(sizes).append('\n')
        }
        // How many game clients the process held (turu_frontend's tabs_log): Sentry can then tell
        // one account's low-memory kills from several (docs/TAB_PROCESSES_PROPOSAL.md, M0).
        val tabsLog = File(dir, "tabs-${exit.pid}.log")
        val tabsText = if (tabsLog.isFile) tabsLog.readText() else null
        val tabs = tabsText?.let(::tabCounts)
        if (tabs != null) {
            report.append("Tabs at the end: ").append(tabs.first).append(" (at most ").append(tabs.second)
                .append(" this run)\n")
        }
        appendDevice(context, activities, report)
        if (tabsText != null) {
            report.append("\nTabs over the run (").append(tabsLog.name).append(", a line per change)\n")
                .append(tabsText)
        }
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
        // Nothing to tell the person about when the tabs played on: listed, without the notice
        // (`turu_frontend::crash_reports`' seen list).
        if (windowAlone) markSeen(dir, file.name)
        // Sentry reports crashes and ANRs itself (with more detail); the rest goes from here.
        if (reason != ApplicationExitInfo.REASON_CRASH && reason != ApplicationExitInfo.REASON_CRASH_NATIVE &&
            reason != ApplicationExitInfo.REASON_ANR
        ) {
            CrashUploads.exit(reasonName(reason), summary, file, tabs, exit.processName, windowAlone)
        }
        Log.w(TAG, "The last run ended: $summary ($file)")
    }

    /**
     * Whether Turu's window alone was closed: the activity's process (process tabs' shell) ended
     * by a kill, not a crash, while a tab's process that started before it still runs. Android
     * closes the shell, which has no foreground service, on its own in the background (2% of a core
     * at most, TURU-ANDROID-D), and the tabs play on. Never in Turu Mas, which has one process.
     */
    private fun windowEndedAlone(context: Context, activities: ActivityManager, exit: ApplicationExitInfo): Boolean {
        if (exit.processName != context.packageName) return false
        when (exit.reason) {
            ApplicationExitInfo.REASON_CRASH,
            ApplicationExitInfo.REASON_CRASH_NATIVE,
            ApplicationExitInfo.REASON_ANR,
            ApplicationExitInfo.REASON_EXIT_SELF,
            ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
            -> return false
        }
        val tab = "${context.packageName}:c"
        return activities.runningAppProcesses.orEmpty().any { process ->
            process.processName.startsWith(tab) && (startedAt(process.pid) ?: Long.MAX_VALUE) < exit.timestamp
        }
    }

    /** When process `pid` started (ms since the epoch), from `/proc/<pid>/stat`; null if unreadable. */
    private fun startedAt(pid: Int): Long? = try {
        val stat = File("/proc/$pid/stat").readText()
        // After the name in parentheses: field 3 on; the start time is field 22, in clock ticks since boot.
        val ticks = stat.substringAfterLast(')').trim().split(' ')[19].toLong()
        val hz = Os.sysconf(OsConstants._SC_CLK_TCK).coerceAtLeast(1L)
        val boot = System.currentTimeMillis() - SystemClock.elapsedRealtime()
        boot + ticks * 1000 / hz
    } catch (_: Exception) {
        null
    }

    /** Adds report `name` to the list of those a notice was shown for. */
    private fun markSeen(dir: File, name: String) {
        try {
            File(dir, ".seen").appendText("\n$name")
        } catch (e: IOException) {
            Log.w(TAG, "Couldn't mark $name seen", e)
        }
    }

    /** The report's hint when only the window was closed ([windowEndedAlone]). */
    private fun windowHint(): String =
        "${app.name}'s tabs kept running: Android closed only its window in the background, which opens again " +
            "when you come back. Nothing was lost."

    private fun summary(exit: ApplicationExitInfo, windowAlone: Boolean = false): String {
        val description = exit.description
        val detail = if (!description.isNullOrEmpty()) " ($description)" else ""
        val turu = if (windowAlone) "${app.name}'s window" else app.name
        return when (exit.reason) {
            ApplicationExitInfo.REASON_LOW_MEMORY -> "Android closed $turu to free memory: the phone ran low"
            ApplicationExitInfo.REASON_CRASH_NATIVE ->
                if (exit.status == 6) {
                    "$turu crashed: native code aborted (signal 6)"
                } else {
                    "$turu crashed in native code (signal ${exit.status})"
                }
            ApplicationExitInfo.REASON_CRASH -> "$turu crashed: a Java exception$detail"
            ApplicationExitInfo.REASON_ANR -> "$turu stopped responding and Android closed it"
            ApplicationExitInfo.REASON_SIGNALED -> "The system killed $turu (signal ${exit.status})$detail"
            ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE ->
                "Android closed $turu for using too much CPU in the background$detail"
            ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "$turu failed to start$detail"
            ApplicationExitInfo.REASON_PERMISSION_CHANGE -> "Android closed $turu after a permission changed"
            ApplicationExitInfo.REASON_DEPENDENCY_DIED -> "Android closed $turu after a process it depends on died"
            ApplicationExitInfo.REASON_FREEZER -> "Android closed $turu while it was frozen in the background"
            ApplicationExitInfo.REASON_EXIT_SELF -> "$turu exited with status ${exit.status}"
            else -> "The system closed $turu$detail"
        }
    }

    /** What the person can do about it, when there is something. */
    private fun hint(exit: ApplicationExitInfo): String? = when (exit.reason) {
        ApplicationExitInfo.REASON_LOW_MEMORY -> app.memoryHint.takeIf { it.isNotEmpty() }
        ApplicationExitInfo.REASON_SIGNALED,
        ApplicationExitInfo.REASON_OTHER,
        ApplicationExitInfo.REASON_UNKNOWN,
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE,
        ApplicationExitInfo.REASON_FREEZER,
        ->
            if (colorOs()) {
                // EXCESSIVE_RESOURCE_USAGE "o-kill:6" on an OPPO A9 2020 running as a foreground
                // service: ColorOS's own background manager (Android's CPU check spares those).
                "If nobody stopped ${app.name}: ColorOS (OPPO, realme, OnePlus) closes apps in the background on " +
                    "its own, even with battery optimisation off. In Settings > Battery > App battery " +
                    "management > ${app.name}, allow background activity and auto launch; also lock ${app.name} in the " +
                    "recent apps (hold its card, then the lock)."
            } else {
                "If nobody stopped ${app.name}: some phones' battery savers (Xiaomi's MIUI and HyperOS, Huawei, " +
                    "Samsung's sleeping apps, ColorOS on OPPO, realme and OnePlus) close apps in the " +
                    "background. Allow ${app.name} to start on its own (Autostart) and set its battery saver to " +
                    "No restrictions."
            }
        else -> null
    }

    /** OPPO, realme and OnePlus phones run ColorOS (OnePlus since Android 12's OxygenOS 12). */
    private fun colorOs(): Boolean =
        Build.MANUFACTURER.lowercase(Locale.US) in setOf("oppo", "realme", "oneplus")

    private fun appendDevice(context: Context, activities: ActivityManager, report: StringBuilder) {
        report.append("Phone: ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append(" (")
            .append(Build.DEVICE).append("), Android ").append(Build.VERSION.RELEASE).append(" (API ")
            .append(Build.VERSION.SDK_INT).append("), build ").append(Build.DISPLAY).append('\n')
        val memory = ActivityManager.MemoryInfo()
        activities.getMemoryInfo(memory)
        report.append("Phone memory: ").append(memory.totalMem / (1024 * 1024)).append(" MB, ")
            .append(memory.availMem / (1024 * 1024)).append(" MB free now, memory class ")
            .append(activities.memoryClass).append(" MB (large ").append(activities.largeMemoryClass)
            .append(" MB)").append(if (activities.isLowRamDevice) ", a low-RAM phone" else "").append('\n')
        try {
            val info = context.packageManager.getPackageInfo(context.packageName, 0)
            report.append(app.name).append(": ").append(info.versionName).append(" (").append(info.longVersionCode).append(")\n")
        } catch (_: Exception) {
            // The report is still useful without it.
        }
        val power = context.getSystemService(PowerManager::class.java)
        report.append("Battery optimisation: ")
            .append(if (power.isIgnoringBatteryOptimizations(context.packageName)) "off for ${app.name}" else "on")
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
