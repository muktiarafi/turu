package com.muktiarafi.turu

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** What the panel's screens ask of the shell. */
class TabActions(
    val select: (Int) -> Unit,
    val close: (Int) -> Unit,
    /** A new tab at the game's login screen. */
    val blank: () -> Unit,
    val accounts: AccountsPanel,
)

/** The panel's width, beside the rail (docs/UI.md, "Tab panel"). */
private val PanelWidth = 320.dp

/**
 * The Tabs item at the top of the rail (64 x 52 dp, plus the cutout's inset): the tab on screen's
 * avatar with its status dot, how many tabs are open, and a warning while another tab waits.
 */
@Composable
fun TabsItem(state: ShellState, onClick: () -> Unit) {
    val tabs = state.tabs
    val active = tabs.active
    val description = buildString {
        append("Tabs, ${tabs.tabs.size} open")
        active?.let { append(", on screen: ${it.label}") }
        if (tabs.othersNeedYou) append(", a tab waits for you")
    }
    Box(
        Modifier
            .width(64.dp + state.cutoutLeftDp.dp)
            .height(60.dp)
            .background(Turu.Panel)
            .padding(start = state.cutoutLeftDp.dp, top = 8.dp)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            Modifier
                .width(56.dp)
                .height(52.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(if (state.open) Turu.AccentTint else Color.Transparent)
                .clickable(onClick = onClick),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Box(contentAlignment = Alignment.Center) {
                Avatar(active?.label ?: "", size = 32.dp, tab = active)
                if (tabs.tabs.size > 1 || tabs.othersNeedYou) {
                    val warn = tabs.othersNeedYou
                    Box(
                        Modifier
                            .align(Alignment.TopEnd)
                            .padding(start = 26.dp)
                            .size(16.dp)
                            .clip(CircleShape)
                            .background(if (warn) Turu.Warning else Turu.Accent),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            if (warn) "!" else "${tabs.tabs.size}",
                            color = if (warn) Turu.Panel else Color.White,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                            lineHeight = 10.sp,
                        )
                    }
                }
            }
            Text(
                "Tabs",
                style = MaterialTheme.typography.labelSmall,
                color = if (state.open) Turu.Text else Turu.Muted,
                modifier = Modifier.padding(top = 2.dp),
            )
        }
    }
}

/** A round avatar with two letters; for a tab, its status dot at the bottom right. */
@Composable
fun Avatar(label: String, size: Dp, tab: ShellTab? = null, tint: Color = Turu.Track) {
    Box(Modifier.size(size)) {
        Box(
            Modifier.fillMaxSize().clip(CircleShape).background(tint),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                initials(label),
                color = Turu.Text,
                fontWeight = FontWeight.SemiBold,
                fontSize = (size.value * 0.36f).sp,
                maxLines = 1,
            )
        }
        if (tab != null) {
            val dot = when {
                tab.down != null -> Turu.Muted
                tab.waits != null -> Turu.Warning
                tab.busy -> Turu.Success
                else -> Turu.Muted
            }
            Box(
                Modifier
                    .align(Alignment.BottomEnd)
                    .size(size * 0.34f)
                    .clip(CircleShape)
                    .background(Turu.Panel)
                    .padding(2.dp)
                    .clip(CircleShape)
                    .background(dot),
            )
        }
    }
}

/**
 * The panel beside the rail, over the dimmed game: the tabs, New tab and Accounts as pages pushed
 * in place. Back, or a tap on the scrim, closes what's on top.
 */
@Composable
fun TabPanel(state: ShellState, actions: TabActions) {
    BackHandler(enabled = state.open) { state.back() }
    Box(Modifier.fillMaxSize()) {
        // The scrim: the game stays in view, dimmed; a tap closes the panel.
        Box(
            Modifier
                .fillMaxSize()
                .background(Color.Black.copy(alpha = 0.48f))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {
                    state.close()
                }
                .semantics { contentDescription = "Close the panel" },
        )
        Surface(
            color = Turu.Panel,
            modifier = Modifier
                .padding(start = 64.dp + state.cutoutLeftDp.dp)
                .width(PanelWidth)
                .fillMaxHeight()
                .border(width = 1.dp, color = Turu.Edge.copy(alpha = 0.6f))
                // The window stays put under the keyboard (adjustNothing): the page shrinks above it.
                .imePadding(),
        ) {
            AnimatedContent(
                targetState = state.page,
                transitionSpec = {
                    val forward = targetState !is Page.Tabs
                    val dir = if (forward) 1 else -1
                    (slideInHorizontally(tween(220)) { it / 6 * dir } + fadeIn(tween(220)))
                        .togetherWith(slideOutHorizontally(tween(160)) { -it / 8 * dir } + fadeOut(tween(120)))
                },
                label = "panel page",
            ) { page ->
                when (page) {
                    Page.Tabs -> TabsPage(state, actions)
                    Page.NewTab -> NewTabPage(state, actions)
                    Page.Accounts -> AccountsPage(state, actions.accounts)
                    is Page.AccountForm -> AccountFormPage(state, actions.accounts, page.original)
                }
            }
        }
    }
}

