package com.muktiarafi.turu.air

import android.content.Context
import com.adobe.fre.FREContext

/**
 * Turu's engine (`libturu_air.so`, crate `turu_air`) and the way back from it to AS3. The engine
 * calls the game by asking [dispatch] (from its own threads) to send AIR a status event; AS3
 * answers through the extension's `reply`, which lands in [nativeReply].
 */
object Bridge {
    init {
        System.loadLibrary("turu_air")
    }

    /** The extension's context, which status events go through. */
    @Volatile
    var context: FREContext? = null

    /** Starts the engine (once per process) with `filesDir` as its data folder. */
    @JvmStatic
    external fun nativeStart(context: Context, filesDir: String)

    /** skua.swf's `ExternalInterface.call(name, ...)`, its arguments as a JSON array. */
    @JvmStatic
    external fun nativeFlashCall(name: String, args: String)

    /** AS3's answer to call `id`, and how long the callback took. */
    @JvmStatic
    external fun nativeReply(id: Long, value: String, runMicros: Int)

    /** The SWF rewritten for AIR (target 0: the game's client, 1: skua.swf, 2: another SWF); null if it can't run there. */
    @JvmStatic
    external fun nativePatch(swf: ByteArray, target: Int): ByteArray?

    @JvmStatic
    fun dispatch(code: String, level: String) {
        context?.dispatchStatusEventAsync(code, level)
    }
}
