package com.muktiarafi.turu

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONArray
import org.json.JSONObject

/** One tab, as the shell's Rust side lists it (`slots::NativeTab`). */
@Immutable
data class ShellTab(
    val id: Int,
    val label: String,
    val busy: Boolean,
    /** What its script waits for the user to answer ("Script Options", "a message box", ...). */
    val waits: String?,
    /** Its process isn't up yet ("starting"). */
    val down: String?,
    /** Its control API and token, where its view models are read (null until it says hello). */
    val api: String? = null,
    val token: String? = null,
)

/** The tab list (`slots::NativeTabs`). */
@Immutable
data class ShellTabs(
    val tabs: List<ShellTab> = emptyList(),
    val shown: Int = 0,
    val railHidden: Boolean = false,
    val full: Boolean = false,
) {
    val active: ShellTab? get() = tabs.firstOrNull { it.id == shown }

    /** Another tab than the one on screen needs the user: it waits for an answer. */
    val othersNeedYou: Boolean get() = tabs.any { it.id != shown && it.waits != null }

    companion object {
        fun parse(json: String): ShellTabs {
            val o = JSONObject(json)
            val list = o.getJSONArray("tabs")
            return ShellTabs(
                tabs = (0 until list.length()).map { i ->
                    val t = list.getJSONObject(i)
                    ShellTab(
                        id = t.getInt("id"),
                        label = t.getString("label"),
                        busy = t.optBoolean("busy"),
                        waits = t.optStringOrNull("waits"),
                        down = t.optStringOrNull("down"),
                        api = t.optStringOrNull("api"),
                        token = t.optStringOrNull("token"),
                    )
                },
                shown = o.getInt("shown"),
                railHidden = o.optBoolean("rail_hidden"),
                full = o.optBoolean("full"),
            )
        }
    }
}

/** What the tab panel shows: its pages, pushed in place (docs/UI.md, "Tab panel"). */
sealed interface Page {
    data object Tabs : Page
    data object NewTab : Page
    data object Accounts : Page

    /** The Script Loader. */
    data object Scripts : Page
    data object Auto : Page
    data object Jump : Page
    data object Logs : Page

    /** Every menu's windows, Hide Side Bar. */
    data object More : Page

    /** Search Scripts. */
    data object Search : Page

    /** Options > Game. */
    data object GameOptions : Page

    /** Options > Application. */
    data object AppOptions : Page

    /** Options > CoreBots. */
    data object CoreBots : Page

    /** Scripts > Scheduler. */
    data object Scheduler : Page

    /** Army > Army Control. */
    data object Army : Page

    /** Diagnostics > Notification Center. */
    data object Notifications : Page

    /** Combat > Stats. */
    data object Stats : Page

    /** Tools > Current Drops. */
    data object CurrentDrops : Page

    /** Combat > Skills. */
    data object Skills : Page

    /** Tools > Loadouts. */
    data object Loadouts : Page

    /** Tools > Fast Travel. */
    data object FastTravel : Page

    /** Plugins > Butler. */
    data object Butler : Page

    /** Plugins > Cosmetics. */
    data object Cosmetics : Page

    /** Combat > Runtime. */
    data object Runtime : Page

    /** Scripts > Edit Script Options (also when a script asks for its options). */
    data object ScriptOptions : Page

    /** Add (`original` null) or edit an account. */
    data class AccountForm(val original: Account?) : Page
}

/** Search Scripts opened to pick a script for another page, which the bar and each row say. */
sealed interface ScriptPick {
    /** "Pick a script to load in 2 clients". */
    val bar: String

    /** Each row's button: "Load in 2 clients". */
    val button: String

    /**
     * Army Control's clients: the checked rows' keys (none: every client) and who that is
     * ("cecilus", "2 clients").
     */
    @Immutable
    data class Army(val keys: List<String>, val who: String) : ScriptPick {
        override val bar get() = "Pick a script to load in $who"
        override val button get() = "Load in $who"
    }

    /** Accounts' launch options: the script launched accounts run. */
    data object Launch : ScriptPick {
        override val bar get() = "Pick the script launched accounts run"
        override val button get() = "Use"
    }
}

/** The shell's screens' state, read by Compose and changed on the main thread. */
class ShellState {
    var tabs by mutableStateOf(ShellTabs())
    var open by mutableStateOf(false)
    var page by mutableStateOf<Page>(Page.Tabs)

