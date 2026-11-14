package com.muktiarafi.turu.air

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.os.Bundle
import android.util.Log

/**
 * Starts the system's document picker for [Documents] and hands back its answer. See-through and
 * without animations, it shows only the picker. Declared in `air/app/app.xml`.
 */
class PickerActivity : Activity() {
    private var answered = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Made again (the picker was up when the system needed the memory): the picker answers
        // this one.
        if (savedInstanceState != null) return
        val kind = intent.getIntExtra(KIND, OPEN)
        val types = intent.getStringExtra(TYPES).orEmpty().lines().filter { it.isNotBlank() }
        val picker = if (kind == CREATE) {
            Intent(Intent.ACTION_CREATE_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType(types.firstOrNull() ?: "application/octet-stream")
                .putExtra(Intent.EXTRA_TITLE, intent.getStringExtra(NAME).orEmpty())
        } else {
            Intent(Intent.ACTION_OPEN_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType(types.singleOrNull() ?: "*/*")
                .apply { if (types.size > 1) putExtra(Intent.EXTRA_MIME_TYPES, types.toTypedArray()) }
        }
        try {
            @Suppress("DEPRECATION")
            startActivityForResult(picker, REQUEST)
        } catch (e: ActivityNotFoundException) {
            Log.w("turu", "No document picker on this phone: $e")
            answer(null)
        }
    }

    @Deprecated("The picker is started with startActivityForResult")
    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQUEST) {
            val uri = if (resultCode == RESULT_OK) data?.data else null
            // The picker's grant on the document ends with this activity, before the copy or the
            // window's save is done: kept until then ([Documents.release]).
            if (uri != null) Documents.hold(uri, data?.flags ?: 0)
            answer(uri)
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        // Gone without an answer (finished by the system): the dialog is cancelled.
        if (!isChangingConfigurations && !answered) answer(null)
    }

    private fun answer(uri: android.net.Uri?) {
        if (answered) return
        answered = true
        val kind = intent.getIntExtra(KIND, OPEN)
        Documents.answer(kind, uri, if (kind == OPEN) intent.getStringExtra(NAME).orEmpty() else "")
        finish()
        @Suppress("DEPRECATION")
        overridePendingTransition(0, 0)
    }

    companion object {
        const val OPEN = 0
        const val CREATE = 1
        private const val REQUEST = 43
        private const val KIND = "kind"
        private const val TYPES = "types"

        /** The open picker's folder, or the save picker's file name. */
        private const val NAME = "name"

        /** Starts the picker of `kind` from AIR's activity; false without one. Any thread. */
        fun start(kind: Int, types: String, name: String): Boolean {
            val activity = Bridge.currentActivity() ?: return false
            activity.runOnUiThread {
                val intent = Intent(activity, PickerActivity::class.java)
                    .putExtra(KIND, kind)
                    .putExtra(TYPES, types)
                    .putExtra(NAME, name)
                try {
                    activity.startActivity(intent)
                    @Suppress("DEPRECATION")
                    activity.overridePendingTransition(0, 0)
                } catch (e: Exception) {
                    Log.w("turu", "Couldn't start the document picker: $e")
                    Documents.answer(kind, null, "")
                }
            }
            return true
        }
    }
}
