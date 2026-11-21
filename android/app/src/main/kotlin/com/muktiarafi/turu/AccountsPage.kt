package com.muktiarafi.turu

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.OutputTransformation
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextObfuscationMode
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedSecureTextField
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.dp

/**
 * An account's row: avatar, its name, the username muted, up to two group or tag chips, and at
 * the end Start or where it is ("On screen", Switch to its tab, "Starting..."; not its tab's
 * number: that's a slot, which the tab list doesn't show). `shown` is the tab on screen. `menu`
 * adds the ⋮ menu (Accounts).
 * With `select` (Accounts) a tap checks it, as Army Control's rows (a long press too): only Start
 * starts; while `selecting`, its avatar turns to a check while selected, and Start and the menu
 * step aside. Without `select` (New tab) a tap starts it. `failed` is why it didn't start;
 * `onAddPassword` makes "No password" a link to the account's form. At large text the avatar
 * steps aside, so the name and its warning keep their words in the panel.
 */
@Composable
fun AccountRow(
    account: Account,
    starting: Boolean,
    onStart: () -> Unit,
    onShow: (Int) -> Unit,
    menu: (@Composable () -> Unit)? = null,
    select: (() -> Unit)? = null,
    selecting: Boolean = false,
    selected: Boolean = false,
    shown: Int? = null,
    failed: String? = null,
    onAddPassword: (() -> Unit)? = null,
) {
    val labels = account.groups + account.tags
    val tap = {
        val tab = account.tab
        when {
            select != null -> select()
            tab != null -> onShow(tab)
            else -> onStart()
        }
    }
    val avatar = selected || LocalDensity.current.fontScale < 1.15f
    ListItem(
        modifier = Modifier
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(MaterialTheme.shapes.medium)
            .background(selectionTint(selected))
            .then(
                if (select != null) {
                    Modifier.combinedClickable(
                        onClickLabel = if (selected) "Clear" else "Select",
                        onLongClickLabel = "Select",
                        onLongClick = select,
                        onClick = tap,
                    )
                } else {
                    Modifier.clickable(enabled = !starting, onClick = tap)
                },
            ),
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        leadingContent = if (!avatar) {
            null
        } else {
            {
                if (selected) {
                    Box(Modifier.size(40.dp).clip(CircleShape).background(Turu.Accent), contentAlignment = Alignment.Center) {
                        Icon(Icons.Rounded.Check, contentDescription = "Selected", tint = Color.White)
                    }
                } else {
                    Avatar(account.label, size = 40.dp)
                }
            }
        },
        supportingContent = {
            Column {
                if (account.label != account.username && !account.masked) {
                    Text(account.username, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                when {
                    // Starts all the same: the tab opens at the login screen, typed by hand.
                    account.tab == null && !account.password && onAddPassword != null -> Text(
                        buildAnnotatedString {
                            // The link on a line of its own: the panel is narrow beside Start.
                            append("No password\n")
                            withLink(
                                LinkAnnotation.Clickable(
                                    "add-password",
                                    TextLinkStyles(SpanStyle(fontWeight = FontWeight.SemiBold, textDecoration = TextDecoration.Underline)),
                                ) { onAddPassword() },
                            ) { append("Add one") }
                        },
                        color = Turu.Warning,
                        maxLines = 2,
                    )
                    account.tab == null && !account.password -> Text("No password", color = Turu.Warning, maxLines = 2)
                    labels.isNotEmpty() -> Row(
                        Modifier.padding(top = 4.dp),
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        for (label in labels.take(2)) TagChip(label)
                        if (labels.size > 2) {
                            Text("+${labels.size - 2}", style = MaterialTheme.typography.labelSmall, color = Turu.Muted)
                        }
                    }
                }
                failed?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = Turu.Danger, maxLines = 2, overflow = TextOverflow.Ellipsis) }
            }
        },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                val tab = account.tab
                when {
                    selecting -> if (tab != null) Text("In a tab", style = MaterialTheme.typography.labelLarge, color = Turu.Muted)
                    tab != null && tab == shown -> Text(
                        "On screen",
                        style = MaterialTheme.typography.labelLarge,
                        color = Turu.Muted,
                        modifier = Modifier.padding(horizontal = 12.dp),
                    )
                    tab != null -> TextButton(onClick = { onShow(tab) }) { Text("Switch to") }
                    starting -> Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 1.5.dp)
                        Spacer(Modifier.width(8.dp))
                        Text("Starting…", style = MaterialTheme.typography.labelLarge, color = Turu.Muted)
                    }
                    // Narrower than the default (24 dp a side), so "No password" keeps its words at 1.3x text.
                    else -> FilledTonalButton(
                        onClick = onStart,
                        modifier = Modifier.heightIn(min = 40.dp),
                        contentPadding = PaddingValues(horizontal = 16.dp),
                    ) { Text("Start") }
                }
                if (!selecting) menu?.invoke()
            }
        },
    ) { Text(account.label, maxLines = 1, overflow = TextOverflow.Ellipsis) }
}

