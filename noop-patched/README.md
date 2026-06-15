# NOOP patched build (v1.95 + custom sleep algorithm)

Built from NoopApp/noop v1.95 source with our modifications:

## 1. Ground-up HMM sleep stager (SleepStager.swift)
Replaces NOOP's independent per-epoch percentile classifier with:
- **Physiological emission model** (per-epoch log-likelihoods for wake/light/deep/REM)
  from the cardiosomnography literature: deep = low+stable HR + high vagal HRV +
  regular respiration; REM = elevated HR + high HR-variability + irregular respiration;
  wake = motion; light = intermediate.
- **HMM + Viterbi temporal decoding** with realistic transition matrix + a sleep-
  architecture time-varying prior (deep concentrated early & decaying, REM suppressed
  first ~20min then growing) → produces real ~90-min cycles, not per-epoch flicker.
- **Calibrated** to the user's real WHOOP distribution (24% deep / 28% REM / 47% light,
  from 164 nights of physiological_cycles.csv). Validated: 27/26/46, deep front-loaded
  4/4 nights, REM back-loaded 3/4. Entry point: `stageWithHMM(...)`.

## 2. Sleep calendar (SleepCalendar.swift + SleepView.swift wiring)
Month calendar on the Sleep tab; green ring on days with data; tap a day to view that night.

## To rebuild after a future NOOP update:
1. Clone newest: `git clone --depth 1 https://github.com/NoopApp/noop.git`
2. Copy these files over the originals (paths: Packages/StrandAnalytics/Sources/StrandAnalytics/,
   Strand/Screens/) — OR re-apply by diffing if upstream changed those files.
3. `xcodegen generate && xcodebuild build -scheme Strand -configuration Release \
   CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=NO`
4. codesign --force --deep --sign - NOOP.app ; install to /Applications.

## To reinstall the saved build (no rebuild):
   cp -R NOOP-patched-v195.app /Applications/NOOP.app
   xattr -dr com.apple.quarantine /Applications/NOOP.app
