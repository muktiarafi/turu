package com.muktiarafi.turu.shared

import android.app.Activity
import android.content.Context
import android.graphics.Color
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
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

/** Where a [NativeField]'s edits go: the game's focused text field, or Turu's own text box. */
interface FieldTarget {
    /** The text or the selection changed (UTF-16 units). */
    fun edited(text: String, start: Int, end: Int)

    /** The keyboard's Send (or Enter) in a one-line field. */
    fun enter()

    /** Back: done typing, the field's owner loses the focus. */
    fun closed()
}

/** What a [NativeField] is for: a password (dots), at most `max` characters (0: no limit), several lines, a number. */
data class FieldStyle(
    val password: Boolean = false,
    val max: Int = 0,
    val hint: String = "",
    val lines: Boolean = false,
    val number: Boolean = false,
)

/**
 * A text field being typed in, as a real Android text field just above the soft keyboard (as
 * AQW's own Android launcher does): the keyboard covers the field, and a native one gets
 * Android's own caret, selection handles, text toolbar (Cut, Copy, Paste, Select all, Translate,
 * Share), suggestions, autocorrect, autofill and clipboard. Each app has one for the game's text
 * fields and one for Turu's own text boxes (egui's).
 *
 * The Rust side shows it when a field takes the focus, with that field's text and selection
 * (Turu: `game_field.rs`, `ui_field.rs`; Turu Mas: the host SWF's `gameTyping`, `overlay.rs`),
 * and keeps it in step when the field changes. Each edit here goes to [target]; the keyboard's
 * Send (or Enter) presses Enter there; Back ends typing: the field loses the focus.
 *
 * This builds the strip (the field and its count, in Turu's colours with the app's `accent`);
 * each app puts it on screen its own way ([attach], [detach]): Turu as a dialog of its own
 * (NativeActivity's views don't show), Turu Mas as a view right of the rail (a window of its own
 * would take AIR's focus). See docs/ANDROID_NATIVE_UI.md.
 */
