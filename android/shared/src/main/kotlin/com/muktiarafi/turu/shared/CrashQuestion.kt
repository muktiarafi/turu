package com.muktiarafi.turu.shared

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.view.WindowInsets
import android.view.WindowInsetsController

/**
 * "Send crash reports?", asked once at first start with Android's own dialog, next to the
 * notification and battery questions. The Rust side decides when and says what
 * (`turu_frontend`'s `logs.rs`, `Platform::ask_crash_uploads`) and keeps the answer. In Turu Mas
 * the dialog, a window of its own, has AIR's focus while it's up: asked once, at first start.
 */
object CrashQuestion {
    /**
     * Shows the question on `activity`: `message`, then `note`; `link` opens `linkUrl` and leaves
     * the dialog open; `no` and `yes`. `answer` gets 1 (yes), 0 (no) or -1 (closed without one:
     * asked again at the next start). On the UI thread.
     */
    fun show(
        activity: Activity,
        title: String,
        message: String,
        note: String,
        link: String,
        linkUrl: String,
        no: String,
        yes: String,
        answer: (Int) -> Unit,
    ) {
        if (activity.isFinishing || activity.isDestroyed) return
        var answered = false
        val reply = { value: Int ->
            if (!answered) {
                answered = true
                answer(value)
            }
        }
        val dialog = AlertDialog.Builder(activity, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle(title)
            .setMessage("$message\n\n$note")
            .setPositiveButton(yes) { _, _ -> reply(1) }
            .setNegativeButton(no) { _, _ -> reply(0) }
            // Set below: it opens the policy without closing the dialog.
            .setNeutralButton(link, null)
            .setOnDismissListener { reply(-1) }
            .create()
        dialog.setOnShowListener {
            dialog.getButton(AlertDialog.BUTTON_NEUTRAL).setOnClickListener {
                runCatching { activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(linkUrl))) }
            }
        }
        dialog.show()
        // The game's bars stay hidden, as over the activity's window.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            dialog.window?.insetsController?.let { insets ->
                insets.hide(WindowInsets.Type.systemBars())
                insets.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        }
    }
}
