package com.muktiarafi.turu

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/**
 * The shell's native screens' look (docs/UI.md, "Native shells", Android): Material 3 with a fixed
 * dark scheme pinned to egui's tokens (`frontend/src/gui/turu/theme.rs`), since Compose and egui
 * share the rail until L5b. No wallpaper colours. Roboto in the M3 type scale.
 */
object Turu {
    val Panel = Color(0xFF11131A)
    val Card = Color(0xFF181B25)
    val Inset = Color(0xFF0C0D13)
    val Edge = Color(0xFF3A3E4E)
    val RowHover = Color(0xFF282C3A)
    val Track = Color(0xFF303442)
    val Accent = Color(0xFF7C6CF0)
    val Text = Color(0xFFE8EAF2)
    val Muted = Color(0xFF8C92A6)
    val Success = Color(0xFF34D399)
    val Warning = Color(0xFFFBBF24)
    val Danger = Color(0xFFEF4444)

    /** Stats' accepted and re-logins counters, as egui's tiles (`stats.rs`'s `BLUE`, `PURPLE`). */
    val Blue = Color(0xFF60A5FA)
    val Purple = Color(0xFFA78BFA)

    /** The selected row and tonal buttons: the accent 22% into the card. */
    val AccentTint = Color(0xFF2B2A4A)

    /** The rail's selected item: the accent 35% into the panel, to stand out from the dark rail. */
    val RailSelected = Color(0xFF373265)
}

private val scheme = darkColorScheme(
    primary = Turu.Accent,
    onPrimary = Color.White,
    primaryContainer = Turu.AccentTint,
    onPrimaryContainer = Turu.Text,
    secondary = Turu.Accent,
    onSecondary = Color.White,
    secondaryContainer = Turu.AccentTint,
    onSecondaryContainer = Turu.Text,
    tertiary = Turu.Success,
    background = Turu.Panel,
    onBackground = Turu.Text,
    surface = Turu.Panel,
    onSurface = Turu.Text,
    surfaceVariant = Turu.Card,
    onSurfaceVariant = Turu.Muted,
    surfaceContainerLowest = Turu.Inset,
    surfaceContainerLow = Turu.Panel,
    surfaceContainer = Turu.Card,
    surfaceContainerHigh = Color(0xFF1F2230),
    surfaceContainerHighest = Turu.Track,
    inverseSurface = Turu.Text,
    inverseOnSurface = Turu.Panel,
    outline = Turu.Edge,
    outlineVariant = Color(0xFF262A36),
    error = Turu.Danger,
    onError = Color.White,
    errorContainer = Color(0xFF3A1A1E),
    onErrorContainer = Color(0xFFFFB4B4),
    scrim = Color.Black,
)

@Composable
fun ShellTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = scheme, content = content)
}
