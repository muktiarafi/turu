package com.muktiarafi.turu

import android.Manifest
import android.app.Activity
import android.app.NativeActivity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.hardware.display.DisplayManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import android.window.OnBackInvokedDispatcher
import java.io.File
import kotlin.concurrent.thread

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
        // The window covers the whole screen, the display cutout included, whether the system bars
        // show or not. With the default cutout mode (Android 14 and older; 15+ with target SDK 35
        // makes it "always"), the window kept clear of a camera on a short edge only while the
        // bars were hidden: the keyboard or a swipe from an edge brought them up, the window and
        // the cutout's inset changed size, and the game was resized with them, flickering.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.setDecorFitsSystemWindows(false)
        }
        started = true
        TuruService.start(this)
        Storage.init(this)
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
        keyInput = keys
        addContentView(keys, ViewGroup.LayoutParams(1, 1))
        keys.requestFocus()
        askOnce()
    }

    private var keyInput: KeyInput? = null

    /** The game's focused text field, above the keyboard (`game_field.rs`). */
    private val gameField = NativeField(this, object : FieldTarget {
        override fun edited(text: String, start: Int, end: Int) = nativeGameFieldEdited(text, start, end)
        override fun enter() = nativeGameFieldEnter()
        override fun closed() = nativeGameFieldClosed()
    })

    /** Turu's focused text box (egui's), above the keyboard (`ui_field.rs`). */
    private val uiField = NativeField(this, object : FieldTarget {
        override fun edited(text: String, start: Int, end: Int) = nativeUiFieldEdited(text, start, end)
        override fun enter() = nativeUiFieldEnter()
        override fun closed() = nativeUiFieldClosed()
    })

    /**
     * Shows a [NativeField] for the game's focused text field: its `text`, `start`..`end` selected
     * (UTF-16 units), a password or not, `max` characters at most (0: no limit). Called on the
     * engine thread (`screen.rs`).
     */
    fun showGameField(text: String, start: Int, end: Int, password: Boolean, max: Int) {
        runOnUiThread { gameField.show(text, start, end, FieldStyle(password, max, "Type in the game")) }
    }

    /** The game changed its focused field: its [NativeField] shows it. Called on the engine thread. */
    fun updateGameField(text: String, start: Int, end: Int) {
        runOnUiThread { gameField.update(text, start, end) }
    }

    /** No game field has the focus now: its [NativeField] goes. Called on the engine thread. */
    fun hideGameField() {
        runOnUiThread { gameField.hide() }
    }

    /**
     * Shows a [NativeField] for Turu's focused text box: its `text`, `start`..`end` selected
     * (UTF-16 units), its `kind` (0 a line, 1 a password, 2 several lines, 3 a number) and `hint`.
     * Called on the engine thread (`ui_field.rs`).
     */
    fun showUiField(text: String, start: Int, end: Int, kind: Int, hint: String) {
        val style = FieldStyle(password = kind == 1, hint = hint, lines = kind == 2, number = kind == 3)
        runOnUiThread { uiField.show(text, start, end, style) }
    }

    /** Turu's focused text box changed: its [NativeField] shows it. Called on the engine thread. */
    fun updateUiField(text: String, start: Int, end: Int) {
        runOnUiThread { uiField.update(text, start, end) }
    }

    /** None of Turu's text boxes has the focus now: its [NativeField] goes. Called on the engine thread. */
    fun hideUiField() {
        runOnUiThread { uiField.hide() }
    }

    /**
     * Shows or hides the soft keyboard for [KeyInput] (`ui.rs`, `screen.rs`). Called on the engine
     * thread; the input method only takes it on the UI thread (Android 15 throws otherwise).
     */
    fun softKeyboard(show: Boolean) {
        runOnUiThread {
            val keys = keyInput ?: return@runOnUiThread
            val methods = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
            if (show) {
                keys.requestFocus()
                methods.showSoftInput(keys, 0)
            } else {
                methods.hideSoftInputFromWindow(keys.windowToken, 0)
            }
        }
    }

    /**
     * Asks "Send crash reports?" with Android's dialog ([CrashQuestion]); the answer goes to
     * [nativeCrashUploadsAnswer]. Called on the engine thread (`platform.rs`).
     */
    fun askCrashUploads(title: String, message: String, note: String, link: String, linkUrl: String, no: String, yes: String) {
        runOnUiThread {
            CrashQuestion.show(this, title, message, note, link, linkUrl, no, yes) { nativeCrashUploadsAnswer(it) }
        }
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

    private fun currentDisplay(): android.view.Display? = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        this.display
    } else {
        @Suppress("DEPRECATION")
        windowManager.defaultDisplay
    }

    private fun sendRefreshRate() {
        currentDisplay()?.let { nativeRefreshRate(it.refreshRate) }
    }

    /**
     * From the Rust side (`ui.rs`, any thread): the game runs at [hz] frames a second, faster than
     * 60 (Set FPS). Asks for the display mode with the slowest refresh rate that keeps up with it
     * (the fastest there is when none does), at the current resolution: a 120 Hz phone otherwise
     * often keeps an app at 60. 0 gives the choice back to the system. The display then calls
     * [displayListener], which passes the new rate on.
     */
    fun preferRefreshRate(hz: Float) {
        runOnUiThread {
            val display = currentDisplay() ?: return@runOnUiThread
            val now = display.mode
            val modes = display.supportedModes.filter {
                it.physicalWidth == now.physicalWidth && it.physicalHeight == now.physicalHeight
            }
            val mode = if (hz <= 0f) {
                null
            } else {
                modes.filter { it.refreshRate >= hz - 0.5f }.minByOrNull { it.refreshRate }
                    ?: modes.maxByOrNull { it.refreshRate }
            }
            val attributes = window.attributes
            val id = mode?.modeId ?: 0
            if (attributes.preferredDisplayModeId != id) {
                attributes.preferredDisplayModeId = id
                window.attributes = attributes
            }
            Log.i("turu", "Display mode for $hz FPS: ${mode?.refreshRate ?: "the system's"} Hz")
        }
    }

    /**
     * The bot windows' "open file" dialog (`saf.rs`): the system picker for `mimeTypes` (one per
     * line). The chosen document is copied into `folder` (the cache when empty) and its copy's
     * path goes back through [nativeDocumentPicked]. Called on the engine thread.
     */
    fun openDocument(mimeTypes: String, folder: String) {
        val types = mimeTypes.lines().filter { it.isNotBlank() }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(types.singleOrNull() ?: "*/*")
        if (types.size > 1) {
            intent.putExtra(Intent.EXTRA_MIME_TYPES, types.toTypedArray())
        }
        pickFolder = folder
        runOnUiThread { startPicker(intent, OPEN_DOCUMENT) }
    }

    /**
     * The bot windows' "save file" dialog (`saf.rs`): the system picker making a document named
     * `fileName`; [nativeDocumentCreated] gets it and a file for the window to write.
     */
    fun createDocument(mimeType: String, fileName: String) {
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(mimeType)
            .putExtra(Intent.EXTRA_TITLE, fileName)
        runOnUiThread { startPicker(intent, CREATE_DOCUMENT) }
    }

    private fun startPicker(intent: Intent, request: Int) {
        try {
            @Suppress("DEPRECATION")
            startActivityForResult(intent, request)
        } catch (e: ActivityNotFoundException) {
            Log.w("turu", "No document picker on this phone: $e")
            answer(request, null)
        }
    }

    /** The answer when there's no document: cancelled, or no picker. */
    private fun answer(request: Int, uri: Uri?) {
        if (request == CREATE_DOCUMENT) {
            if (uri == null) {
                nativeDocumentCreated(null, null)
            } else {
                val path = try {
                    Storage.saveFile(uri).absolutePath
                } catch (e: Exception) {
                    Log.w("turu", "Couldn't make a file to save into: $e")
                    null
                }
                nativeDocumentCreated(path?.let { uri.toString() }, path)
            }
            return
        }
        if (uri == null) {
            nativeDocumentPicked(null)
            return
        }
        val folder = pickFolder
        // Off the main thread: a document can be large, or on the network (Drive).
        thread(name = "Turu Pick") {
            val path = try {
                Storage.copyDocument(uri, if (folder.isEmpty()) Storage.pickedFolder() else File(folder)).absolutePath
            } catch (e: Exception) {
                Log.w("turu", "Couldn't copy the chosen document: $e")
                null
            }
            nativeDocumentPicked(path)
        }
    }

    @Deprecated("The pickers use startActivityForResult, which NativeActivity has")
    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == OPEN_DOCUMENT || requestCode == CREATE_DOCUMENT) {
            answer(requestCode, if (resultCode == Activity.RESULT_OK) data?.data else null)
        }
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

    /** Where the "open" picker's document is copied (`openDocument`). */
    @Volatile
    private var pickFolder = ""

    companion object {
        private const val OPEN_DOCUMENT = 41
        private const val CREATE_DOCUMENT = 42

        /**
         * Whether an activity started in this process: the service started without one is a
         * restart after Android killed the process (`restart.rs`).
         */
        @Volatile
        @JvmStatic
        var started = false
            private set

        /** The "open" picker's answer: the copy's path, null when cancelled (`saf.rs`). */
        @JvmStatic
        private external fun nativeDocumentPicked(path: String?)

        /** The "save" picker's answer: the document and the file to write, nulls when cancelled. */
        @JvmStatic
        private external fun nativeDocumentCreated(uri: String?, path: String?)

        /** The soft keyboard's height in window pixels, 0 when it's down (`ui.rs`). */
        @JvmStatic
        private external fun nativeKeyboardHeight(pixels: Int)

        /**
         * How far the display cutout (a camera's notch or punch hole) reaches in from the window's
         * left and right edges, in window pixels; 0 without one (`ui.rs`).
         */
        @JvmStatic
        private external fun nativeCutout(left: Int, right: Int)

        /** The game's [NativeField] changed its text or selection: the game's field takes it (`game_field.rs`). */
        @JvmStatic
        external fun nativeGameFieldEdited(text: String, start: Int, end: Int)

        /** The keyboard's Send (or Enter) in the game's [NativeField]: Enter in the game. */
        @JvmStatic
        external fun nativeGameFieldEnter()

        /** The game's [NativeField] was closed (Back): the game's field loses the focus. */
        @JvmStatic
        external fun nativeGameFieldClosed()

        /** The UI's [NativeField] changed its text or selection: Turu's text box takes it (`ui_field.rs`). */
        @JvmStatic
        external fun nativeUiFieldEdited(text: String, start: Int, end: Int)

        /** The keyboard's Send (or Enter) in the UI's [NativeField]: Enter in Turu's text box. */
        @JvmStatic
        external fun nativeUiFieldEnter()

        /** The UI's [NativeField] was closed (Back): Turu's text box loses the focus. */
        @JvmStatic
        external fun nativeUiFieldClosed()

        /** The answer to "Send crash reports?": 1 yes, 0 no, -1 closed without one (`platform.rs`). */
        @JvmStatic
        private external fun nativeCrashUploadsAnswer(answer: Int)

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
