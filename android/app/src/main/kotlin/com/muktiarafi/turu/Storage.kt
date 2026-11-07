package com.muktiarafi.turu

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * What the Rust side keeps or reads through Android's storage APIs (`storage.rs`, `saf.rs`):
 *
 * * Account passwords (the Accounts window's `SecretStore`): encrypted with an AES-GCM key that
 *   lives in the Android Keystore (it never leaves it), one file per account in the app's
 *   internal storage (`noBackupFilesDir/passwords`), out of the shared data folder `adb` reads.
 * * Documents the system picker chose (`TuruActivity.openDocument` / `createDocument`): copying
 *   a picked one into a folder, writing a saved one from the file the window wrote.
 *
 * The Rust side reaches this class through the reference [nativeInit] hands it, so keep these
 * methods static (`@JvmStatic`) with the same names and parameter types.
 */
object Storage {
    private const val KEY_ALIAS = "turu-passwords"
    private const val GCM_TAG_BITS = 128

    private lateinit var context: Context

    init {
        System.loadLibrary("turu_android")
    }

    /** Called by the activity and the service before the engine needs it. */
    fun init(context: Context) {
        this.context = context.applicationContext
        nativeInit()
    }

    @JvmStatic
    private external fun nativeInit()

    // Passwords.

    private fun passwords() = File(context.noBackupFilesDir, "passwords").apply { mkdirs() }

    /** One file per account: its name in hex (any username is a valid file name then). */
    private fun passwordFile(name: String) =
        File(passwords(), name.toByteArray(Charsets.UTF_8).joinToString("") { "%02x".format(it) })

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(KEY_ALIAS, null) as SecretKey?)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    /** Saves `password` for `name` (lowercased by the Rust side): the IV's length, the IV, the ciphertext. */
    @JvmStatic
    @Synchronized
    fun savePassword(name: String, password: String) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val iv = cipher.iv
        val sealed = cipher.doFinal(password.toByteArray(Charsets.UTF_8))
        val file = passwordFile(name)
        val temp = File(file.parentFile, file.name + ".tmp")
        temp.writeBytes(byteArrayOf(iv.size.toByte()) + iv + sealed)
        if (!temp.renameTo(file)) {
            temp.delete()
            throw java.io.IOException("couldn't save the password file")
        }
    }

    /** The saved password for `name`, null when there's none. */
    @JvmStatic
    @Synchronized
    fun password(name: String): String? {
        val file = passwordFile(name)
        if (!file.exists()) {
            return null
        }
        val bytes = file.readBytes()
        val ivLength = bytes[0].toInt()
        val iv = bytes.copyOfRange(1, 1 + ivLength)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(GCM_TAG_BITS, iv))
        return String(cipher.doFinal(bytes, 1 + ivLength, bytes.size - 1 - ivLength), Charsets.UTF_8)
    }

    /** Forgets the password for `name` (none saved is fine). */
    @JvmStatic
    @Synchronized
    fun deletePassword(name: String) {
        passwordFile(name).delete()
    }

    // Documents.

    /** Where a picked document is copied when the Rust side named no folder. */
    fun pickedFolder(): File = File(context.cacheDir, "picked/${System.nanoTime()}")

    /** A file for the window to write a saved document into, named like the document. */
    fun saveFile(uri: Uri): File {
        val folder = File(context.cacheDir, "saves/${System.nanoTime()}").apply { mkdirs() }
        return File(folder, displayName(uri) ?: "document")
    }

    /** The document's name as the picker shows it, made safe as a file name. */
    fun displayName(uri: Uri): String? {
        val name = context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst() && !it.isNull(0)) it.getString(0) else null
        } ?: uri.lastPathSegment?.substringAfterLast('/')
        return name?.replace(Regex("[/\\\\:*?\"<>|\\u0000]"), "_")?.trim()?.takeIf { it.isNotEmpty() && it != "." && it != ".." }
    }

    /** Copies the document at `uri` into `folder` (made if needed) under its own name. */
    fun copyDocument(uri: Uri, folder: File): File {
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
     * deletes the file. Null when it worked, else why not.
     */
    @JvmStatic
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
    }
}
