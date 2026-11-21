package com.muktiarafi.turu

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.CosmeticItemView
import com.muktiarafi.turu.models.CosmeticsView
import com.muktiarafi.turu.models.OutfitView
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import org.json.JSONArray
import org.json.JSONObject

/**
 * Cosmetics for the tab on screen (`turu_frontend::models::cosmetics`): what the game actions
 * report and this page's catalog query are the slot's; the catalog, favorites and outfits are
 * its process's. Read only while the page shows; opening it reads the colors and loads the
 * catalog.
 */
class CosmeticsPanel {
    var view by mutableStateOf<CosmeticsView?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    private var client: ModelClient? = null
    private var following: ShellTab? = null
    private val json = Json { ignoreUnknownKeys = true }

    fun follow(tab: ShellTab?) {
        if (tab?.id == following?.id && tab?.api == following?.api && client != null) return
        following = tab
        client?.stop()
        client = null
        view = null
        val api = tab?.api ?: return
        val token = tab.token ?: return
        client = ModelClient(
            api, token, models = listOf("cosmetics"), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<CosmeticsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/cosmetics doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also {
            it.start()
            it.act("cosmetics", "do", JSONObject().put("action", "open"))
        }
    }

    fun act(action: String, body: JSONObject = JSONObject()) {
        error = null
        client?.act("cosmetics", "do", body.put("action", action)) { error = it }
    }

    fun query(field: String, value: Any) = act("query", JSONObject().put(field, value))
}

private enum class CosmeticsTab(val label: String) { Copy("Copy"), Catalog("Catalog"), Outfits("Outfits"), Colors("Colors") }

private fun hexColor(hex: String): Color? = hex.trim().removePrefix("#").takeIf { it.length == 6 }?.toLongOrNull(16)?.let {
    Color(0xFF000000 or it) // impeccable-disable-line raw-color: the avatar color the box holds
}

/**
 * Plugins > Cosmetics (docs/UI.md, "Native shells"; VibeSkua's Cosmetics plugin): Copy (a room
 * player's look, the target's, a character page's; gender), Catalog (search, category, sort,
 * favorites, Equip), Outfits (save the look, equip or delete one) and Colors (six colors and the
 * themed presets). Everything it changes is client side; Reset to Real Gear in ⋮ puts the real
 * look back. Manual SWF is the full window's.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CosmeticsPage(state: ShellState, panel: CosmeticsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { panel.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { panel.follow(state.tabs.active) }
    val view = panel.view
    var tab by rememberSaveable { mutableStateOf(CosmeticsTab.Copy) }

    Column(Modifier.fillMaxSize()) {
        PageBar("Cosmetics", onBack = { state.back() }) {
            FullWindowMenu("Manual SWF, and the catalog beside its item", fullWindow) { close ->
                DropdownMenuItem(
                    text = {
                        Column {
                            Text("Reset to Real Gear")
                            Text("Your equipped items, colors and gender again", style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                        }
                    },
                    enabled = view?.busy == false,
                    onClick = { close(); panel.act("reset") },
                )
                DropdownMenuItem(
                    text = { Text("Check catalog updates") },
                    enabled = view?.catalog?.busy == false,
                    onClick = { close(); panel.act("check-updates") },
                )
                HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            }
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        // With the keyboard up the segments and the foot step aside, so the field shows.
        val typing = WindowInsets.isImeVisible
        if (!typing) SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) {
            CosmeticsTab.entries.forEachIndexed { i, t ->
                SegmentedButton(
                    selected = tab == t,
                    onClick = { tab = t },
                    shape = SegmentedButtonDefaults.itemShape(i, CosmeticsTab.entries.size),
                    colors = SegmentedButtonDefaults.colors(
                        activeContainerColor = Turu.AccentTint,
                        activeContentColor = Turu.Text,
                        inactiveContainerColor = Color.Transparent,
                        inactiveContentColor = Turu.Muted,
                        activeBorderColor = Turu.Edge,
                        inactiveBorderColor = Turu.Edge,
                    ),
                    icon = {},
                ) { Text(t.label, maxLines = 1) }
            }
        }
        when (tab) {
            CosmeticsTab.Copy -> CopyTab(view, panel, Modifier.weight(1f))
            CosmeticsTab.Catalog -> CatalogTab(view, panel, Modifier.weight(1f))
            CosmeticsTab.Outfits -> OutfitsTab(view, panel, Modifier.weight(1f))
            CosmeticsTab.Colors -> ColorsTab(view, panel, Modifier.weight(1f))
        }
        // The last action's outcome (a spinner while one runs), only when there is one.
        val outcome = panel.error?.let { it to Turu.Danger }
            ?: view.status.takeIf { it.isNotEmpty() }?.let { it to (if ("error" in it) Turu.Danger else Turu.Muted) }
        if (typing || (outcome == null && !view.busy)) return@Column
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(Modifier.fillMaxWidth().heightIn(min = 40.dp).padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            if (view.busy) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                Spacer(Modifier.width(8.dp))
            }
            outcome?.let { (text, color) -> Text(text, style = MaterialTheme.typography.bodySmall, color = color, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
    }
}

@Composable
private fun Label(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = Turu.Muted, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp))
}

/** Copy a look: a room player's, the target's, a character page's; then gender. */
@Composable
private fun CopyTab(view: CosmeticsView, panel: CosmeticsPanel, modifier: Modifier) {
    var player by rememberSaveable { mutableStateOf("") }
    var character by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(view.players) { if (player !in view.players) player = view.players.firstOrNull() ?: "" }
    LazyColumn(modifier) {
        item {
            Label("A player in this room")
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) {
                    Picker(
                        label = "Player",
                        value = player,
                        options = view.players.zip(view.playerLabels),
                        loading = false,
                        empty = "Nobody else is in this room",
                        none = "Nobody else here",
                        onOpen = { panel.act("players") },
                    ) { player = it }
                }
                Spacer(Modifier.width(8.dp))
                OutlinedButton(onClick = { panel.act("copy-player", JSONObject().put("name", player)) }, enabled = !view.busy && player.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Copy") }
            }
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                FilledTonalButton(onClick = { panel.act("copy-target") }, enabled = !view.busy, modifier = Modifier.heightIn(min = 48.dp)) { Text("Copy my target's look") }
            }
            Label("Any character, from its character page")
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                val copy = { if (character.isNotBlank() && !view.busy) panel.act("copy-page", JSONObject().put("name", character.trim())) }
                OutlinedTextField(
                    value = character,
                    onValueChange = { character = it },
                    placeholder = { Text("Character name") },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { copy() }),
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                OutlinedButton(onClick = copy, enabled = !view.busy && character.isNotBlank(), modifier = Modifier.heightIn(min = 48.dp)) { Text("Copy") }
            }
            Text(
                "Its gear, gender and colors as account.aq.com shows them.",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
            Label("Gender")
            Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for ((label, g) in listOf("Female" to "F", "Male" to "M", "Swap" to "")) {
                    OutlinedButton(onClick = { panel.act("gender", JSONObject().put("gender", g)) }, modifier = Modifier.heightIn(min = 48.dp)) { Text(label) }
                }
            }
            Text(
                "Only you see these changes. Reset to Real Gear in ⋮ puts your own look back.",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 12.dp),
            )
        }
    }
}

