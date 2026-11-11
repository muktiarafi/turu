package com.muktiarafi.turu.air

import android.util.Log
import com.adobe.fre.FREContext

/**
 * Turu's Rust library (`libturu_air.so`, crate `turu_air`) and the way back from it to AS3.
 * Rust calls [dispatch] from its own thread to send AIR a status event; AS3 answers through the
 * extension's `reply`, which lands in [nativeReply].
 */
object Bridge {
    init {
        System.loadLibrary("turu_air")
    }

    /** The extension's context, which status events go through. */
    @Volatile
    var context: FREContext? = null

    /** Times `calls` round trips Rust -> AS3 -> Rust; the summary comes as a "done" event. */
    @JvmStatic
    external fun nativeBench(calls: Int)

    /** AS3's answer to call `id`. */
    @JvmStatic
    external fun nativeReply(id: Int, value: String)

    @JvmStatic
    fun dispatch(code: String, level: String) {
        if (code == "done") {
            Log.i("TuruAir", "bench: $level")
        }
        context?.dispatchStatusEventAsync(code, level)
    }

    /** A plain JNI call from Rust, to time against the round trips. */
    @JvmStatic
    fun echo(value: Int): Int = value
}