/** A page's top bar: a back arrow (or none on the first page), the title, actions at the end. */
@Composable
fun PageBar(title: String, onBack: (() -> Unit)?, actions: @Composable () -> Unit = {}) {
    Row(
        Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onBack != null) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = "Back")
            }
        } else {
            Spacer(Modifier.width(12.dp))
        }
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.weight(1f).padding(start = 4.dp),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        actions()
    }
}

/** The status line under a tab's name. */
private fun statusLine(tab: ShellTab): String = when {
    tab.down != null -> "Starting…"
    tab.waits != null -> "Waits for you: ${tab.waits}"
    tab.busy -> "Script running"
    else -> "Idle"
}

@Composable
private fun TabsPage(state: ShellState, actions: TabActions) {
    var closing by remember { mutableStateOf<ShellTab?>(null) }
    val tabs = state.tabs
    Column(Modifier.fillMaxSize()) {
        PageBar("Tabs", onBack = null) {
            Text(
                "${tabs.tabs.size} of 4",
                style = MaterialTheme.typography.labelMedium,
                color = Turu.Muted,
                modifier = Modifier.padding(end = 16.dp),
            )
        }
        LazyColumn(Modifier.weight(1f)) {
            items(tabs.tabs, key = { it.id }) { tab ->
                val selected = tab.id == tabs.shown
                ListItem(
                    modifier = Modifier
                        .padding(horizontal = 8.dp, vertical = 2.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .clickable {
                            actions.select(tab.id)
                            state.close()
                        },
                    colors = ListItemDefaults.colors(
                        containerColor = if (selected) Turu.AccentTint else Color.Transparent,
                    ),
                    leadingContent = { Avatar(tab.label, size = 40.dp, tab = tab) },
                    headlineContent = { Text(tab.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            if (tab.down != null) {
                                CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = Turu.Muted)
                                Spacer(Modifier.width(6.dp))
                            }
                            Text(
                                statusLine(tab),
                                color = if (tab.waits != null) Turu.Warning else Turu.Muted,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    },
                    trailingContent = if (tabs.tabs.size > 1) {
                        {
                            IconButton(onClick = { if (tab.busy) closing = tab else actions.close(tab.id) }) {
                                Icon(Icons.Rounded.Close, contentDescription = "Close ${tab.label}'s tab", tint = Turu.Muted)
                            }
                        }
                    } else {
                        null
                    },
                )
            }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(
            Modifier.fillMaxWidth().padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (tabs.full) {
                Text(
                    "4 tabs at most",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Turu.Muted,
                    modifier = Modifier.weight(1f).padding(start = 4.dp),
                )
            } else {
                FilledTonalButton(
                    onClick = {
                        actions.accounts.reload()
                        state.push(Page.NewTab)
                    },
                    modifier = Modifier.weight(1f).height(48.dp),
                ) {
                    Icon(Icons.Rounded.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("New tab")
                }
            }
            Spacer(Modifier.width(8.dp))
            TextButton(
                onClick = {
                    actions.accounts.reload()
                    state.push(Page.Accounts)
                },
                modifier = Modifier.height(48.dp),
            ) { Text("Accounts") }
        }
    }
    closing?.let { tab ->
        AlertDialog(
            onDismissRequest = { closing = null },
            title = { Text("Close the tab?") },
            text = { Text("${tab.label}'s script is running. Closing the tab stops it.") },
            confirmButton = {
                TextButton(onClick = {
                    actions.close(tab.id)
                    closing = null
                }) { Text("Close tab", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { closing = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** New tab: an account to start in it, or a blank tab at the game's login screen. */
@Composable
private fun NewTabPage(state: ShellState, actions: TabActions) {
    val accounts = actions.accounts
    Column(Modifier.fillMaxSize()) {
        PageBar("New tab", onBack = { state.back() }) {
            TextButton(onClick = { state.push(Page.Accounts) }) { Text("Manage") }
        }
        LazyColumn(Modifier.weight(1f)) {
            if (accounts.loaded && accounts.list.accounts.isEmpty()) {
                item {
                    Text(
                        "No saved accounts. Add one in Accounts, or log in by hand.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                    )
                }
            }
            items(accounts.list.accounts, key = { it.username }) { account ->
                AccountRow(
                    account = account,
                    starting = account.username in accounts.starting,
                    onStart = {
                        accounts.start(account)
                        state.close()
                    },
                    onShow = { tab ->
                        actions.select(tab)
                        state.close()
                    },
                )
            }
            item {
                ListItem(
                    modifier = Modifier
                        .padding(horizontal = 8.dp, vertical = 2.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .clickable {
                            actions.blank()
                            state.close()
                        },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    leadingContent = {
                        Box(
                            Modifier.size(40.dp).clip(CircleShape).background(Turu.Card),
                            contentAlignment = Alignment.Center,
                        ) { Icon(Icons.Rounded.Person, contentDescription = null, tint = Turu.Muted) }
                    },
                    headlineContent = { Text("Blank tab") },
                    supportingContent = { Text("Log in by hand", color = Turu.Muted) },
                )
            }
        }
    }
}
