package com.muktiarafi.turu

import android.Manifest
import android.app.NativeActivity
import android.content.Intent
import android.content.pm.PackageManager
import android.hardware.display.DisplayManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.window.OnBackInvokedDispatcher

/**
 * NativeActivity, which runs the Rust side (libturu_android.so), plus the foreground service
 * that keeps the process and its network while the app is in the background. Full screen: the
 * status and navigation bars would sit over Turu's nav bar (apps draw under them since Android
 * 15); a swipe from the edge shows them for a moment. Asks once for what running in the
 * background needs: showing the service's notification, and leaving Turu out of battery
 * optimisation (or Doze cuts its network while the screen is off).
 */
class TuruActivity : NativeActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Before super.onCreate starts the Rust side, which shows a notice for new reports.
        CrashUploads.start(applicationContext)
        CrashReports.collect(applicationContext)
        super.onCreate(savedInstanceState)
        // NativeActivity's onCreate sets the window to "unspecified, resize", over the manifest's
        // setting. Unspecified shows the keyboard whenever the window comes to the front (start,
        // back from the background), as KeyInput holds the focus as a text editor; always hidden
        // leaves it to a game field taking the focus (`ui.rs`). Nothing: the window stays as it is
        // under the keyboard (Turu shows the field above it).
        window.setSoftInputMode(
            WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_HIDDEN or
                WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING,
        )
        TuruService.start(this)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT) {
                moveTaskToBack(true)
            }
        }
        // The display's refresh rate, for frame interpolation's pacing (`ui.rs`): now and whenever
        // the display changes mode (phones with adaptive rates switch between 60 and 120 Hz).
        sendRefreshRate()
        displays().registerDisplayListener(displayListener, null)
        // The keyboard's height, for the strip above it that shows what the game's text field holds.
        window.decorView.setOnApplyWindowInsetsListener { view, insets ->
            nativeKeyboardHeight(keyboardHeight(insets))
            // Held sideways, a punch-hole camera sits at the left or right edge, where the strip's
            // text would be under it.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                val cutout = insets.displayCutout
                nativeCutout(cutout?.safeInsetLeft ?: 0, cutout?.safeInsetRight ?: 0)
            }
            view.onApplyWindowInsets(insets)
        }
        // What the soft keyboard types into (KeyInput: a tap on delete works).
        val keys = KeyInput(this)
        addContentView(keys, ViewGroup.LayoutParams(1, 1))
        keys.requestFocus()
        askOnce()
    }

    private fun askOnce() {
        val asked = getSharedPreferences("asked", MODE_PRIVATE)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED &&
            !asked.getBoolean("notifications", false)
        ) {
            asked.edit().putBoolean("notifications", true).apply()
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
            return // the battery question next time: one dialog at a time
        }
        val power = getSystemService(PowerManager::class.java)
        if (!power.isIgnoringBatteryOptimizations(packageName) && !asked.getBoolean("battery", false)) {
            asked.edit().putBoolean("battery", true).apply()
            startActivity(
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")),
            )
        }
    }

    override fun onDestroy() {
        displays().unregisterDisplayListener(displayListener)
        super.onDestroy()
    }

    private val displayListener = object : DisplayManager.DisplayListener {
        override fun onDisplayAdded(displayId: Int) {}

        override fun onDisplayRemoved(displayId: Int) {}

        override fun onDisplayChanged(displayId: Int) = sendRefreshRate()
    }

    private fun displays() = getSystemService(DISPLAY_SERVICE) as DisplayManager

    private fun sendRefreshRate() {
        val current = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            this.display
        } else {
            @Suppress("DEPRECATION")
            windowManager.defaultDisplay
        }
        current?.let { nativeRefreshRate(it.refreshRate) }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<String>, results: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, results)
        TuruService.refresh()
        askOnce()
    }

    /**
     * Back goes to the background like Home, instead of closing the activity: the game and a
     * script keep running either way, but this keeps the screen as it was.
     */
    @Deprecated("Android 13+ calls the back callback set in onCreate instead")
    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        moveTaskToBack(true)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) {
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.insetsController?.let { insets ->
                insets.hide(WindowInsets.Type.systemBars())
                insets.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = (
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or
                    View.SYSTEM_UI_FLAG_FULLSCREEN or
                    View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
                    View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                    View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                )
        }
    }

    companion object {
        /** The soft keyboard's height in window pixels, 0 when it's down (`ui.rs`). */
        @JvmStatic
        private external fun nativeKeyboardHeight(pixels: Int)

        /**
         * How far the display cutout (a camera's notch or punch hole) reaches in from the window's
         * left and right edges, in window pixels; 0 without one (`ui.rs`).
         */
        @JvmStatic
        private external fun nativeCutout(left: Int, right: Int)

        /** The display's refresh rate in Hz, for frame interpolation's pacing (`ui.rs`). */
        @JvmStatic
        private external fun nativeRefreshRate(hz: Float)

        private fun keyboardHeight(insets: WindowInsets): Int {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                return if (insets.isVisible(WindowInsets.Type.ime())) insets.getInsets(WindowInsets.Type.ime()).bottom else 0
            }
            // Full screen with the navigation bar hidden: what's left at the bottom is the keyboard.
            @Suppress("DEPRECATION")
            return insets.systemWindowInsetBottom
        }
    }
}
