package com.noop.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.noop.analytics.BalanceRead
import com.noop.analytics.RestScorer
import com.noop.analytics.WeeklyDigest
import com.noop.analytics.WeeklyDigestEngine
import com.noop.analytics.WeeklyMetric
import com.noop.analytics.WeeklyMetricSummary
import com.noop.data.DailyMetric
import kotlin.math.abs
import kotlin.math.roundToInt

// MARK: - Weekly Digest (#208)
//
// A deterministic, offline "week in review". Kotlin parity for the macOS/iOS
// WeeklyDigestView. Reads the merged daily history from the view model, pulls each
// tracked metric into a "yyyy-MM-dd"→value map, and feeds the pure
// WeeklyDigestEngine to produce a Monday-anchored summary: per-metric this-week
// mean + week-over-week delta + vs-baseline, the biggest movers, a strain-vs-recovery
// balance read, and 1–2 plain-English focal points. No AI, no network.
//
// Two surfaces are exposed so navigation can wire whichever it wants:
//   • WeeklyDigestCard  — an embeddable card (drop into Today / Trends).
//   • WeeklyDigestScreen — a full ScreenScaffold screen (for a nav destination).
// Both share WeeklyDigestContent so they never drift. Framing is informational
// (non-clinical), consistent with the app disclaimer.

/**
 * Build the weekly digest for the week containing today's logical local day from a
 * [DailyMetric] history. Extracts each metric into a day→value map and hands it to the
 * pure engine.
 */
fun buildWeeklyDigest(
    days: List<DailyMetric>,
    anchorDay: String = logicalDayKeyNow(),
): WeeklyDigest {
    val charge = HashMap<String, Double>()
    val effort = HashMap<String, Double>()
    val rest = HashMap<String, Double>()
    val rhr = HashMap<String, Double>()
    val hrv = HashMap<String, Double>()
    for (d in days) {
        d.recovery?.let { charge[d.day] = it }
        d.strain?.let { effort[d.day] = it }
        // Rest = the sleep-performance composite recomputed on the persisted day.
        RestScorer.restFromDaily(d)?.let { rest[d.day] = it }
        d.restingHr?.let { rhr[d.day] = it.toDouble() }
        d.avgHrv?.let { hrv[d.day] = it }
    }
    return WeeklyDigestEngine.build(
        byMetric = mapOf(
            WeeklyMetric.CHARGE to charge,
            WeeklyMetric.EFFORT to effort,
            WeeklyMetric.REST to rest,
            WeeklyMetric.RHR to rhr,
            WeeklyMetric.HRV to hrv,
        ),
        anchorDay = anchorDay,
    )
}

// MARK: - Embeddable card

/**
 * The weekly digest as a single card (for Today / Trends). Renders nothing when there's
 * no data this week, so it's safe to always place.
 */
@Composable
fun WeeklyDigestCard(vm: AppViewModel, modifier: Modifier = Modifier) {
    val days by vm.recentDays.collectAsStateWithLifecycle()
    val digest = buildWeeklyDigest(days)
    if (digest.isEmpty) return
    NoopCard(modifier = modifier) {
        WeeklyDigestContent(digest = digest, compact = true)
    }
}

// MARK: - Full screen

/** The weekly digest as a full screen (for a nav destination). */
@Composable
fun WeeklyDigestScreen(vm: AppViewModel) {
    val days by vm.recentDays.collectAsStateWithLifecycle()
    ScreenScaffold(title = "Week in review", subtitle = "Your Monday-to-Sunday, read in one glance.") {
        val digest = buildWeeklyDigest(days)
        if (digest.isEmpty) {
            DataPendingNote(
                title = "No readings this week yet",
                body = "Wear your strap or import your WHOOP export in Data Sources. Once this week has a " +
                    "day or two of data, your week-in-review appears here.",
            )
        } else {
            NoopCard { WeeklyDigestContent(digest = digest, compact = false) }
        }
    }
}

// MARK: - Shared content

private val MONTHS = arrayOf(
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
)

private val DISPLAY_ORDER = listOf(
    WeeklyMetric.CHARGE, WeeklyMetric.EFFORT, WeeklyMetric.REST, WeeklyMetric.HRV, WeeklyMetric.RHR,
)

/**
 * The inner content shared by the card and the full screen. [compact] trims the metric
 * grid to the headline rows for the card; the full screen shows everything plus a footer.
 */
