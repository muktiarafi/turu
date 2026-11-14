package com.muktiarafi.turu.air

import android.app.Activity
import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import com.adobe.fre.FREContext
import com.muktiarafi.turu.shared.CrashQuestion
import com.muktiarafi.turu.shared.CrashUploads
import com.muktiarafi.turu.shared.FieldStyle
import com.muktiarafi.turu.shared.FieldTarget
import com.muktiarafi.turu.shared.PackageUpdate
import org.json.JSONObject

/**
 * Turu's engine (`libturu_air.so`, crate `turu_air`) and the way back from it to AS3 and the
 * activity. The engine calls the game by asking [dispatch] (from its own threads) to send AIR a
 * status event; AS3 answers through the extension's `reply`, which lands in [nativeReply]. Turu's
 * windows are drawn into a [TuruOverlay] over AIR's view, which [gameInsets] moves right of the
 * rail.
 */
object Bridge {
    init {
        System.loadLibrary("turu_air")
    }

    /** The extension's context, which status events go through. */
    @Volatile
    var context: FREContext? = null

    @Volatile
    private var activity: Activity? = null

    /** AIR's view (moved by [gameInsets]) and Turu's over it. */
    private var airView: View? = null
    private var overlay: TuruOverlay? = null

    /** Starts the engine (once per process) with `filesDir` as its data folder. */
    @JvmStatic
    external fun nativeStart(context: Context, filesDir: String)

    /** Client `client`'s turu.swf called `ExternalInterface.call(name, ...)` (args as a JSON array; -1: the host SWF). */
    @JvmStatic
    external fun nativeFlashCall(name: String, args: String, client: Int)

    /** AS3's answer to call `id`, and how long the callback took. */
    @JvmStatic
    external fun nativeReply(id: Long, value: String, runMicros: Int)

    /** The SWF rewritten for AIR (target 0: the game's client, 2: another SWF); null if it can't run there. */
    @JvmStatic
    external fun nativePatch(swf: ByteArray, target: Int): ByteArray?

    /** Downloads a game file through Turu's disk cache; a status event "fetched" says when. */
    @JvmStatic
    external fun nativeFetch(id: Long, url: String)

    /** The bytes of download `id`, once; null if there are none. */
    @JvmStatic
    external fun nativeFetchResult(id: Long): ByteArray?

    /** The overlay's surface came or changed (null: it went). */
    @JvmStatic
    external fun nativeSurface(surface: android.view.Surface?, density: Float)

    /** A finger on the UI: 0 down, 1 move, 2 up, 3 cancelled; pixels. */
    @JvmStatic
    external fun nativeTouch(phase: Int, x: Float, y: Float)

    /** Whether a finger coming down there is the UI's (else AIR's). */
    @JvmStatic
    external fun nativeOnUi(x: Float, y: Float): Boolean

    /** The soft keyboard typed `text` into the UI's text box. */
    @JvmStatic
    external fun nativeText(text: String)

    /** A key for the UI's text box: 0 Backspace, 1 Enter, 2 Delete, 3 left, 4 right. */
    @JvmStatic
    external fun nativeKey(key: Int)

    /** The window's insets: the soft keyboard's height and the display cutout's sides, pixels. */
    @JvmStatic
    external fun nativeInsets(keyboard: Int, cutoutLeft: Int, cutoutRight: Int)

    /** The game's focused text field above the keyboard, and the activity it's in. */
    /** The game's focused text field above the keyboard (`gameTyping`). */
    private var gameField: ViewField? = null

    /** Turu's focused text box above the keyboard (`overlay.rs`). */
    private var uiField: ViewField? = null

    /** The game's focused field as the host last reported it: shown again when Turu's text box is done. */
    private var lastGameTyping = ""

    /** What the game's [ViewField] does with an edit: a status event "typing" to the host SWF. */
    private val gameTarget = object : FieldTarget {
        override fun edited(text: String, start: Int, end: Int) = send(
            JSONObject().put("op", "set").put("text", text).put("start", start).put("end", end),
        )

        override fun enter() = send(JSONObject().put("op", "enter"))
        override fun closed() = send(JSONObject().put("op", "close"))
        private fun send(request: JSONObject) = dispatch("typing", request.toString())
    }

