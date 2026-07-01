package com.noop.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.LocalDrink
import androidx.compose.material.icons.filled.WaterDrop
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.noop.analytics.HydrationGoal
import com.noop.analytics.HydrationStore
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale

// MARK: - Hydration detail (MVP, opt-in, local-only)
//
// Design-Reset compliant: a clean progress ring (GlowRing, blue accent, no bloom on the light field), the
// three quick-log buttons (Sip / Cup / Bottle) in the secondary NoopButton style, today's logged entries
// as a small list, and a 7-day mini bar history. Flat cards (NoopCard), NoopMetrics spacing, tokens only,
// no gold. Mirrors the iOS HydrationView. The hydration day total + history come from the local-only
// HydrationStore series; the goal is the pure HydrationGoal engine (profile sex + today's Effort bump).

/** The reset accent blue (matches NoopButton's pinned iOS `StrandPalette.accent`: #234F9E / #60A0E0). */
private val hydrationAccent: Color
    @Composable get() = if (Palette.isLight) Color(0xFF234F9E) else Color(0xFF60A0E0)

/** Upper bound (ml) for a single custom hydration log (#798) - a sane cap so a stray digit can't bank an
 *  absurd 50-litre day. A 3-litre container covers any realistic bottle/jug. Mirrors the iOS clamp. */
private const val MAX_CUSTOM_ML: Int = 3000

/** Parse a custom-amount field to a clamped whole-ml value in 1..[MAX_CUSTOM_ML], or null when the text
 *  isn't a usable positive number. Pure + side-effect-free so the dialog's confirm gate is unit-testable. */
internal fun parseCustomHydrationMl(text: String): Int? {
    val n = text.trim().toIntOrNull() ?: return null
    if (n <= 0) return null
    return n.coerceAtMost(MAX_CUSTOM_ML)
}

/**
 * The Hydration detail screen. The goal's effort bump uses today's Effort/strain (0..100) read from the
 * view-model's `today` row (null leaves the bump at 0). The screen reads/writes via [viewModel].repo;
 * the SharedPreferences-backed profile sex is read once. A log tap appends to the local-only day total
 * and refreshes the ring + history.
 */
