package com.muktiarafi.turu

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.PowerManager
import android.util.Log
import android.view.Surface
import androidx.core.os.BundleCompat

/**
 * M1 spike (docs/TAB_PROCESSES_PROPOSAL.md): one account's engine in a process of its own
 * (`android:process=":c1"`, `":c2"`), while `turu-slots.txt` is in the data folder. A foreground
 * service with a notification of its own, started and bound by [TuruActivity] (the shell), which
 * sends it its surface, touches and probes through a [Messenger]; the engine is the Rust side's
 * (`slots.rs`), set up from `turu-args-c<N>.txt`.
 */
abstract class SlotService(private val slot: Int) : Service() {
    private lateinit var wakeLock: PowerManager.WakeLock

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "Slot $slot: service created in process ${android.os.Process.myPid()}")
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Turu slots", NotificationManager.IMPORTANCE_LOW),
        )
        val notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Turu slot $slot")
            .setContentText("Running (process ${android.os.Process.myPid()})")
            .setOngoing(true)
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION + slot, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION + slot, notification)
        }
        // The spike keeps the CPU awake all along (the normal service only while a script runs).
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "turu:slot$slot")
        wakeLock.setReferenceCounted(false)
        wakeLock.acquire()
        Storage.init(applicationContext)
        val files = getExternalFilesDir(null) ?: filesDir
        nativeStartSlot(applicationContext, files.absolutePath, slot)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onDestroy() {
        Log.i(TAG, "Slot $slot: service destroyed")
        if (wakeLock.isHeld) wakeLock.release()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder = messenger.binder

    /** The shell that last handed its surface over, watched so that its death lets the surface go. */
    private var shell: IBinder? = null
    private val shellDied = IBinder.DeathRecipient {
        Log.w(TAG, "Slot $slot: the shell died; letting its surface go")
        nativeDetach()
    }

    private fun watchShell(replyTo: Messenger?) {
        val binder = replyTo?.binder ?: return
        if (binder == shell) return
        shell?.unlinkToDeath(shellDied, 0)
        try {
            binder.linkToDeath(shellDied, 0)
            shell = binder
        } catch (e: android.os.RemoteException) {
            Log.w(TAG, "Slot $slot: the shell is gone already: $e")
            shell = null
        }
    }

    private val messenger = Messenger(object : Handler(Looper.getMainLooper()) {
        override fun handleMessage(msg: Message) {
            val answer = when (msg.what) {
                MSG_ATTACH -> {
                    val data = msg.data
                    data.classLoader = Surface::class.java.classLoader
                    val surface = BundleCompat.getParcelable(data, "surface", Surface::class.java)
                    if (surface == null || !surface.isValid) {
                        0
                    } else {
                        watchShell(msg.replyTo)
                        nativeAttach(surface, data.getFloat("density"))
                    }
                }
                MSG_DETACH -> nativeDetach()
                MSG_RESIZE -> {
                    nativeResize()
                    1
                }
                MSG_TOUCH -> {
                    val data = msg.data
                    val kinds = data.getIntArray("kinds") ?: IntArray(0)
                    val ids = data.getIntArray("ids") ?: IntArray(0)
                    val xs = data.getFloatArray("xs") ?: FloatArray(0)
                    val ys = data.getFloatArray("ys") ?: FloatArray(0)
                    for (i in kinds.indices) nativeTouch(kinds[i], ids[i], xs[i], ys[i])
                    1
                }
                MSG_LOCK -> nativeLockProbe(msg.arg1)
                else -> -1
            }
            msg.replyTo?.let { reply ->
                try {
                    reply.send(Message.obtain(null, msg.what, answer, msg.arg2))
                } catch (e: Exception) {
                    Log.w(TAG, "Slot $slot: couldn't answer the shell: $e")
                }
            }
        }
    })

    companion object {
        const val TAG = "turu-slots"
        private const val CHANNEL = "turu-slots"
        private const val NOTIFICATION = 100
        const val MSG_ATTACH = 1
        const val MSG_DETACH = 2
        const val MSG_RESIZE = 3
        const val MSG_TOUCH = 4
        const val MSG_LOCK = 5

        init {
            System.loadLibrary("turu_android")
        }

        @JvmStatic
        private external fun nativeStartSlot(context: Context, filesDir: String, slot: Int)

        @JvmStatic
        private external fun nativeAttach(surface: Surface, density: Float): Int

        @JvmStatic
        private external fun nativeDetach(): Int

        @JvmStatic
        private external fun nativeResize()

        @JvmStatic
        private external fun nativeTouch(kind: Int, id: Int, x: Float, y: Float)

        @JvmStatic
        private external fun nativeLockProbe(action: Int): Int
    }
}

class Slot1Service : SlotService(1)

class Slot2Service : SlotService(2)
