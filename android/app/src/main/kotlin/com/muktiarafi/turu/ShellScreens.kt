package com.muktiarafi.turu

import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.ViewCompositionStrategy
import org.json.JSONObject

/**
 * The shell's own screens, in Compose over the slot's `SurfaceView` (docs/NATIVE_UI_PROPOSAL.md,
 * L5a; docs/UI.md, "Native shells", Android): the Tabs item at the top of the rail, and the panel
 * it opens (the tabs, New tab, Accounts). The slots draw no tab strip and start their rail below
 * the item (`slots::RAIL_TOP`). Two views: the item, always there; the panel, only while open, so
 * touches elsewhere go to the slot on screen ([hits]).
 */
class ShellScreens(private val activity: TuruActivity) {
    val state = ShellState()

    /** The shell's tab actions, as if from the slot on screen (`SlotShell.asShown`). */
    private val actions = TabActions(
        select = { id -> activity.shell?.asShown(JSONObject().put("type", "select_tab").put("id", id).toString()) },
        close = { id -> activity.shell?.asShown(JSONObject().put("type", "close_tab").put("id", id).toString()) },
        blank = { activity.shell?.asShown(JSONObject().put("type", "open_tab").toString()) },
        accounts = AccountsPanel { actions -> activity.shell?.actions(actions) },
        // The slot's egui windows the Script Loader opens, while they aren't native: in its
        // picture, with the panel out of the way.
        scripts = ScriptsPanel { window ->
            activity.shell?.toShown(JSONObject().put("type", "open_window").put("window", window).toString())
            close()
        },
    )

    private val item = ComposeView(activity).apply {
        isFocusable = false
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindow)
        setContent { ShellTheme { TabsItem(state) { if (state.open) close() else open(Page.Tabs) } } }
    }

    private val panel = ComposeView(activity).apply {
        visibility = View.GONE
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindow)
        setContent { ShellTheme { TabPanel(state, actions) } }
    }

    init {
        state.onOpenChanged = { open ->
            panel.visibility = if (open) View.VISIBLE else View.GONE
            // The Script Loader reads the tab's models only while it shows.
            if (!open) actions.scripts.follow(null)
            // The keys go back to the slot (egui's text boxes, the game) once the panel is gone.
            if (!open) activity.focusSurface()
        }
    }

    fun attach(parent: ViewGroup) {
        val wrap = ViewGroup.LayoutParams.WRAP_CONTENT
        val full = ViewGroup.LayoutParams.MATCH_PARENT
        parent.addView(item, FrameLayout.LayoutParams(wrap, wrap))
        parent.addView(panel, FrameLayout.LayoutParams(full, full))
        updateItem()
    }

    /** The shell's tab list (`slots::NativeTabs`), on the main thread. */
    fun tabs(json: String) {
        state.tabs = ShellTabs.parse(json)
        updateItem()
        // A tab opened or closed: where each account plays changed.
        if (state.open) actions.accounts.reload()
    }

    /** A window the shell has natively, opened in a slot's egui menus (`ToShell::OpenScreen`). */
    fun openScreen(screen: String) {
        when (screen) {
            "accounts" -> open(Page.Accounts)
            "scripts" -> open(Page.Scripts)
        }
    }

    fun open(page: Page) {
        actions.accounts.reload()
        state.openAt(page)
    }

    fun close() = state.close()

    /** The display cutout on the left, in window pixels: the item and panel keep clear of it. */
    fun cutout(left: Int) {
        state.cutoutLeftDp = left / activity.resources.displayMetrics.density
    }

    /** The item hides with the slot's rail (Menu > Hide Side Bar) and before there are tabs. */
    private fun updateItem() {
        val hidden = state.tabs.railHidden || state.tabs.tabs.isEmpty()
        item.visibility = if (hidden) View.GONE else View.VISIBLE
        if (hidden && state.open) state.close()
    }

    /** Whether a finger down at window (`x`, `y`) lands on a native screen, not the slot's. */
    fun hits(x: Float, y: Float): Boolean {
        if (panel.visibility == View.VISIBLE) return true
        if (item.visibility != View.VISIBLE) return false
        val at = IntArray(2)
        item.getLocationInWindow(at)
        return x >= at[0] && y >= at[1] && x < at[0] + item.width && y < at[1] + item.height
    }
}