@Composable
private fun TagChip(label: String) {
    Text(
        label,
        style = MaterialTheme.typography.labelSmall,
        color = Turu.Text,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier
            .background(Turu.Track, MaterialTheme.shapes.small)
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

/** Whether the list shows `account` with this filter (a group or tag, or none) and search. */
private fun shows(account: Account, filter: String?, search: String): Boolean {
    val inFilter = filter == null || account.groups.contains(filter) || account.tags.any { it.equals(filter, true) }
    val needle = search.trim()
    val found = needle.isEmpty() ||
        account.label.contains(needle, true) ||
        account.username.contains(needle, true) ||
        account.displayName.contains(needle, true) ||
        account.tags.any { it.contains(needle, true) }
    return inFilter && found
}

/**
 * Accounts: search, filter chips, the list with Start and a ⋮ menu each, launch options at the
 * foot. A long press selects (docs/UI.md, "Accounts"): the bar then says how many, with Start
 * (as many as there are free tabs, asking first when that's fewer), Add to group, and in ⋮
 * Select all and, under a group's chip, Remove from the group. ⋮ also removes the group shown.
 */
@Composable
fun AccountsPage(state: ShellState, accounts: AccountsPanel, select: (Int) -> Unit, fullWindow: () -> Unit) {
    var search by remember { mutableStateOf("") }
    var filter by remember { mutableStateOf<String?>(null) }
    var removing by remember { mutableStateOf<Account?>(null) }
    // The selected accounts' usernames: a long press starts selecting, ✕ or Back ends it.
    var selected by remember { mutableStateOf(setOf<String>()) }
    var grouping by remember { mutableStateOf(false) }
    var startingMany by remember { mutableStateOf<List<Account>?>(null) }
    var droppingGroup by remember { mutableStateOf<String?>(null) }
    val list = accounts.list
    // Read again on coming back (Search Scripts may have set the launch script).
    LaunchedEffect(Unit) { accounts.reload() }
    val shown = list.accounts.filter { shows(it, filter, search) }
    // Accounts that left the list can't stay selected.
    LaunchedEffect(list) { selected = selected.filterTo(mutableSetOf()) { u -> list.accounts.any { it.username == u } } }
    val selecting = selected.isNotEmpty()
    BackHandler(enabled = selecting) { selected = emptySet() }
    val picked = list.accounts.filter { it.username in selected }
    val group = filter?.takeIf { it in list.groups }
    fun toggle(account: Account) {
        selected = if (account.username in selected) selected - account.username else selected + account.username
    }
    // A tab opening (or closing) changes the rows: "Starting…" turns to "Switch to".
    LaunchedEffect(state.tabs.tabs.size) { accounts.reload() }
    Column(Modifier.fillMaxSize()) {
        if (selecting) {
            SelectionBar(
                count = picked.size,
                canStart = picked.any { it.tab == null },
                group = group,
                onClose = { selected = emptySet() },
                onStart = { startingMany = picked.filter { it.tab == null } },
                onGroup = { grouping = true },
                onSelectAll = { selected = selected + shown.map { it.username } },
                onRemoveFromGroup = { g -> accounts.removeFromGroup(g, picked); selected = emptySet() },
            )
        } else {
            PageBar("Accounts", onBack = { state.back() }) {
                IconButton(onClick = { state.push(Page.AccountForm(null)) }) {
                    Icon(Icons.Rounded.Add, contentDescription = "Add an account")
                }
                FullWindowMenu("The same list, laid out as VibeSkua's", fullWindow) { close ->
                    if (group != null) {
                        DropdownMenuItem(
                            text = { Text("Remove the group $group…", color = Turu.Danger) },
                            onClick = { close(); droppingGroup = group },
                        )
                        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
                    }
                }
            }
        }
        OutlinedTextField(
            value = search,
            onValueChange = { search = it },
            placeholder = { Text("Search accounts") },
            leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
            trailingIcon = if (search.isNotEmpty()) {
                { IconButton(onClick = { search = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } }
            } else {
                null
            },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
        )
        val filters = list.groups + list.tags.filter { tag -> list.groups.none { it.equals(tag, true) } }
        if (filters.isNotEmpty()) {
            LazyRow(
                Modifier.padding(top = 8.dp),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                item { Filter("All", filter == null) { filter = null } }
                items(filters) { name -> Filter(name, filter == name) { filter = if (filter == name) null else name } }
            }
        }
        LaunchRow(list) { state.push(Page.LaunchOptions) }
        LazyColumn(Modifier.weight(1f)) {
            if (shown.isNotEmpty()) {
                item(key = "header") {
                    ListHeader(
                        shown = shown,
                        selected = selected,
                        tabs = state.tabs.tabs.size,
                        limit = state.tabs.limit,
                        group = group.takeIf { !selecting },
                        onAll = { all -> selected = if (all) selected + shown.map { it.username } else selected - shown.map { it.username }.toSet() },
                        onStartGroup = { startingMany = shown.filter { it.tab == null } },
                    )
                }
            }
            if (accounts.loaded && shown.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(20.dp)) {
                        Text(
                            if (list.accounts.isEmpty()) "No accounts yet" else "No accounts match",
                            style = MaterialTheme.typography.titleSmall,
                        )
                        Text(
                            if (list.accounts.isEmpty()) {
                                "Add one to start it in a tab with a tap."
                            } else {
                                "Clear the search or pick All."
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            color = Turu.Muted,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                        if (list.accounts.isEmpty()) {
                            FilledTonalButton(
                                onClick = { state.push(Page.AccountForm(null)) },
                                modifier = Modifier.padding(top = 12.dp).height(48.dp),
                            ) { Text("Add account") }
                        }
                    }
                }
            }
            items(shown, key = { it.username }) { account ->
                val index = list.accounts.indexOf(account)
                AccountRow(
                    account = account,
                    starting = account.username in accounts.starting,
                    // Through the tab check, as several are; the panel stays to show how it went.
                    onStart = { startingMany = listOf(account) },
                    onShow = { tab ->
                        select(tab)
                        state.close()
                    },
                    menu = {
                        AccountMenu(
                            label = account.label,
                            canMoveUp = index > 0,
                            canMoveDown = index < list.accounts.size - 1,
                            onEdit = { state.push(Page.AccountForm(account)) },
                            onMove = { accounts.move(account, it) },
                            onRemove = { removing = account },
                        )
                    },
                    select = { toggle(account) },
                    selecting = selecting,
                    selected = account.username in selected,
                    shown = state.tabs.shown,
                    failed = accounts.failed[account.username],
                    onAddPassword = { state.push(Page.AccountForm(account)) }.takeIf { list.keepsPasswords },
                )
            }
        }
    }
    removing?.let { account ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text("Remove ${account.label}?") },
            text = { Text("The account leaves the list, and its saved password goes with it.") },
            confirmButton = {
                TextButton(onClick = {
                    accounts.remove(account)
                    removing = null
                }) { Text("Remove", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    startingMany?.let { wanted ->
        val tabs = state.tabs
        val free = tabs.free
        val go = { start: List<Account> ->
            startingMany = null
            selected = emptySet()
            accounts.startMany(start)
        }
        if (wanted.size <= free) {
            // They all fit: no question.
            LaunchedEffect(wanted) { go(wanted) }
        } else {
            val fits = wanted.take(free)
            val left = wanted.drop(free)
            AlertDialog(
                onDismissRequest = { startingMany = null },
                title = { Text(if (free == 0) "No tab is free" else "Only $free of ${tabs.limit} tabs ${if (free == 1) "is" else "are"} free") },
                text = {
                    Text(
                        when {
                            free > 0 ->
                                "Start ${fits.joinToString(", ") { it.label }}? " +
                                    "${left.joinToString(", ") { it.label }} won't start: Tab Limit is ${tabs.limit}."
                            tabs.raisable ->
                                "${tabs.tabs.size} ${if (tabs.tabs.size == 1) "tab is" else "tabs are"} open, as many as your Tab Limit allows. " +
                                    "Raise it, or close a tab (Tabs) to start ${if (wanted.size == 1) wanted[0].label else "another"}."
                            else ->
                                "${tabs.tabs.size} tabs are open, the most Turu runs at once. Close one (Tabs) to start ${if (wanted.size == 1) wanted[0].label else "another"}."
                        },
                        color = Turu.Muted,
                    )
                },
                confirmButton = {
                    if (free > 0) {
                        TextButton(onClick = { go(fits) }) { Text("Start $free") }
                    } else if (tabs.raisable) {
                        TextButton(onClick = {
                            startingMany = null
                            state.raiseTabLimit()
                        }) { Text("Raise Tab Limit") }
                    }
                },
                dismissButton = {
                    TextButton(onClick = { startingMany = null }) {
                        Text(if (free > 0) "Cancel" else if (tabs.raisable) "Not now" else "OK")
                    }
                },
                containerColor = Turu.Card,
            )
        }
    }
    if (grouping) {
        GroupDialog(
            count = picked.size,
            groups = list.groups,
            onDismiss = { grouping = false },
        ) { name ->
            accounts.addToGroup(name, picked) { failed -> if (failed == null) { grouping = false; selected = emptySet() } }
        }
    }
    droppingGroup?.let { g ->
        AlertDialog(
            onDismissRequest = { droppingGroup = null },
            title = { Text("Remove the group $g?") },
            text = { Text("Its accounts stay in the list; only the group goes.", color = Turu.Muted) },
            confirmButton = {
                TextButton(onClick = {
                    droppingGroup = null
                    filter = null
                    accounts.removeGroup(g)
                }) { Text("Remove", color = Turu.Danger) }
            },
            dismissButton = { TextButton(onClick = { droppingGroup = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

/** The selection's own bar: ✕, how many, Start, and ⋮ with Add to group, Select all and Remove from the group. */
@Composable
private fun SelectionBar(
    count: Int,
    canStart: Boolean,
    group: String?,
    onClose: () -> Unit,
    onStart: () -> Unit,
    onGroup: () -> Unit,
    onSelectAll: () -> Unit,
    onRemoveFromGroup: (String) -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).background(Turu.AccentTint).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onClose) { Icon(Icons.Rounded.Close, contentDescription = "Clear the selection") }
        // The count alone, as Android's selection bars: the panel is 320 dp.
        Text(
            "$count",
            style = MaterialTheme.typography.titleMedium,
            maxLines = 1,
            modifier = Modifier.weight(1f).padding(start = 4.dp).semantics { contentDescription = "$count selected" },
        )
        TextButton(onClick = onStart, enabled = canStart, modifier = Modifier.heightIn(min = 48.dp)) { Text("Start", maxLines = 1) }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "More for the selection") }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                DropdownMenuItem(text = { Text("Add to group…") }, onClick = { menu = false; onGroup() })
                DropdownMenuItem(text = { Text("Select all shown") }, onClick = { menu = false; onSelectAll() })
                if (group != null) {
                    DropdownMenuItem(text = { Text("Remove from $group") }, onClick = { menu = false; onRemoveFromGroup(group) })
                }
            }
        }
    }
}

/**
 * Add to group: a tap on a group puts the selected accounts in it, or a new name (the keyboard's
 * Done adds too: in landscape it covers the dialog's buttons).
 */
@Composable
private fun GroupDialog(count: Int, groups: List<String>, onDismiss: () -> Unit, onAdd: (String) -> Unit) {
    var name by remember { mutableStateOf("") }
    val add = { if (name.isNotBlank()) onAdd(name.trim()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (count == 1) "Add 1 account to a group" else "Add $count accounts to a group") },
        text = {
            Column {
                if (groups.isNotEmpty()) {
                    LazyColumn(Modifier.heightIn(max = 168.dp)) {
                        items(groups) { g ->
                            Text(
                                g,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clip(MaterialTheme.shapes.small)
                                    .clickable { onAdd(g) }
                                    .heightIn(min = 48.dp)
                                    .padding(horizontal = 8.dp, vertical = 14.dp),
                            )
                        }
                    }
                    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f), modifier = Modifier.padding(vertical = 8.dp))
                }
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("New group") },
                    singleLine = true,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { add() }),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = { TextButton(onClick = add, enabled = name.isNotBlank()) { Text("Add") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
        containerColor = Turu.Card,
    )
}

@Composable
private fun Filter(label: String, selected: Boolean, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label, maxLines = 1) },
        colors = FilterChipDefaults.filterChipColors(
            selectedContainerColor = Turu.AccentTint,
            selectedLabelColor = Turu.Text,
            labelColor = Turu.Muted,
        ),
        border = FilterChipDefaults.filterChipBorder(
            enabled = true,
            selected = selected,
            borderColor = Turu.Edge,
            selectedBorderColor = Turu.Accent.copy(alpha = 0.6f),
        ),
        modifier = Modifier.height(40.dp),
    )
}

@Composable
private fun AccountMenu(
    label: String,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onEdit: () -> Unit,
    onMove: (Int) -> Unit,
    onRemove: () -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Rounded.MoreVert, contentDescription = "More for $label", tint = Turu.Muted)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            DropdownMenuItem(text = { Text("Edit") }, onClick = { open = false; onEdit() })
            DropdownMenuItem(
                text = { Text("Move up") },
                leadingIcon = { Icon(Icons.Rounded.KeyboardArrowUp, contentDescription = null) },
                enabled = canMoveUp,
                onClick = { open = false; onMove(-1) },
            )
            DropdownMenuItem(
                text = { Text("Move down") },
                leadingIcon = { Icon(Icons.Rounded.KeyboardArrowDown, contentDescription = null) },
                enabled = canMoveDown,
                onClick = { open = false; onMove(1) },
            )
            DropdownMenuItem(text = { Text("Remove…", color = Turu.Danger) }, onClick = { open = false; onRemove() })
        }
    }
}

