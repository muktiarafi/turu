package com.muktiarafi.turu;

import android.content.Context;
import android.os.SystemClock;
import android.text.InputType;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;

/**
 * What the soft keyboard types into: an invisible view holding the focus, so the keyboard talks
 * to Turu's connection instead of the stand-in Android gives a window without a text view. The
 * stand-in turns typed text into key events but drops "delete before the cursor", which keyboards
 * send for a tap on delete while they think there's text (holding delete sends keys instead).
 *
 * <p>Its editor is {@link InputType#TYPE_NULL}, which asks the keyboard for key events only (no
 * suggestions, no composing); a delete request that comes anyway becomes Backspace or Delete
 * keys. The keys reach the Rust side as all of the window's do (NativeActivity takes its input
 * queue). The keyboard is still shown and hidden for NativeActivity's own view (`ui.rs`, through
 * ANativeActivity_showSoftInput), which {@link #checkInputConnectionProxy} accepts.
 */
final class KeyInput extends View {
    KeyInput(Context context) {
        super(context);
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public boolean checkInputConnectionProxy(View view) {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo info) {
        info.inputType = InputType.TYPE_NULL;
        // Landscape: no full-screen editor over the game.
        info.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN
                | EditorInfo.IME_ACTION_NONE;
        return new Connection(this);
    }

    private static final class Connection extends BaseInputConnection {
        Connection(View view) {
            super(view, false);
        }

        @Override
        public boolean deleteSurroundingText(int before, int after) {
            press(KeyEvent.KEYCODE_DEL, before);
            press(KeyEvent.KEYCODE_FORWARD_DEL, after);
            return true;
        }

        @Override
        public boolean deleteSurroundingTextInCodePoints(int before, int after) {
            return deleteSurroundingText(before, after);
        }

        /** Presses `code` `times` times, as a soft keyboard's key. */
        private void press(int code, int times) {
            for (int i = 0; i < times; i++) {
                long now = SystemClock.uptimeMillis();
                int flags = KeyEvent.FLAG_SOFT_KEYBOARD | KeyEvent.FLAG_KEEP_TOUCH_MODE;
                sendKeyEvent(new KeyEvent(now, now, KeyEvent.ACTION_DOWN, code, 0, 0,
                        KeyCharacterMap.VIRTUAL_KEYBOARD, 0, flags));
                sendKeyEvent(new KeyEvent(now, now, KeyEvent.ACTION_UP, code, 0, 0,
                        KeyCharacterMap.VIRTUAL_KEYBOARD, 0, flags));
            }
        }
    }
}
