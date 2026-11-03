package com.muktiarafi.turu;

import android.app.NativeActivity;
import android.os.Bundle;

/**
 * NativeActivity, which runs the Rust side (libturu_android.so), plus the foreground service
 * that keeps the process and its network while the app is in the background.
 */
public class TuruActivity extends NativeActivity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        TuruService.start(this);
    }
}
