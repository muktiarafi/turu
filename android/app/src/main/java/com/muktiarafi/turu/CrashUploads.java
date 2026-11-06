package com.muktiarafi.turu;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import org.json.JSONObject;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

import io.sentry.Attachment;
import io.sentry.Breadcrumb;
import io.sentry.Sentry;
import io.sentry.SentryLevel;
import io.sentry.android.core.SentryAndroid;

/**
 * Crash reports sent to Turu's developers through Sentry, once the user said yes (the question
 * Turu asks once, and Options > Application's switch: {@code sendCrashReports} in
 * {@code settings.json}). Sentry catches Java and native crashes as they happen (its NDK part,
 * so a phone that keeps tombstones to itself still reports), reads tombstones and ANRs from
 * Android's exit records, and sends them at the next start. A Rust panic's message goes with
 * the native crash ({@link #panicked}); exits Sentry doesn't catch (low memory, a battery
 * saver's kill) go as {@link CrashReports}' reports ({@link #exit}).
 *
 * <p>Only builds made with a DSN have it ({@code SENTRY_DSN}, see build.gradle.kts): without one,
 * {@link #available} is false, nothing starts and the question isn't asked. Sentry sends no
 * personal data ({@code sendDefaultPii} off: no IP address, no account), no screenshots and no
 * logs.
 */
final class CrashUploads {
    private static final String TAG = "turu";
    private static final Handler main = new Handler(Looper.getMainLooper());

    private static Context context;
    private static boolean started;

    private CrashUploads() {}

    /** Whether this build can send crash reports. */
    static boolean available() {
        return !BuildConfig.SENTRY_DSN.isEmpty();
    }

    /** Starts Sentry if the user said yes. Once per process, before the engine starts. */
    static synchronized void start(Context appContext) {
        if (context != null) {
            return;
        }
        context = appContext.getApplicationContext();
        if (available() && consented(context)) {
            init();
        }
    }

    /** The user's answer, from Turu's settings (any thread). */
    static void setEnabled(boolean on) {
        main.post(() -> {
            synchronized (CrashUploads.class) {
                if (!available() || context == null) {
                    return;
                }
                if (on && !started) {
                    init();
                } else if (!on && started) {
                    Sentry.close();
                    started = false;
                    Log.i(TAG, "Crash reports: off");
                }
            }
        });
    }

    /**
     * A Rust panic, from the panic hook just before the abort: its message goes with the native
     * crash Sentry records next (the tombstone, when the phone has one, also holds it).
     */
    static void panicked(String message) {
        if (!started) {
            return;
        }
        try {
            Breadcrumb crumb = new Breadcrumb();
            crumb.setCategory("panic");
            crumb.setLevel(SentryLevel.FATAL);
            crumb.setMessage(message);
            Sentry.addBreadcrumb(crumb);
            Sentry.setExtra("panic", message);
            Sentry.setTag("panic", "yes");
        } catch (Throwable ignored) {
            // About to abort anyway.
        }
    }

    /** An exit Sentry doesn't catch itself (low memory, a kill), with Turu's report attached. */
    static void exit(String reason, String summary, File report) {
        if (!started) {
            return;
        }
        Sentry.captureMessage(summary, SentryLevel.WARNING, scope -> {
            scope.setTag("exit_reason", reason);
            scope.addAttachment(new Attachment(report.getAbsolutePath(), report.getName(), "text/plain"));
        });
    }

    private static void init() {
        SentryAndroid.init(context, options -> {
            options.setDsn(BuildConfig.SENTRY_DSN);
            options.setRelease("turu-android@" + BuildConfig.VERSION_NAME);
            options.setDist(String.valueOf(BuildConfig.VERSION_CODE));
            options.setEnvironment(BuildConfig.DEBUG ? "debug" : "release");
            options.setSendDefaultPii(false);
            options.setAttachScreenshot(false);
            options.setAttachViewHierarchy(false);
            // Native crashes: the NDK handler as they happen, merged with the tombstone where
            // Android hands it over (Android 12+), the raw tombstone attached.
            options.setEnableNdk(true);
            options.setTombstoneEnabled(true);
            options.setAttachRawTombstone(true);
        });
        started = true;
        Log.i(TAG, "Crash reports: on");
    }

    /** {@code sendCrashReports} in the data folder's settings.json (Turu's {@code AppSettings}). */
    private static boolean consented(Context context) {
        File files = context.getExternalFilesDir(null);
        File settings = new File(files != null ? files : context.getFilesDir(), "settings.json");
        try {
            String text = new String(Files.readAllBytes(settings.toPath()), StandardCharsets.UTF_8);
            JSONObject json = new JSONObject(text);
            return json.optBoolean("sendCrashReports", false);
        } catch (Exception e) {
            // No settings yet: not asked yet.
            return false;
        }
    }
}
