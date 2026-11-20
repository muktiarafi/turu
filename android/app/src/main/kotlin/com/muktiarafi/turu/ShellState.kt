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

    /** Add (`original` null) or edit an account. */
    data class AccountForm(val original: Account?) : Page
}

/** The shell's screens' state, read by Compose and changed on the main thread. */
class ShellState {
    var tabs by mutableStateOf(ShellTabs())
    var open by mutableStateOf(false)
    var page by mutableStateOf<Page>(Page.Tabs)

    /** The display cutout on the rail's side, in dp: the item grows by it, as egui's rail does. */
    var cutoutLeftDp by mutableStateOf(0f)

    /** Told when the panel opens or closes (`ShellScreens` shows its view only while open). */
    var onOpenChanged: (Boolean) -> Unit = {}

    /** The pages under the one shown, for Back. */
    private val stack = ArrayDeque<Page>()

    fun openAt(at: Page) {
        stack.clear()
        page = at
        if (!open) {
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