@Composable
fun WeeklyDigestContent(digest: WeeklyDigest, compact: Boolean = false) {
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        // Header.
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Overline("Week in review")
                Text(weekRangeLabel(digest), style = NoopType.title2, color = Palette.textPrimary)
            }
            Text(
                "${digest.daysWithData}/7 days",
                style = NoopType.footnote,
                color = Palette.textSecondary,
                modifier = Modifier.semantics {
                    contentDescription = "${digest.daysWithData} of 7 days had data this week"
                },
            )
        }

        // Focal points — the plain-English read, most salient first.
        if (digest.focalPoints.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                digest.focalPoints.forEach { FocalRow(it) }
            }
        }

        HorizontalDivider(color = Palette.hairline)

        // Per-metric rows.
        val rows = (if (compact) listOf(WeeklyMetric.CHARGE, WeeklyMetric.EFFORT, WeeklyMetric.REST)
        else DISPLAY_ORDER).mapNotNull { digest.summary(it) }
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            rows.forEach { MetricRow(it) }
        }

        if (!compact) {
            HorizontalDivider(color = Palette.hairline)
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                digest.sleepConsistencySD?.let { sd ->
                    Text(
                        "Sleep steadiness: Rest varied ±${fmt1(sd)} pts night to night.",
                        style = NoopType.footnote,
                        color = Palette.textTertiary,
                    )
                }
                Text(digest.balance.sentence, style = NoopType.footnote, color = Palette.textTertiary)
                Text(
                    "Informational only — not medical advice.",
                    style = NoopType.footnote,
                    color = Palette.textTertiary,
                )
            }
        }
    }
}

@Composable
private fun FocalRow(line: String) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.Top,
        modifier = Modifier.semantics(mergeDescendants = true) { contentDescription = line },
    ) {
        Icon(
            Icons.Filled.AutoAwesome,
            contentDescription = null,
            tint = Palette.accent,
            modifier = Modifier.size(16.dp),
        )
        Text(line, style = NoopType.subhead, color = Palette.textPrimary)
    }
}

@Composable
private fun MetricRow(s: WeeklyMetricSummary) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) { contentDescription = rowAccessibility(s) },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            s.metric.label,
            style = NoopType.subhead,
            color = Palette.textSecondary,
            modifier = Modifier.width(92.dp),
        )
        Text(
            meanText(s),
            style = NoopType.bodyNumber,
            color = Palette.textPrimary,
            modifier = Modifier.width(64.dp),
        )
        Spacer(Modifier.weight(1f))
        DeltaChip(s)
    }
}

@Composable
private fun DeltaChip(s: WeeklyMetricSummary) {
    val tone = chipTone(s)
    val arrow: ImageVector = when {
        s.wowDelta > 0 -> Icons.Filled.ArrowUpward
        s.wowDelta < 0 -> Icons.Filled.ArrowDownward
        else -> Icons.Filled.Remove
    }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(3.dp),
        modifier = Modifier
            .background(tone.copy(alpha = 0.12f), RoundedCornerShape(Metrics.cornerPill))
            .padding(horizontal = 8.dp, vertical = 3.dp)
            .clearAndSetSemantics { },
    ) {
        Icon(arrow, contentDescription = null, tint = tone, modifier = Modifier.size(10.dp))
        Text(deltaText(s), style = NoopType.captionNumber, color = tone)
    }
}

// MARK: - Formatting

private fun weekRangeLabel(digest: WeeklyDigest): String =
    "${shortDate(digest.weekStart)} – ${shortDate(digest.weekEnd)}"

/** "Jun 8" from "2026-06-08", via the engine's own pure parse (no Calendar). */
private fun shortDate(ymd: String): String {
    val p = WeeklyDigestEngine.parseYMD(ymd) ?: return ymd
    val name = if (p[1] in 1..12) MONTHS[p[1] - 1] else p[1].toString()
    return "$name ${p[2]}"
}

private fun meanText(s: WeeklyMetricSummary): String {
    if (s.thisWeek.n == 0) return "—"
    val v = s.thisWeek.mean.roundToInt()
    return if (s.metric.unit.isEmpty()) "$v" else "$v ${s.metric.unit}"
}

private fun deltaText(s: WeeklyMetricSummary): String {
    if (s.weekOverWeek.current.n == 0 || s.weekOverWeek.previous.n == 0) return "new"
    val pct = s.weekOverWeek.pctChange
    return if (pct != null && abs(pct) >= 1) "${abs(pct).roundToInt()}%" else fmt1(abs(s.wowDelta))
}

/**
 * Tone: good moves green, bad moves rose, flat/uncomparable grey — folding in each
 * metric's higherIsBetter (so a Resting-HR rise reads as a warning).
 */
private fun chipTone(s: WeeklyMetricSummary): Color = when (s.wowGoodness) {
    1 -> Palette.statusPositive
    -1 -> Palette.statusCritical
    else -> Palette.textTertiary
}

private fun rowAccessibility(s: WeeklyMetricSummary): String {
    val mean = meanText(s)
    if (s.weekOverWeek.current.n == 0 || s.weekOverWeek.previous.n == 0) {
        return "${s.metric.label}: $mean this week, no comparison."
    }
    val dir = if (s.wowDelta > 0) "up" else if (s.wowDelta < 0) "down" else "unchanged"
    val frame = when (s.wowGoodness) {
        1 -> ", a good sign"
        -1 -> ", worth a look"
        else -> ""
    }
    return "${s.metric.label}: $mean this week, $dir ${deltaText(s)} week over week$frame."
}

private fun fmt1(x: Double): String = ((x * 10).roundToInt() / 10.0).toString()
