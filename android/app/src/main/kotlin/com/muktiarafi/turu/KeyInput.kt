package com.muktiarafi.turu

import android.content.Context
import android.os.SystemClock
import android.text.InputType
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection

/**
 * What the soft keyboard types into: an invisible view holding the focus, so the keyboard talks
 * to Turu's connection instead of the stand-in Android gives a window without a text view. The
 * stand-in turns typed text into key events but drops "delete before the cursor", which keyboards
 * send for a tap on delete while they think there's text (holding delete sends keys instead).
 *
 * Its editor is [InputType.TYPE_NULL], which asks the keyboard for key events only (no
 * suggestions, no composing); a delete request that comes anyway becomes Backspace or Delete
 * keys. The keys reach the Rust side as all of the window's do (NativeActivity takes its input
 * queue). The keyboard is shown and hidden for this view on the UI thread
 * (`TuruActivity.softKeyboard`).
 */
internal class KeyInput(context: Context) : View(context) {
    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun onCheckIsTextEditor(): Boolean = true

    override fun checkInputConnectionProxy(view: View?): Boolean = true

    override fun onCreateInputConnection(info: EditorInfo): InputConnection {
        info.inputType = InputType.TYPE_NULL
        // Landscape: no full-screen editor over the game.
        info.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_ACTION_NONE
        return Connection(this)
    }

    private class Connection(view: View) : BaseInputConnection(view, false) {
        override fun deleteSurroundingText(before: Int, after: Int): Boolean {
            press(KeyEvent.KEYCODE_DEL, before)
            press(KeyEvent.KEYCODE_FORWARD_DEL, after)
            return true
        }

        override fun deleteSurroundingTextInCodePoints(before: Int, after: Int): Boolean =
            deleteSurroundingText(before, after)

        /** Presses `code` `times` times, as a soft keyboard's key. */
        private fun press(code: Int, times: Int) {
            repeat(times) {
                val now = SystemClock.uptimeMillis()
                val flags = KeyEvent.FLAG_SOFT_KEYBOARD or KeyEvent.FLAG_KEEP_TOUCH_MODE
                for (action in intArrayOf(KeyEvent.ACTION_DOWN, KeyEvent.ACTION_UP)) {
                    sendKeyEvent(KeyEvent(now, now, action, code, 0, 0, KeyCharacterMap.VIRTUAL_KEYBOARD, 0, flags))
                }
            }
        }
    }
}
