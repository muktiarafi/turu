package com.muktiarafi.turu;

import android.Manifest;
import android.app.NativeActivity;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.PowerManager;
import android.provider.Settings;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.window.OnBackInvokedDispatcher;

/**
 * NativeActivity, which runs the Rust side (libturu_android.so), plus the foreground service
 * that keeps the process and its network while the app is in the background. Full screen: the
 * status and navigation bars would sit over Turu's nav bar (apps draw under them since Android
 * 15); a swipe from the edge shows them for a moment. Asks once for what running in the
 * background needs: showing the service's notification, and leaving Turu out of battery
 * optimisation (or Doze cuts its network while the screen is off).
 */
public class TuruActivity extends NativeActivity {
    /** The soft keyboard's height in window pixels, 0 when it's down (`ui.rs`). */
    private static native void nativeKeyboardHeight(int pixels);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Before super.onCreate starts the Rust side, which shows a notice for new reports.
        CrashReports.collect(getApplicationContext());
        super.onCreate(savedInstanceState);
        TuruService.start(this);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> moveTaskToBack(true));
        }
        // The window stays as it is under the keyboard (adjustNothing); Turu shows what the game's
        // text field holds above it instead.
        getWindow().getDecorView().setOnApplyWindowInsetsListener((view, insets) -> {
            nativeKeyboardHeight(keyboardHeight(insets));
            return view.onApplyWindowInsets(insets);
        });
        // What the soft keyboard types into (KeyInput: a tap on delete works).
        KeyInput keys = new KeyInput(this);
        addContentView(keys, new ViewGroup.LayoutParams(1, 1));
        keys.requestFocus();
        askOnce();
    }

    @SuppressWarnings("deprecation")
    private static int keyboardHeight(WindowInsets insets) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            return insets.isVisible(WindowInsets.Type.ime()) ? insets.getInsets(WindowInsets.Type.ime()).bottom : 0;
        }
        // Full screen with the navigation bar hidden: what's left at the bottom is the keyboard.
        return insets.getSystemWindowInsetBottom();
    }

    private void askOnce() {
        SharedPreferences asked = getSharedPreferences("asked", MODE_PRIVATE);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
                && !asked.getBoolean("notifications", false)) {
            asked.edit().putBoolean("notifications", true).apply();
            requestPermissions(new String[] {Manifest.permission.POST_NOTIFICATIONS}, 1);
            return; // the battery question next time: one dialog at a time
        }
        PowerManager power = getSystemService(PowerManager.class);
        if (!power.isIgnoringBatteryOptimizations(getPackageName()) && !asked.getBoolean("battery", false)) {
            asked.edit().putBoolean("battery", true).apply();
            startActivity(new Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                    Uri.parse("package:" + getPackageName())));
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(requestCode, permissions, results);
        TuruService.refresh();
        askOnce();
    }

    /**
     * Back goes to the background like Home, instead of closing the activity: the game and a
     * script keep running either way, but this keeps the screen as it was.
     */
    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        moveTaskToBack(true);
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (!hasFocus) {
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController insets = getWindow().getInsetsController();
            if (insets != null) {
                insets.hide(WindowInsets.Type.systemBars());
                insets.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
    }
}