/** What every Start uses, in a line above the list ("Starts on Twig · runs Gold farm"); a tap opens Launch options. */
@Composable
private fun LaunchRow(list: AccountList, open: () -> Unit) {
    val script = list.script?.substringAfterLast('/')?.removeSuffix(".ts")
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 4.dp)
            .clip(MaterialTheme.shapes.medium)
            .clickable(onClickLabel = "change the launch options", onClick = open)
            .heightIn(min = 48.dp)
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // One line, so the rows keep the panel's height: the script's name is what gets cut.
        Text(
            buildString {
                append("Starts on ")
                append(list.server.ifEmpty { "any server" })
                if (script != null) append(if (list.autoStart) " · runs " else " · loads ").append(script)
            },
            style = MaterialTheme.typography.bodyMedium,
            color = Turu.Muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = Turu.Muted)
    }
}

/**
 * Above the rows: check all shown or none, how many accounts and tabs, and with a group shown,
 * Start it (its accounts without a tab, through the tab check).
 */
@Composable
private fun ListHeader(
    shown: List<Account>,
    selected: Set<String>,
    tabs: Int,
    limit: Int,
    group: String?,
    onAll: (Boolean) -> Unit,
    onStartGroup: () -> Unit,
) {
    val checked = shown.count { it.username in selected }
    val box = when {
        checked == 0 -> ToggleableState.Off
        checked == shown.size -> ToggleableState.On
        else -> ToggleableState.Indeterminate
    }
    Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        TriStateCheckbox(
            state = box,
            onClick = { onAll(box == ToggleableState.Off) },
            modifier = Modifier.semantics { contentDescription = if (box == ToggleableState.Off) "Select all shown" else "Clear the selection" },
        )
        Text(
            buildString {
                append(if (checked > 0) "$checked of ${shown.size} selected" else if (shown.size == 1) "1 account" else "${shown.size} accounts")
                append(" · $tabs of $limit tabs open")
            },
            style = MaterialTheme.typography.labelLarge,
            color = if (checked > 0) Turu.Text else Turu.Muted,
            maxLines = 2,
            modifier = Modifier.weight(1f),
        )
        val idle = shown.count { it.tab == null }
        if (group != null && idle > 0) {
            TextButton(onClick = onStartGroup, modifier = Modifier.heightIn(min = 48.dp)) {
                Text(if (idle == 1) "Start 1" else "Start $idle", maxLines = 1)
            }
        }
    }
}