    /** What Turu's [ViewField] does with an edit: the overlay's text box takes it (`overlay.rs`). */
    private val uiTarget = object : FieldTarget {
        override fun edited(text: String, start: Int, end: Int) = nativeUiFieldEdited(text, start, end)
        override fun enter() = nativeUiFieldEnter()
        override fun closed() {
            nativeUiFieldClosed()
            gameTyping(lastGameTyping)
        }
    }

    /** The field `current` if it's for `activity`, else a new one there (the old one gone). */
    private fun fieldFor(current: ViewField?, activity: Activity, target: FieldTarget): ViewField =
        current?.takeIf { it.activity === activity } ?: run {
            current?.hide()
            ViewField(activity, target)
        }

    /** Right of Turu's rail: where the fields go. */
    private fun fieldLeft(): Int = (airView?.layoutParams as? ViewGroup.MarginLayoutParams)?.leftMargin ?: 0

    /**
     * The game's focused text field, from the host SWF (`gameTyping`) as JSON (`text`, `caret`,
     * `selStart`, `selEnd`, `password`, `max`), or empty when no field you type in has the focus:
     * its [ViewField] shows it, follows it, or goes. Not while Turu's text box has a field.
     */
    @JvmStatic
    fun gameTyping(json: String) {
        val activity = activity ?: return
        activity.runOnUiThread {
            lastGameTyping = json
            val field = fieldFor(gameField, activity, gameTarget).also { gameField = it }
            val typing = json.takeIf { it.isNotEmpty() }?.let { runCatching { JSONObject(it) }.getOrNull() }
            if (typing == null || uiField?.shown == true) {
                field.hide()
                return@runOnUiThread
            }
            val text = typing.optString("text")
            val caret = typing.optInt("caret", text.length)
            val start = typing.optInt("selStart", caret)
            val end = typing.optInt("selEnd", caret)
            val style = FieldStyle(typing.optBoolean("password"), typing.optInt("max"), "Type in the game")
            if (field.shown && field.style == style) {
                field.update(text, start, end)
            } else {
                field.show(text, start, end, style, fieldLeft())
            }
        }
    }

    /**
     * Shows a [ViewField] for Turu's focused text box: its `text`, `start`..`end` selected
     * (UTF-16 units), its `kind` (0 a line, 1 a password, 2 several lines, 3 a number) and `hint`.
     * The game's field steps aside meanwhile. Called on the overlay's thread.
     */
    @JvmStatic
    fun showUiField(text: String, start: Int, end: Int, kind: Int, hint: String) {
        val activity = activity ?: return
        val style = FieldStyle(password = kind == 1, hint = hint, lines = kind == 2, number = kind == 3)
        activity.runOnUiThread {
            gameField?.hide()
            val field = fieldFor(uiField, activity, uiTarget).also { uiField = it }
            field.show(text, start, end, style, fieldLeft())
        }
    }

    /** Turu's focused text box changed: its [ViewField] shows it. Called on the overlay's thread. */
    @JvmStatic
    fun updateUiField(text: String, start: Int, end: Int) {
        activity?.runOnUiThread { uiField?.update(text, start, end) }
    }

    /** None of Turu's text boxes has the focus now: its field goes, and the game's comes back. */
    @JvmStatic
    fun hideUiField() {
        activity?.runOnUiThread {
            val field = uiField ?: return@runOnUiThread
            if (field.shown) {
                field.hide()
                gameTyping(lastGameTyping)
            }
        }
    }

    /** Where a native field is up, (top, left) in pixels from the overlay's corner ([nativeFieldPlace]). */
    private var fieldPlace: Pair<Int, Int>? = null

    /** A field above the keyboard moved, came or went: the overlay keeps clear of it. */
    fun fieldsMoved() {
        val view = overlay
        val place = view?.let { listOfNotNull(uiField, gameField).firstNotNullOfOrNull { field -> field.place(it) } }
        if (place != fieldPlace) {
            fieldPlace = place
            nativeFieldPlace(place?.first ?: -1, place?.second ?: -1)
        }
    }

    /**
     * The overlay draws nothing below `top` and right of `left` (pixels from its corner), where a
     * field above the keyboard is (it's a surface over every view); -1: nothing to keep clear of.
     */
    @JvmStatic
    external fun nativeFieldPlace(top: Int, left: Int)

