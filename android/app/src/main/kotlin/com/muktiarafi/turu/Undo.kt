package com.muktiarafi.turu

import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier

/** A snackbar's message, and its Undo (null: a plain notice, gone after a few seconds). */
class Notice(val text: String, val undo: (() -> Unit)? = null)

/**
 * The page's snackbar (Material's transient feedback): what a Clear took, with Undo for a while,
 * or a short notice ("Sent"). A new notice replaces the one showing.
 */
@Composable
fun NoticeHost(notice: Notice?, modifier: Modifier = Modifier, gone: () -> Unit) {
    val host = remember { SnackbarHostState() }
    LaunchedEffect(notice) {
        val n = notice ?: return@LaunchedEffect
        host.currentSnackbarData?.dismiss()
        val result = host.showSnackbar(
            message = n.text,
            actionLabel = if (n.undo != null) "Undo" else null,
            duration = if (n.undo != null) SnackbarDuration.Long else SnackbarDuration.Short,
        )
        if (result == SnackbarResult.ActionPerformed) n.undo?.invoke()
        gone()
    }
    SnackbarHost(host, modifier) { data ->
        Snackbar(data, containerColor = Turu.RowHover, contentColor = Turu.Text, actionColor = Turu.Accent)
    }
}
