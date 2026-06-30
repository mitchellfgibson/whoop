#!/usr/bin/env bash
#
# noop-overnight-capture.sh — keep the Mac (and its Bluetooth radio) awake so NOOP
# can stream your WHOOP 5.0's HR + R-R all night. This is the ONLY way to get sleep
# on a 5.0: the strap won't serve its stored sleep over Bluetooth (that offload is
# gated — see the whoop5-offload-dead-end notes), so we must capture the signal LIVE.
# In the morning, score it with: goose-sleep-from-noop
#
# Usage:
#   Scripts/noop-overnight-capture.sh              # run until you Ctrl-C in the morning
#   Scripts/noop-overnight-capture.sh --hours 9    # auto-stop after 9 hours
#
# What it does:
#   1. Makes sure NOOP is running (launches it if not).
#   2. Holds a power assertion that prevents system sleep (`caffeinate -s`) so the
#      Bluetooth link stays alive. The display can still sleep.
#   3. Reminds you of the two things only YOU can do: wear the strap, and quit the
#      official WHOOP phone app (a strap pairs with one host at a time).

set -euo pipefail

HOURS=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --hours) HOURS="${2:-}"; shift 2 ;;
    -h|--help) grep '^#' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

NOOP_DB="$HOME/Library/Application Support/OpenWhoop/whoop.sqlite"

echo "──────────────────────────────────────────────────────────"
echo "  NOOP overnight capture (WHOOP 5.0 live sleep)"
echo "──────────────────────────────────────────────────────────"

# 1. Ensure NOOP is running.
if ! pgrep -f "MacOS/NOOP" >/dev/null 2>&1; then
  echo "  • Launching NOOP…"
  open -a NOOP || open /Applications/NOOP.app
  sleep 3
else
  echo "  • NOOP is already running."
fi

# 2. Pre-flight reminders (the human-only parts).
echo
echo "  BEFORE YOU SLEEP — two things only you can do:"
echo "    1. Put the WHOOP strap ON (it only streams while worn)."
echo "    2. Force-quit the official WHOOP app on your phone"
echo "       (a strap pairs with ONE host at a time — the phone steals it)."
echo "    3. In NOOP: Settings → Strap → 'Keep connected in the background' = ON."
echo

# 3. Show current capture so you can confirm HR is flowing right now.
if command -v sqlite3 >/dev/null 2>&1 && [[ -f "$NOOP_DB" ]]; then
  last=$(sqlite3 "$NOOP_DB" "SELECT bpm || ' bpm @ ' || datetime(ts,'unixepoch','localtime') FROM hrSample ORDER BY ts DESC LIMIT 1;" 2>/dev/null || true)
  echo "  • Latest HR sample in NOOP: ${last:-<none yet — wait for the strap to connect>}"
fi
echo

# 4. Hold the Mac awake.
if [[ -n "$HOURS" ]]; then
  secs=$(python3 -c "print(int(float('$HOURS')*3600))")
  echo "  • Keeping the Mac awake for ${HOURS}h (until $(date -v +"${secs}"S '+%a %H:%M' 2>/dev/null || date -d "+${secs} seconds" '+%a %H:%M')). Ctrl-C to stop early."
  echo "──────────────────────────────────────────────────────────"
  exec caffeinate -s -t "$secs"
else
  echo "  • Keeping the Mac awake until you press Ctrl-C in the morning."
  echo "  • In the morning, score your night with:"
  echo "        ./Rust/core/target/debug/goose-sleep-from-noop"
  echo "──────────────────────────────────────────────────────────"
  exec caffeinate -s
fi