    /**
     * A finger came down outside the fields above the keyboard (on Turu's windows or the game):
     * both close, with the keyboard, and their owners lose the focus. On the UI thread.
     */
    fun touchedOutsideFields() {
        uiField?.takeIf { it.shown }?.close()
        // Closing Turu's field brings the game's back for its focused field: that one goes too.
        gameField?.takeIf { it.shown }?.close()
    }

    /** The UI's [ViewField] changed its text or selection: Turu's text box takes it (`overlay.rs`). */
    @JvmStatic
    external fun nativeUiFieldEdited(text: String, start: Int, end: Int)

    /** The keyboard's Send (or Enter) in the UI's [ViewField]: Enter in Turu's text box. */
    @JvmStatic
    external fun nativeUiFieldEnter()

    /** The UI's [ViewField] was closed (Back): Turu's text box loses the focus. */
    @JvmStatic
    external fun nativeUiFieldClosed()

    /** The notification's buttons: "start", "pause" (or resume) and "stop" ([TuruAirService]). */
    @JvmStatic
    external fun nativeAction(action: String)

    /** The script's state for the notification, every 2 s when it changed (`notify.rs`). */
    @JvmStatic
    fun status(title: String, text: String, running: Boolean, paused: Boolean, startable: Boolean, awake: Boolean) {
        TuruAirService.status(title, text, running, paused, startable, awake)
    }

    @JvmStatic
    fun dispatch(code: String, level: String) {
        context?.dispatchStatusEventAsync(code, level)
    }

    /** Whether [attachOverlay] follows AIR's activities (registered once per process). */
    private var following = false

    /**
     * Puts [TuruOverlay] over AIR's view, once per activity. The process can outlive its activity:
     * swiped out of the recent apps, the app keeps running (the foreground service), and opening
     * it again makes a new activity of AIR's class, while AS3 (and so `startEngine`) goes on as it
     * was. So the first call follows that class's activities from then on: each new one gets an
     * overlay of its own (the old one went with the old activity), and becomes [activity].
     */
    fun attachOverlay(activity: Activity) {
        this.activity = activity
        if (!following) {
            following = true
            follow(activity)
        }
        activity.runOnUiThread {
            if (overlay?.context === activity) return@runOnUiThread
            val content = activity.findViewById<ViewGroup>(android.R.id.content)
            val air = content.getChildAt(0)
            airView = air
            // The host SWF fits the games to this size itself (status event "layout": "width
            // height"): AIR keeps the screen's size for its stage when its view gets shorter (as
            // for a soft keyboard), which the tab strip's margin does.
            air.addOnLayoutChangeListener { v, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom ->
                if (right - left != oldRight - oldLeft || bottom - top != oldBottom - oldTop) {
                    dispatch("layout", "${v.width} ${v.height}")
                }
            }
            air.post { dispatch("layout", "${air.width} ${air.height}") }
            val view = TuruOverlay(activity)
            content.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
            overlay = view
        }
    }