@Composable
fun HydrationScreen(viewModel: AppViewModel) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    val today by viewModel.today.collectAsStateWithLifecycle()
    val strain = today?.strain

    val sex = remember { ProfileStore.from(context).sex }
    val goalMl = remember(sex, strain) { HydrationGoal.dailyGoalMl(sex, strain) }

    // Today's running total + the per-day history, loaded off the gesture path and refreshed after a log.
    var totalMl by remember { mutableStateOf(0.0) }
    var history by remember { mutableStateOf<List<Pair<String, Double>>>(emptyList()) }
    // A simple reload key the log taps bump so the LaunchedEffect re-reads the store.
    var reloadTick by remember { mutableStateOf(0) }
    LaunchedEffect(reloadTick) {
        totalMl = runCatching { HydrationStore.total(viewModel.repo) }.getOrDefault(0.0)
        history = runCatching { HydrationStore.history(viewModel.repo, days = 7) }.getOrDefault(emptyList())
    }

    // #798 - the LAST amount logged this session, so the detail can offer a one-tap "Undo" that removes
    // exactly that container from the day total. The single-total schema can't address an individual log,
    // so undo subtracts the last known amount (clamped at 0 by the store). Reset to null after an undo so
    // the affordance only appears when there's something to take back. Not persisted - a relaunch shows
    // the day total but no pending undo, which is honest (you can still correct via the custom dialog).
    var lastLoggedMl by remember { mutableStateOf<Int?>(null) }
    // The custom-amount dialog (the "custom container size" - log any ml the quick buttons don't cover).
    var showCustom by remember { mutableStateOf(false) }

    val log: (Int) -> Unit = { amount ->
        scope.launch {
            runCatching { HydrationStore.log(viewModel.repo, amount) }
            if (amount > 0) lastLoggedMl = amount
            reloadTick += 1
        }
    }
    // Remove [amount] ml from the day total (the undo / delete-a-log path, #798). Clears the pending undo.
    val remove: (Int) -> Unit = { amount ->
        scope.launch {
            runCatching { HydrationStore.remove(viewModel.repo, amount) }
            lastLoggedMl = null
            reloadTick += 1
        }
    }

    val fraction = if (goalMl > 0) (totalMl / goalMl).toFloat() else 0f
    val accent = hydrationAccent

    // #798 - the custom-amount entry. Logs any whole-ml amount the Sip/Cup/Bottle quick buttons don't
    // cover (a bespoke container), clamped to a sane 1..MAX_CUSTOM_ML, then routed through the SAME
    // additive `log` path so it accumulates into the day total and refreshes the ring + history.
    if (showCustom) {
        CustomAmountDialog(
            accent = accent,
            onDismiss = { showCustom = false },
            onConfirm = { ml ->
                showCustom = false
                log(ml)
            },
        )
    }

    // PERF (#707): lazy scaffold — each top-level section is one `item { }`. Order + spacing unchanged
    // (LazyColumn reproduces the eager `spacedBy(20.dp)`); only on-screen cards compose + are
    // accessibility-walked. All children are unconditional, so every wrap is a bare `item { }`.
    LazyScreenScaffold(
        title = "Hydration",
        subtitle = "Your fluid intake today, on this phone only.",
    ) {
        // RING — total vs goal, in litres. GlowRing already shows the clean full-circle track with no
        // bloom on the light field; the blue accent keeps it on the reset palette (no gold).
        item {
        NoopCard(padding = 20.dp) {
            Column(
                modifier = Modifier.fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    GlowRing(
                        fraction = fraction,
                        value = totalMl / 1000.0,
                        color = accent,
                        diameter = 184.dp,
                        lineWidth = 14.dp,
                        showsLabel = false,
                    )
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Text(
                            String.format(Locale.US, "%.1f", totalMl / 1000.0),
                            style = NoopType.title1.copy(fontWeight = FontWeight.Bold, fontSize = 40.sp),
                            color = Palette.textPrimary,
                        )
                        Text(
                            String.format(Locale.US, "of %.1f L", goalMl / 1000.0),
                            style = NoopType.subhead,
                            color = Palette.textSecondary,
                        )
                    }
                }
                Text(
                    "${kotlin.math.min(100, (fraction * 100).toInt())}% of today's goal",
                    style = NoopType.footnote,
                    color = Palette.textTertiary,
                )
            }
        }
        }

        // LOG BUTTONS — Sip / Cup / Bottle, secondary style, equal width.
        item {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.fillMaxWidth()) {
            NoopButton(
                text = "Sip",
                leadingIcon = Icons.Filled.WaterDrop,
                kind = NoopButtonKind.Secondary,
                modifier = Modifier.weight(1f),
            ) { log(HydrationGoal.SIP_ML) }
            NoopButton(
                text = "Cup",
                leadingIcon = Icons.Filled.LocalDrink,
                kind = NoopButtonKind.Secondary,
                modifier = Modifier.weight(1f),
            ) { log(HydrationGoal.CUP_ML) }
            NoopButton(
                text = "Bottle",
                leadingIcon = Icons.Filled.LocalDrink,
                kind = NoopButtonKind.Secondary,
                modifier = Modifier.weight(1f),
            ) { log(HydrationGoal.BOTTLE_ML) }
        }
        }
        // #798 - "Custom" (a bespoke container size). Full-width secondary button under the quick-add row;
        // opens the custom-amount dialog so any ml the presets don't cover can be logged.
        item {
        NoopButton(
            text = "Custom amount",
            leadingIcon = Icons.Filled.Add,
            kind = NoopButtonKind.Secondary,
            modifier = Modifier.fillMaxWidth(),
        ) { showCustom = true }
        }
        item {
        Text(
            "Sip ${HydrationGoal.SIP_ML} ml · Cup ${HydrationGoal.CUP_ML} ml · Bottle ${HydrationGoal.BOTTLE_ML} ml",
            style = NoopType.footnote,
            color = Palette.textTertiary,
        )
        }

        // 7-DAY HISTORY — flat mini bars, today on the right.
        item {
        NoopCard(padding = 18.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Overline("Last 7 days")
                HydrationHistoryBars(history = history, goalMl = goalMl, accent = accent)
            }
        }
        }

        // TODAY'S TOTAL as a single read-out row (the MVP "logged entries" — the day total is the running
        // sum the store banks; per-tap rows aren't separately persisted, so we show the honest day figure).
        item {
        NoopCard(padding = 18.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Overline("Today")
                if (totalMl <= 0.0) {
                    Text(
                        "No drinks logged yet. Tap Sip, Cup or Bottle to start.",
                        style = NoopType.subhead,
                        color = Palette.textSecondary,
                    )
                } else {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            Icons.Filled.WaterDrop,
                            contentDescription = null,
                            tint = accent,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(Modifier.width(10.dp))
                        Text(
                            "Logged today",
                            style = NoopType.subhead,
                            color = Palette.textPrimary,
                            modifier = Modifier.weight(1f),
                        )
                        Text(
                            "${totalMl.toInt()} ml",
                            style = NoopType.headline.copy(fontWeight = FontWeight.SemiBold),
                            color = Palette.textPrimary,
                        )
                    }
                    // #798 - undo / correct affordances. "Undo last" appears once something has been logged
                    // this session and removes exactly that container from the day total. "Clear today" zeroes
                    // the day's total outright (the delete-everything correction). Both route through the
                    // store's clamped remove/set, so the total never goes negative.
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.fillMaxWidth()) {
                        lastLoggedMl?.let { last ->
                            NoopButton(
                                text = "Undo last ($last ml)",
                                leadingIcon = Icons.AutoMirrored.Filled.Undo,
                                kind = NoopButtonKind.Secondary,
                                modifier = Modifier.weight(1f),
                            ) { remove(last) }
                        }
                        NoopButton(
                            text = "Clear today",
                            leadingIcon = Icons.Filled.Delete,
                            kind = NoopButtonKind.Secondary,
                            modifier = Modifier.weight(1f),
                        ) { remove(totalMl.toInt()) }
                    }
                }
            }
        }
        }

        item {
        Text(
            "A simple goal that adjusts to your effort. General wellness guidance, not medical advice.",
            style = NoopType.footnote,
            color = Palette.textTertiary,
            textAlign = TextAlign.Start,
        )
        }
    }
}

