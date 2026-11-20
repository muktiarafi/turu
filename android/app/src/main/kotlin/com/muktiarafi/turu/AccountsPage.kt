package com.muktiarafi.turu

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * An account's row: avatar, its name, the username muted, up to two group or tag chips, and at
 * the end Start or where it is ("In tab 2", "Starting..."). `menu` adds the ⋮ menu (Accounts).
 */
@Composable
fun AccountRow(
    account: Account,
    starting: Boolean,
    onStart: () -> Unit,
    onShow: (Int) -> Unit,
    menu: (@Composable () -> Unit)? = null,
) {
    val labels = account.groups + account.tags
    ListItem(
        modifier = Modifier
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .clickable(enabled = !starting) {
                val tab = account.tab
                if (tab != null) onShow(tab) else onStart()
            },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        leadingContent = { Avatar(account.label, size = 40.dp) },
        headlineContent = { Text(account.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Column {
                if (account.label != account.username) {
                    Text(account.username, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                when {
                    account.tab == null && !account.password -> Text("No password", color = Turu.Warning, maxLines = 1, overflow = TextOverflow.Ellipsis)
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
            }
        },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                val tab = account.tab
                when {
                    tab != null -> TextButton(onClick = { onShow(tab) }) { Text("In tab $tab") }
                    starting -> Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 1.5.dp)
                        Spacer(Modifier.width(8.dp))
                        Text("Starting…", style = MaterialTheme.typography.labelLarge, color = Turu.Muted)
                    }
                    else -> FilledTonalButton(onClick = onStart, modifier = Modifier.heightIn(min = 40.dp)) { Text("Start") }
                }
                menu?.invoke()
            }
        },
    )
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
            .background(Turu.Track, RoundedCornerShape(6.dp))
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

/** Whether the list shows `account` with this filter (a group or tag, or none) and search. */
private fun shows(account: Account, filter: String?, search: String): Boolean {
    val inFilter = filter == null || account.groups.contains(filter) || account.tags.any { it.equals(filter, true) }
    val needle = search.trim()
    val found = needle.isEmpty() ||
        account.username.contains(needle, true) ||
        account.displayName.contains(needle, true) ||
        account.tags.any { it.contains(needle, true) }
    return inFilter && found
}

/** Accounts: search, filter chips, the list with Start and a ⋮ menu each, launch options at the foot. */
@Composable
fun AccountsPage(state: ShellState, accounts: AccountsPanel) {
    var search by remember { mutableStateOf("") }
    var filter by remember { mutableStateOf<String?>(null) }
    var removing by remember { mutableStateOf<Account?>(null) }
    val list = accounts.list
    Column(Modifier.fillMaxSize()) {
        PageBar("Accounts", onBack = { state.back() }) {
            IconButton(onClick = { state.push(Page.AccountForm(null)) }) {
                Icon(Icons.Rounded.Add, contentDescription = "Add an account")
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
            shape = RoundedCornerShape(12.dp),
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
        val shown = list.accounts.filter { shows(it, filter, search) }
        LazyColumn(Modifier.weight(1f).padding(top = 4.dp)) {
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
                    onStart = {
                        accounts.start(account)
                        state.close()
                    },
                    onShow = { state.close() },
                    menu = {
                        AccountMenu(
                            canMoveUp = index > 0,
                            canMoveDown = index < list.accounts.size - 1,
                            onEdit = { state.push(Page.AccountForm(account)) },
                            onMove = { accounts.move(account, it) },
                            onRemove = { removing = account },
                        )
                    },
                )
            }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        LaunchOptions(accounts)
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
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onEdit: () -> Unit,
    onMove: (Int) -> Unit,
    onRemove: () -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Rounded.MoreVert, contentDescription = "More", tint = Turu.Muted)
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

/** Launch options, folded at the foot: the server (with its ping) and the script, with Auto-start. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LaunchOptions(accounts: AccountsPanel) {
    var unfolded by remember { mutableStateOf(false) }
    val list = accounts.list
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .clickable {
                    unfolded = !unfolded
                    if (unfolded) accounts.loadServers()
                }
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text("Launch options", style = MaterialTheme.typography.labelLarge)
                val script = list.script?.substringAfterLast('/')?.removeSuffix(".ts")
                Text(
                    buildString {
                        append(list.server.ifEmpty { "Any server" })
                        append(" · ")
                        append(script ?: "No script")
                        if (script != null && list.autoStart) append(", starts")
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = Turu.Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Icon(
                if (unfolded) Icons.Rounded.KeyboardArrowDown else Icons.Rounded.KeyboardArrowUp,
                contentDescription = if (unfolded) "Fold the launch options" else "Show the launch options",
                tint = Turu.Muted,
            )
        }
        AnimatedVisibility(unfolded) {
            Column(
                Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
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
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.heightIn(min = 48.dp)) {
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
            shape = RoundedCornerShape(12.dp),
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

/** Add or Edit: username, password (to the Keystore), display name, tags. */
@Composable
fun AccountFormPage(state: ShellState, accounts: AccountsPanel, original: Account?) {
    var username by remember { mutableStateOf(original?.username ?: "") }
    var password by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }
    var displayName by remember { mutableStateOf(original?.displayName ?: "") }
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
                    accounts.save(original, username, password, displayName, tags.split(',')) { failed ->
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
                value = username,
                onValueChange = { username = it },
                label = { Text("Username") },
                singleLine = true,
                isError = taken != null,
                supportingText = taken?.let { { Text("${it.label} is already in the list") } },
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, imeAction = ImeAction.Next, autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            if (taken != null) {
                TextButton(onClick = { state.back(); state.push(Page.AccountForm(taken)) }, modifier = Modifier.padding(top = 0.dp)) {
                    Text("Edit it")
                }
            }
            if (accounts.list.keepsPasswords) {
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text("Password") },
                    placeholder = if (original?.password == true) {
                        { Text("Saved; type to change it") }
                    } else {
                        null
                    },
                    singleLine = true,
                    visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
                    trailingIcon = {
                        TextButton(onClick = { showPassword = !showPassword }) { Text(if (showPassword) "Hide" else "Show") }
                    },
                    supportingText = { Text("Kept in the Keystore, not the accounts file") },
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Next),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            OutlinedTextField(
                value = displayName,
                onValueChange = { displayName = it },
                label = { Text("Display name") },
                placeholder = { Text(username.ifBlank { "Shown instead of the username" }) },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = tags,
                onValueChange = { tags = it },
                label = { Text("Tags") },
                placeholder = { Text("farm, alt") },
                supportingText = { Text("Separated by commas") },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                colors = fieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth(),
            )
            error?.let { Text(it, color = Turu.Danger, style = MaterialTheme.typography.bodyMedium) }
        }
    }
}
