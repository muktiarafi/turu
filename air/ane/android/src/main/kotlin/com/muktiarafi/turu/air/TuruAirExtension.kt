package com.muktiarafi.turu.air

import android.content.Intent
import android.os.Build
import android.util.Log
import com.adobe.fre.FREByteArray
import com.adobe.fre.FREContext
import com.adobe.fre.FREExtension
import com.adobe.fre.FREFunction
import com.adobe.fre.FREObject

/**
 * Turu's AIR native extension (docs/AIR_PROPOSAL.md): starts Turu's engine ([Bridge]) and passes
 * calls between it and skua.swf, patches SWFs for AIR, and starts and stops [TuruAirService], the
 * foreground service that keeps the game running off screen.
 */
class TuruAirExtension : FREExtension {
    override fun initialize() {}

    override fun createContext(type: String?): FREContext = Context()

    override fun dispose() {}

    private class Context : FREContext() {
        override fun getFunctions(): Map<String, FREFunction> = mapOf(
            "startService" to FREFunction { context, args -> service(context, true, args) },
            "stopService" to FREFunction { context, args -> service(context, false, args) },
            "startEngine" to FREFunction { context, _ ->
                Bridge.context = context
                val activity = context.activity
                // The Ruffle build's data folder: shared storage, reachable with adb.
                val dir = activity.getExternalFilesDir(null) ?: activity.filesDir
                Bridge.nativeStart(activity.applicationContext, dir.absolutePath)
                FREObject.newObject(dir.absolutePath)
            },
            "flashCall" to FREFunction { _, args ->
                val name = args?.getOrNull(0)?.asString
                if (name != null) {
                    Bridge.nativeFlashCall(name, args.getOrNull(1)?.asString ?: "[]")
                }
                null
            },
            "reply" to FREFunction { _, args ->
                val id = args?.getOrNull(0)?.asDouble?.toLong()
                if (id != null) {
                    Bridge.nativeReply(id, args.getOrNull(1)?.asString ?: "null", args.getOrNull(2)?.asInt ?: 0)
                }
                null
            },
            "patchSwf" to FREFunction { _, args -> patch(args) },
        )

        override fun dispose() {}
    }

    private companion object {
        /** `patchSwf(bytes, target)`: the SWF rewritten for AIR as a new ByteArray, or null. */
        fun patch(args: Array<FREObject?>?): FREObject? = try {
            val input = args?.getOrNull(0) as FREByteArray
            input.acquire()
            val bytes = ByteArray(input.length.toInt())
            try {
                input.bytes.get(bytes)
            } finally {
                input.release()
            }
            Bridge.nativePatch(bytes, args.getOrNull(1)?.asInt ?: 0)?.let { output ->
                val result = FREByteArray.newByteArray()
                result.setProperty("length", FREObject.newObject(output.size))
                result.acquire()
                try {
                    result.bytes.put(output)
                } finally {
                    result.release()
                }
                result
            }
        } catch (e: Exception) {
            Log.e("TuruAir", "patchSwf failed", e)
            null
        }

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
