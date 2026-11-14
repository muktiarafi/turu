package com.muktiarafi.turu

import android.app.Dialog
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.TextWatcher
import android.text.method.PasswordTransformationMethod
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

/**
 * The game's focused text field, as a real Android text field just above the soft keyboard (as
 * AQW's own Android launcher does): the keyboard covers the game's field, and a native one gets
 * Android's own caret, selection handles, text toolbar (Cut, Copy, Paste, Select all, Translate,
 * Share), suggestions and clipboard.
 *
 * The Rust side shows it when a game field takes the focus, with that field's text and selection
 * (`game_field.rs`, `GameField::sync`), and keeps it in step when the game changes the field.
 * Each edit here goes to the game field ([nativeGameFieldEdited]); the keyboard's Send (or Enter)
 * presses Enter in the game; Back ends typing: the game field loses the focus.
 *
 * NativeActivity draws its window itself, so its views don't show: the field is a window of its
 * own, a dialog along the bottom of the screen that lets touches outside it through to the game.
 * Its bottom padding is the keyboard's height, which puts the field right above it.
 */
internal class GameField(private val activity: TuruActivity) {
    private var dialog: Dialog? = null
    private var edit: Field? = null
    private var count: TextView? = null
    private var max = 0

    /** Setting what the game holds: not an edit to send back. */
    private var fromGame = false

