package com.muktiarafi.turu.air

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Account passwords for the Accounts window (`passwords.rs`, the engine's `SecretStore`), as the
 * Ruffle build's `Storage` keeps them: encrypted with an AES-GCM key that lives in the Android
 * Keystore (it never leaves it), one file per account in the app's internal storage
 * (`noBackupFilesDir/passwords`), out of the shared data folder `adb` reads.
 *
 * `Bridge.nativeStart` looks this class up for the engine's threads, so keep these methods static
 * (`@JvmStatic`) with the same names and parameter types.
 */
object Passwords {
    private const val KEY_ALIAS = "turu-passwords"
    private const val GCM_TAG_BITS = 128

    private lateinit var context: Context

    /** Called before `Bridge.nativeStart`. */
    fun init(context: Context) {
        this.context = context.applicationContext
    }

    private fun folder() = File(context.noBackupFilesDir, "passwords").apply { mkdirs() }

    /** One file per account: its name in hex (any username is a valid file name then). */
    private fun file(name: String) =
        File(folder(), name.toByteArray(Charsets.UTF_8).joinToString("") { "%02x".format(it) })

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
        val file = file(name)
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
        val file = file(name)
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
        file(name).delete()
    }
}
