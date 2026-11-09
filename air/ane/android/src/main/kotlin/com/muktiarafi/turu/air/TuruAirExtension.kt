package com.muktiarafi.turu.air

import android.content.Intent
import android.os.Build
import com.adobe.fre.FREContext
import com.adobe.fre.FREExtension
import com.adobe.fre.FREFunction
import com.adobe.fre.FREObject

/**
 * Turu's AIR native extension (docs/AIR_PROPOSAL.md, spike A0): starts and stops
 * [TuruAirService], the foreground service that keeps the game running off screen.
 * The bridge to Turu's engine comes later.
 */
class TuruAirExtension : FREExtension {
    override fun initialize() {}

    override fun createContext(type: String?): FREContext = Context()

    override fun dispose() {}

    private class Context : FREContext() {
        override fun getFunctions(): Map<String, FREFunction> = mapOf(
            "startService" to FREFunction { context, args -> service(context, true, args) },
            "stopService" to FREFunction { context, args -> service(context, false, args) },
        )

        override fun dispose() {}
    }

    private companion object {
        /** Starts the service (with the notification's text, the first argument) or stops it. */
        fun service(context: FREContext, start: Boolean, args: Array<FREObject?>?): FREObject? {
            val ok = try {
                val activity = context.activity
                val intent = Intent(activity, TuruAirService::class.java)
                if (start) {
                    args?.firstOrNull()?.asString?.let { intent.putExtra(TuruAirService.EXTRA_TEXT, it) }
                    if (Build.VERSION.SDK_INT >= 26) {
                        activity.startForegroundService(intent)
                    } else {
                        activity.startService(intent)
                    }
                } else {
                    activity.stopService(intent)
                }
                true
            } catch (e: Exception) {
                context.dispatchStatusEventAsync("error", e.toString())
                false
            }
            return runCatching { FREObject.newObject(ok) }.getOrNull()
        }
    }
}
