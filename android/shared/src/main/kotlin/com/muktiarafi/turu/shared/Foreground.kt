package com.muktiarafi.turu.shared

import android.app.Activity
import android.app.Application
import android.content.Context
import android.os.Bundle

/**
 * Which of the app's activities is on screen, if one is: [Alerts] posts only while none is (the
 * beep and Turu's windows say it on screen), and [PackageUpdate] shows Android's install
 * confirmation from it.
 */
object Foreground {
    /** The app's activity on screen, or null. */
    @Volatile
    var activity: Activity? = null
        private set

    private val resumedListeners = mutableListOf<(Activity) -> Unit>()

    private var watching = false

    /**
     * Follows the app's activities from now on, once. `current` is an activity already on screen
     * (resumed before this could hear it): Turu Mas's, whose activity is AIR's. Turu calls this
     * from its activity's `onCreate`, before it resumes.
     */
    @Synchronized
    fun watch(context: Context, current: Activity? = null) {
        if (watching) return
        watching = true
        activity = current
        (context.applicationContext as Application).registerActivityLifecycleCallbacks(
            object : Application.ActivityLifecycleCallbacks {
                override fun onActivityResumed(a: Activity) {
                    activity = a
                    synchronized(this@Foreground) { resumedListeners.toList() }.forEach { it(a) }
                }

                override fun onActivityPaused(a: Activity) {
                    if (activity === a) activity = null
                }

                override fun onActivityCreated(a: Activity, savedInstanceState: Bundle?) {}
                override fun onActivityStarted(a: Activity) {}
                override fun onActivityStopped(a: Activity) {}
                override fun onActivitySaveInstanceState(a: Activity, outState: Bundle) {}
                override fun onActivityDestroyed(a: Activity) {}
            },
        )
    }

    /** Calls `listener` each time one of the app's activities comes to the screen (main thread). */
    @Synchronized
    fun onResumed(listener: (Activity) -> Unit) {
        resumedListeners.add(listener)
    }
}
