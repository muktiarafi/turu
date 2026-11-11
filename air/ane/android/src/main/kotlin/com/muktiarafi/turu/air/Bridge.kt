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

    /** skua.swf's `ExternalInterface.call(name, ...)`, its arguments as a JSON array. */
    @JvmStatic
    external fun nativeFlashCall(name: String, args: String)

    /** AS3's answer to call `id`, and how long the callback took. */
    @JvmStatic
    external fun nativeReply(id: Long, value: String, runMicros: Int)

    /** The SWF rewritten for AIR (target 0: the game's client, 1: skua.swf, 2: another SWF); null if it can't run there. */
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

    /** The game's focused text field as JSON (from the host SWF); empty when there's none. */
    @JvmStatic
    external fun nativeGameTyping(json: String)

    /** The notification's buttons: "start", "pause" (or resume) and "stop" ([TuruAirService]). */
    @JvmStatic
    external fun nativeAction(action: String)

    /** The script's state for the notification, every 2 s when it changed (`notify.rs`). */
    @JvmStatic
    fun status(title: String, text: String, running: Boolean, paused: Boolean, startable: Boolean) {
        TuruAirService.status(title, text, running, paused, startable)
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
            airView = content.getChildAt(0)
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
}
