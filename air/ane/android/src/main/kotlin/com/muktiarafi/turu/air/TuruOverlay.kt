package com.muktiarafi.turu.air

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.PixelFormat
import android.os.Build
import android.text.InputType
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection

/**
 * Turu's windows over the game in AIR (docs/AIR_PROPOSAL.md): a see-through view above AIR's,
 * which the engine draws the nav rail and the bot windows into (`overlay.rs`).
 *
 * A finger is the UI's when it comes down on the rail or a window (the engine says,
 * `Bridge.nativeOnUi`); the others fall through to AIR's view below, as this view doesn't take
 * them. It's also the soft keyboard's target while one of the UI's text boxes has the focus: key
 * events only ([InputType.TYPE_NULL], as the Ruffle build's `KeyInput`), which every keyboard sends
 * (a plain text editor gets composing text from some, Samsung's among them, which this view
 * doesn't keep). It passes the window's insets on: the keyboard's height and the display cutout.
 */
@SuppressLint("ViewConstructor")
class TuruOverlay(activity: Activity) : SurfaceView(activity), SurfaceHolder.Callback {
    private val density = activity.resources.displayMetrics.density

    /** Whether the gesture under way is the UI's. */
    private var ours = false

    init {
        setZOrderOnTop(true)
        holder.setFormat(PixelFormat.TRANSLUCENT)
        holder.addCallback(this)
        isFocusable = true
        isFocusableInTouchMode = true
        setOnApplyWindowInsetsListener { view, insets ->
            sendInsets(insets)
            view.onApplyWindowInsets(insets)
        }
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        requestApplyInsets()
    }

    private fun sendInsets(insets: WindowInsets) {
        val keyboard = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (insets.isVisible(WindowInsets.Type.ime())) insets.getInsets(WindowInsets.Type.ime()).bottom else 0
        } else {
            @Suppress("DEPRECATION")
            insets.systemWindowInsetBottom
        }
        val cutout = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) insets.displayCutout else null
        Bridge.nativeInsets(keyboard, cutout?.safeInsetLeft ?: 0, cutout?.safeInsetRight ?: 0)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {}

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        Bridge.nativeSurface(holder.surface, density)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Bridge.nativeSurface(null, density)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        val x = event.getX(event.actionIndex)
        val y = event.getY(event.actionIndex)
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // A finger down here is outside the text fields above the keyboard (they're over
                // this view, the keyboard is a window of its own): typing ends, as with Back.
                Bridge.touchedOutsideFields()
                ours = Bridge.nativeOnUi(x, y)
                if (!ours) {
                    // The game's, but egui hears it as a tap there too: one on the game closes an
                    // open menu or popup, as a click elsewhere does (the Ruffle build's
                    // `Screen::touch`). The rest of the gesture is AIR's alone.
                    Bridge.nativeTouch(0, x, y)
                    Bridge.nativeTouch(2, x, y)
                    return false
                }
                Bridge.nativeTouch(0, x, y)
            }
            MotionEvent.ACTION_MOVE -> if (ours) Bridge.nativeTouch(1, event.x, event.y)
            MotionEvent.ACTION_UP -> if (ours) {
                Bridge.nativeTouch(2, x, y)
                ours = false
            }
            MotionEvent.ACTION_CANCEL -> if (ours) {
                Bridge.nativeTouch(3, x, y)
                ours = false
            }
        }
        return ours
    }

    override fun onCheckIsTextEditor(): Boolean = true

    /** Key events only (no suggestions, no composing), as the Ruffle build's `KeyInput` does. */
    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = InputType.TYPE_NULL
        // Landscape: no full-screen editor over the game.
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_ACTION_NONE
        return object : BaseInputConnection(this, false) {
            // A keyboard that sends text anyway.
            override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
                if (text.isNotEmpty()) {
                    Bridge.nativeText(text.toString())
                }
                return true
            }

            override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                repeat(beforeLength) { Bridge.nativeKey(0) }
                repeat(afterLength) { Bridge.nativeKey(2) }
                return true
            }

            override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean =
                deleteSurroundingText(beforeLength, afterLength)

            override fun performEditorAction(actionCode: Int): Boolean {
                Bridge.nativeKey(1)
                return true
            }
        }
    }

    /** The keyboard's keys (soft, from the connection above, or hardware) into the UI's text box. */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        when (keyCode) {
            KeyEvent.KEYCODE_DEL -> Bridge.nativeKey(0)
            KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> Bridge.nativeKey(1)
            KeyEvent.KEYCODE_FORWARD_DEL -> Bridge.nativeKey(2)
            KeyEvent.KEYCODE_DPAD_LEFT -> Bridge.nativeKey(3)
            KeyEvent.KEYCODE_DPAD_RIGHT -> Bridge.nativeKey(4)
            else -> {
                val char = event.unicodeChar
                if (char <= 0) return super.onKeyDown(keyCode, event)
                Bridge.nativeText(String(Character.toChars(char)))
            }
        }
        return true
    }

    /** A keyboard's text sent as one event (`ACTION_MULTIPLE` with characters). */
    override fun onKeyMultiple(keyCode: Int, repeatCount: Int, event: KeyEvent): Boolean {
        val characters = event.characters
        if (keyCode == KeyEvent.KEYCODE_UNKNOWN && !characters.isNullOrEmpty()) {
            Bridge.nativeText(characters)
            return true
        }
        repeat(repeatCount) { onKeyDown(keyCode, event) }
        return true
    }
}
