package com.muktiarafi.turu

import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.ViewCompositionStrategy
import com.muktiarafi.turu.models.BankAct
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
        it.openOptions = { state.push(Page.ScriptOptions) }
    }

    /** The shell's tab actions, as if from the slot on screen (`SlotShell.asShown`). */
    private val actions = TabActions(
        select = { id -> activity.shell?.asShown(JSONObject().put("type", "select_tab").put("id", id).toString()) },
        close = { id -> activity.shell?.asShown(JSONObject().put("type", "close_tab").put("id", id).toString()) },
        startAgain = { id -> activity.shell?.toSlot(id, JSONObject().put("type", "start_again").toString()) },
        blank = { activity.shell?.asShown(JSONObject().put("type", "open_tab").toString()) },
        accounts = AccountsPanel { actions -> activity.shell?.actions(actions) },
        scripts = scripts,
        window = ::openWindow,
        hideRail = { railHidden(true) },
        bank = {
            close()
            models.act("bank", BankAct.OPEN, BankAct.JsonSerializer)
        },
        search = SearchPanel(),
        gameOptions = GameOptionsPanel(),
        appOptions = AppOptionsPanel(),
        coreBots = CoreBotsPanel(),
        scheduler = SchedulerPanel(),
        crashes = CrashesPanel(),
        army = ArmyPanel(),
        notifications = NotificationsPanel(),
        stats = StatsPanel(),
        drops = DropsPanel(),
        skills = SkillsPanel(),
        loadouts = LoadoutsPanel(),
        fastTravel = FastTravelPanel(),
        butler = ButlerPanel(),
        cosmetics = CosmeticsPanel(),
        runtime = RuntimePanel(),
        notifyDrop = NotifyDropPanel(),
        update = UpdatePanel(),
        loader = LoaderPanel(),
        grabber = GrabberPanel(),
        junk = JunkPanel(),
        console = ConsolePanel(),
        scriptOptions = ScriptOptionsPanel(),
        armyTravel = ArmyTravelPanel(),
        armyClient = ArmyClientPanel(),
        packetLogger = packetLoggerPanel(),
        spammer = spammerPanel(),
        interceptor = interceptorPanel(),
    )

    private val railActions = RailActions(
        open = { page -> if (state.open && state.page == page) close() else open(page) },
        bank = { models.act("bank", BankAct.OPEN, BankAct.JsonSerializer) },
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
        // Hidden only once the closing slide is over (and the panel wasn't opened again meanwhile).
        setContent { ShellTheme { TabPanel(state, actions) { if (!state.open) visibility = View.GONE } } }
    }

    private val prompts = PromptsPanel()

    /** The prompts' own view: always attached and visible, drawing nothing but their dialog. */
    private val promptsView = ComposeView(activity).apply {
        isFocusable = false
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindow)
        setContent { ShellTheme { PromptsHost(state, prompts, window = ::openWindow, open = ::openScreen, report = ::openCrash) } }
    }

    init {
        state.onOpenChanged = { open ->
            // Closing leaves the view up for the panel's slide; TabPanel hides it when that's done.
            if (open) panel.visibility = View.VISIBLE
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
        parent.addView(promptsView, FrameLayout.LayoutParams(0, 0))
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
            "army" -> open(Page.Army)
            "notifications" -> open(Page.Notifications)
            "stats" -> open(Page.Stats)
            "current_drops" -> open(Page.CurrentDrops)
            "skills" -> open(Page.Skills)
            "loadouts" -> open(Page.Loadouts)
            "fast_travel" -> open(Page.FastTravel)
            "butler" -> open(Page.Butler)
            "cosmetics" -> open(Page.Cosmetics)
            "runtime" -> open(Page.Runtime)
            "notify_drop" -> open(Page.NotifyDrop)
            "client_update" -> open(Page.Update)
            "loader" -> open(Page.Loader)
            "grabber" -> open(Page.Grabber)
            "junk_items" -> open(Page.JunkItems)
            "console" -> open(Page.Console)
            "packet_logger" -> open(Page.PacketLogger)
            "spammer" -> open(Page.Spammer)
            "interceptor" -> open(Page.Interceptor)
            "script_options" -> open(Page.ScriptOptions)
            "logs" -> open(Page.Logs)
            "crashes" -> {
                state.logsKind = "crashes"
                open(Page.Logs)
            }
        }
    }

    /** Logs > Crashes on the report `name` (the crash notice's Show report); Back goes to the list. */
    fun openCrash(name: String) {
        state.logsKind = "crashes"
        open(Page.Logs)
        state.push(Page.CrashReport(name))
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
