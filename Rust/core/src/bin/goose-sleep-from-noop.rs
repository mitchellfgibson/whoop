//! Live-only sleep for a WHOOP 5.0: detect and score last night's sleep from the
//! overnight HR + R-R signal that the strap streams continuously, with NO band
//! historical offload (which is gated on a 5.0 — see whoop5-offload-dead-end memory).
//!
//! Input: NOOP's local database (`~/Library/Application Support/OpenWhoop/whoop.sqlite`),
//! tables `hrSample(deviceId, ts, bpm)` and `rrInterval(deviceId, ts, rrMs)`.
//!
//! Pipeline:
//!   1. Pull HR (and RR) for an overnight window (default: 6pm yesterday → noon today).
//!   2. Detect the main sleep window = the longest contiguous low/stable-HR stretch
//!      (HR ≤ min + 25% of the min→median range), tolerating brief arousals.
//!   3. Derive the SleepV1 features (duration, HR dip, disturbances, HRV from RR).
//!   4. Score with goose_sleep_v1 and print the result.
//!
//! Usage:
//!   goose-sleep-from-noop [--db PATH] [--window-start-hour 18] [--window-end-hour 12]
//!                         [--night-offset-days 0] [--sleep-need-minutes 480] [--json]

use std::collections::BTreeMap;

use goose_core::metrics::{goose_sleep_v1, SleepInput, SleepV1Input};
use goose_core::tool_args::{args, flag, value};
use goose_core::{GooseError, GooseResult};
use rusqlite::Connection;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

#[derive(Clone, Copy)]
struct HrSample {
    ts: i64,
    bpm: i64,
}