    /** Search Scripts picks a script for another page while this is set ([ScriptPick]). */
    var scriptPick by mutableStateOf<ScriptPick?>(null)

    /** The display cutout on the rail's side, in dp: the item grows by it, as egui's rail does. */
    var cutoutLeftDp by mutableStateOf(0f)

    /** More's menu on show (its title), kept while the panel is closed: More opens where it was. */
    var moreMenu by mutableStateOf<String?>(null)

    /** The rail's items a short screen folded away (their labels), for More to offer. */
    var folded by mutableStateOf<Set<String>>(emptySet())

    /** Told when the panel opens or closes (`ShellScreens` shows its view only while open). */
    var onOpenChanged: (Boolean) -> Unit = {}

    /** The pages under the one shown, for Back. */
    private val stack = ArrayDeque<Page>()

    /**
     * Counts the panel's openings: the panel starts its pages afresh on each, so the page it opens
     * on shows at once instead of crossing from the one it last closed on.
     */
    var openings by mutableStateOf(0)
        private set

    fun openAt(at: Page) {
        stack.clear()
        page = at
        if (!open) {
            openings++
            open = true
            onOpenChanged(true)
        }
    }

    fun push(next: Page) {
        stack.addLast(page)
        page = next
    }

    /** Back: the page under this one, or the panel closes. */
    fun back() {
        val under = stack.removeLastOrNull()
        if (under != null) page = under else close()
    }

    fun close() {
        if (!open) return
        open = false
        stack.clear()
        page = Page.Tabs
        onOpenChanged(false)
    }
}

/** An account, as `shell_accounts.rs` lists it. */
@Immutable
data class Account(
    val username: String,
    val label: String,
    val displayName: String,
    val tags: List<String>,
    val groups: List<String>,
    /** The tab it plays in, if one is open for it. */
    val tab: Int?,
    /** A password is saved: it logs in by itself. */
    val password: Boolean,
)

/** The account list and the launch options. */
@Immutable
data class AccountList(
    val accounts: List<Account> = emptyList(),
    val groups: List<String> = emptyList(),
    val tags: List<String> = emptyList(),
    val server: String = "",
    val script: String? = null,
    val autoStart: Boolean = false,
    /** Favourite and recent scripts, for the launch options. */
    val scripts: List<String> = emptyList(),
    val keepsPasswords: Boolean = true,
) {
    companion object {
        fun parse(json: String): AccountList {
            if (json == "null") return AccountList()
            val o = JSONObject(json)
            val list = o.getJSONArray("accounts")
            return AccountList(
                accounts = (0 until list.length()).map { i ->
                    val a = list.getJSONObject(i)
                    Account(
                        username = a.getString("username"),
                        label = a.getString("label"),
                        displayName = a.optString("displayName"),
                        tags = a.getJSONArray("tags").strings(),
                        groups = a.getJSONArray("groups").strings(),
                        tab = if (a.isNull("tab")) null else a.getInt("tab"),
                        password = a.optBoolean("password"),
                    )
                },
                groups = o.getJSONArray("groups").strings(),
                tags = o.getJSONArray("tags").strings(),
                server = o.optString("server"),
                script = o.optStringOrNull("script"),
                autoStart = o.optBoolean("autoStart"),
                scripts = o.optJSONArray("scripts")?.strings().orEmpty(),
                keepsPasswords = o.optBoolean("keepsPasswords", true),
            )
        }
    }
}

/** A server for the launch options: its name and its status line ("Yorumi - 42ms - 120/1000"). */
@Immutable
data class Server(val name: String, val line: String, val online: Boolean)

/** Two letters for a tab's or an account's avatar: "Hero Two" is "HT", "dod" is "DO". */
fun initials(label: String): String {
    val words = label.trim().split(Regex("[\\s_.-]+")).filter { it.isNotEmpty() }
    val letters = when {
        words.size >= 2 -> "${words[0].first()}${words[1].first()}"
        words.size == 1 -> words[0].take(2)
        else -> "?"
    }
    return letters.uppercase()
}

private fun JSONArray.strings(): List<String> = (0 until length()).map { getString(it) }

private fun JSONObject.optStringOrNull(name: String): String? = if (isNull(name)) null else optString(name)
