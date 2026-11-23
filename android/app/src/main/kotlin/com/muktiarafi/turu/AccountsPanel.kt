package com.muktiarafi.turu

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.muktiarafi.turu.models.AccountsAnswer
import com.muktiarafi.turu.models.AccountsAsk
import com.muktiarafi.turu.models.AccountsView
import com.muktiarafi.turu.models.ServersView
import com.muktiarafi.turu.models.ShellAction
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.json.Json
import java.util.concurrent.Executors

/**
 * The account list behind the shell's native Accounts and New tab (docs/UI.md, "Native shells").
 * The list and passwords are the Rust side's (`shell_accounts.rs`): `accounts.json` in the data
 * folder and the Keystore. Reads and writes run on a thread of their own (the Keystore decrypts);
 * what comes back is set on the main thread, where Compose reads it.
 */
class AccountsPanel(private val runActions: (List<ShellAction>) -> Unit) {
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

    /** Why an account didn't start (its row shows it), until it's started again. */
    var failed by mutableStateOf(emptyMap<String, String>())
        private set

    private val work = Executors.newSingleThreadExecutor { Thread(it, "Turu Accounts") }
    private val main = Handler(Looper.getMainLooper())

    fun reload() {
        work.execute {
            val read = runCatching { AccountList.from(JSON.decodeFromString(AccountsView.serializer().nullable, nativeAccounts())) }
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
            val read = runCatching { JSON.decodeFromString(ServersView.serializer(), nativeServers()) }
            main.post {
                val view = read.getOrElse { serversError = it.message; return@post }
                view.error?.let { serversError = it; return@post }
                servers = view.servers.map { Server(it.name, it.line, it.online) }
                serversError = null
                // The pings are measured in the background ("..." until then): read them again.
                if (tries > 1 && servers.orEmpty().any { it.online && it.line.contains(" - ... ") }) {
                    main.postDelayed({ loadServers(tries - 1) }, 4000)
                }
            }
        }
    }

    /** Starts `account` in a new tab, with the launch options (New tab's rows). */
    fun start(account: Account) = startMany(listOf(account))

    /**
     * Starts each of `accounts` in a new tab of its own (the caller keeps them to the free tabs).
     * An error, or no tab opened at all (a full pool opens nothing), shows on their rows.
     */
    fun startMany(accounts: List<Account>) {
        if (accounts.isEmpty()) return
        val names = accounts.map { it.username }
        starting = starting + names
        failed = failed - names.toSet()
        ask(AccountsAsk.startMany(names), opened = { actions -> if (actions.isEmpty()) fail(names, "No tab was free to start it in") }) { error ->
            if (!error.isNullOrEmpty()) fail(names, error)
        }
    }

    private fun fail(names: List<String>, why: String) {
        starting = starting - names.toSet()
        failed = failed + names.associateWith { why }
    }

    /** Puts `accounts` in `group` (made if new); `done` gets the error, or null. */
    fun addToGroup(group: String, accounts: List<Account>, done: (String?) -> Unit = {}) =
        ask(AccountsAsk.addToGroup(group, accounts.map { it.username }), done = done)

    fun removeFromGroup(group: String, accounts: List<Account>) =
        ask(AccountsAsk.removeFromGroup(group, accounts.map { it.username })) {}

    fun removeGroup(group: String) = ask(AccountsAsk.removeGroup(group)) {}

    /** Adds or changes an account; `done` gets the error, or null once saved. */
    fun save(original: Account?, username: String, password: String, displayName: String, tags: List<String>, done: (String?) -> Unit) {
        ask(AccountsAsk.save(original?.username, username, password.ifEmpty { null }, displayName, tags), done = done)
    }

    fun remove(account: Account) = ask(AccountsAsk.remove(account.username)) {}

    fun move(account: Account, by: Int) =
        ask(AccountsAsk.move(account.username, by)) {}

    fun launch(server: String, script: String?, autoStart: Boolean) {
        ask(AccountsAsk.launch(server, script, autoStart)) {}
    }

    /** Sends `ask`; the answer's actions are the activity's (and `opened`'s), an error goes to `done`. */
    private fun ask(ask: AccountsAsk, opened: (List<ShellAction>) -> Unit = {}, done: (String?) -> Unit) {
        work.execute {
            val answer = runCatching {
                val json = nativeAccountsDo(JSON.encodeToString(AccountsAsk.JsonSerializer, ask))
                JSON.decodeFromString(AccountsAnswer.serializer(), json)
            }.getOrElse { AccountsAnswer(error = it.message ?: it.toString(), actions = emptyList()) }
            main.post {
                val error = answer.error
                if (error != null) {
                    done(error)
                } else {
                    runActions(answer.actions)
                    opened(answer.actions)
                    done(null)
                }
                reload()
            }
        }
    }

    companion object {
        private const val TAG = "turu-shell"

        /** The Rust side's answers; one from a newer side may carry more fields. */
        private val JSON = Json { ignoreUnknownKeys = true }

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
