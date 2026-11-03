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
import android.os.IBinder;

/**
 * Keeps Turu running in the background: without a foreground service Android cuts a background
 * app's network after a few seconds, which drops the game. It only posts its notification; the
 * game and the bot run on the Rust side's engine thread. Wake lock, battery settings and the
 * notification's actions are docs/ANDROID_PROPOSAL.md's phase 4.
 */
public class TuruService extends Service {
    private static final String CHANNEL = "turu";
    private static final int NOTIFICATION = 1;

    static void start(Context context) {
        context.startForegroundService(new Intent(context, TuruService.class));
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager manager = getSystemService(NotificationManager.class);
        manager.createNotificationChannel(
                new NotificationChannel(CHANNEL, "Turu running", NotificationManager.IMPORTANCE_LOW));
        PendingIntent open = PendingIntent.getActivity(
                this, 0, new Intent(this, TuruActivity.class), PendingIntent.FLAG_IMMUTABLE);
        Notification notification = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_media_play)
                .setContentTitle("Turu")
                .setContentText("The game and the bot are running")
                .setContentIntent(open)
                .setOngoing(true)
                .build();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        } else {
            startForeground(NOTIFICATION, notification);
        }
        // Not restarted on its own: the engine starts with the activity.
        return START_NOT_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