    /** Shows the field with `text`, `start`..`end` selected (UTF-16 units, as the game's). */
    fun show(text: String, start: Int, end: Int, password: Boolean, max: Int) {
        hide(false)
        this.max = max
        val dialog = Dialog(activity)
        val field = Field(activity, this)
        // Single line first: it sets its own transformation, over a password's dots.
        field.isSingleLine = true
        field.inputType = if (password) {
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
        } else {
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
        }
        if (password) {
            field.transformationMethod = PasswordTransformationMethod.getInstance()
        }
        // Landscape: the field here, not the keyboard's full-screen editor.
        field.imeOptions = EditorInfo.IME_ACTION_SEND or EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            EditorInfo.IME_FLAG_NO_FULLSCREEN
        if (max > 0) {
            field.filters = arrayOf(InputFilter.LengthFilter(max))
        }
        field.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
        field.setTextColor(TEXT)
        field.setHintTextColor(MUTED)
        field.hint = "Type in the game"
        field.highlightColor = ACCENT and 0x00FFFFFF or 0x66000000
        field.background = null
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            field.textCursorDrawable = GradientDrawable().apply {
                setColor(ACCENT)
                setSize(dp(2), 0)
            }
            field.textSelectHandle?.setTint(ACCENT)
            field.textSelectHandleLeft?.setTint(ACCENT)
            field.textSelectHandleRight?.setTint(ACCENT)
        }
        field.setOnEditorActionListener { _, action, event ->
            val enter = event?.keyCode == KeyEvent.KEYCODE_ENTER && event.action == KeyEvent.ACTION_DOWN
            if (action == EditorInfo.IME_ACTION_SEND || action == EditorInfo.IME_ACTION_DONE || enter) {
                TuruActivity.nativeGameFieldEnter()
                true
            } else {
                false
            }
        }
        val count = TextView(activity).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            setTextColor(MUTED)
            setPadding(dp(12), 0, 0, 0)
            visibility = if (max > 0) android.view.View.VISIBLE else android.view.View.GONE
        }
        val bar = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(CARD)
            setPadding(dp(16), dp(4), dp(16), dp(4))
            addView(field, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            addView(count, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        // Above the keyboard, clear of a camera cutout at the side (held sideways).
        bar.setOnApplyWindowInsetsListener { view, insets ->
            val keyboard = keyboardHeight(insets)
            var left = 0
            var right = 0
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                left = insets.displayCutout?.safeInsetLeft ?: 0
                right = insets.displayCutout?.safeInsetRight ?: 0
            }
            view.setPadding(dp(16) + left, dp(4), dp(16) + right, dp(4) + keyboard)
            insets
        }
        dialog.setContentView(bar)
        dialog.setCancelable(true)
        dialog.setCanceledOnTouchOutside(false)
        dialog.setOnCancelListener { close() }
        dialog.window?.let { window ->
            window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            window.setGravity(Gravity.BOTTOM)
            window.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
            // Touches outside the field go to the game.
            window.addFlags(WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL)
            window.setSoftInputMode(
                WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE or
                    WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING,
            )
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                window.attributes = window.attributes.apply {
                    layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
                }
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.setDecorFitsSystemWindows(false)
            }
        }
        this.dialog = dialog
        this.edit = field
        this.count = count
        set(text, start, end)
        field.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) {
                showCount()
                edited()
            }
        })
        dialog.show()
        // The game's bars stay hidden, as over the activity's window.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            dialog.window?.insetsController?.let { insets ->
                insets.hide(WindowInsets.Type.systemBars())
                insets.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        }
        field.requestFocus()
        field.post {
            val methods = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
            methods.showSoftInput(field, InputMethodManager.SHOW_IMPLICIT)
        }
    }

    /** The game changed its field: shows its `text` and selection, unless that's what's here. */
    fun update(text: String, start: Int, end: Int) {
        val field = edit ?: return
        if (field.text.toString() == text && field.selectionStart == start && field.selectionEnd == end) {
            return
        }
        set(text, start, end)
    }

    /** Hides the field; `close` tells the game too (the user closed it). */
    fun hide(close: Boolean = false) {
        val dialog = dialog ?: return
        this.dialog = null
        edit = null
        count = null
        dialog.setOnCancelListener(null)
        dialog.dismiss()
        if (close) {
            TuruActivity.nativeGameFieldClosed()
        }
    }

    fun close() = hide(true)

    private fun set(text: String, start: Int, end: Int) {
        val field = edit ?: return
        fromGame = true
        try {
            if (field.text.toString() != text) {
                field.setText(text)
            }
            val length = field.text.length
            field.setSelection(start.coerceIn(0, length), end.coerceIn(0, length))
        } finally {
            fromGame = false
        }
        showCount()
    }

    private fun showCount() {
        val field = edit ?: return
        if (max > 0) {
            count?.text = "${field.text.length}/$max"
            count?.setTextColor(if (field.text.length >= max) DANGER else MUTED)
        }
    }

    /** The text or the selection changed here: the game's field takes it. */
    fun edited() {
        val field = edit ?: return
        if (fromGame) {
            return
        }
        TuruActivity.nativeGameFieldEdited(field.text.toString(), field.selectionStart, field.selectionEnd)
    }

    private fun dp(value: Int): Int = (value * activity.resources.displayMetrics.density).toInt()

    /**
     * An EditText that says when its selection moves (the game's caret follows), and closes on
     * Back: the keyboard going down alone doesn't (a hardware keyboard's key hides it).
     */
    private class Field(context: Context, private val owner: GameField) : EditText(context) {
        override fun onKeyPreIme(keyCode: Int, event: KeyEvent): Boolean {
            if (keyCode == KeyEvent.KEYCODE_BACK && event.action == KeyEvent.ACTION_UP) {
                owner.close()
                return true
            }
            return super.onKeyPreIme(keyCode, event)
        }

        override fun onSelectionChanged(selStart: Int, selEnd: Int) {
            super.onSelectionChanged(selStart, selEnd)
            // Called from EditText's constructor too, before `owner` is set.
            @Suppress("SENSELESS_COMPARISON")
            if (owner != null) {
                owner.edited()
            }
        }
    }

    companion object {
        // Turu's egui colours (`frontend/src/gui/turu/theme.rs`).
        private val CARD = Color.rgb(24, 27, 37)
        private val TEXT = Color.rgb(232, 234, 242)
        private val MUTED = Color.rgb(140, 146, 166)
        private val ACCENT = Color.rgb(0x7C, 0x6C, 0xF0)
        private val DANGER = Color.rgb(0xEF, 0x44, 0x44)

        private fun keyboardHeight(insets: WindowInsets): Int {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                return if (insets.isVisible(WindowInsets.Type.ime())) insets.getInsets(WindowInsets.Type.ime()).bottom else 0
            }
            @Suppress("DEPRECATION")
            return insets.systemWindowInsetBottom
        }
    }
}
