package com.muktiarafi.turu

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * One of a few, as Material 3 Expressive's connected button group (it takes the place of
 * segmented buttons): the buttons share the width, the outer ends round, and the chosen one
 * takes the accent tint and rounds fully. Each reads out as a radio button.
 *
 * [description] names a button whose label is a symbol ("≥" reads "At least").
 */
@Composable
fun <T> ConnectedChoice(
    options: List<T>,
    selected: T,
    onPick: (T) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    description: ((T) -> String)? = null,
    label: @Composable RowScope.(T) -> Unit,
) {
    val colors = ToggleButtonDefaults.colors(
        // A step above the card: with no outline, the unchosen buttons still read as buttons.
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        contentColor = Turu.Muted,
        checkedContainerColor = Turu.AccentTint,
        checkedContentColor = Turu.Text,
    )
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        options.forEachIndexed { i, option ->
            val checked = option == selected
            ToggleButton(
                checked = checked,
                onCheckedChange = { if (!checked) onPick(option) },
                enabled = enabled,
                shapes = when (i) {
                    0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                    options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                    else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                },
                colors = colors,
                contentPadding = PaddingValues(horizontal = 8.dp),
                modifier = Modifier
                    .weight(1f)
                    .heightIn(min = 48.dp)
                    .semantics {
                        role = Role.RadioButton
                        description?.let { contentDescription = it(option) }
                    },
            ) { label(option) }
        }
    }
}