fn run() -> GooseResult<()> {
    let args = args();
    let db_path = match value(&args, "--db")? {
        Some(p) => p,
        None => default_noop_db_path()?,
    };
    let window_start_hour: i64 = opt_i64(&args, "--window-start-hour")?.unwrap_or(18);
    let window_end_hour: i64 = opt_i64(&args, "--window-end-hour")?.unwrap_or(12);
    let night_offset_days: i64 = opt_i64(&args, "--night-offset-days")?.unwrap_or(0);
    let sleep_need_minutes: f64 = opt_f64(&args, "--sleep-need-minutes")?.unwrap_or(480.0);
    let as_json = flag(&args, "--json");

    let conn = Connection::open(&db_path)
        .map_err(|e| GooseError::message(format!("cannot open {db_path}: {e}")))?;

    // Window: [start_hour the evening before the target morning] → [end_hour the target morning].
    let now = unix_now();
    let target_midnight = local_midnight(now) - night_offset_days * 86_400;
    let window_start = target_midnight - (24 - window_start_hour) * 3_600;
    let window_end = target_midnight + window_end_hour * 3_600;

    let hr = load_hr(&conn, window_start, window_end)?;
    if hr.len() < 60 {
        return finish_unavailable(
            as_json,
            &format!(
                "only {} HR samples in the overnight window ({} → {} local) — need continuous overnight wear with NOOP capturing. Was the Mac awake and NOOP connected all night?",
                hr.len(),
                fmt_local(window_start),
                fmt_local(window_end)
            ),
        );
    }

    let Some(win) = detect_sleep_window(&hr) else {
        return finish_unavailable(
            as_json,
            "no sustained low-HR sleep window found in the overnight signal (need a ≥2h stable low-HR stretch)",
        );
    };

    // ----- Derive SleepV1 features from the detected window -----
    let sleep_samples = &hr[win.start_idx..=win.end_idx];
    let start_ts = sleep_samples.first().unwrap().ts;
    let end_ts = sleep_samples.last().unwrap().ts;
    let duration_minutes = ((end_ts - start_ts) as f64 / 60.0).max(1.0);

    let bpms: Vec<f64> = sleep_samples.iter().map(|s| s.bpm as f64).collect();
    let sleep_avg = bpms.iter().sum::<f64>() / bpms.len() as f64;
    let sleep_min = bpms.iter().cloned().fold(f64::INFINITY, f64::min);

    // Pre-sleep awake HR = mean of the hour before sleep onset (for the HR-dip feature).
    let pre = load_hr(&conn, start_ts - 3_600, start_ts)?;
    let pre_avg = if pre.is_empty() {
        win.median
    } else {
        pre.iter().map(|s| s.bpm as f64).sum::<f64>() / pre.len() as f64
    };
    let hr_dip_percent = if pre_avg > 0.0 {
        ((pre_avg - sleep_min) / pre_avg * 100.0).max(0.0)
    } else {
        0.0
    };

    // Disturbances = SUSTAINED arousals (HR above threshold for ≥ ~90s), counted
    // on the ~2-min-smoothed signal so sample-to-sample noise isn't mistaken for
    // waking.
    let win_ts: Vec<i64> = sleep_samples.iter().map(|s| s.ts).collect();
    let smooth_window = rolling_median_time(&bpms, &win_ts, 120);
    const MIN_DISTURBANCE_SECONDS: i64 = 90;
    let mut disturbances: u32 = 0;
    let mut above_since: Option<i64> = None;
    let mut counted_this_run = false;
    for (i, &b) in smooth_window.iter().enumerate() {
        if b > win.threshold {
            let since = *above_since.get_or_insert(sleep_samples[i].ts);
            if !counted_this_run && sleep_samples[i].ts - since >= MIN_DISTURBANCE_SECONDS {
                disturbances += 1;
                counted_this_run = true;
            }
        } else {
            above_since = None;
            counted_this_run = false;
        }
    }

    // HR trend across the night (bpm/hour) — negative = HR settling, a good sign.
    let hr_trend = {
        let n = bpms.len() as f64;
        let hours = (duration_minutes / 60.0).max(0.001);
        let first_q = bpms[..bpms.len() / 4.max(1)].iter().sum::<f64>()
            / (bpms.len() / 4).max(1) as f64;
        let last_q = bpms[bpms.len() - bpms.len() / 4.max(1)..]
            .iter()
            .sum::<f64>()
            / (bpms.len() / 4).max(1) as f64;
        let _ = n;
        (last_q - first_q) / hours
    };

    // Data coverage = fraction of the window with HR samples (vs an ideal ~1/s cadence,
    // capped — NOOP's standard-profile cadence varies).
    let expected = ((end_ts - start_ts).max(1)) as f64;
    let coverage = (sleep_samples.len() as f64 / expected).min(1.0);

    let start_rfc = rfc3339_utc(start_ts);
    let end_rfc = rfc3339_utc(end_ts);

    let input = SleepV1Input {
        sleep: SleepInput {
            start_time: start_rfc.clone(),
            end_time: end_rfc.clone(),
            sleep_duration_minutes: duration_minutes,
            sleep_need_minutes,
            time_in_bed_minutes: duration_minutes,
            midpoint_deviation_minutes: 0.0,
            disturbance_count: disturbances,
            sleep_latency_minutes: 0.0,
            wake_after_sleep_onset_minutes: 0.0,
            wake_episode_count: disturbances,
            stage_minutes: BTreeMap::new(),
            heart_rate_dip_percent: Some(hr_dip_percent),
            input_ids: vec![],
        },
        model_status: Default::default(),
        prior_nights: vec![],
        stage_segments: vec![],
        rolling_sleep_debt_minutes: 0.0,
        bedtime_deviation_minutes: 0.0,
        wake_time_deviation_minutes: 0.0,
        sleep_hr_average_bpm: Some(sleep_avg),
        sleep_hr_min_bpm: Some(sleep_min),
        pre_sleep_awake_hr_average_bpm: Some(pre_avg),
        sleep_hr_trend_bpm_per_hour: Some(hr_trend),
        naps_minutes: 0.0,
        prior_day_strain: None,
        data_coverage_fraction: Some(coverage),
    };

    let result = goose_sleep_v1(&input);
    let Some(output) = result.output else {
        return finish_unavailable(
            as_json,
            &format!("scorer returned no output: {:?}", result.errors),
        );
    };

    if as_json {
        let v = serde_json::json!({
            "ok": true,
            "window": {
                "start_local": fmt_local(start_ts),
                "end_local": fmt_local(end_ts),
                "duration_minutes": duration_minutes,
                "sample_count": sleep_samples.len(),
                "coverage_fraction": coverage,
            },
            "hr": {
                "sleep_avg_bpm": sleep_avg,
                "sleep_min_bpm": sleep_min,
                "pre_sleep_awake_avg_bpm": pre_avg,
                "hr_dip_percent": hr_dip_percent,
                "hr_trend_bpm_per_hour": hr_trend,
                "disturbances": disturbances,
            },
            "score": output,
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        return Ok(());
    }

    println!("══════════════════════════════════════════════════════");
    println!("  SLEEP (live, from overnight HR — WHOOP 5.0)");
    println!("══════════════════════════════════════════════════════");
    println!("  Window     {} → {}", fmt_local(start_ts), fmt_local(end_ts));
    println!(
        "  Duration   {:.1} h  ({:.0} min, {} samples, {:.0}% coverage)",
        duration_minutes / 60.0,
        duration_minutes,
        sleep_samples.len(),
        coverage * 100.0
    );
    println!("  ──────────────────────────────────────────────────");
    println!("  SLEEP SCORE       {:>5.0} / 100", output.score_0_to_100);
    println!(
        "  Performance       {:>5.0} %   (got {:.1}h of {:.1}h need)",
        output.sleep_performance_fraction * 100.0,
        output.sleep_duration_minutes / 60.0,
        output.sleep_need_minutes / 60.0,
    );
    println!(
        "  Efficiency        {:>5.0} %",
        output.sleep_efficiency_fraction * 100.0
    );
    println!("  Sleep debt        {:>5.0} min", output.sleep_debt_minutes);
    println!("  Confidence        {:>5.0} %", output.confidence_0_to_1 * 100.0);
    println!("  ──────────────────────────────────────────────────");
    println!("  HR avg / min      {:>3.0} / {:.0} bpm", sleep_avg, sleep_min);
    println!("  Pre-sleep HR      {:>3.0} bpm", pre_avg);
    println!(
        "  Overnight HR dip  {:>5.1} %   ({} disturbances)",
        hr_dip_percent, disturbances
    );
    if let Some(deep) = nonzero(output.deep_sleep_minutes) {
        println!("  Deep / REM        {:.0} / {:.0} min", deep, output.rem_sleep_minutes);
    }
    println!("══════════════════════════════════════════════════════");
    println!(
        "  model: {} ({})",
        output.model_status_label, output.algorithm_version
    );
    Ok(())
}

struct SleepWindow {
    start_idx: usize,
    end_idx: usize,
    threshold: f64,
    median: f64,
}

/// Longest contiguous run where HR sits near the night's minimum, tolerating brief
/// arousals (spikes above threshold for ≤ `max_spike` consecutive samples).
fn detect_sleep_window(hr: &[HrSample]) -> Option<SleepWindow> {
    let bpms: Vec<f64> = hr.iter().map(|s| s.bpm as f64).collect();
    let ts: Vec<i64> = hr.iter().map(|s| s.ts).collect();
    // Smooth over a ~2-minute window so brief spikes can't create phantom wake
    // crossings — cadence-independent (works at 2s or 30s sample spacing).
    let smooth = rolling_median_time(&bpms, &ts, 120);

    let mut sorted = smooth.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    // Build the threshold around the SLEEP band, not the night's absolute min.
    // The asleep HR distribution is the lower cluster; take its center (the
    // median of the lower half = the 25th percentile) as the sleep level, and
    // sit the threshold above it by a fraction of the sleep→awake spread. This
    // keeps REM swells (which ride a few bpm above deep sleep) inside the window
    // while still excluding genuine wake (HR near the night median and up).
    let sleep_level = sorted[sorted.len() / 4]; // ~25th percentile
    let threshold = sleep_level + 0.55 * (median - sleep_level).max(1.0);

    // Two-pass detection (robust to fragmentation):
    //   1. Collect every contiguous run of at/under-threshold samples.
    //   2. Merge adjacent runs separated by a short wake gap (an arousal, not a
    //      real wake) — gaps up to MAX_GAP_SECONDS are bridged.
    //   3. Pick the single longest merged span as the night's main sleep.
    // A merge-by-gap approach can't be fragmented by an in-sleep HR swing the way
    // a running state machine is.
    const MAX_GAP_SECONDS: i64 = 25 * 60; // bridge arousals/REM excursions up to ~25 min

    // Pass 1: raw asleep runs as (start_idx, end_idx).
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut cur: Option<(usize, usize)> = None;
    for (i, &b) in smooth.iter().enumerate() {
        if b <= threshold {
            match cur {
                Some((_s, ref mut e)) => *e = i,
                None => cur = Some((i, i)),
            }
        } else if let Some(run) = cur.take() {
            runs.push(run);
        }
    }
    if let Some(run) = cur.take() {
        runs.push(run);
    }
    if runs.is_empty() {
        return None;
    }

    // Pass 2: merge runs whose wake gap (in time) is short.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut acc = runs[0];
    for &(s, e) in &runs[1..] {
        let gap = hr[s].ts - hr[acc.1].ts;
        if gap <= MAX_GAP_SECONDS {
            acc.1 = e; // bridge the arousal
        } else {
            merged.push(acc);
            acc = (s, e);
        }
    }
    merged.push(acc);

    // Pass 3: longest merged span.
    let (best_start, best_end) = merged
        .into_iter()
        .max_by_key(|&(s, e)| hr[e].ts - hr[s].ts)
        .unwrap();
    let best_len = hr[best_end].ts - hr[best_start].ts;

    // Need ≥2h to call it sleep.
    if best_len < 2 * 3_600 {
        return None;
    }
    Some(SleepWindow {
        start_idx: best_start,
        end_idx: best_end,
        threshold,
        median,
    })
}

/// Centered rolling median over a TIME window (seconds) — cadence-independent, so
/// it smooths real noise whether samples arrive every 2s or every 30s. Suppresses
/// brief spikes without shifting the signal in time.
fn rolling_median_time(values: &[f64], ts: &[i64], window_seconds: i64) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let half = window_seconds / 2;
    let n = values.len();
    let (mut lo, mut hi) = (0usize, 0usize);
    (0..n)
        .map(|i| {
            let center = ts[i];
            while lo < n && ts[lo] < center - half {
                lo += 1;
            }
            if hi < lo {
                hi = lo;
            }
            while hi < n && ts[hi] <= center + half {
                hi += 1;
            }
            let mut slice: Vec<f64> = values[lo..hi.max(i + 1)].to_vec();
            slice.sort_by(|a, b| a.partial_cmp(b).unwrap());
            slice[slice.len() / 2]
        })
        .collect()
}

fn load_hr(conn: &Connection, start: i64, end: i64) -> GooseResult<Vec<HrSample>> {
    let mut stmt = conn
        .prepare("SELECT ts, bpm FROM hrSample WHERE ts >= ?1 AND ts < ?2 ORDER BY ts ASC")
        .map_err(|e| GooseError::message(format!("hrSample query: {e}")))?;
    let rows = stmt
        .query_map([start, end], |r| {
            Ok(HrSample {
                ts: r.get(0)?,
                bpm: r.get(1)?,
            })
        })
        .map_err(|e| GooseError::message(format!("hrSample map: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| GooseError::message(format!("hrSample row: {e}")))?);
    }
    Ok(out)
}

fn finish_unavailable(as_json: bool, reason: &str) -> GooseResult<()> {
    if as_json {
        println!("{}", serde_json::json!({ "ok": false, "reason": reason }));
    } else {
        println!("Sleep unavailable — {reason}");
    }
    Ok(())
}

fn default_noop_db_path() -> GooseResult<String> {
    let home = std::env::var("HOME")
        .map_err(|_| GooseError::message("HOME not set; pass --db explicitly"))?;
    Ok(format!(
        "{home}/Library/Application Support/OpenWhoop/whoop.sqlite"
    ))
}

fn nonzero(v: f64) -> Option<f64> {
    if v > 0.0 {
        Some(v)
    } else {
        None
    }
}

// ----- tiny time helpers (epoch seconds; avoid a chrono dep) -----

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Local midnight (start of today) for the current process timezone, via libc localtime.
fn local_midnight(now: i64) -> i64 {
    let off = local_utc_offset_seconds(now);
    let local = now + off;
    let local_midnight = local - local.rem_euclid(86_400);
    local_midnight - off
}

fn fmt_local(ts: i64) -> String {
    let off = local_utc_offset_seconds(ts);
    let (y, mo, d, h, mi, _s) = civil(ts + off);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}")
}

fn rfc3339_utc(ts: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(ts);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Days-from-civil algorithm (Howard Hinnant) → (Y, M, D, h, m, s) from epoch seconds (UTC).
fn civil(ts: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let (h, mi, s) = (secs / 3_600, (secs % 3_600) / 60, secs % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, h, mi, s)
}

/// Local UTC offset in seconds via libc localtime_r/gmtime_r (handles DST for the given ts).
fn local_utc_offset_seconds(ts: i64) -> i64 {
    unsafe extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
        fn timegm(tm: *mut Tm) -> i64;
    }
    #[repr(C)]
    #[derive(Default)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const i8,
    }
    unsafe {
        let mut tm = Tm::default();
        let t = ts;
        if localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        // tm_gmtoff is the local offset from UTC in seconds (BSD/macOS).
        let off = tm.tm_gmtoff;
        if off != 0 {
            return off;
        }
        // Fallback: derive from timegm of the broken-down local time.
        let back = timegm(&mut tm);
        back - ts
    }
}

fn opt_i64(args: &[String], flag_name: &str) -> GooseResult<Option<i64>> {
    match value(args, flag_name)? {
        Some(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|_| GooseError::message(format!("{flag_name} must be an integer"))),
        None => Ok(None),
    }
}

fn opt_f64(args: &[String], flag_name: &str) -> GooseResult<Option<f64>> {
    match value(args, flag_name)? {
        Some(v) => v
            .parse::<f64>()
            .map(Some)
            .map_err(|_| GooseError::message(format!("{flag_name} must be a number"))),
        None => Ok(None),
    }
}
