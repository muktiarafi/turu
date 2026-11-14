package com.muktiarafi.turu.air

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
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject

/**
 * The game's focused text field (AIR's), as a real Android text field just above the soft keyboard
 * (as AQW's own Android launcher does, and Turu's `GameField`): the keyboard covers the game's
 * field, and a native one gets Android's own caret, selection handles, text toolbar (Cut, Copy,
 * Paste, Select all, Translate, Share), suggestions and clipboard.
 *
 * The host SWF reports the focused field each frame it changes (`gameTyping`, JSON): [Bridge]
 * shows, updates or hides this from it. Each edit here goes back to the host as a status event
 * "typing" (`{"op": "set", "text", "start", "end"}`), which puts it in the game's field; the
 * keyboard's Send (or Enter) is `{"op": "enter"}`, Enter in the game; Back is `{"op": "close"}`:
 * done typing, the game's field loses the focus.
 *
 * The field is a view along the bottom of the activity, right of Turu's rail, over AIR's view (a
 * window of its own, as Turu's, would take the window focus from AIR, which then drops the game
 * field's focus). Its bottom padding is the keyboard's height, which puts it right above it.
 */
internal class GameField(val activity: Activity) {
    private var bar: LinearLayout? = null
    private var edit: Field? = null
    private var count: TextView? = null
    var max = 0
        private set

    /** Whether the field is up, and for what kind of game field (a password, `maxChars`). */
    val shown: Boolean get() = bar != null
    var password = false
        private set

    /** Setting what the game holds: not an edit to send back. */
    private var fromGame = false

    /**
     * Shows the field with `text`, `start`..`end` selected (UTF-16 units, as the game's), `left`
     * pixels in from the left: right of Turu's rail, which the overlay draws over every view.
     */
    fun show(text: String, start: Int, end: Int, password: Boolean, max: Int, left: Int) {
        hide(false)
        this.max = max
        this.password = password
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
                send(JSONObject().put("op", "enter"))
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
        // Over AIR's view and Turu's overlay, along the bottom; touches elsewhere go to them.
        val content = activity.findViewById<ViewGroup>(android.R.id.content)
        val place = FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM)
        place.leftMargin = left
        content.addView(bar, place)
        bar.requestApplyInsets()
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
        val bar = bar ?: return
        this.bar = null
        val field = edit
        edit = null
        count = null
        val methods = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        methods.hideSoftInputFromWindow(bar.windowToken, 0)
        field?.clearFocus()
        (bar.parent as? ViewGroup)?.removeView(bar)
        if (close) {
            send(JSONObject().put("op", "close"))
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
        send(
            JSONObject()
                .put("op", "set")
                .put("text", field.text.toString())
                .put("start", field.selectionStart)
                .put("end", field.selectionEnd),
        )
    }

    /** A status event "typing" to the host SWF. */
    private fun send(request: JSONObject) = Bridge.dispatch("typing", request.toString())

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
        // Turu Mas's egui colours (`frontend/src/gui/turu/theme.rs`, the `air` feature's gold).
        private val CARD = Color.rgb(24, 27, 37)
        private val TEXT = Color.rgb(232, 234, 242)
        private val MUTED = Color.rgb(140, 146, 166)
        private val ACCENT = Color.rgb(0xD4, 0xA7, 0x2C)
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