abstract class NativeField(
    val activity: Activity,
    private val target: FieldTarget,
    private val accent: Int,
) {
    /** The strip while it's up: the field and its count. */
    protected var bar: LinearLayout? = null
        private set
    private var edit: Field? = null
    private var count: TextView? = null
    private var max = 0

    /** Whether the field is up, and for what kind of field. */
    val shown: Boolean get() = bar != null
    var style = FieldStyle()
        private set

    /** Setting what the field's owner holds: not an edit to send back. */
    private var fromOwner = false

    /** Puts the strip on screen (along the bottom, above the keyboard). */
    protected abstract fun attach(bar: LinearLayout)

    /** Takes the strip off screen, `field` its text field (the keyboard goes with it). */
    protected abstract fun detach(bar: LinearLayout, field: EditText?)

    /** Shows the field with `text`, `start`..`end` selected (UTF-16 units, as the owner's). */
    fun show(text: String, start: Int, end: Int, style: FieldStyle) {
        hide(false)
        this.style = style
        val password = style.password
        val max = style.max
        this.max = max
        val field = Field(activity, this)
        if (style.lines) {
            field.isSingleLine = false
            field.maxLines = 4
            field.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        } else {
            // Single line first: it sets its own transformation, over a password's dots.
            field.isSingleLine = true
            field.inputType = when {
                password -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
                style.number -> InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_SIGNED or
                    InputType.TYPE_NUMBER_FLAG_DECIMAL
                else -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
            }
        }
        if (password) {
            field.transformationMethod = PasswordTransformationMethod.getInstance()
        }
        // Landscape: the field here, not the keyboard's full-screen editor. Several lines: Enter
        // starts a new one.
        val action = if (style.lines) EditorInfo.IME_ACTION_NONE else EditorInfo.IME_ACTION_SEND
        field.imeOptions = action or EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        if (max > 0) {
            field.filters = arrayOf(InputFilter.LengthFilter(max))
        }
        field.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
        field.setTextColor(TEXT)
        field.setHintTextColor(MUTED)
        field.hint = style.hint
        field.highlightColor = accent and 0x00FFFFFF or 0x66000000
        field.background = null
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            field.textCursorDrawable = GradientDrawable().apply {
                setColor(accent)
                setSize(dp(2), 0)
            }
            field.textSelectHandle?.setTint(accent)
            field.textSelectHandleLeft?.setTint(accent)
            field.textSelectHandleRight?.setTint(accent)
        }
        field.setOnEditorActionListener { _, editorAction, event ->
            val enter = event?.keyCode == KeyEvent.KEYCODE_ENTER && event.action == KeyEvent.ACTION_DOWN
            val send = editorAction == EditorInfo.IME_ACTION_SEND || editorAction == EditorInfo.IME_ACTION_DONE
            if (!style.lines && (send || enter)) {
                target.enter()
                true
            } else {
                false
            }
        }
        val count = TextView(activity).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            setTextColor(MUTED)
            setPadding(dp(12), 0, 0, 0)
            visibility = if (max > 0) View.VISIBLE else View.GONE
        }
        val bar = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(CARD)
            setPadding(dp(16), dp(4), dp(16), dp(4))
            addView(field, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            addView(count, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        this.bar = bar
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
        attach(bar)
        field.requestFocus()
        field.post {
            // Laid out now: the text scrolled to the caret at the field's real width.
            edit?.let { set(it.text.toString(), it.selectionStart, it.selectionEnd) }
            field.scrollTo(0, 0)
            field.bringPointIntoView(field.selectionEnd)
            val methods = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
            methods.showSoftInput(field, InputMethodManager.SHOW_IMPLICIT)
        }
    }

    /** The owner changed its field: shows its `text` and selection, unless that's what's here. */
    fun update(text: String, start: Int, end: Int) {
        val field = edit ?: return
        if (field.text.toString() == text && field.selectionStart == start && field.selectionEnd == end) {
            return
        }
        set(text, start, end)
    }

    /** Hides the field; `close` tells its owner too (the user closed it). */
    fun hide(close: Boolean = false) {
        val bar = bar ?: return
        this.bar = null
        val field = edit
        edit = null
        count = null
        detach(bar, field)
        if (close) {
            target.closed()
        }
    }

    /** The user closed the field (Back, a tap outside): its owner loses the focus. */
    fun close() = hide(true)

    private fun set(text: String, start: Int, end: Int) {
        val field = edit ?: return
        fromOwner = true
        try {
            if (field.text.toString() != text) {
                field.setText(text)
            }
            val length = field.text.length
            field.setSelection(start.coerceIn(0, length), end.coerceIn(0, length))
        } finally {
            fromOwner = false
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

    /** The text or the selection changed here: the owner's field takes it. */
    fun edited() {
        val field = edit ?: return
        if (fromOwner) {
            return
        }
        target.edited(field.text.toString(), field.selectionStart, field.selectionEnd)
    }

    protected fun dp(value: Int): Int = (value * activity.resources.displayMetrics.density).toInt()

    /**
     * An EditText that says when its selection moves (the owner's caret follows), and closes on
     * Back: the keyboard going down alone doesn't (a hardware keyboard's key hides it).
     */
    private class Field(context: Context, private val owner: NativeField) : EditText(context) {
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
        // Turu's egui colours (`frontend/src/gui/turu/theme.rs`); the accent is each app's.
        private val CARD = Color.rgb(24, 27, 37)
        private val TEXT = Color.rgb(232, 234, 242)
        private val MUTED = Color.rgb(140, 146, 166)
        private val DANGER = Color.rgb(0xEF, 0x44, 0x44)

        /** Turu's accent (violet) and Turu Mas's (gold), as `theme.rs`'s `ACCENT`. */
        val TURU_ACCENT = Color.rgb(0x7C, 0x6C, 0xF0)
        val TURU_MAS_ACCENT = Color.rgb(0xD4, 0xA7, 0x2C)

        /** The soft keyboard's height in `insets`, 0 while it's down. */
        fun keyboardHeight(insets: WindowInsets): Int {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                return if (insets.isVisible(WindowInsets.Type.ime())) insets.getInsets(WindowInsets.Type.ime()).bottom else 0
            }
            @Suppress("DEPRECATION")
            return insets.systemWindowInsetBottom
        }
    }
}
