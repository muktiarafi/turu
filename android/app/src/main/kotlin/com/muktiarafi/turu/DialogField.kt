package com.muktiarafi.turu

import android.app.Dialog
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Build
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.widget.EditText
import android.widget.LinearLayout
import com.muktiarafi.turu.shared.FieldTarget
import com.muktiarafi.turu.shared.NativeField

/**
 * Turu's [NativeField] (the shared module's): NativeActivity draws its window itself, so its
 * views don't show, and the strip is a window of its own, a dialog along the bottom of the screen
 * that lets touches outside it through to the game. A tap outside it and the keyboard closes it,
 * as Back does. Its bottom padding puts the field right above the keyboard ([fitAboveKeyboard]).
 */
internal class DialogField(activity: TuruActivity, target: FieldTarget) :
    NativeField(activity, target, TURU_ACCENT) {
    private var dialog: Dialog? = null

    override fun attach(bar: LinearLayout) {
        // Clear of a camera cutout at the side (held sideways).
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
        // Right above the keyboard, checked before each draw.
        bar.viewTreeObserver.addOnPreDrawListener {
            fitAboveKeyboard(bar)
        }
        val dialog = Dialog(activity)
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
        dialog.show()
        // The game's bars stay hidden, as over the activity's window.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            dialog.window?.insetsController?.let { insets ->
                insets.hide(WindowInsets.Type.systemBars())
                insets.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        }
    }

    override fun detach(bar: LinearLayout, field: EditText?) {
        val dialog = dialog ?: return
        this.dialog = null
        dialog.setOnCancelListener(null)
        dialog.dismiss()
    }

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
}
