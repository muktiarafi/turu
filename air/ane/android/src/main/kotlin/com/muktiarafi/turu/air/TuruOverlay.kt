package com.muktiarafi.turu.air

import android.annotation.SuppressLint
import android.app.Activity
import android.graphics.PixelFormat
import android.text.InputType
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection

/**
 * Turu's windows over the game in AIR (docs/AIR_PROPOSAL.md): a see-through view above AIR's,
 * which the engine draws the nav rail and the bot windows into (`overlay.rs`).
 *
 * A finger is the UI's when it comes down on the rail or a window (the engine says,
 * `Bridge.nativeOnUi`); the others fall through to AIR's view below, as this view doesn't take
 * them. It's also the soft keyboard's target while one of the UI's text boxes has the focus.
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
                ours = Bridge.nativeOnUi(x, y)
                if (!ours) {
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

    /** Key events only (no suggestions), as the Ruffle build's `KeyInput` does. */
    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        return object : BaseInputConnection(this, false) {
            override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
                if (text.isNotEmpty()) {
                    Bridge.nativeText(text.toString())
                }
                return true
            }

            override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                repeat(beforeLength.coerceAtLeast(1)) { Bridge.nativeKey(0) }
                return true
            }

            override fun sendKeyEvent(event: KeyEvent): Boolean {
                if (event.action == KeyEvent.ACTION_DOWN) {
                    when (event.keyCode) {
                        KeyEvent.KEYCODE_DEL -> Bridge.nativeKey(0)
                        KeyEvent.KEYCODE_ENTER -> Bridge.nativeKey(1)
                        KeyEvent.KEYCODE_FORWARD_DEL -> Bridge.nativeKey(2)
                        KeyEvent.KEYCODE_DPAD_LEFT -> Bridge.nativeKey(3)
                        KeyEvent.KEYCODE_DPAD_RIGHT -> Bridge.nativeKey(4)
                        else -> event.unicodeChar.takeIf { it > 0 }?.let { Bridge.nativeText(it.toChar().toString()) }
                    }
                }
                return true
            }

            override fun performEditorAction(actionCode: Int): Boolean {
                Bridge.nativeKey(1)
                return true
            }
        }
    }
}
