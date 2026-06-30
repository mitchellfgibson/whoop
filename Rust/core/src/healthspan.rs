//! Healthspan: a transparent, WHOOP-Age-*style* physiological-age estimate.
//!
//! This module computes two numbers from longitudinal health metrics:
//!
//! - **WHOOP Age** (`whoop_age_years`): an estimate of physiological age. It is
//!   the chronological age plus the sum of per-metric "year deltas" derived from
//!   a long (~6 month) average. Slow-moving by design.
//! - **Pace of Aging** (`pace_of_aging`): a speedometer in the range `-1.0..=3.0`
//!   comparing the recent (~30 day) window against the long baseline. `1.0` means
//!   physiological age is drifting up in step with time; below `1.0` is "aging
//!   slower"; above `1.0` is "aging faster".
//!
//! ## Independence and provenance
//!
//! Goose is not affiliated with WHOOP. WHOOP's exact thresholds are proprietary
//! and are **not** used here. Every constant in [`Reference`] is a documented,
//! tunable value grounded in published longevity/epidemiology research, with the
//! anchoring principle that adult all-cause mortality risk rises roughly 10% per
//! chronological year — so a metric deviation worth "+10% risk" is treated as
//! "+1 year". These are first-pass v1 constants meant to be tuned against real
//! data; they are deliberately all collected in one place so tuning is one edit.
//!
//! ## Design
//!
//! This module is intentionally **pure**: it has no store/DB dependency. Callers
//! gather the metric series, average them over the relevant windows, and hand a
//! [`HealthspanInput`] in. That keeps the model trivially unit-testable and lets
//! the bridge layer own where the numbers come from. Each contributing metric
//! reports its own year delta in [`MetricContribution`] so the UI can explain
//! exactly which habits are aging the user up or down.

use serde::{Deserialize, Serialize};

pub const GOOSE_HEALTHSPAN_V0_ID: &str = "goose.healthspan.v0";
pub const GOOSE_HEALTHSPAN_V0_VERSION: &str = "0.1.0";

/// Target coverage, in days, before each output is considered non-preliminary.
///
/// Pace compares the recent window to baseline, so it stabilizes faster than the
/// slow WHOOP Age baseline. We light up a *preliminary* score after only a week
/// "for fun", but flag it until these targets are met.
pub const PACE_TARGET_DAYS: u32 = 30;
pub const WHOOP_AGE_TARGET_DAYS: u32 = 180;
/// Minimum coverage before we will emit any score at all.
pub const MINIMUM_DAYS_FOR_PRELIMINARY: u32 = 7;

/// Per-metric reference values and "years per unit of deviation" sensitivities.
///
/// All values are tunable. Each field documents its unit, its "optimal" anchor,
/// and the source/rationale for the sensitivity. `*_years_per_*` fields express
/// how many physiological years a one-unit deviation from the optimum adds (a
/// positive year delta = ages you up).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Reference {
    // --- Fitness ---
    /// Optimal resting heart rate (bpm). Below this contributes no penalty.
    pub rhr_optimal_bpm: f64,
    /// Anchor: +10 bpm RHR ≈ +1 year → 0.1 years per bpm above optimal.
    pub rhr_years_per_bpm_above: f64,

    /// Optimal VO2 max (ml/kg/min). VO2 max is the strongest single longevity
    /// signal, so deltas can move WHOOP Age substantially.
    pub vo2max_optimal: f64,
    /// Years added per 1 ml/kg/min *below* optimal (subtracts years when above).
    pub vo2max_years_per_unit_below: f64,

    /// Optimal lean body mass fraction of total mass (0..1). Optional input.
    pub lean_mass_optimal_fraction: f64,
    /// Years added per 0.01 (1 percentage point) below optimal.
    pub lean_mass_years_per_point_below: f64,

    // --- Sleep ---
    /// Optimal nightly sleep (hours).
    pub sleep_optimal_hours: f64,
    /// Years added per hour of nightly sleep below optimal.
    pub sleep_years_per_hour_below: f64,
    /// Optimal sleep consistency (0..1, 1 = perfectly regular schedule).
    pub sleep_consistency_optimal: f64,
    /// Years added per 0.1 below optimal consistency.
    pub sleep_consistency_years_per_tenth_below: f64,

    // --- Strain / activity ---
    /// Optimal weekly minutes in HR zones 1–3 (aerobic base).
    pub zone1_3_optimal_weekly_minutes: f64,
    /// Years added per 60 weekly minutes below optimal.
    pub zone1_3_years_per_hour_below: f64,
    /// Optimal weekly minutes in HR zones 4–5 (high intensity).
    pub zone4_5_optimal_weekly_minutes: f64,
    /// Years added per 30 weekly minutes below optimal.
    pub zone4_5_years_per_half_hour_below: f64,
    /// Optimal weekly minutes of strength activity.
    pub strength_optimal_weekly_minutes: f64,
    /// Years added per 30 weekly minutes below optimal.
    pub strength_years_per_half_hour_below: f64,
    /// Optimal daily steps.
    pub steps_optimal_daily: f64,
    /// Years added per 1000 daily steps below optimal.
    pub steps_years_per_thousand_below: f64,

    /// Per-metric clamp: each metric's |year delta| is capped at this many years
    /// so no single bad input can dominate the estimate.
    pub per_metric_clamp_years: f64,
}

