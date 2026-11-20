package com.muktiarafi.turu

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.Executors

/**
 * The account list behind the shell's native Accounts and New tab (docs/UI.md, "Native shells").
 * The list and passwords are the Rust side's (`shell_accounts.rs`): `accounts.json` in the data
 * folder and the Keystore. Reads and writes run on a thread of their own (the Keystore decrypts);
 * what comes back is set on the main thread, where Compose reads it.
 */
class AccountsPanel(private val runActions: (String) -> Unit) {
    var list by mutableStateOf(AccountList())
        private set

    /** Loaded once at least. */
    var loaded by mutableStateOf(false)
        private set

    /** The server list for the launch options, `null` until fetched. */
    var servers by mutableStateOf<List<Server>?>(null)
        private set
    var serversError by mutableStateOf<String?>(null)
        private set

    /** The accounts being started, until their tab shows in the list. */
    var starting by mutableStateOf(emptySet<String>())
        private set

    private val work = Executors.newSingleThreadExecutor { Thread(it, "Turu Accounts") }
    private val main = Handler(Looper.getMainLooper())

    fun reload() {
        work.execute {
            val read = runCatching { AccountList.parse(nativeAccounts()) }
            main.post {
                read.onSuccess {
                    list = it
                    starting = starting.filterTo(mutableSetOf()) { name ->
                        it.accounts.none { a -> a.username.equals(name, true) && a.tab != null }
                    }
                }.onFailure { Log.w(TAG, "Accounts: couldn't read the list: $it") }
                loaded = true
            }
        }
    }

    /** Fetches the servers (and starts measuring their pings); again later for the pings. */
    fun loadServers(tries: Int = 3) {
        work.execute {
            val read = runCatching { nativeServers() }
            main.post {
                val json = read.getOrNull()
                if (json == null || json.trimStart().startsWith("{")) {
                    serversError = json?.let { JSONObject(it).optString("error") } ?: read.exceptionOrNull()?.message
                    return@post
                }
                val array = JSONArray(json)
                servers = (0 until array.length()).map {
                    val s = array.getJSONObject(it)
                    Server(s.getString("name"), s.getString("line"), s.optBoolean("online"))
                }
                serversError = null
                // The pings are measured in the background ("..." until then): read them again.
                if (tries > 1 && servers.orEmpty().any { it.online && it.line.contains(" - ... ") }) {
                    main.postDelayed({ loadServers(tries - 1) }, 4000)
                }
            }
        }
    }

    /** Starts `account` in a new tab, with the launch options. */
    fun start(account: Account) {
        starting = starting + account.username
        ask(JSONObject().put("do", "start").put("username", account.username)) {}
    }

    /** Starts each of `accounts` in a new tab of its own (the caller keeps them to the free tabs). */
    fun startMany(accounts: List<Account>) {
        if (accounts.isEmpty()) return
        starting = starting + accounts.map { it.username }
        ask(JSONObject().put("do", "startMany").put("usernames", JSONArray(accounts.map { it.username }))) {}
    }

    /** Puts `accounts` in `group` (made if new); `done` gets the error, or null. */
    fun addToGroup(group: String, accounts: List<Account>, done: (String?) -> Unit = {}) =
        ask(JSONObject().put("do", "addToGroup").put("group", group).put("usernames", JSONArray(accounts.map { it.username })), done)

    fun removeFromGroup(group: String, accounts: List<Account>) =
        ask(JSONObject().put("do", "removeFromGroup").put("group", group).put("usernames", JSONArray(accounts.map { it.username }))) {}

    fun removeGroup(group: String) = ask(JSONObject().put("do", "removeGroup").put("group", group)) {}

    /** Adds or changes an account; `done` gets the error, or null once saved. */
    fun save(original: Account?, username: String, password: String, displayName: String, tags: List<String>, done: (String?) -> Unit) {
        val ask = JSONObject().put("do", "save")
            .put("original", original?.username ?: JSONObject.NULL)
            .put("username", username)
            .put("password", password.ifEmpty { null } ?: JSONObject.NULL)
            .put("displayName", displayName)
            .put("tags", JSONArray(tags))
        ask(ask, done)
    }

    fun remove(account: Account) = ask(JSONObject().put("do", "remove").put("username", account.username)) {}

    fun move(account: Account, by: Int) =
        ask(JSONObject().put("do", "move").put("username", account.username).put("by", by)) {}

    fun launch(server: String, script: String?, autoStart: Boolean) {
        val ask = JSONObject().put("do", "launch").put("server", server)
            .put("script", script ?: JSONObject.NULL).put("autoStart", autoStart)
        ask(ask) {}
    }

    /** Sends `ask`; the answer's actions are the activity's, an error goes to `done`. */
    private fun ask(ask: JSONObject, done: (String?) -> Unit) {
        work.execute {
            val answer = runCatching { nativeAccountsDo(ask.toString()) }.getOrElse { "{\"error\":\"${it.message}\"}" }
            main.post {
                if (answer.trimStart().startsWith("{")) {
                    done(JSONObject(answer).optString("error"))
                } else {
                    runActions(answer)
                    done(null)
                }
                reload()
            }
        }
    }

    companion object {
        private const val TAG = "turu-shell"

        init {
            System.loadLibrary("turu_android")
        }

        @JvmStatic
        private external fun nativeAccounts(): String

        @JvmStatic
        private external fun nativeAccountsDo(json: String): String

        @JvmStatic
        private external fun nativeServers(): String
    }
}
