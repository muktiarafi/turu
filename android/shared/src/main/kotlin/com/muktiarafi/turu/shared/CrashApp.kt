package com.muktiarafi.turu.shared

/**
 * What [CrashReports] and [CrashUploads] say about the app they run in (Turu's Android app or
 * Turu Mas), handed to both at start.
 *
 * * `name`: the app's name in reports ("Turu", "Turu Mas").
 * * `sentryDsn`: where crash reports go; empty for a build without one (nothing is sent, and the
 *   user isn't asked).
 * * `release`: Sentry's release name before the version ("turu-android", "turu-mas"), as the
 *   release workflow uploads the symbols under.
 * * `client`: a tag that tells the app's reports apart in Sentry's project, which takes both; null
 *   for none.
 * * `memoryHint`: what a report of a low-memory kill suggests.
 */
data class CrashApp(
    val name: String,
    val sentryDsn: String,
    val release: String,
    val client: String?,
    val memoryHint: String,
)