impl Default for Reference {
    /// First-pass v1 constants. Sources noted inline; tune against real cohorts.
    fn default() -> Self {
        Self {
            // RHR: lower is better; ~50–60 bpm is a common "fit adult" optimum.
            rhr_optimal_bpm: 55.0,
            rhr_years_per_bpm_above: 0.10, // anchor: +10 bpm ≈ +1 yr
            // VO2 max: elite/healthy ~50; each point below meaningfully raises risk.
            vo2max_optimal: 50.0,
            vo2max_years_per_unit_below: 0.30,
            // Lean mass: ~0.80 lean fraction as a healthy reference.
            lean_mass_optimal_fraction: 0.80,
            lean_mass_years_per_point_below: 0.15,
            // Sleep: 8h optimum; chronic short sleep raises mortality risk.
            sleep_optimal_hours: 8.0,
            sleep_years_per_hour_below: 0.50,
            sleep_consistency_optimal: 0.90,
            sleep_consistency_years_per_tenth_below: 0.40,
            // Aerobic base: ~150 min/wk moderate (WHO) → zones 1–3.
            zone1_3_optimal_weekly_minutes: 150.0,
            zone1_3_years_per_hour_below: 0.30,
            // High intensity: ~75 min/wk vigorous → zones 4–5.
            zone4_5_optimal_weekly_minutes: 75.0,
            zone4_5_years_per_half_hour_below: 0.25,
            // Strength: ~2×/wk ≈ 60 min/wk.
            strength_optimal_weekly_minutes: 60.0,
            strength_years_per_half_hour_below: 0.20,
            // Steps: ~8000/day associated with lower mortality.
            steps_optimal_daily: 8000.0,
            steps_years_per_thousand_below: 0.15,
            per_metric_clamp_years: 10.0,
        }
    }
}

/// Averaged metric values for one window (recent ~30d or long ~180d baseline).
///
/// All fields are `Option`; missing inputs simply contribute no year delta and
/// are reported as `available: false` so the UI can show coverage honestly.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct MetricWindow {
    pub resting_hr_bpm: Option<f64>,
    pub vo2max: Option<f64>,
    /// Lean body mass as a fraction of total mass (0..1). Optional metric.
    pub lean_mass_fraction: Option<f64>,
    pub sleep_hours: Option<f64>,
    /// Sleep consistency 0..1.
    pub sleep_consistency: Option<f64>,
    pub zone1_3_weekly_minutes: Option<f64>,
    pub zone4_5_weekly_minutes: Option<f64>,
    pub strength_weekly_minutes: Option<f64>,
    pub steps_daily: Option<f64>,
}

/// Full input to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthspanInput {
    /// Chronological age in years.
    pub chronological_age_years: f64,
    /// Long baseline window (~180 days) used for WHOOP Age.
    pub baseline_window: MetricWindow,
    /// Recent window (~30 days) used for Pace of Aging.
    pub recent_window: MetricWindow,
    /// Days of data actually present in the baseline window.
    pub baseline_days_present: u32,
    /// Days of data actually present in the recent window.
    pub recent_days_present: u32,
    /// Optional override of the reference constants (defaults if absent).
    #[serde(default)]
    pub reference: Option<Reference>,
}

