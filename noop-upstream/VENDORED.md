# Vendored: upstream NOOP (the buildable iOS app)

This directory is a **verbatim vendored copy of upstream NOOP**, added 2026-06-30 so this
repo contains the complete, buildable app — not just the `noop-patched/` file subset.

- **Source:** https://github.com/NoopApp/noop  (mirror: https://noop.fans/NoopApp/noop)
- **Commit:** `e5b347fecf7cda4d757c6e7ec94137413528ce80`
- **Version:** v7.7.0, build 164 (2026-06-30)
- **License:** PolyForm Noncommercial 1.0.0 (see `LICENSE` in this directory — noncommercial only).
- The upstream `.git` was **not** copied (kept the tree lean). To update: re-clone upstream
  and re-vendor, or `git remote add` upstream in a fresh clone.

## Why this is here
The iOS app you run (NOOP, `com.mitchygib.noop`) is built from this project — internal name
**Strand** (`Strand.xcodeproj`). It was never fully in this repo before; only edited slices lived
in `../noop-patched/`, which is missing the Xcode project and several Swift modules
(`StrandDesign`, `StrandImport`, full `WhoopStore`/`WhoopProtocol`, …). Those all live here.

## The v20/v21 firmware decode — already handled upstream
The 2026-06-17 WHOOP 5.0 firmware flip that broke history sync is **already decoded in this
version**, in Swift:
- `Packages/WhoopProtocol/Sources/WhoopProtocol/Interpreter.swift` — v20 (five i32 blocks gated by
  presence bytes) and v21 (i16 channels) history decode.
- Tests: `Packages/WhoopProtocol/Tests/WhoopProtocolTests/Whoop5HistoricalV2021Tests.swift`,
  `RejectedHistoryTests.swift`.

Verified against our own 528-frame corpus (`../docs/whoop5-v20-v21-corpus/`): upstream reads the
**same bytes** we reverse-engineered (their `frame@28` == our `payload@20`, identical values). One
difference worth noting — for **v21**, upstream decodes **3 channels × 100 samples (300)** and stops;
our independent Rust analysis found a **second group of 300** (6×100 = 600 samples/frame) at offsets
620/820/1020 that upstream leaves on the floor. See `../docs/whoop5-v20-v21-corpus/DECODE-SPEC.md`.
That's a potential upstream improvement (2× the v21 motion), not a blocker — upstream already works.

The separate Rust decoder in `../Rust/core` (protocol.rs) is for the *old Goose* core, which this
NOOP app does **not** use. NOOP decodes in Swift. Keep that in mind before wiring the two together.

## Build / install (you drive this — repo is just made ready)
The `Strand.xcodeproj` here was regenerated with XcodeGen (`xcodegen generate`, from `project.yml`).
Three paths, easiest first:

1. **Sideload the official `.ipa` (no Xcode).** Add this source to AltStore/SideStore and install:
   `https://raw.githubusercontent.com/NoopApp/noop/main/altstore-source.json`
   Signs on your iPhone with your free Apple ID; auto-updates. (Free-ID caveats: 7-day re-sign,
   HealthKit + Live Activity may be limited.) — details in `docs/IOS.md`.

2. **Build from source in Xcode.** Open `Strand.xcodeproj`, select the `NOOPiOS` scheme, set your
   signing team, pick your iPhone, Run. (Regenerate the project first with `xcodegen generate` if
   `project.yml` changed.)

3. **Command line** (what a headless build looks like — set your own team/bundle id):
   `xcodebuild -project Strand.xcodeproj -scheme NOOPiOS -destination 'platform=iOS,name=<device>' -allowProvisioningUpdates DEVELOPMENT_TEAM=R4X6JDGYLF build`

Your phone currently runs an **older** build than this 7.7.0, so building/sideloading this updates
you to the version that already has the v20/v21 history fix.
