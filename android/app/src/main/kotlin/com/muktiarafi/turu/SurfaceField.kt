package com.muktiarafi.turu

import android.content.Context
import android.graphics.Rect
import android.os.Build
import android.view.Gravity
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import com.muktiarafi.turu.shared.FieldTarget
import com.muktiarafi.turu.shared.NativeField

/**
 * Turu's [NativeField] (the shared module's): a view along the bottom of the activity, over
 * GameActivity's SurfaceView, where the game and Turu's windows are drawn (a view over a
 * SurfaceView draws on top of it). Its bottom padding is the keyboard's height, from the window's
 * insets, which puts it right above the keyboard (the window doesn't move: `adjustNothing`). A
 * finger down outside it closes it, as Back does, and still reaches what it landed on
 * ([TuruActivity.dispatchTouchEvent], [touchedOutside]); a tap on the keyboard isn't one, its
 * window is above the activity's.
 */
internal class SurfaceField(private val turu: TuruActivity, target: FieldTarget) :
    NativeField(turu, target, TURU_ACCENT) {
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
        val content = activity.findViewById<ViewGroup>(android.R.id.content)
        val place = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
            Gravity.BOTTOM,
        )
        content.addView(bar, place)
        bar.requestApplyInsets()
    }

    override fun detach(bar: LinearLayout, field: EditText?) {
        val methods = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        methods.hideSoftInputFromWindow(bar.windowToken, 0)
        field?.clearFocus()
        (bar.parent as? ViewGroup)?.removeView(bar)
        // What the keyboard types into when no field is up.
        turu.focusKeys()
    }

    /**
     * Whether a finger down at `x`, `y` (on the screen) missed the field while it's up: then it
     * closes, as Back does ([TuruActivity.dispatchTouchEvent]).
     */
    fun touchedOutside(x: Float, y: Float): Boolean {
        val bar = bar ?: return false
        if (!bar.isLaidOut) return false
        val area = Rect()
        bar.getGlobalVisibleRect(area)
        return !area.contains(x.toInt(), y.toInt())
    }
}