/// One metric's contribution to WHOOP Age, for UI breakdowns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricContribution {
    /// Stable key, e.g. `"resting_heart_rate"`.
    pub metric: String,
    /// Was this metric present in the baseline window?
    pub available: bool,
    /// The averaged value used (None when unavailable).
    pub value: Option<f64>,
    pub unit: String,
    /// Years this metric added (positive) or subtracted (negative).
    pub year_delta: f64,
}

/// Coverage and preliminary status for a single output.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Coverage {
    pub days_present: u32,
    pub days_target: u32,
    /// True until `days_present >= days_target`.
    pub preliminary: bool,
}

impl Coverage {
    fn new(days_present: u32, days_target: u32) -> Self {
        Self {
            days_present,
            days_target,
            preliminary: days_present < days_target,
        }
    }
}

/// The computed Healthspan summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthspanSummary {
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub chronological_age_years: f64,
    /// Estimated physiological age.
    pub whoop_age_years: f64,
    /// `whoop_age_years - chronological_age_years` (positive = older than age).
    pub age_delta_years: f64,
    /// Speedometer in `-1.0..=3.0`.
    pub pace_of_aging: f64,
    /// Per-metric breakdown (baseline window), most-aging first.
    pub contributions: Vec<MetricContribution>,
    pub whoop_age_coverage: Coverage,
    pub pace_coverage: Coverage,
}

/// Outcome of attempting to compute a summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HealthspanResult {
    /// Enough data for at least a preliminary estimate.
    Ready(HealthspanSummary),
    /// Not enough data yet; reports how close we are.
    Warming {
        days_present: u32,
        days_required: u32,
    },
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// Compute the year delta for a "lower is better above optimum" metric (e.g. RHR):
/// only values above optimum add years.
fn delta_above(value: f64, optimal: f64, years_per_unit: f64, clamp_years: f64) -> f64 {
    let raw = (value - optimal).max(0.0) * years_per_unit;
    clamp(raw, -clamp_years, clamp_years)
}

/// Compute the year delta for a "higher is better" metric (e.g. VO2 max, sleep,
/// steps): below optimum adds years, above optimum subtracts them.
fn delta_below(value: f64, optimal: f64, years_per_unit: f64, clamp_years: f64) -> f64 {
    let raw = (optimal - value) * years_per_unit;
    clamp(raw, -clamp_years, clamp_years)
}

