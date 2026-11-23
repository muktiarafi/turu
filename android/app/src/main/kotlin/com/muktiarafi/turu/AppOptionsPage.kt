package com.muktiarafi.turu

import android.app.ActivityManager
import android.util.Log
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.AppOptionsAct
import com.muktiarafi.turu.models.AppOptionsView
import com.muktiarafi.turu.models.OptionValue
import com.muktiarafi.turu.models.OptionView
import kotlinx.serialization.json.Json
import kotlin.math.ceil

/**
 * Application Options' copy of the tab on screen (`turu_frontend::models::application_options`),
 * read only while the page shows. Every change is saved at once, as in the egui window.
 */
class AppOptionsPanel {
    var view by mutableStateOf<AppOptionsView?>(null)
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
            api, token, models = listOf(Api.appOptions), logs = emptyList(),
            onView = { _, text ->
                runCatching { json.decodeFromString<AppOptionsView>(text) }
                    .onSuccess { view = it }
                    .onFailure { Log.e(ModelClient.TAG, "/models/app-options doesn't decode: $it") }
            },
            onLog = { _, _, _ -> },
        ).also { it.start() }
    }

    private fun act(action: AppOptionsAct) {
        error = null
        val client = client ?: return
        client.act(Api.appOptions, action) { error = it }
    }

    fun set(option: OptionView, value: Any) = act(AppOptionsAct.set(option.section, option.label, optionValue(value)))

    fun run(option: OptionView) = act(AppOptionsAct.run(option.label))
}

/**
 * Options > Application (docs/UI.md, "Native shells"; VibeSkua's Application Options): the search
 * and the sections as chips, the settings under their headings, each saved as it changes. The Web
 * Monitor's links, QR code and keys open the full window. Clear Flash Cache asks first, and so
 * does raising Tab Limit above [TAB_LIMIT_SAFE] (with this phone's memory beside what the tabs take).
 */
