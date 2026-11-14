package com.muktiarafi.turu.air

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import java.io.File
import kotlin.concurrent.thread

/**
 * The bot windows' file dialogs through Android's system picker (`saf.rs`, with
 * `turu_frontend::documents`), as the Ruffle build's `TuruActivity` and `Storage` have them:
 *
 * * Open ([open]): `ACTION_OPEN_DOCUMENT`; the chosen document is copied into the folder the
 *   engine named (a script: the scripts folder's `Imported/`) or the app's cache, and that copy's
 *   path goes to [Bridge.nativeDocumentPicked].
 * * Save ([create]): `ACTION_CREATE_DOCUMENT`; [Bridge.nativeDocumentCreated] gets the document
 *   and a file for the window to write, which [writeDocument] then copies into it.
 *
 * AIR's activity is AIR's, and only documented ways into the runtime are allowed, so the picker
 * is started by [PickerActivity], a see-through activity of the extension's own, which hands the
 * result back here.
 */
object Documents {
    private lateinit var context: Context

    /** Called before `Bridge.nativeStart`. */
    fun init(context: Context) {
        this.context = context.applicationContext
    }

    /**
     * Shows the "open" picker for `mimeTypes` (one a line), the document to be copied into `folder`
     * (empty: the app's cache). False without an activity to start it from.
     */
    fun open(mimeTypes: String, folder: String): Boolean =
        PickerActivity.start(PickerActivity.OPEN, mimeTypes, folder)

    /** Shows the "save" picker making a `mimeType` document named `fileName`, like [open]. */
    fun create(mimeType: String, fileName: String): Boolean =
        PickerActivity.start(PickerActivity.CREATE, mimeType, fileName)

    /**
     * Keeps access to the document at `uri` past the picker's activity (whose grant ends with it),
     * as the picker's result `flags` allow; [release] gives it up.
     */
    internal fun hold(uri: Uri, flags: Int) {
        val modes = flags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        if (flags and Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION == 0 || modes == 0) return
        runCatching { context.contentResolver.takePersistableUriPermission(uri, modes) }
            .onFailure { Log.w("turu", "Couldn't keep access to the document: $it") }
    }

    /** Gives up the access [hold] kept to the document at `uri`. */
    private fun release(uri: Uri) {
        val held = context.contentResolver.persistedUriPermissions.firstOrNull { it.uri == uri } ?: return
        val modes = (if (held.isReadPermission) Intent.FLAG_GRANT_READ_URI_PERMISSION else 0) or
            (if (held.isWritePermission) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0)
        runCatching { context.contentResolver.releasePersistableUriPermission(uri, modes) }
    }

    /** The picker's answer (`uri` null when cancelled, or no picker): the engine gets its own. */
    internal fun answer(kind: Int, uri: Uri?, folder: String) {
        if (kind == PickerActivity.CREATE) {
            if (uri == null) {
                Bridge.nativeDocumentCreated(null, null)
                return
            }
            val path = try {
                saveFile(uri).absolutePath
            } catch (e: Exception) {
                Log.w("turu", "Couldn't make a file to save into: $e")
                null
            }
            if (path == null) release(uri)
            Bridge.nativeDocumentCreated(path?.let { uri.toString() }, path)
            return
        }
        if (uri == null) {
            Bridge.nativeDocumentPicked(null)
            return
        }
        // Off the main thread: a document can be large, or on the network (Drive).
        thread(name = "Turu Pick") {
            val path = try {
                copyDocument(uri, if (folder.isEmpty()) pickedFolder() else File(folder)).absolutePath
            } catch (e: Exception) {
                Log.w("turu", "Couldn't copy the chosen document: $e")
                null
            }
            release(uri)
            Bridge.nativeDocumentPicked(path)
        }
    }

    /** Where a picked document is copied when the engine named no folder. */
    private fun pickedFolder(): File = File(context.cacheDir, "picked/${System.nanoTime()}")

    /** A file for the window to write a saved document into, named like the document. */
    private fun saveFile(uri: Uri): File {
        val folder = File(context.cacheDir, "saves/${System.nanoTime()}").apply { mkdirs() }
        return File(folder, displayName(uri) ?: "document")
    }

    /** The document's name as the picker shows it, made safe as a file name. */
    private fun displayName(uri: Uri): String? {
        val name = context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst() && !it.isNull(0)) it.getString(0) else null
        } ?: uri.lastPathSegment?.substringAfterLast('/')
        return name?.replace(Regex("[/\\\\:*?\"<>|\\u0000]"), "_")?.trim()?.takeIf { it.isNotEmpty() && it != "." && it != ".." }
    }

    /** Copies the document at `uri` into `folder` (made if needed) under its own name. */
    private fun copyDocument(uri: Uri, folder: File): File {
        folder.mkdirs()
        val file = File(folder, displayName(uri) ?: "document")
        val temp = File(folder, file.name + ".part")
        val input = context.contentResolver.openInputStream(uri) ?: throw java.io.IOException("the document can't be read")
        input.use { source -> temp.outputStream().use { source.copyTo(it) } }
        if (file.exists()) {
            file.delete()
        }
        if (!temp.renameTo(file)) {
            temp.delete()
            throw java.io.IOException("couldn't write ${file.path}")
        }
        return file
    }

    /**
     * Writes the file at `path` into the document at `uri` (the one the save dialog made), then
     * deletes the file. Null when it worked, else why not. From the engine's thread.
     */
    fun writeDocument(uri: String, path: String): String? = try {
        val file = File(path)
        val output = context.contentResolver.openOutputStream(Uri.parse(uri), "wt")
            ?: throw java.io.IOException("the document can't be written")
        output.use { target -> file.inputStream().use { it.copyTo(target) } }
        file.delete()
        file.parentFile?.delete()
        null
    } catch (e: Exception) {
        e.toString()
    } finally {
        release(Uri.parse(uri))
    }
}