/// Build the per-metric contributions for a window. Returns the contributions and
/// the summed year delta over available metrics.
fn contributions_for(window: &MetricWindow, reference: &Reference) -> (Vec<MetricContribution>, f64) {
    let clamp_years = reference.per_metric_clamp_years;
    let mut out = Vec::new();

    // Helper closure that pushes a contribution and accumulates the delta.
    let mut push = |metric: &str, value: Option<f64>, unit: &str, delta_fn: &dyn Fn(f64) -> f64| {
        match value {
            Some(v) => {
                let delta = delta_fn(v);
                out.push(MetricContribution {
                    metric: metric.to_string(),
                    available: true,
                    value: Some(v),
                    unit: unit.to_string(),
                    year_delta: delta,
                });
            }
            None => out.push(MetricContribution {
                metric: metric.to_string(),
                available: false,
                value: None,
                unit: unit.to_string(),
                year_delta: 0.0,
            }),
        }
    };

    push(
        "resting_heart_rate",
        window.resting_hr_bpm,
        "bpm",
        &|v| delta_above(v, reference.rhr_optimal_bpm, reference.rhr_years_per_bpm_above, clamp_years),
    );
    push(
        "vo2_max",
        window.vo2max,
        "ml_per_kg_per_min",
        &|v| delta_below(v, reference.vo2max_optimal, reference.vo2max_years_per_unit_below, clamp_years),
    );
    push(
        "lean_body_mass",
        window.lean_mass_fraction,
        "fraction",
        &|v| {
            // Convert "per point (0.01)" sensitivity into per-fraction units.
            delta_below(
                v,
                reference.lean_mass_optimal_fraction,
                reference.lean_mass_years_per_point_below * 100.0,
                clamp_years,
            )
        },
    );
    push(
        "sleep_hours",
        window.sleep_hours,
        "hours",
        &|v| delta_below(v, reference.sleep_optimal_hours, reference.sleep_years_per_hour_below, clamp_years),
    );
    push(
        "sleep_consistency",
        window.sleep_consistency,
        "fraction",
        &|v| {
            // "per tenth (0.1)" sensitivity → per-fraction units.
            delta_below(
                v,
                reference.sleep_consistency_optimal,
                reference.sleep_consistency_years_per_tenth_below * 10.0,
                clamp_years,
            )
        },
    );
    push(
        "zone_1_3_weekly_minutes",
        window.zone1_3_weekly_minutes,
        "minutes_per_week",
        &|v| {
            // "per hour (60 min)" sensitivity → per-minute units.
            delta_below(
                v,
                reference.zone1_3_optimal_weekly_minutes,
                reference.zone1_3_years_per_hour_below / 60.0,
                clamp_years,
            )
        },
    );
    push(
        "zone_4_5_weekly_minutes",
        window.zone4_5_weekly_minutes,
        "minutes_per_week",
        &|v| {
            // "per half hour (30 min)" sensitivity → per-minute units.
            delta_below(
                v,
                reference.zone4_5_optimal_weekly_minutes,
                reference.zone4_5_years_per_half_hour_below / 30.0,
                clamp_years,
            )
        },
    );
    push(
        "strength_weekly_minutes",
        window.strength_weekly_minutes,
        "minutes_per_week",
        &|v| {
            delta_below(
                v,
                reference.strength_optimal_weekly_minutes,
                reference.strength_years_per_half_hour_below / 30.0,
                clamp_years,
            )
        },
    );
    push(
        "daily_steps",
        window.steps_daily,
        "steps_per_day",
        &|v| {
            // "per thousand" sensitivity → per-step units.
            delta_below(
                v,
                reference.steps_optimal_daily,
                reference.steps_years_per_thousand_below / 1000.0,
                clamp_years,
            )
        },
    );

    let total: f64 = out.iter().map(|c| c.year_delta).sum();
    (out, total)
}