/**
 * The catalog: search, category chips, kind and sort scroll away with the items (a landscape
 * phone has no height to pin them), then the page's items with a heart and Equip; the count and
 * the page arrows stay at the foot.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CatalogTab(view: CosmeticsView, panel: CosmeticsPanel, modifier: Modifier) {
    val c = view.catalog
    var query by remember { mutableStateOf(c.query) }
    LaunchedEffect(query) {
        if (query != c.query) {
            delay(350)
            panel.query("query", query)
        }
    }
    Column(modifier) {
        LazyColumn(Modifier.weight(1f)) {
            item(key = "search") {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text("Search a name or an ID") },
                    leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null, tint = Turu.Muted) },
                    trailingIcon = { if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear the search") } },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
                )
            }
            item(key = "chips") {
                LazyRow(contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(c.categories) { cat ->
                        FilterChip(
                            selected = cat == c.category,
                            onClick = { panel.query("category", cat) },
                            label = { Text(cat) },
                            leadingIcon = if (cat == "Favorites") { { Icon(Icons.Rounded.Favorite, contentDescription = null, modifier = Modifier.size(16.dp)) } } else null,
                            colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text),
                            modifier = Modifier.heightIn(min = 48.dp),
                        )
                    }
                }
            }
            item(key = "sort") {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (c.category == "Weapon") {
                        Box(Modifier.weight(1f)) {
                            Picker(label = "Kind", value = c.weaponKind, options = listOf("" to "All kinds") + c.weaponKinds.map { it to it }, loading = false, none = "All kinds") {
                                panel.query("weaponKind", it)
                            }
                        }
                    }
                    Box(Modifier.weight(1f)) {
                        Picker(label = "Sort", value = c.sort, options = c.sorts.map { it to it }, loading = false) { panel.query("sort", it) }
                    }
                }
            }
            if (c.busy) {
                item(key = "busy") {
                    Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(8.dp))
                        Text(c.status, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                    }
                }
            } else if (c.items.isEmpty()) {
                item(key = "empty") {
                    Text(
                        if (c.category == "Favorites") "No favorites yet: the heart on an item keeps it here." else "Nothing matches. Try another name, or All.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Turu.Muted,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
            items(c.items, key = { it.key }) { ItemRow(it, view.busy, panel) }
        }
        HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            val first = c.page.toLong() * 50 + 1
            val last = minOf(first + c.items.size - 1, c.matches.toLong())
            Text(
                if (c.matches == 0u) c.status else "%,d to %,d of %,d".format(first, last, c.matches.toLong()),
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).padding(start = 8.dp),
            )
            IconButton(onClick = { panel.query("page", (c.page - 1u).toLong()) }, enabled = c.page > 0u) {
                Icon(Icons.AutoMirrored.Rounded.KeyboardArrowLeft, contentDescription = "Previous page")
            }
            IconButton(onClick = { panel.query("page", (c.page + 1u).toLong()) }, enabled = c.page + 1u < c.pages) {
                Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = "Next page")
            }
        }
    }
}

@Composable
private fun ItemRow(item: CosmeticItemView, busy: Boolean, panel: CosmeticsPanel) {
    val at = JSONObject().put("key", item.key)
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(item.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${item.category} · #${item.id}", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        IconButton(onClick = { panel.act("favorite", JSONObject().put("key", item.key)) }) {
            Icon(
                if (item.favorite) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder,
                contentDescription = if (item.favorite) "Remove ${item.name} from favorites" else "Add ${item.name} to favorites",
                tint = if (item.favorite) Turu.Danger else Turu.Muted,
            )
        }
        TextButton(onClick = { panel.act("equip", at) }, enabled = !busy, modifier = Modifier.heightIn(min = 48.dp)) { Text("Equip") }
    }
}

/** Saved outfits: Save current look, then each with what it holds, Equip and ⋮ Delete. */
@Composable
private fun OutfitsTab(view: CosmeticsView, panel: CosmeticsPanel, modifier: Modifier) {
    var saving by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf<OutfitView?>(null) }
    Column(modifier) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("${view.outfits.size} saved ${if (view.outfits.size == 1) "outfit" else "outfits"}", style = MaterialTheme.typography.bodySmall, color = Turu.Muted, modifier = Modifier.weight(1f))
            Button(onClick = { saving = true }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Save current look") }
        }
        LazyColumn(Modifier.weight(1f)) {
            if (view.outfits.isEmpty()) {
                item {
                    Text("No outfits yet. Put a look together (copy one, equip items), then Save current look keeps it under a name.", style = MaterialTheme.typography.bodyMedium, color = Turu.Muted, modifier = Modifier.padding(16.dp))
                }
            }
            items(view.outfits, key = { "${it.position}:${it.name}" }) { o -> OutfitRow(o, panel) { deleting = o } }
        }
    }
    if (saving) {
        NameDialog(title = "Save current look", initial = "My outfit", confirm = "Save", onDismiss = { saving = false }) { name ->
            saving = false
            panel.act("save-look", JSONObject().put("name", name))
        }
    }
    deleting?.let { o ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text("Delete this outfit?") },
            text = { Text("${o.name}. The items stay in the catalog.") },
            confirmButton = { TextButton(onClick = { deleting = null; panel.act("delete-set", JSONObject().put("index", o.position.toLong()).put("name", o.name)) }) { Text("Delete", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
}

@Composable
private fun OutfitRow(o: OutfitView, panel: CosmeticsPanel, onDelete: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(o.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                (if (o.slots.isEmpty()) "Empty" else o.slots.joinToString(" · ")) + if (o.created.isNotEmpty()) " · ${o.created}" else "",
                style = MaterialTheme.typography.bodySmall,
                color = Turu.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        TextButton(onClick = { panel.act("equip-set", JSONObject().put("index", o.position.toLong()).put("name", o.name)) }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Equip") }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "${o.name}: delete", tint = Turu.Muted) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = Turu.Card) {
                DropdownMenuItem(text = { Text("Delete…", color = Turu.Danger) }, onClick = { menu = false; onDelete() })
            }
        }
    }
    HorizontalDivider(color = Turu.Edge.copy(alpha = 0.4f), modifier = Modifier.padding(horizontal = 16.dp))
}

/** Six colors (a swatch and its hex), Apply and Read from avatar, then the themed presets. */
@Composable
private fun ColorsTab(view: CosmeticsView, panel: CosmeticsPanel, modifier: Modifier) {
    val hex = remember { mutableStateListOf("", "", "", "", "", "") }
    // A read (opening, Read from avatar, a copied look) fills the boxes, once per read.
    var seen by remember { mutableStateOf<UInt?>(null) }
    LaunchedEffect(view.colorsCount) {
        if (view.colors.size == 6 && view.colorsCount != seen) {
            view.colors.forEachIndexed { i, h -> hex[i] = h }
        }
        seen = view.colorsCount
    }
    val apply = { panel.act("colors", JSONObject().put("hex", JSONArray(hex.toList()))) }
    LazyColumn(modifier) {
        itemsIndexed(view.colorRows) { i, row ->
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                val swatch = hexColor(hex[i])
                Box(
                    Modifier
                        .size(28.dp)
                        .clip(CircleShape)
                        .background(swatch ?: Color.Transparent)
                        .border(1.dp, Turu.Edge, CircleShape),
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(row.label)
                    Text(row.detail, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
                OutlinedTextField(
                    value = hex[i],
                    onValueChange = { hex[i] = it.take(7) },
                    placeholder = { Text("#RRGGBB") },
                    singleLine = true,
                    isError = hex[i].isNotBlank() && swatch == null,
                    textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    modifier = Modifier.width(130.dp),
                )
            }
        }
        item {
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = apply, enabled = hex.all { it.isBlank() || hexColor(it) != null }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Apply colors") }
                OutlinedButton(onClick = { panel.act("read-colors") }, modifier = Modifier.heightIn(min = 48.dp)) { Text("Read from avatar") }
            }
            if (view.ccStatus.isNotEmpty()) {
                Text(view.ccStatus, style = MaterialTheme.typography.bodySmall, color = Turu.Success, modifier = Modifier.padding(horizontal = 16.dp))
            }
            Label("Themed presets · a tap applies one (skin and hair stay)")
        }
        items(view.presets, key = { it.name }) { p ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .clickable {
                        p.colors.forEachIndexed { i, h -> hex[i] = h }
                        apply()
                    }
                    .heightIn(min = 56.dp)
                    .padding(horizontal = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    for (h in p.colors) {
                        Box(Modifier.size(18.dp).clip(CircleShape).background(hexColor(h) ?: Color.Transparent).border(1.dp, Turu.Edge, CircleShape))
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(p.name)
                    Text(p.detail, style = MaterialTheme.typography.bodySmall, color = Turu.Muted)
                }
            }
        }
        item { Spacer(Modifier.padding(bottom = 12.dp)) }
    }
}