/**
 * The 7-day mini bar history: one flat rounded bar per day, height = day total ÷ goal (clamped to 1), with
 * the weekday initial beneath. Today's bar is the accent blue; prior days a muted accent. Empty days render
 * a faint track. Pure Compose Canvas + a row of labels (design-reset flat style, no gridlines/axes).
 */
@Composable
private fun HydrationHistoryBars(
    history: List<Pair<String, Double>>,
    goalMl: Int,
    accent: Color,
) {
    if (history.isEmpty()) {
        Text("No history yet.", style = NoopType.footnote, color = Palette.textTertiary)
        return
    }
    val goal = goalMl.coerceAtLeast(1).toDouble()
    val track = Palette.textPrimary.copy(alpha = 0.10f)
    val priorBar = accent.copy(alpha = 0.45f)
    val lastIndex = history.lastIndex
    val maxMl = history.maxOf { it.second }
    // Scale the bars to the LARGER of the goal and the biggest day, so an over-goal day doesn't clip.
    val ceiling = kotlin.math.max(goal, maxMl).coerceAtLeast(1.0)

    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Canvas(modifier = Modifier.fillMaxWidth().height(96.dp)) {
            val n = history.size
            val gap = 10.dp.toPx()
            val barW = (size.width - gap * (n - 1)) / n
            val corner = 6.dp.toPx()
            history.forEachIndexed { i, (_, ml) ->
                val x = i * (barW + gap)
                // Track (full-height faint bar).
                drawRoundRectBar(x, 0f, barW, size.height, corner, track)
                val frac = (ml / ceiling).toFloat().coerceIn(0f, 1f)
                if (frac > 0f) {
                    val h = size.height * frac
                    val color = if (i == lastIndex) accent else priorBar
                    drawRoundRectBar(x, size.height - h, barW, h, corner, color)
                }
            }
        }
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            history.forEach { (dayKey, _) ->
                Text(
                    weekdayInitial(dayKey),
                    style = NoopType.overline.copy(letterSpacing = 0.2.sp),
                    color = Palette.textTertiary,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.weight(1f),
                )
            }
        }
    }
}

/** Draw one rounded-rect bar via the Canvas draw scope (small helper so the bar loop stays readable). */
private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawRoundRectBar(
    x: Float,
    y: Float,
    w: Float,
    h: Float,
    corner: Float,
    color: Color,
) {
    drawRoundRect(
        color = color,
        topLeft = Offset(x, y),
        size = Size(w, h),
        cornerRadius = androidx.compose.ui.geometry.CornerRadius(corner, corner),
    )
}

/** The single-letter weekday for a yyyy-MM-dd key (M T W T F S S), or "·" when unparseable. */
private fun weekdayInitial(dayKey: String): String =
    runCatching {
        LocalDate.parse(dayKey).format(DateTimeFormatter.ofPattern("EEE", Locale.US)).take(1)
    }.getOrDefault("·")

/**
 * #798 - the custom-amount dialog. A single numeric ml field (the "custom container size") with a clamped
 * confirm: the Log button is disabled until the text parses to a positive whole-ml value (1..MAX_CUSTOM_ML
 * via [parseCustomHydrationMl]), and confirming hands the parsed amount back to the caller's additive log.
 * Tokens-only on the hydration-blue field; mirrors the iOS custom-amount sheet.
 */
@Composable
private fun CustomAmountDialog(
    accent: Color,
    onDismiss: () -> Unit,
    onConfirm: (Int) -> Unit,
) {
    var text by remember { mutableStateOf("") }
    val parsed = parseCustomHydrationMl(text)
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Palette.surfaceOverlay,
        title = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    Icons.Filled.WaterDrop,
                    contentDescription = null,
                    tint = accent,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(10.dp))
                Text("Custom amount", style = NoopType.title2, color = Palette.textPrimary)
            }
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = text,
                    onValueChange = { new -> text = new.filter { it.isDigit() }.take(5) },
                    label = { Text("Millilitres (ml)", style = NoopType.footnote) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Palette.textPrimary,
                        unfocusedTextColor = Palette.textPrimary,
                        cursorColor = accent,
                        focusedBorderColor = accent,
                        unfocusedBorderColor = Palette.hairline,
                        focusedLabelColor = accent,
                        unfocusedLabelColor = Palette.textSecondary,
                        focusedContainerColor = Palette.surfaceInset,
                        unfocusedContainerColor = Palette.surfaceInset,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    "Enter any amount from 1 to $MAX_CUSTOM_ML ml.",
                    style = NoopType.footnote,
                    color = if (text.isNotEmpty() && parsed == null) Palette.statusWarning else Palette.textTertiary,
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { parsed?.let(onConfirm) }, enabled = parsed != null) {
                Text("Log", color = if (parsed != null) accent else Palette.textTertiary)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = Palette.textSecondary)
            }
        },
    )
}