/**
 * Accounts > Launch options: the server (with its ping) and the script, with Auto-start, that
 * every Start uses. The script is a favourite or recent one, or any from Search scripts…
 * (Search Scripts, picking).
 */
@Composable
fun LaunchOptionsPage(state: ShellState, accounts: AccountsPanel) {
    LaunchedEffect(Unit) {
        accounts.reload()
        accounts.loadServers()
    }
    val list = accounts.list
    Column(Modifier.fillMaxSize()) {
        PageBar("Launch options", onBack = { state.back() })
        Column(
            Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(start = 12.dp, end = 12.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                "Every Start in Accounts and New tab uses these.",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 4.dp),
            )
            Picker(
                label = "Server",
                value = list.server,
                options = accounts.servers?.map { it.name to it.line }
                    ?: listOf(list.server to (accounts.serversError?.let { "${list.server} (couldn't list the servers)" } ?: list.server)),
                loading = accounts.servers == null && accounts.serversError == null,
            ) { accounts.launch(it, list.script, list.autoStart) }
            Picker(
                label = "Script",
                value = list.script ?: "",
                options = listOf("" to "No script") +
                    (list.scripts + listOfNotNull(list.script)).distinct().map { it to it.substringAfterLast('/').removeSuffix(".ts") },
                loading = false,
            ) { accounts.launch(list.server, it.ifEmpty { null }, list.autoStart) }
            OutlinedButton(
                onClick = {
                    state.scriptPick = ScriptPick.Launch
                    state.push(Page.Search)
                },
                modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
            ) {
                Icon(Icons.Rounded.Search, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text("Search scripts…")
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .heightIn(min = 48.dp)
                    .clickable(enabled = list.script != null) { accounts.launch(list.server, list.script, !list.autoStart) }
                    .padding(horizontal = 4.dp),
            ) {
                Text(
                    "Start the script once logged in",
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (list.script == null) Turu.Muted else Turu.Text,
                    modifier = Modifier.weight(1f),
                )
                Switch(
                    checked = list.autoStart,
                    enabled = list.script != null,
                    onCheckedChange = { accounts.launch(list.server, list.script, it) },
                )
            }
        }
    }
}