    private fun follow(first: Activity) {
        val air = first.javaClass
        first.application.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityResumed(a: Activity) {
                if (a.javaClass == air && overlay?.context !== a) attachOverlay(a)
            }

            override fun onActivityDestroyed(a: Activity) {
                if (overlay?.context === a) {
                    overlay = null
                    airView = null
                }
                if (activity === a) activity = null
            }

            override fun onActivityCreated(a: Activity, savedInstanceState: Bundle?) {}
            override fun onActivityStarted(a: Activity) {}
            override fun onActivityPaused(a: Activity) {}
            override fun onActivityStopped(a: Activity) {}
            override fun onActivitySaveInstanceState(a: Activity, outState: Bundle) {}
        })
    }

    /** The game's place: `left` pixels right of the screen's edge (the rail), `top` below its top. */
    @JvmStatic
    fun gameInsets(left: Int, top: Int) {
        activity?.runOnUiThread {
            val view = airView ?: return@runOnUiThread
            val params = view.layoutParams as? ViewGroup.MarginLayoutParams ?: return@runOnUiThread
            if (params.leftMargin != left || params.topMargin != top) {
                params.leftMargin = left
                params.topMargin = top
                view.layoutParams = params
            }
        }
    }

    /** Brings the soft keyboard up for the UI's text box, or down. */
    @JvmStatic
    fun softKeyboard(shown: Boolean) {
        val activity = activity ?: return
        activity.runOnUiThread {
            val view = overlay ?: return@runOnUiThread
            val keyboard = activity.getSystemService(InputMethodManager::class.java)
            if (shown) {
                view.requestFocus()
                keyboard.showSoftInput(view, 0)
            } else {
                keyboard.hideSoftInputFromWindow(view.windowToken, 0)
            }
        }
    }

    @JvmStatic
    fun openUrl(url: String) {
        val activity = activity ?: return
        activity.runOnUiThread {
            runCatching { activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }
        }
    }

    /** Android's share sheet (mail, a chat, Drive) with `text`. */
    @JvmStatic
    fun shareText(subject: String, text: String) {
        val activity = activity ?: return
        activity.runOnUiThread {
            val send = Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_SUBJECT, subject)
                .putExtra(Intent.EXTRA_TEXT, text)
            runCatching { activity.startActivity(Intent.createChooser(send, subject)) }
        }
    }

    @JvmStatic
    fun copyText(text: String) {
        val activity = activity ?: return
        activity.runOnUiThread {
            activity.getSystemService(ClipboardManager::class.java)
                .setPrimaryClip(ClipData.newPlainText("Turu", text))
        }
    }

    /**
     * Asks "Send crash reports?" with Android's dialog ([CrashQuestion]); the answer goes to
     * [nativeCrashUploadsAnswer]. False without an activity to ask on. From the overlay's thread.
     */
    @JvmStatic
    fun askCrashUploads(title: String, message: String, note: String, link: String, linkUrl: String, no: String, yes: String): Boolean {
        val activity = activity ?: return false
        activity.runOnUiThread {
            CrashQuestion.show(activity, title, message, note, link, linkUrl, no, yes) { nativeCrashUploadsAnswer(it) }
        }
        return true
    }

    /** The answer to "Send crash reports?": 1 yes, 0 no, -1 closed without one (`platform.rs`). */
    @JvmStatic
    external fun nativeCrashUploadsAnswer(answer: Int)

    /**
     * Installs an update's APK at `path` with Android's installer ([PackageUpdate]); the outcome
     * goes to [nativePackageInstalled]. Null when it started, else why not. From the update's
     * thread (`platform.rs`).
     */
    @JvmStatic
    fun installPackage(path: String): String? {
        val activity = activity ?: return "open Turu Mas to install the update"
        return PackageUpdate.install(activity, path) { status, message -> nativePackageInstalled(status, message) }
    }

    /** How Android's installer ended [installPackage]'s install ([PackageUpdate]'s status). */
    @JvmStatic
    external fun nativePackageInstalled(status: Int, message: String?)

    /** AIR's activity, while there is one ([PickerActivity] starts from it). */
    fun currentActivity(): Activity? = activity

    /** The bot windows' "open file" dialog: the system picker ([Documents.open]). From `saf.rs`. */
    @JvmStatic
    fun openDocument(mimeTypes: String, folder: String): Boolean = Documents.open(mimeTypes, folder)

    /** The bot windows' "save file" dialog: the system picker ([Documents.create]). From `saf.rs`. */
    @JvmStatic
    fun createDocument(mimeType: String, fileName: String): Boolean = Documents.create(mimeType, fileName)

    /** Copies the file a window saved into its document ([Documents.writeDocument]); null, or why not. */
    @JvmStatic
    fun writeDocument(uri: String, path: String): String? = Documents.writeDocument(uri, path)

    /** The "open" picker's document, copied to `path`; null when cancelled (`saf.rs`). */
    @JvmStatic
    external fun nativeDocumentPicked(path: String?)

    /** The "save" picker made the document `uri`, for the window to write `path`; nulls when cancelled. */
    @JvmStatic
    external fun nativeDocumentCreated(uri: String?, path: String?)

    /** Whether this build can send crash reports ([CrashUploads]). */
    @JvmStatic
    fun crashUploadsAvailable(): Boolean = CrashUploads.available()

    /** The user's answer to "Send crash reports?" (the question, or Options > Application). */
    @JvmStatic
    fun setCrashUploads(on: Boolean) = CrashUploads.setEnabled(on)

    /** A Rust panic's message, from the panic hook just before the abort. */
    @JvmStatic
    fun panicked(message: String) = CrashUploads.panicked(message)
}
