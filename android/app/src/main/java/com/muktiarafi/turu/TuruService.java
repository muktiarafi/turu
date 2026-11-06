package com.muktiarafi.turu;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;

/**
 * Keeps Turu running in the background: without a foreground service Android cuts a background
 * app's network after a few seconds, which drops the game. It starts the engine (the Rust side,
 * also when Android restarts the service after killing the process, with no activity), shows
 * the script's state with Stop and Pause, and holds a wake lock while a script runs.
 */
public class TuruService extends Service {
    private static final String CHANNEL = "turu";
    private static final int NOTIFICATION = 1;
    static final String ACTION_STOP = "com.muktiarafi.turu.STOP";
    static final String ACTION_PAUSE = "com.muktiarafi.turu.PAUSE";

    private static TuruService instance;
    private static final Handler main = new Handler(Looper.getMainLooper());

    private PowerManager.WakeLock wakeLock;
    private String title = "Turu";
    private String text = "Starting";
    private boolean running;
    private boolean paused;

    static {
        System.loadLibrary("turu_android");
    }

    private static native void nativeStart(Context context, String filesDir);

    private static native void nativeAction(String action);

    private static native void nativeTrimMemory(int level);

    /** For the Rust side, which can only reach this class: see {@link CrashUploads}. */
    static boolean crashUploadsAvailable() {
        return CrashUploads.available();
    }

    static void setCrashUploads(boolean on) {
        CrashUploads.setEnabled(on);
    }

    static void panicked(String message) {
        CrashUploads.panicked(message);
    }

    static void start(Context context) {
        context.startForegroundService(new Intent(context, TuruService.class));
    }

    /** Called by the Rust side (any thread) when the bot's state changes. */
    static void status(String title, String text, boolean running, boolean paused) {
        main.post(() -> {
            if (instance != null) {
                instance.show(title, text, running, paused);
            }
        });
    }

    /** Posts the notification again: it's dropped while the permission isn't granted yet. */
    static void refresh() {
        main.post(() -> {
            if (instance != null) {
                instance.show(instance.title, instance.text, instance.running, instance.paused);
            }
        });
    }

    @Override
    public void onCreate() {
        super.onCreate();
        instance = this;
        PowerManager power = getSystemService(PowerManager.class);
        wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "turu:script");
        wakeLock.setReferenceCounted(false);
        getSystemService(NotificationManager.class).createNotificationChannel(
                new NotificationChannel(CHANNEL, "Turu running", NotificationManager.IMPORTANCE_LOW));
        startForegroundWith(build());
        CrashUploads.start(getApplicationContext());
        CrashReports.collect(getApplicationContext());
        java.io.File files = getExternalFilesDir(null);
        nativeStart(getApplicationContext(), (files != null ? files : getFilesDir()).getAbsolutePath());
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent != null ? intent.getAction() : null;
        if (ACTION_STOP.equals(action)) {
            nativeAction("stop");
        } else if (ACTION_PAUSE.equals(action)) {
            nativeAction("pause");
        }
        // Started again after Android killed the process: the engine logs back in and runs
        // turu-args.txt's script.
        return START_STICKY;
    }

    @Override
    public void onDestroy() {
        instance = null;
        if (wakeLock.isHeld()) {
            wakeLock.release();
        }
        super.onDestroy();
    }

    /** The phone is low on memory, or the UI went away: Turu frees what it can spare (`memory.rs`). */
    @Override
    public void onTrimMemory(int level) {
        super.onTrimMemory(level);
        if (level >= TRIM_MEMORY_RUNNING_LOW) {
            nativeTrimMemory(level);
        }
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    private void show(String title, String text, boolean running, boolean paused) {
        this.title = title;
        this.text = text;
        this.running = running;
        this.paused = paused;
        // The CPU stays awake while a script runs (the screen may go off).
        if (running && !wakeLock.isHeld()) {
            wakeLock.acquire();
        } else if (!running && wakeLock.isHeld()) {
            wakeLock.release();
        }
        getSystemService(NotificationManager.class).notify(NOTIFICATION, build());
    }

    private void startForegroundWith(Notification notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        } else {
            startForeground(NOTIFICATION, notification);
        }
    }

    private PendingIntent action(String action, int code) {
        Intent intent = new Intent(this, TuruService.class).setAction(action);
        return PendingIntent.getService(this, code, intent, PendingIntent.FLAG_IMMUTABLE);
    }

    private Notification build() {
        PendingIntent open = PendingIntent.getActivity(
                this, 0, new Intent(this, TuruActivity.class), PendingIntent.FLAG_IMMUTABLE);
        Notification.Builder builder = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(title)
                .setContentText(text)
                .setContentIntent(open)
                .setOngoing(true)
                .setOnlyAlertOnce(true);
        if (running) {
            builder.addAction(new Notification.Action.Builder(
                    null, paused ? "Resume" : "Pause", action(ACTION_PAUSE, 1)).build());
            builder.addAction(new Notification.Action.Builder(null, "Stop", action(ACTION_STOP, 2)).build());
        }
        return builder.build();
    }
}
