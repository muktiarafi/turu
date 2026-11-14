package com.muktiarafi.turu.air

import android.app.Activity
import android.content.Context
import android.os.Build
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import com.muktiarafi.turu.shared.FieldStyle
import com.muktiarafi.turu.shared.FieldTarget
import com.muktiarafi.turu.shared.NativeField

/**
 * Turu Mas's [NativeField] (the shared module's): a view along the bottom of the activity, right
 * of Turu's rail, over AIR's view (a window of its own, as Turu's, would take the window focus
 * from AIR, which then drops the game field's focus). Its bottom padding is the keyboard's
 * height, which puts it right above it. Turu's overlay, a surface over every view, keeps clear of
 * it ([Bridge.fieldsMoved]); a finger down outside it closes it ([Bridge.touchedOutsideFields]).
 */
internal class ViewField(activity: Activity, target: FieldTarget) : NativeField(activity, target, TURU_MAS_ACCENT) {
    /** Pixels in from the left: right of Turu's rail ([show]). */
    private var left = 0

    /** [NativeField.show], `left` pixels in from the left: right of Turu's rail. */
    fun show(text: String, start: Int, end: Int, style: FieldStyle, left: Int) {
        this.left = left
        show(text, start, end, style)
    }

    override fun attach(bar: LinearLayout) {
        // Above the keyboard, clear of a camera cutout at the side (held sideways).
        bar.setOnApplyWindowInsetsListener { view, insets ->
            val keyboard = keyboardHeight(insets)
            var cutoutLeft = 0
            var cutoutRight = 0
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                cutoutLeft = insets.displayCutout?.safeInsetLeft ?: 0
                cutoutRight = insets.displayCutout?.safeInsetRight ?: 0
            }
            view.setPadding(dp(16) + cutoutLeft, dp(4), dp(16) + cutoutRight, dp(4) + keyboard)
            insets
        }
        // Over AIR's view and Turu's overlay, along the bottom; touches elsewhere go to them.
        val content = activity.findViewById<ViewGroup>(android.R.id.content)
        val place = FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM)
        place.leftMargin = left
        content.addView(bar, place)
        // Turu's overlay keeps clear of the field (it would draw its windows over it).
        bar.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> Bridge.fieldsMoved() }
        bar.requestApplyInsets()
    }

    override fun detach(bar: LinearLayout, field: EditText?) {
        val methods = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        methods.hideSoftInputFromWindow(bar.windowToken, 0)
        field?.clearFocus()
        (bar.parent as? ViewGroup)?.removeView(bar)
        Bridge.fieldsMoved()
    }

    /** Where the field's top left corner is while it's up, (top, left) in pixels from `view`'s. */
    fun place(view: View): Pair<Int, Int>? {
        val bar = bar ?: return null
        if (!bar.isLaidOut) return null
        val here = IntArray(2)
        bar.getLocationOnScreen(here)
        val there = IntArray(2)
        view.getLocationOnScreen(there)
        return Pair(here[1] - there[1], here[0] - there[0])
    }
}
