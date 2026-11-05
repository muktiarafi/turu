package com.muktiarafi.turu;

import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.PackageInfo;
import android.os.Build;
import android.os.PowerManager;
import android.util.Log;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;
import java.util.Locale;

/**
 * How the last runs ended, from Android's own record (ActivityManager's historical exit
 * reasons, Android 11 and later): a report in the data folder's {@code crash/} for each exit
 * Turu didn't ask for, so a crash, a low-memory kill or a battery saver's kill is known after
 * the fact without logcat. Native crashes come with their tombstone (Android 12 and later), read
 * by {@link Tombstone}, plus the raw file next to the report. The Rust side lists the reports
 * (Diagnostics > Logs > Crashes, {@code turu_frontend::crash_reports}) and shows a notice for new
 * ones. Runs once per process, before the engine starts.
 */
final class CrashReports {
    private static final String TAG = "turu";
    /** The most exits Android is asked for; it keeps a few per app. */
    private static final int MAX_EXITS = 16;
    /** An ANR's trace can be several MB; the start has the main thread. */
    private static final int MAX_TRACE_BYTES = 512 * 1024;

    private static boolean collected;

    private CrashReports() {}

    /** Writes reports for the exits since the last call (in an earlier process). */
    static synchronized void collect(Context context) {
        if (collected) {
            return;
        }
        collected = true;
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            return;
        }
        try {
            collectNow(context);
        } catch (Throwable e) {
            Log.w(TAG, "Couldn't read how the last runs ended", e);
        }
    }

    private static void collectNow(Context context) {
        File files = context.getExternalFilesDir(null);
        File dir = new File(files != null ? files : context.getFilesDir(), "crash");
        SharedPreferences prefs = context.getSharedPreferences("crash", Context.MODE_PRIVATE);
        long last = prefs.getLong("lastExit", 0);
        ActivityManager activities = context.getSystemService(ActivityManager.class);
        List<ApplicationExitInfo> exits =
                activities.getHistoricalProcessExitReasons(context.getPackageName(), 0, MAX_EXITS);
        long newest = last;
        // Newest first; the reports' times sort them anyway.
        for (ApplicationExitInfo exit : exits) {
            if (exit.getTimestamp() <= last) {
                continue;
            }
            newest = Math.max(newest, exit.getTimestamp());
            if (!unexpected(exit)) {
                continue;
            }
            try {
                write(context, activities, dir, exit);
            } catch (Throwable e) {
                Log.w(TAG, "Couldn't write the report of an exit", e);
            }
        }
        prefs.edit().putLong("lastExit", newest).apply();
    }

    /** Not a stop someone asked for (Force stop, an update, Turu's own clean exit). */
    private static boolean unexpected(ApplicationExitInfo exit) {
        switch (exit.getReason()) {
            case ApplicationExitInfo.REASON_USER_REQUESTED:
            case ApplicationExitInfo.REASON_USER_STOPPED:
            case ApplicationExitInfo.REASON_PACKAGE_UPDATED:
            case ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE:
                return false;
            case ApplicationExitInfo.REASON_EXIT_SELF:
                return exit.getStatus() != 0;
            default:
                return true;
        }
    }

    private static void write(Context context, ActivityManager activities, File dir, ApplicationExitInfo exit)
            throws IOException {
        if (!dir.isDirectory() && !dir.mkdirs()) {
            throw new IOException("Couldn't make " + dir);
        }
        long when = exit.getTimestamp();
        String name = "exit-" + new SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.US).format(new Date(when))
                + "-" + reasonName(exit.getReason()).toLowerCase(Locale.US);
        byte[] trace = readTrace(exit);
        boolean tombstone = trace != null && exit.getReason() == ApplicationExitInfo.REASON_CRASH_NATIVE;
        StringBuilder report = new StringBuilder();
        report.append(summary(exit)).append("\n\n");
        String hint = hint(exit);
        if (hint != null) {
            report.append(hint).append("\n\n");
        }
        report.append("When: ").append(new SimpleDateFormat("yyyy-MM-dd HH:mm:ss Z", Locale.US).format(new Date(when)))
                .append('\n');
        report.append("Reason: ").append(reasonName(exit.getReason())).append(" (").append(exit.getReason())
                .append("), status ").append(exit.getStatus()).append('\n');
        if (exit.getDescription() != null) {
            report.append("Description: ").append(exit.getDescription()).append('\n');
        }
        report.append("Was: ").append(importance(exit.getImportance())).append('\n');
        // Android often leaves PSS at 0.
        report.append("Memory at the end: RSS ").append(exit.getRss() / 1024).append(" MB");
        if (exit.getPss() > 0) {
            report.append(", PSS ").append(exit.getPss() / 1024).append(" MB");
        }
        report.append('\n');
        appendDevice(context, activities, report);
        if (tombstone) {
            String formatted = Tombstone.format(trace);
            File raw = new File(dir, name + "-tombstone.pb");
            writeBytes(raw, trace);
            report.append("\nTombstone (raw: ").append(raw.getName()).append(")\n");
            report.append(formatted != null ? formatted : "Couldn't read it; the raw file has it.\n");
        } else if (trace != null) {
            report.append("\nTrace\n").append(new String(trace, StandardCharsets.UTF_8));
            if (trace.length >= MAX_TRACE_BYTES) {
                report.append("\n... (cut)\n");
            }
        }
        File file = new File(dir, name + ".txt");
        try (Writer out = new OutputStreamWriter(new FileOutputStream(file), StandardCharsets.UTF_8)) {
            out.write(report.toString());
        }
        // The list sorts by this: the exit's time, not the report's.
        file.setLastModified(when);
        Log.w(TAG, "The last run ended: " + summary(exit) + " (" + file + ")");
    }

    private static String summary(ApplicationExitInfo exit) {
        String description = exit.getDescription();
        String detail = description != null && !description.isEmpty() ? " (" + description + ")" : "";
        switch (exit.getReason()) {
            case ApplicationExitInfo.REASON_LOW_MEMORY:
                return "Android closed Turu to free memory: the phone ran low";
            case ApplicationExitInfo.REASON_CRASH_NATIVE:
                return exit.getStatus() == 6
                        ? "Turu crashed: native code aborted (signal 6)"
                        : "Turu crashed in native code (signal " + exit.getStatus() + ")";
            case ApplicationExitInfo.REASON_CRASH:
                return "Turu crashed: a Java exception" + detail;
            case ApplicationExitInfo.REASON_ANR:
                return "Turu stopped responding and Android closed it";
            case ApplicationExitInfo.REASON_SIGNALED:
                return "The system killed Turu (signal " + exit.getStatus() + ")" + detail;
            case ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE:
                return "Android closed Turu for using too much CPU in the background" + detail;
            case ApplicationExitInfo.REASON_INITIALIZATION_FAILURE:
                return "Turu failed to start" + detail;
            case ApplicationExitInfo.REASON_PERMISSION_CHANGE:
                return "Android closed Turu after a permission changed";
            case ApplicationExitInfo.REASON_DEPENDENCY_DIED:
                return "Android closed Turu after a process it depends on died";
            case ApplicationExitInfo.REASON_FREEZER:
                return "Android closed Turu while it was frozen in the background";
            case ApplicationExitInfo.REASON_EXIT_SELF:
                return "Turu exited with status " + exit.getStatus();
            default:
                return "The system closed Turu" + detail;
        }
    }

    /** What the person can do about it, when there is something. */
    private static String hint(ApplicationExitInfo exit) {
        switch (exit.getReason()) {
            case ApplicationExitInfo.REASON_LOW_MEMORY:
                return "One game client uses about 400 MB. Closing other apps while Turu runs helps.";
            case ApplicationExitInfo.REASON_SIGNALED:
            case ApplicationExitInfo.REASON_OTHER:
            case ApplicationExitInfo.REASON_UNKNOWN:
            case ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE:
            case ApplicationExitInfo.REASON_FREEZER:
                return "If nobody stopped Turu: some phones' battery savers (Xiaomi's MIUI and HyperOS, Huawei, "
                        + "Samsung's sleeping apps) close apps in the background. Allow Turu to start on its own "
                        + "(Autostart) and set its battery saver to No restrictions.";
            default:
                return null;
        }
    }

    private static void appendDevice(Context context, ActivityManager activities, StringBuilder report) {
        report.append("Phone: ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append(" (")
                .append(Build.DEVICE).append("), Android ").append(Build.VERSION.RELEASE).append(" (API ")
                .append(Build.VERSION.SDK_INT).append("), build ").append(Build.DISPLAY).append('\n');
        ActivityManager.MemoryInfo memory = new ActivityManager.MemoryInfo();
        activities.getMemoryInfo(memory);
        report.append("Phone memory: ").append(memory.totalMem / (1024 * 1024)).append(" MB, ")
                .append(memory.availMem / (1024 * 1024)).append(" MB free now\n");
        try {
            PackageInfo info = context.getPackageManager().getPackageInfo(context.getPackageName(), 0);
            report.append("Turu: ").append(info.versionName).append(" (").append(info.getLongVersionCode())
                    .append(")\n");
        } catch (Exception ignored) {
            // The report is still useful without it.
        }
        PowerManager power = context.getSystemService(PowerManager.class);
        report.append("Battery optimisation: ")
                .append(power.isIgnoringBatteryOptimizations(context.getPackageName()) ? "off for Turu" : "on")
                .append(", background restricted: ").append(activities.isBackgroundRestricted() ? "yes" : "no")
                .append(" (now)\n");
    }

    private static byte[] readTrace(ApplicationExitInfo exit) {
        try (InputStream in = exit.getTraceInputStream()) {
            if (in == null) {
                return null;
            }
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buffer = new byte[16 * 1024];
            int limit = exit.getReason() == ApplicationExitInfo.REASON_CRASH_NATIVE ? Integer.MAX_VALUE : MAX_TRACE_BYTES;
            int n;
            while (out.size() < limit && (n = in.read(buffer)) > 0) {
                out.write(buffer, 0, Math.min(n, limit - out.size()));
            }
            return out.toByteArray();
        } catch (IOException e) {
            Log.w(TAG, "Couldn't read an exit's trace", e);
            return null;
        }
    }

    private static void writeBytes(File file, byte[] bytes) throws IOException {
        try (FileOutputStream out = new FileOutputStream(file)) {
            out.write(bytes);
        }
    }

    private static String importance(int importance) {
        switch (importance) {
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND:
                return "on screen";
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND_SERVICE:
                return "in the background with its notification (foreground service)";
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_VISIBLE:
                return "visible";
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_PERCEPTIBLE:
                return "perceptible";
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_SERVICE:
                return "a background service";
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_CACHED:
                return "cached in the background";
            case ActivityManager.RunningAppProcessInfo.IMPORTANCE_GONE:
                return "gone";
            default:
                return "importance " + importance;
        }
    }

    private static String reasonName(int reason) {
        switch (reason) {
            case ApplicationExitInfo.REASON_EXIT_SELF: return "EXIT_SELF";
            case ApplicationExitInfo.REASON_SIGNALED: return "SIGNALED";
            case ApplicationExitInfo.REASON_LOW_MEMORY: return "LOW_MEMORY";
            case ApplicationExitInfo.REASON_CRASH: return "CRASH";
            case ApplicationExitInfo.REASON_CRASH_NATIVE: return "CRASH_NATIVE";
            case ApplicationExitInfo.REASON_ANR: return "ANR";
            case ApplicationExitInfo.REASON_INITIALIZATION_FAILURE: return "INITIALIZATION_FAILURE";
            case ApplicationExitInfo.REASON_PERMISSION_CHANGE: return "PERMISSION_CHANGE";
            case ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE: return "EXCESSIVE_RESOURCE_USAGE";
            case ApplicationExitInfo.REASON_USER_REQUESTED: return "USER_REQUESTED";
            case ApplicationExitInfo.REASON_USER_STOPPED: return "USER_STOPPED";
            case ApplicationExitInfo.REASON_DEPENDENCY_DIED: return "DEPENDENCY_DIED";
            case ApplicationExitInfo.REASON_OTHER: return "OTHER";
            case ApplicationExitInfo.REASON_FREEZER: return "FREEZER";
            case ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE: return "PACKAGE_STATE_CHANGE";
            case ApplicationExitInfo.REASON_PACKAGE_UPDATED: return "PACKAGE_UPDATED";
            default: return "UNKNOWN";
        }
    }
}