/// Compute a Healthspan summary, or report that we are still warming up.
pub fn compute(input: &HealthspanInput) -> HealthspanResult {
    if input.baseline_days_present < MINIMUM_DAYS_FOR_PRELIMINARY {
        return HealthspanResult::Warming {
            days_present: input.baseline_days_present,
            days_required: MINIMUM_DAYS_FOR_PRELIMINARY,
        };
    }

    let reference = input.reference.unwrap_or_default();

    // WHOOP Age comes from the long baseline window.
    let (mut contributions, baseline_total) = contributions_for(&input.baseline_window, &reference);
    let whoop_age_years = input.chronological_age_years + baseline_total;
    let age_delta_years = whoop_age_years - input.chronological_age_years;

    // Sort contributions most-aging first for the UI breakdown; unavailable last.
    contributions.sort_by(|a, b| {
        b.year_delta
            .partial_cmp(&a.year_delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Pace of Aging: compare the recent window's total year delta to the baseline.
    // A worse recent window (more years added) → faster pace. We map the gap onto
    // the -1.0..=3.0 range, with parity (1.0) when recent matches baseline.
    let (_recent_contribs, recent_total) = contributions_for(&input.recent_window, &reference);
    let pace_of_aging = pace_from_totals(recent_total, baseline_total);

    HealthspanResult::Ready(HealthspanSummary {
        algorithm_id: GOOSE_HEALTHSPAN_V0_ID.to_string(),
        algorithm_version: GOOSE_HEALTHSPAN_V0_VERSION.to_string(),
        chronological_age_years: input.chronological_age_years,
        whoop_age_years,
        age_delta_years,
        pace_of_aging,
        contributions,
        whoop_age_coverage: Coverage::new(input.baseline_days_present, WHOOP_AGE_TARGET_DAYS),
        pace_coverage: Coverage::new(input.recent_days_present, PACE_TARGET_DAYS),
    })
}

/// Map (recent_total_years, baseline_total_years) onto a Pace in `-1.0..=3.0`.
///
/// Parity (recent == baseline) is `1.0`. Each year that the recent window is
/// *worse* than baseline adds `PACE_YEARS_PER_STEP` to the pace; each year better
/// subtracts it. Clamped to the documented range.
fn pace_from_totals(recent_total: f64, baseline_total: f64) -> f64 {
    /// How much one year of recent-vs-baseline gap moves the speedometer.
    const PACE_YEARS_PER_STEP: f64 = 0.5;
    let gap = recent_total - baseline_total;
    clamp(1.0 + gap * PACE_YEARS_PER_STEP, -1.0, 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window where every metric sits exactly at its optimum → zero year delta.
    fn optimal_window() -> MetricWindow {
        let r = Reference::default();
        MetricWindow {
            resting_hr_bpm: Some(r.rhr_optimal_bpm),
            vo2max: Some(r.vo2max_optimal),
            lean_mass_fraction: Some(r.lean_mass_optimal_fraction),
            sleep_hours: Some(r.sleep_optimal_hours),
            sleep_consistency: Some(r.sleep_consistency_optimal),
            zone1_3_weekly_minutes: Some(r.zone1_3_optimal_weekly_minutes),
            zone4_5_weekly_minutes: Some(r.zone4_5_optimal_weekly_minutes),
            strength_weekly_minutes: Some(r.strength_optimal_weekly_minutes),
            steps_daily: Some(r.steps_optimal_daily),
        }
    }

    fn ready(result: HealthspanResult) -> HealthspanSummary {
        match result {
            HealthspanResult::Ready(s) => s,
            HealthspanResult::Warming { .. } => panic!("expected Ready, got Warming"),
        }
    }

    #[test]
    fn warming_up_below_minimum_days() {
        let input = HealthspanInput {
            chronological_age_years: 30.0,
            baseline_window: optimal_window(),
            recent_window: optimal_window(),
            baseline_days_present: 3,
            recent_days_present: 3,
            reference: None,
        };
        match compute(&input) {
            HealthspanResult::Warming {
                days_present,
                days_required,
            } => {
                assert_eq!(days_present, 3);
                assert_eq!(days_required, MINIMUM_DAYS_FOR_PRELIMINARY);
            }
            HealthspanResult::Ready(_) => panic!("expected Warming"),
        }
    }

    #[test]
    fn optimal_inputs_match_chronological_age() {
        let input = HealthspanInput {
            chronological_age_years: 30.0,
            baseline_window: optimal_window(),
            recent_window: optimal_window(),
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        assert!((summary.whoop_age_years - 30.0).abs() < 1e-9);
        assert!((summary.age_delta_years - 0.0).abs() < 1e-9);
        // Recent equals baseline → pace at parity.
        assert!((summary.pace_of_aging - 1.0).abs() < 1e-9);
        // Full windows → not preliminary.
        assert!(!summary.whoop_age_coverage.preliminary);
        assert!(!summary.pace_coverage.preliminary);
    }

    #[test]
    fn rhr_anchor_ten_bpm_is_one_year() {
        // Only RHR deviates: +10 bpm over optimum should add ~1 year.
        let mut window = optimal_window();
        let r = Reference::default();
        window.resting_hr_bpm = Some(r.rhr_optimal_bpm + 10.0);
        let input = HealthspanInput {
            chronological_age_years: 40.0,
            baseline_window: window,
            recent_window: optimal_window(),
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        assert!(
            (summary.age_delta_years - 1.0).abs() < 1e-9,
            "expected +1 year, got {}",
            summary.age_delta_years
        );
        let rhr = summary
            .contributions
            .iter()
            .find(|c| c.metric == "resting_heart_rate")
            .unwrap();
        assert!((rhr.year_delta - 1.0).abs() < 1e-9);
        assert!(rhr.available);
    }

    #[test]
    fn good_fitness_subtracts_years() {
        // High VO2 max and plenty of steps → physiologically younger.
        let mut window = optimal_window();
        window.vo2max = Some(60.0); // 10 above optimum → -3 years
        let input = HealthspanInput {
            chronological_age_years: 45.0,
            baseline_window: window,
            recent_window: optimal_window(),
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        assert!(
            summary.whoop_age_years < 45.0,
            "expected younger than 45, got {}",
            summary.whoop_age_years
        );
        assert!((summary.age_delta_years - (-3.0)).abs() < 1e-9);
    }

    #[test]
    fn missing_metric_is_unavailable_not_penalized() {
        let mut window = optimal_window();
        window.lean_mass_fraction = None; // optional, omitted
        let input = HealthspanInput {
            chronological_age_years: 30.0,
            baseline_window: window,
            recent_window: optimal_window(),
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        // Still equals chronological age; missing metric contributes 0.
        assert!((summary.whoop_age_years - 30.0).abs() < 1e-9);
        let lean = summary
            .contributions
            .iter()
            .find(|c| c.metric == "lean_body_mass")
            .unwrap();
        assert!(!lean.available);
        assert_eq!(lean.value, None);
        assert_eq!(lean.year_delta, 0.0);
    }

    #[test]
    fn preliminary_flag_set_under_target_windows() {
        let input = HealthspanInput {
            chronological_age_years: 25.0,
            baseline_window: optimal_window(),
            recent_window: optimal_window(),
            baseline_days_present: 10, // ≥ minimum (7) but < 180
            recent_days_present: 10,   // < 30
            reference: None,
        };
        let summary = ready(compute(&input));
        assert!(summary.whoop_age_coverage.preliminary);
        assert!(summary.pace_coverage.preliminary);
        assert_eq!(summary.whoop_age_coverage.days_target, WHOOP_AGE_TARGET_DAYS);
        assert_eq!(summary.pace_coverage.days_target, PACE_TARGET_DAYS);
    }

    #[test]
    fn worse_recent_window_raises_pace_above_one() {
        // Baseline optimal, but recent window has poor sleep and high RHR.
        let mut recent = optimal_window();
        recent.sleep_hours = Some(6.0); // 2h short
        recent.resting_hr_bpm = Some(75.0); // 20 bpm high
        let input = HealthspanInput {
            chronological_age_years: 35.0,
            baseline_window: optimal_window(),
            recent_window: recent,
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        assert!(
            summary.pace_of_aging > 1.0,
            "expected pace > 1.0, got {}",
            summary.pace_of_aging
        );
        assert!(summary.pace_of_aging <= 3.0);
    }

    #[test]
    fn pace_is_clamped_to_range() {
        // Catastrophically bad recent window should clamp at 3.0, not exceed it.
        let recent = MetricWindow {
            resting_hr_bpm: Some(140.0),
            vo2max: Some(10.0),
            lean_mass_fraction: Some(0.3),
            sleep_hours: Some(2.0),
            sleep_consistency: Some(0.1),
            zone1_3_weekly_minutes: Some(0.0),
            zone4_5_weekly_minutes: Some(0.0),
            strength_weekly_minutes: Some(0.0),
            steps_daily: Some(0.0),
        };
        let input = HealthspanInput {
            chronological_age_years: 50.0,
            baseline_window: optimal_window(),
            recent_window: recent,
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        assert!((summary.pace_of_aging - 3.0).abs() < 1e-9);
    }

    #[test]
    fn contributions_sorted_most_aging_first() {
        let mut window = optimal_window();
        window.resting_hr_bpm = Some(85.0); // big positive delta
        let input = HealthspanInput {
            chronological_age_years: 30.0,
            baseline_window: window,
            recent_window: optimal_window(),
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        // First contribution should be the worst (largest positive) one.
        let first = &summary.contributions[0];
        assert_eq!(first.metric, "resting_heart_rate");
        for pair in summary.contributions.windows(2) {
            assert!(pair[0].year_delta >= pair[1].year_delta);
        }
    }

    #[test]
    fn summary_round_trips_through_json() {
        let input = HealthspanInput {
            chronological_age_years: 33.0,
            baseline_window: optimal_window(),
            recent_window: optimal_window(),
            baseline_days_present: 200,
            recent_days_present: 40,
            reference: None,
        };
        let summary = ready(compute(&input));
        let json = serde_json::to_string(&summary).unwrap();
        let decoded: HealthspanSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary, decoded);
    }
}
