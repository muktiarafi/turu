package com.muktiarafi.turu

import android.Manifest
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.hardware.display.DisplayManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import android.view.MotionEvent
import android.view.WindowInsets
import android.view.WindowManager
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.androidgamesdk.GameActivity
import com.muktiarafi.turu.shared.CrashQuestion
import com.muktiarafi.turu.shared.CrashReports
import com.muktiarafi.turu.shared.CrashUploads
import com.muktiarafi.turu.shared.FieldStyle
import com.muktiarafi.turu.shared.FieldTarget
import com.muktiarafi.turu.shared.Foreground
import com.muktiarafi.turu.shared.PackageUpdate
import java.io.File
import kotlin.concurrent.thread

/**
 * AndroidX's GameActivity, which runs the Rust side (libturu_android.so) and draws it in a
 * SurfaceView of its view hierarchy, plus the foreground service
 * that keeps the process and its network while the app is in the background. Full screen: the
 * status and navigation bars would sit over Turu's nav bar (apps draw under them since Android
 * 15); a swipe from the edge shows them for a moment. Asks once for what running in the
 * background needs: showing the service's notification, and leaving Turu out of battery
 * optimisation (or Doze cuts its network while the screen is off).
 */
class TuruActivity : GameActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Before super.onCreate starts the Rust side, which shows a notice for new reports.
        CrashUploads.start(applicationContext, CRASH_APP)
        CrashReports.collect(applicationContext, CRASH_APP)
        super.onCreate(savedInstanceState)
        // Before it resumes: alerts wait while it's on screen, the install confirmation shows on it.
        Foreground.watch(this)
        // GameTextInput listens to the surface's keys and takes them into its text as well, which
        // came back as text after the key had typed already. Keys reach the Rust side as key
        // events; the text input brings only what a keyboard commits without keys (`lib.rs`).
        mSurfaceView?.setOnKeyListener(null)
        // The window's soft input mode, set again here (NativeActivity set "unspecified, resize" in
        // its onCreate, over the manifest's). Unspecified showed the keyboard whenever the window
        // came to the front, as a text editor held the focus; always hidden leaves it to a field
        // taking the focus. Nothing: the window stays as it is under the keyboard (Turu shows the
        // field above it).
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
        WindowCompat.setDecorFitsSystemWindows(window, false)
        started = true
        // Before the shell: it keeps a new tab's password with the Keystore.
        Storage.init(this)
        // Each tab is a process of its own (M6): the engines run in the slot services' processes,
        // and this activity is only their shell (SlotShell).
        val files = getExternalFilesDir(null) ?: filesDir
        val view = mSurfaceView
        if (view != null) {
            shell = SlotShell(this, view, resources.displayMetrics.density).also { it.start(files.absolutePath) }
            handleSlotIntent(intent)
        }
        // Back goes to the background like Home, instead of closing the activity: the game and a
        // script keep running either way, but this keeps the screen as it was. AndroidX's
        // dispatcher: Android 13+'s back callback (and its predictive back) and older Back alike.
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                moveTaskToBack(true)
            }
        })
        // The display's refresh rate, for frame interpolation's pacing (`ui.rs`): now and whenever
        // the display changes mode (phones with adaptive rates switch between 60 and 120 Hz).
        sendRefreshRate()
        displays().registerDisplayListener(displayListener, null)
        // The keyboard's height, for the strip above it that shows what the game's text field holds.
        window.decorView.setOnApplyWindowInsetsListener { view, insets ->
            val keyboard = keyboardHeight(insets)
            nativeKeyboardHeight(keyboard)
            // Held sideways, a punch-hole camera sits at the left or right edge, where the strip's
            // text would be under it.
            var (left, right) = 0 to 0
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                val cutout = insets.displayCutout
                left = cutout?.safeInsetLeft ?: 0
                right = cutout?.safeInsetRight ?: 0
                nativeCutout(left, right)
            }
            // With process tabs, the slots draw Turu's UI: they lay it out around these too.
            shell?.insets(keyboard, left, right)
            view.onApplyWindowInsets(insets)
        }
        askOnce()
    }

    /** The game's focused text field, above the keyboard (`game_field.rs`). */
    private val gameField = SurfaceField(this, object : FieldTarget {
        override fun edited(text: String, start: Int, end: Int) {
            shell?.fieldEdited(SlotShell.GAME, text, start, end)
        }

        override fun enter() {
            shell?.fieldEnter(SlotShell.GAME)
        }

        override fun closed() {
            shell?.fieldClosed(SlotShell.GAME)
        }
    })

    /** Turu's focused text box (egui's), above the keyboard (`ui_field.rs`). */
    private val uiField = SurfaceField(this, object : FieldTarget {
        override fun edited(text: String, start: Int, end: Int) {
            shell?.fieldEdited(SlotShell.UI, text, start, end)
        }

        override fun enter() {
            shell?.fieldEnter(SlotShell.UI)
        }

        override fun closed() {
            shell?.fieldClosed(SlotShell.UI)
        }
    })

    /**
     * Shows a [SurfaceField] for the game's focused text field: its `text`, `start`..`end` selected
     * (UTF-16 units), a password or not, `max` characters at most (0: no limit). Called on the
     * engine thread (`screen.rs`).
     */
    fun showGameField(text: String, start: Int, end: Int, password: Boolean, max: Int) {
        runOnUiThread { gameField.show(text, start, end, FieldStyle(password, max, "Type in the game")) }
    }

    /** The game changed its focused field: its [SurfaceField] shows it. Called on the engine thread. */
    fun updateGameField(text: String, start: Int, end: Int) {
        runOnUiThread { gameField.update(text, start, end) }
    }

    /** No game field has the focus now: its [SurfaceField] goes. Called on the engine thread. */
    fun hideGameField() {
        runOnUiThread { gameField.hide() }
    }

    /**
     * Shows a [SurfaceField] for Turu's focused text box: its `text`, `start`..`end` selected
     * (UTF-16 units), its `kind` (0 a line, 1 a password, 2 several lines, 3 a number) and `hint`.
     * Called on the engine thread (`ui_field.rs`).
     */
    fun showUiField(text: String, start: Int, end: Int, kind: Int, hint: String) {
        val style = FieldStyle(password = kind == 1, hint = hint, lines = kind == 2, number = kind == 3)
        runOnUiThread { uiField.show(text, start, end, style) }
    }

    /** Turu's focused text box changed: its [SurfaceField] shows it. Called on the engine thread. */
    fun updateUiField(text: String, start: Int, end: Int) {
        runOnUiThread { uiField.update(text, start, end) }
    }

    /** None of Turu's text boxes has the focus now: its [SurfaceField] goes. Called on the engine thread. */
    fun hideUiField() {
        runOnUiThread { uiField.hide() }
    }

    /**
     * Gives GameActivity's surface the focus again once a [SurfaceField] is gone: its text input is
     * what the keyboard types into for egui's own text boxes (`ui::soft_keyboard`).
     */
    fun focusSurface() {
        mSurfaceView?.requestFocus()
    }

    /**
     * A finger down outside a [SurfaceField] that's up closes it, as Back does; the touch still
     * goes on to what it landed on (the game, or Turu's windows). With process tabs, a gesture that
     * starts on the field is the field's (the views'); every other goes to the tab on screen.
     */
    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        shell?.let {
            if (event.actionMasked == MotionEvent.ACTION_DOWN) {
                // As without process tabs: a finger down outside the field above the keyboard closes it.
                for (field in arrayOf(gameField, uiField)) {
                    if (field.touchedOutside(event.rawX, event.rawY)) field.close()
                }
                // A tap on the field moves its cursor, a long press selects: the EditText's, not the
                // game's, which would take the focus off the game's field (or Turu's box) and close
                // the strip.
                touchOnField = arrayOf(gameField, uiField).any { field ->
                    field.touchedInside(event.rawX, event.rawY)
                }
            }
            val onField = touchOnField
            if (event.actionMasked == MotionEvent.ACTION_UP || event.actionMasked == MotionEvent.ACTION_CANCEL) {
                touchOnField = false
            }
            if (onField) return super.dispatchTouchEvent(event)
            it.touch(event)
            return true
        }
        if (event.actionMasked == MotionEvent.ACTION_DOWN) {
            for (field in arrayOf(gameField, uiField)) {
                if (field.touchedOutside(event.rawX, event.rawY)) field.close()
            }
        }
        return super.dispatchTouchEvent(event)
    }

    /**
     * Process tabs: the shell's Rust side's actions from its own thread (keys for the slot on screen,
     * `slot_shell.rs`'s `to_shown`), carried out on the main thread as the slots' messages are.
     */
    fun shellActions(actions: String) {
        runOnUiThread { shell?.actions(actions) }
    }

    /** Opens `url` in the browser (or the app for the link). Called on the engine thread (`platform.rs`). */
    fun openUrl(url: String) {
        runOnUiThread {
            try {
                startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
            } catch (e: ActivityNotFoundException) {
                Log.w("turu", "Nothing opens $url: $e")
            }
        }
    }

    /** Android's share sheet (mail, a chat, Drive) with `text`. Called on the engine thread. */
    fun shareText(subject: String, text: String) {
        runOnUiThread {
            val send = Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_SUBJECT, subject)
                .putExtra(Intent.EXTRA_TEXT, text)
            try {
                startActivity(Intent.createChooser(send, null))
            } catch (e: ActivityNotFoundException) {
                Log.w("turu", "Nothing shares text: $e")
            }
        }
    }

    /**
     * Puts `text` on the clipboard (egui's Copy, the game's Ctrl+C), on the UI thread as Turu Mas's
     * `Bridge.copyText`. Called on the engine thread.
     */
    fun copyText(text: String) {
        runOnUiThread {
            getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Turu", text))
        }
    }

    /**
     * Installs an update's APK at `path` with Android's installer ([PackageUpdate]); the outcome
     * goes to [nativePackageInstalled]. Null when it started, else why not. Called on the update's
     * thread (`platform.rs`).
     */
    fun installPackage(path: String): String? =
        PackageUpdate.install(this, path) { status, message -> nativePackageInstalled(status, message) }

    /**
     * The clipboard's text, or null when it holds none (the game's Ctrl+V). Called on the engine
     * thread and answered there: the game waits for it, and `ClipboardManager` reads from any thread.
     */
    fun clipText(): String? {
        val clip = getSystemService(ClipboardManager::class.java).primaryClip ?: return null
        if (clip.itemCount < 1) return null
        return clip.getItemAt(0).coerceToText(this)?.toString()
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

    /** The shell over the tabs' processes (the slots). */
    private var shell: SlotShell? = null

    /** Process tabs: whether the gesture under way started on a [SurfaceField] ([dispatchTouchEvent]). */
    private var touchOnField = false

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleSlotIntent(intent)
    }

    /**
     * Process tabs' adb controls (`am start -n com.muktiarafi.turu/.TuruActivity ...`), as the tab
     * strip's: `--ei slot N` shows slot N's tab, `--ei openTab 1` opens one at the login screen,
     * `--ei closeTab N` closes slot N's.
     */
    private fun handleSlotIntent(intent: Intent?) {
        val shell = shell ?: return
        val extras = intent?.extras ?: return
        if (extras.containsKey("slot")) shell.asShown("""{"type":"select_tab","id":${extras.getInt("slot")}}""")
        if (extras.containsKey("openTab")) shell.asShown("""{"type":"open_tab"}""")
        if (extras.containsKey("closeTab")) shell.asShown("""{"type":"close_tab","id":${extras.getInt("closeTab")}}""")
    }

    override fun onResume() {
        super.onResume()
        // Process tabs: the slots post their alerts only while the app isn't on screen.
        shell?.onScreen(true)
    }

    override fun onPause() {
        shell?.onScreen(false)
        super.onPause()
    }

    override fun onDestroy() {
        shell?.stop()
        displays().unregisterDisplayListener(displayListener)
        super.onDestroy()
    }

    private val displayListener = object : DisplayManager.DisplayListener {
        override fun onDisplayAdded(displayId: Int) {}

        override fun onDisplayRemoved(displayId: Int) {}

        override fun onDisplayChanged(displayId: Int) = sendRefreshRate()
    }

    private fun displays() = getSystemService(DISPLAY_SERVICE) as DisplayManager

    private fun currentDisplay(): android.view.Display = ContextCompat.getDisplayOrDefault(this)

    private fun sendRefreshRate() {
        val hz = currentDisplay().refreshRate
        nativeRefreshRate(hz)
        // The slots draw the game: they pace frame interpolation by it.
        shell?.refreshRate(hz)
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
            val display = currentDisplay()
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
     * A slot's "open file" dialog (`saf.rs`, through [SlotShell]): the system picker for `mimeTypes`
     * (one per line). The chosen document is copied into `folder` (the cache when empty) and its
     * copy's path goes back to the slot ([documentPicked]).
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
     * A slot's "save file" dialog (`saf.rs`, through [SlotShell]): the system picker making a
     * document named `fileName`; the slot gets it and a file for the window to write
     * ([documentCreated]).
     */
    fun createDocument(mimeType: String, fileName: String) {
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(mimeType)
            .putExtra(Intent.EXTRA_TITLE, fileName)
        runOnUiThread { startPicker(intent, CREATE_DOCUMENT) }
    }

    /** The pickers' launchers (the Activity Result API): each answers [answer] with its document. */
    private val openPicker = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
        answer(OPEN_DOCUMENT, if (it.resultCode == Activity.RESULT_OK) it.data?.data else null)
    }
    private val createPicker = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
        answer(CREATE_DOCUMENT, if (it.resultCode == Activity.RESULT_OK) it.data?.data else null)
    }

    private fun startPicker(intent: Intent, request: Int) {
        try {
            (if (request == OPEN_DOCUMENT) openPicker else createPicker).launch(intent)
        } catch (e: ActivityNotFoundException) {
            Log.w("turu", "No document picker on this phone: $e")
            answer(request, null)
        }
    }

    /** The answer when there's no document: cancelled, or no picker. */
    private fun answer(request: Int, uri: Uri?) {
        if (request == CREATE_DOCUMENT) {
            if (uri == null) {
                documentCreated(null, null)
            } else {
                val path = try {
                    Storage.saveFile(uri).absolutePath
                } catch (e: Exception) {
                    Log.w("turu", "Couldn't make a file to save into: $e")
                    null
                }
                documentCreated(path?.let { uri.toString() }, path)
            }
            return
        }
        if (uri == null) {
            documentPicked(null)
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
            documentPicked(path)
        }
    }

    /** The "open" picker's answer, to the slot that asked. */
    private fun documentPicked(path: String?) {
        shell?.documentPicked(path)
    }

    /** The "save" picker's answer, as [documentPicked]. */
    private fun documentCreated(uri: String?, path: String?) {
        shell?.documentCreated(uri, path)
    }


    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<String>, results: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, results)
        askOnce()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) {
            return
        }
        WindowCompat.getInsetsController(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
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



        /** The soft keyboard's height in window pixels, 0 when it's down (`ui.rs`). */
        @JvmStatic
        private external fun nativeKeyboardHeight(pixels: Int)

        /**
         * How far the display cutout (a camera's notch or punch hole) reaches in from the window's
         * left and right edges, in window pixels; 0 without one (`ui.rs`).
         */
        @JvmStatic
        private external fun nativeCutout(left: Int, right: Int)

        /** How Android's installer ended [installPackage]'s install ([PackageUpdate]'s status). */
        @JvmStatic
        external fun nativePackageInstalled(status: Int, message: String?)







        /** The answer to "Send crash reports?": 1 yes, 0 no, -1 closed without one (`platform.rs`). */
        @JvmStatic
        private external fun nativeCrashUploadsAnswer(answer: Int)

        /** The display's refresh rate in Hz, for frame interpolation's pacing (`ui.rs`). */
        @JvmStatic
        private external fun nativeRefreshRate(hz: Float)

        private fun keyboardHeight(insets: WindowInsets): Int {
            val compat = WindowInsetsCompat.toWindowInsetsCompat(insets)
            val ime = WindowInsetsCompat.Type.ime()
            return if (compat.isVisible(ime)) compat.getInsets(ime).bottom else 0
        }
    }
}
