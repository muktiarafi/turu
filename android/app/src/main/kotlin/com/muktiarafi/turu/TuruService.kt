package com.muktiarafi.turu

import com.muktiarafi.turu.shared.Alerts
import com.muktiarafi.turu.shared.CrashApp
import com.muktiarafi.turu.shared.CrashUploads

/** What Turu's crash records and uploads say about it (the shared module's `CrashApp`). */
internal val CRASH_APP = CrashApp(
    name = "Turu",
    sentryDsn = BuildConfig.SENTRY_DSN,
    release = "turu-android",
    client = null,
    memoryHint = "One game client uses about 400 MB. Closing other apps while Turu runs helps.",
)

/**
 * The statics the Rust side calls in every process (the shell's and each slot's): crash reports,
 * alerts and the GPU's warnings. Until M6 it was also the foreground service that ran the tabs in
 * the app's own process; each tab's slot ([SlotService]) does that now.
 *
 * The Rust side reaches only this class's `@JvmStatic` methods, bound in `android/src/java.rs`:
 * change them there too (a mismatch stops the app at start).
 */
class TuruService private constructor() {
    companion object {
        init {
            System.loadLibrary("turu_android")
        }

        /** For the Rust side, which can only reach this class: see [CrashUploads]. */
        @JvmStatic
        fun crashUploadsAvailable(): Boolean = CrashUploads.available()

        @JvmStatic
        fun setCrashUploads(on: Boolean) = CrashUploads.setEnabled(on)

        @JvmStatic
        fun panicked(message: String) = CrashUploads.panicked(message)

        /** Called by the Rust side (any thread): a warning that goes with the next crash report. */
        @JvmStatic
        fun breadcrumb(category: String, message: String) = CrashUploads.breadcrumb(category, message)

        /**
         * Called by the Rust side (any thread): an alert ([Alerts]: 0 a watched drop, 1 a script
         * stopped), posted while the app isn't on screen.
         */
        @JvmStatic
        fun alert(kind: Int, title: String, text: String) {
            Alerts.post(kind, title, text)
        }

        /** Called by the Rust side (any thread): the GPU dropped the device. */
        @JvmStatic
        fun gpuLost(why: String) = CrashUploads.gpuLost(why)
    }
}
