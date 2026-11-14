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
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

/** Where a [NativeField]'s edits go: the game's focused text field, or Turu's own text box. */
internal interface FieldTarget {
    /** The text or the selection changed (UTF-16 units). */
    fun edited(text: String, start: Int, end: Int)

    /** The keyboard's Send (or Enter) in a one-line field. */
    fun enter()

    /** Back: done typing, the field's owner loses the focus. */
    fun closed()
}

/** What a [NativeField] is for: a password (dots), at most `max` characters (0: no limit), several lines, a number. */
internal data class FieldStyle(
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
 * Share), suggestions, autocorrect, autofill and clipboard. One for the game's text fields, one
 * for Turu's own text boxes (egui's).
 *
 * The Rust side shows it when a field takes the focus, with that field's text and selection
 * (`game_field.rs`, `ui_field.rs`), and keeps it in step when the field changes. Each edit here
 * goes to [target]; the keyboard's Send (or Enter) presses Enter there; Back ends typing: the
 * field loses the focus.
 *
 * NativeActivity draws its window itself, so its views don't show: the field is a window of its
 * own, a dialog along the bottom of the screen that lets touches outside it through to the game.
 * Its bottom padding is the keyboard's height, which puts the field right above it.
 */
internal class NativeField(private val activity: TuruActivity, private val target: FieldTarget) {
    private var dialog: Dialog? = null
    private var edit: Field? = null
    private var count: TextView? = null
    private var max = 0

    /** Setting what the field's owner holds: not an edit to send back. */
    private var fromOwner = false

    /** Whether the field is up, and for what kind of field. */
    val shown: Boolean get() = dialog != null
    var style = FieldStyle()
        private set

    /** Shows the field with `text`, `start`..`end` selected (UTF-16 units, as the owner's). */
    fun show(text: String, start: Int, end: Int, style: FieldStyle) {
        hide(false)
        this.style = style
        val password = style.password
        val max = style.max
        this.max = max
        val dialog = Dialog(activity)
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
            if (!style.lines && (action == EditorInfo.IME_ACTION_SEND || action == EditorInfo.IME_ACTION_DONE || enter)) {
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
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                val left = dp(16) + (insets.displayCutout?.safeInsetLeft ?: 0)
                val right = dp(16) + (insets.displayCutout?.safeInsetRight ?: 0)
                if (view.paddingLeft != left || view.paddingRight != right) {
                    view.setPadding(left, view.paddingTop, right, view.paddingBottom)
                }
            }
            insets
        }
        // Right above the keyboard, checked before each draw (`fitAboveKeyboard`).
        bar.viewTreeObserver.addOnPreDrawListener {
            fitAboveKeyboard(bar)
        }
        // No title row above the field (the activity's theme gives dialogs an empty one).
        dialog.requestWindowFeature(Window.FEATURE_NO_TITLE)
        dialog.setContentView(bar)
        dialog.setCancelable(true)
        // A tap outside the field and the keyboard ends typing, as Back does (and still reaches
        // what it was on: the game, or Turu's windows).
        dialog.setCanceledOnTouchOutside(true)
        dialog.setOnCancelListener { close() }
        dialog.window?.let { window ->
            window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            window.setGravity(Gravity.BOTTOM)
            window.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
            // Touches outside the field go to the game; the dialog hears of them (`ACTION_OUTSIDE`)
            // and closes. A tap on the keyboard isn't one: its window is above this one.
            window.addFlags(
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            )
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

    /** The owner changed its field: shows its `text` and selection, unless that's what's here. */
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
            target.closed()
        }
    }

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

    private fun dp(value: Int): Int = (value * activity.resources.displayMetrics.density).toInt()

    /**
     * Puts `bar`'s field right above the keyboard: its bottom padding is how much of its window
     * the keyboard covers. Where the system keeps the window above the keyboard (Android 15+)
     * that's nothing; elsewhere the window stays at the screen's bottom, under the keyboard. The
     * dialog's own insets can't say which: read while the keyboard slides up, they went stale
     * once the window moved (the field then floated a field's height too high). So the
     * keyboard's height comes from the activity's window, which stays put, and the window's
     * place from the screen. False (skip this draw) when the padding changed.
     */
    private fun fitAboveKeyboard(bar: View): Boolean {
        val root = activity.window.decorView
        val keyboard = root.rootWindowInsets?.let { keyboardHeight(it) } ?: 0
        val place = IntArray(2)
        bar.getLocationOnScreen(place)
        val rootPlace = IntArray(2)
        root.getLocationOnScreen(rootPlace)
        val screenBottom = rootPlace[1] + root.height
        val windowBottom = place[1] + bar.height
        val covered = (keyboard - (screenBottom - windowBottom)).coerceAtLeast(0)
        val bottom = dp(4) + covered
        if (bar.paddingBottom == bottom) {
            return true
        }
        bar.setPadding(bar.paddingLeft, bar.paddingTop, bar.paddingRight, bottom)
        return false
    }

    /**
     * An EditText that says when its selection moves (the game's caret follows), and closes on
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
