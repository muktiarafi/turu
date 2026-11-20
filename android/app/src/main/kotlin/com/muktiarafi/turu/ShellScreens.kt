package com.muktiarafi.turu

import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.ViewCompositionStrategy
import org.json.JSONObject

/**
 * The shell's own screens, in Compose over the slot's `SurfaceView` (docs/NATIVE_UI_PROPOSAL.md,
 * L5a and L5b; docs/UI.md, "Native shells", Android): the native rail down the left (the Tabs
 * item, the rail's items, the script chip and run controls), and the panel its items open beside
 * it (the tabs, New tab, Accounts, the Script Loader, Auto, Jump, Logs, More). The slots draw no
 * rail and keep its room (`ToClient::Tabs`' `native_rail`). Three views: the rail, or while it's
 * hidden the `›` handle; the panel, only while open, under the rail so the rail stays in use.
 * Touches elsewhere go to the slot on screen ([hits]).
 */
class ShellScreens(private val activity: TuruActivity) {
    val state = ShellState()

    /** The tab on screen's view models, read for the rail and its pages. */
    private val models = TabModels()

    private val scripts = ScriptsPanel(models) { window -> openWindow(window) }.also {
        it.openSearch = { state.push(Page.Search) }
    }

    /** The shell's tab actions, as if from the slot on screen (`SlotShell.asShown`). */
    private val actions = TabActions(
        select = { id -> activity.shell?.asShown(JSONObject().put("type", "select_tab").put("id", id).toString()) },
        close = { id -> activity.shell?.asShown(JSONObject().put("type", "close_tab").put("id", id).toString()) },
        blank = { activity.shell?.asShown(JSONObject().put("type", "open_tab").toString()) },
        accounts = AccountsPanel { actions -> activity.shell?.actions(actions) },
        scripts = scripts,
        window = ::openWindow,
        hideRail = { railHidden(true) },
        search = SearchPanel(),
        gameOptions = GameOptionsPanel(),
        appOptions = AppOptionsPanel(),
        coreBots = CoreBotsPanel(),
        scheduler = SchedulerPanel(),
    )

    private val railActions = RailActions(
        open = { page -> if (state.open && state.page == page) close() else open(page) },
        bank = { models.act("bank", "open") },
        window = ::openWindow,
        scripts = scripts,
    )

    private val rail = ComposeView(activity).apply {
        isFocusable = false
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindow)
        setContent {
            ShellTheme {
                NativeRail(state, models, railActions) { if (state.open && state.page == Page.Tabs) close() else open(Page.Tabs) }
                BankNote(models)
            }
        }
    }

    private val handle = ComposeView(activity).apply {
        isFocusable = false
        visibility = View.GONE
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindow)
        setContent { ShellTheme { RailHandle(state.cutoutLeftDp.let { androidx.compose.ui.unit.Dp(it) }) { railHidden(false) } } }
    }

    private val panel = ComposeView(activity).apply {
        visibility = View.GONE
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindow)
        setContent { ShellTheme { TabPanel(state, actions) } }
    }

    init {
        state.onOpenChanged = { open ->
            panel.visibility = if (open) View.VISIBLE else View.GONE
            // The keys go back to the slot (egui's text boxes, the game) once the panel is gone.
            if (!open) activity.focusSurface()
        }
    }

    fun attach(parent: ViewGroup) {
        val wrap = ViewGroup.LayoutParams.WRAP_CONTENT
        val full = ViewGroup.LayoutParams.MATCH_PARENT
        parent.addView(panel, FrameLayout.LayoutParams(full, full))
        parent.addView(rail, FrameLayout.LayoutParams(wrap, full))
        parent.addView(handle, FrameLayout.LayoutParams(wrap, wrap, Gravity.START or Gravity.CENTER_VERTICAL))
        updateRail()
    }

    /** The shell's tab list (`slots::NativeTabs`), on the main thread. */
    fun tabs(json: String) {
        state.tabs = ShellTabs.parse(json)
        models.follow(state.tabs.active)
        updateRail()
        // A tab opened or closed: where each account plays changed.
        if (state.open) actions.accounts.reload()
    }

    /** A window the shell has natively, opened in a slot's egui menus (`ToShell::OpenScreen`). */
    fun openScreen(screen: String) {
        when (screen) {
            "accounts" -> open(Page.Accounts)
            "scripts" -> open(Page.Scripts)
            "search" -> open(Page.Search)
            "game_options" -> open(Page.GameOptions)
            "app_options" -> open(Page.AppOptions)
            "core_bots" -> open(Page.CoreBots)
            "scheduler" -> open(Page.Scheduler)
        }
    }

    fun open(page: Page) {
        if (page == Page.Accounts || page == Page.NewTab) actions.accounts.reload()
        state.openAt(page)
    }

    fun close() = state.close()

    /** One of the slot on screen's egui windows (`Windows::open`'s key), the panel out of its way. */
    private fun openWindow(window: String) {
        activity.shell?.toShown(JSONObject().put("type", "open_window").put("window", window).toString())
        close()
    }

    /** Hides or shows the rail in every tab: each gives the game the room and keeps the choice. */
    private fun railHidden(hidden: Boolean) {
        if (hidden) close()
        activity.shell?.toAll(JSONObject().put("type", "rail_hidden").put("hidden", hidden).toString())
    }

    /** The display cutout on the left, in window pixels: the rail and panel keep clear of it. */
    fun cutout(left: Int) {
        state.cutoutLeftDp = left / activity.resources.displayMetrics.density
    }

    /** The rail hides with the tab on screen's choice (`side_bar_hidden`), its handle showing. */
    private fun updateRail() {
        val noTabs = state.tabs.tabs.isEmpty()
        val hidden = state.tabs.railHidden
        rail.visibility = if (hidden || noTabs) View.GONE else View.VISIBLE
        handle.visibility = if (hidden && !noTabs) View.VISIBLE else View.GONE
        if (hidden && state.open) state.close()
    }

    /** Whether a finger down at window (`x`, `y`) lands on a native screen, not the slot's. */
    fun hits(x: Float, y: Float): Boolean {
        if (panel.visibility == View.VISIBLE) return true
        return listOf<View>(rail, handle).any { view ->
            if (view.visibility != View.VISIBLE) return@any false
            val at = IntArray(2)
            view.getLocationInWindow(at)
            x >= at[0] && y >= at[1] && x < at[0] + view.width && y < at[1] + view.height
        }
    }
}
