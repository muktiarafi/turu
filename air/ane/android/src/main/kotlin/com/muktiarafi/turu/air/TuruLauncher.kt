package com.muktiarafi.turu.air

import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.os.Bundle
import android.util.Log

/**
 * The app's launcher instead of AIR's `AppEntry` (docs/AIR_PROPOSAL.md, "One APK or two?").
 * In one app with both clients it would start Ruffle's `TuruActivity` or AIR's game by the
 * user's choice; for spike A0 it starts AIR's, to prove ADT lets an extension own the launcher.
 */
class TuruLauncher : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // ADT names AIR's activity `<package>.AIRAppEntry` (the package with or without `air.`).
        val air = ComponentName(packageName, "$packageName.AIRAppEntry")
        Log.i("TuruAir", "launcher: starting ${air.className}")
        startActivity(Intent().setComponent(air).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        finish()
    }
}