/** A drop-down field: `options` are (value, what the menu shows). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Picker(
    label: String,
    value: String,
    options: List<Pair<String, String>>,
    loading: Boolean,
    onOpen: () -> Unit = {},
    empty: String? = null,
    /** What it says while nothing is chosen. */
    none: String = "None",
    onPick: (String) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val shown = options.firstOrNull { it.first == value }?.second?.substringBefore(" - ") ?: value
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = {
        if (it && !open) onOpen()
        open = it
    }) {
        OutlinedTextField(
            value = shown.ifEmpty { none },
            onValueChange = {},
            readOnly = true,
            label = { Text(label) },
            trailingIcon = {
                if (loading) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else ExposedDropdownMenuDefaults.TrailingIcon(expanded = open)
            },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            colors = fieldColors(),
            modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }, containerColor = Turu.Card) {
            if (options.isEmpty() && empty != null) {
                DropdownMenuItem(text = { Text(empty, color = Turu.Muted) }, onClick = {}, enabled = false)
            }
            for ((option, text) in options) {
                DropdownMenuItem(
                    text = { Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    onClick = {
                        open = false
                        if (option != value) onPick(option)
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

@Composable
fun fieldColors() = OutlinedTextFieldDefaults.colors(
    focusedBorderColor = Turu.Accent,
    unfocusedBorderColor = Turu.Edge,
    focusedContainerColor = Turu.Inset,
    unfocusedContainerColor = Turu.Inset,
    cursorColor = Turu.Accent,
    focusedLabelColor = Turu.Accent,
    unfocusedLabelColor = Turu.Muted,
    unfocusedPlaceholderColor = Turu.Muted,
    focusedPlaceholderColor = Turu.Muted,
)

/**
 * Streamer Mode's mask for the name fields: a dot per letter, as a password field shows. One
 * replacement per letter, so the caret can still sit between letters (a whole-range replacement
 * maps as one block).
 */
private val MaskNames = OutputTransformation {
    for (i in 0 until length) replace(i, i + 1, "\u2022")
}

/** Add or Edit: username, password (to the Keystore), display name, tags. */
@Composable
fun AccountFormPage(state: ShellState, accounts: AccountsPanel, original: Account?) {
    val usernameField = rememberTextFieldState(original?.username ?: "")
    val username = usernameField.text.toString()
    val passwordField = rememberTextFieldState()
    var showPassword by remember { mutableStateOf(false) }
    val displayNameField = rememberTextFieldState(original?.displayName ?: "")
    val displayName = displayNameField.text.toString()
    // Streamer Mode: the names are masked like the password until Show.
    var showNames by remember { mutableStateOf(!accounts.list.streamer) }
    val names = if (showNames) null else MaskNames
    // A masked field also keeps the keyboard's suggestion strip from repeating what's typed.
    val namesKeyboard = if (showNames) KeyboardType.Ascii else KeyboardType.Password
    val namesToggle: @Composable () -> Unit = {
        TextButton(onClick = { showNames = !showNames }) { Text(if (showNames) "Hide" else "Show") }
    }
    var tags by remember { mutableStateOf(original?.tags?.joinToString(", ") ?: "") }
    var error by remember { mutableStateOf<String?>(null) }
    var saving by remember { mutableStateOf(false) }
    val taken = accounts.list.accounts.firstOrNull {
        it.username.equals(username.trim(), true) && !it.username.equals(original?.username, true)
    }
    LaunchedEffect(username) { error = null }
    Column(Modifier.fillMaxSize()) {
        // Add/Save in the bar: with the keyboard up (landscape) the page has room for a field or two.
        PageBar(if (original == null) "Add account" else "Edit account", onBack = { state.back() }) {
            Button(
                enabled = username.isNotBlank() && taken == null && !saving,
                onClick = {
                    saving = true
                    accounts.save(original, username, passwordField.text.toString(), displayName, tags.split(',')) { failed ->
                        saving = false
                        if (failed == null) state.back() else error = failed
                    }
                },
                modifier = Modifier.padding(end = 8.dp).heightIn(min = 40.dp),
            ) { Text(if (original == null) "Add" else "Save") }
        }
        Column(
            Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedTextField(
                state = usernameField,
                label = { Text("Username") },
                lineLimits = TextFieldLineLimits.SingleLine,
                isError = taken != null,
                outputTransformation = names,
                trailingIcon = if (accounts.list.streamer) namesToggle else null,
                supportingText = when {
                    taken != null -> { { Text("${taken.label} is already in the list") } }
                    accounts.list.streamer -> { { Text("Hidden while Streamer Mode is on") } }
                    else -> null
                },
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = namesKeyboard, imeAction = ImeAction.Next, autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            if (taken != null) {
                TextButton(onClick = { state.back(); state.push(Page.AccountForm(taken)) }, modifier = Modifier.padding(top = 0.dp)) {
                    Text("Edit it")
                }
            }
            if (accounts.list.keepsPasswords) {
                OutlinedSecureTextField(
                    state = passwordField,
                    label = { Text("Password") },
                    placeholder = if (original?.password == true) {
                        { Text("Saved; type to change it") }
                    } else {
                        null
                    },
                    // Hidden as before (1.5's default, System, would show the last letter typed).
                    textObfuscationMode = if (showPassword) TextObfuscationMode.Visible else TextObfuscationMode.Hidden,
                    trailingIcon = {
                        TextButton(onClick = { showPassword = !showPassword }) { Text(if (showPassword) "Hide" else "Show") }
                    },
                    supportingText = { Text("Kept in the Keystore, not the accounts file") },
                    shape = MaterialTheme.shapes.medium,
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Next),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            OutlinedTextField(
                state = displayNameField,
                label = { Text("Display name") },
                placeholder = { Text(if (showNames) username.ifBlank { "Shown instead of the username" } else "Shown instead of the username") },
                lineLimits = TextFieldLineLimits.SingleLine,
                outputTransformation = names,
                trailingIcon = if (accounts.list.streamer) namesToggle else null,
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = if (showNames) KeyboardOptions(imeAction = ImeAction.Next) else KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Next),
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = tags,
                onValueChange = { tags = it },
                label = { Text("Tags") },
                placeholder = { Text("farm, alt") },
                supportingText = { Text("Separated by commas") },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth(),
            )
            error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodyMedium) }
        }
    }
}