@Composable
fun AppOptionsPage(state: ShellState, options: AppOptionsPanel, fullWindow: () -> Unit) {
    DisposableEffect(Unit) { onDispose { options.follow(null) } }
    LaunchedEffect(state.tabs.active?.id, state.tabs.active?.api) { options.follow(state.tabs.active) }
    val view = options.view
    var search by remember { mutableStateOf("") }
    // Raise Tab Limit: the setting found, also when the page already shows.
    LaunchedEffect(state.optionsFind) {
        if (state.optionsFind.isNotEmpty()) {
            search = state.optionsFind
            state.optionsFind = ""
        }
    }
    var section by remember { mutableStateOf<String?>(null) }
    var clearing by remember { mutableStateOf<OptionView?>(null) }
    // Tab Limit asked above the safe number, and what for; a refusal puts its field back.
    var raising by remember { mutableStateOf<Pair<OptionView, Long>?>(null) }
    var refused by remember { mutableIntStateOf(0) }
    Column(Modifier.fillMaxSize()) {
        PageBar("Application Options", onBack = { state.back() }) {
            FullWindowMenu("The Web Monitor's links, QR code and keys", fullWindow)
        }
        if (view == null) {
            Text("Reading the tab…", color = Turu.Muted, modifier = Modifier.padding(20.dp))
            return@Column
        }
        OutlinedTextField(
            value = search,
            onValueChange = { search = it },
            placeholder = { Text("Search application options") },
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
        val needle = search.trim().lowercase()
        val found = view.options.filter { needle.isEmpty() || it.label.lowercase().contains(needle) || it.description.lowercase().contains(needle) }
        LazyRow(
            Modifier.padding(top = 8.dp),
            contentPadding = PaddingValues(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item { Chip("All ${found.size}", section == null) { section = null } }
            items(view.sections) { s ->
                Chip("${s.chip} ${found.count { it.section == s.chip }}", section == s.chip) { section = if (section == s.chip) null else s.chip }
            }
        }
        val shown = found.filter { section == null || it.section == section }
        LazyColumn(Modifier.weight(1f).padding(top = 4.dp)) {
            if (shown.isEmpty()) {
                item { Text("No application options match", color = Turu.Muted, modifier = Modifier.padding(20.dp)) }
            }
            for (s in view.sections) {
                val rows = shown.filter { it.section == s.chip }
                if (rows.isEmpty()) continue
                item(key = "title " + s.chip) {
                    Text(s.title, style = MaterialTheme.typography.labelLarge, color = Turu.Accent, modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 4.dp))
                }
                for (o in rows) {
                    item(key = o.section + o.label) {
                        key(if (o.label == TAB_LIMIT) refused else 0) {
                            OptionRow(
                                o,
                                edited = false,
                                inGame = true,
                                onSet = { value ->
                                    val asks = o.label == TAB_LIMIT && value is Long && value > TAB_LIMIT_SAFE && o.value.long() <= TAB_LIMIT_SAFE
                                    if (asks) raising = o to value else options.set(o, value)
                                },
                                onAction = { if (o.label == "Clear Flash Cache") clearing = o else options.run(o) },
                                fullWindow = fullWindow,
                            )
                        }
                    }
                }
            }
        }
        (options.error?.let { it to true } ?: view.note?.let { it to view.failed })?.let { (text, failed) ->
            HorizontalDivider(color = Turu.Edge.copy(alpha = 0.5f))
            Text(
                text,
                style = MaterialTheme.typography.bodySmall,
                color = if (failed) Turu.Danger else Turu.Muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
    }
    clearing?.let { o ->
        AlertDialog(
            onDismissRequest = { clearing = null },
            title = { Text("Clear the Flash cache?") },
            text = { Text("The game's saved data goes, such as the remembered login. Restart Turu afterwards.") },
            confirmButton = { TextButton(onClick = { clearing = null; options.run(o) }) { Text("Clear", color = Turu.Danger) } },
            dismissButton = { TextButton(onClick = { clearing = null }) { Text("Cancel") } },
            containerColor = Turu.Card,
        )
    }
    raising?.let { (o, tabs) ->
        val keep = o.value.long()
        val refuse: () -> Unit = {
            raising = null
            refused += 1
        }
        AlertDialog(
            onDismissRequest = refuse,
            title = { Text("Allow up to $tabs tabs?") },
            text = { Text(tabLimitWarning(tabs, phoneMemoryGb()), color = Turu.Muted) },
            confirmButton = { TextButton(onClick = { raising = null; options.set(o, tabs) }) { Text("Allow $tabs") } },
            dismissButton = { TextButton(onClick = refuse) { Text("Keep $keep") } },
            containerColor = Turu.Card,
        )
    }
}

private fun OptionValue.long() = (this as? OptionValue.number)?.value ?: 0L

/** Tab Limit's label in Application Options. */
private const val TAB_LIMIT = "Tab Limit"

/** The Tab Limit most phones hold, its default (`application_options::TAB_LIMIT_SAFE`): above it, raising it asks first. */
private const val TAB_LIMIT_SAFE = 4L

/** What `tabs` tabs take beside the phone's memory (`gb`, 0 when unknown). */
internal fun tabLimitWarning(tabs: Long, gb: Int): String {
    // In tenths of a GB: 200 MB a tab at least, 400 at most.
    val gbs = { tenths: Long -> if (tenths % 10 == 0L) "${tenths / 10}" else "${tenths / 10}.${tenths % 10}" }
    val need = "${gbs(tabs * 2)} to ${gbs(tabs * 4)} GB"
    val phone = if (gb > 0) " of this phone's $gb GB, which Android and your other apps share too" else ""
    return "Each tab takes 200 to 400 MB, so $tabs need about $need$phone. " +
        "When memory runs short, Android closes tabs; each comes back, logs in and starts its script again."
}

/** This phone's memory in whole GB, as sold (0 when unknown). */
@Composable
private fun phoneMemoryGb(): Int {
    val context = LocalContext.current
    return remember {
        val manager = context.getSystemService(ActivityManager::class.java) ?: return@remember 0
        val info = ActivityManager.MemoryInfo().also { manager.getMemoryInfo(it) }
        // Android reports a little under what the phone is sold with (8 GB reads as 7.5 GiB).
        ceil(info.totalMem / (1024.0 * 1024 * 1024)).toInt()
    }
}

@Composable
private fun Chip(label: String, selected: Boolean, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label, maxLines = 1) },
        colors = FilterChipDefaults.filterChipColors(selectedContainerColor = Turu.AccentTint, selectedLabelColor = Turu.Text, labelColor = Turu.Muted),
        border = FilterChipDefaults.filterChipBorder(enabled = true, selected = selected, borderColor = Turu.Edge, selectedBorderColor = Turu.Accent.copy(alpha = 0.6f)),
        modifier = Modifier.heightIn(min = 40.dp),
    )
}
