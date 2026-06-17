import Foundation

/// What prompted a sync attempt. Mirrors WHOOP (15-min periodic floor + event-triggered "process now"
/// syncs + the strap's own prompt events + manual), adapted to iOS.
enum BackfillTrigger {
    case periodic    // the repeating timer while connected+bonded
    case connect     // a (re)connect / bond confirmation
    case foreground  // the app became active (scenePhase .active)
    case manual      // the user tapped "Sync now"
    case strap       // an incoming strap EVENT packet (WHOOP's HighFreqSyncPrompt analog)
}

/// Pure rate-limiter for historical-offload kicks. No BLE/store deps.
///
/// ADAPTIVE cadence (the key to fast catch-up): when the strap is CAUGHT UP, the periodic offload
/// runs on WHOOP's battery-friendly ~15-min floor. But when there's a BACKLOG (the last session
/// banked data without reaching HISTORY_COMPLETE — i.e. more is waiting in the strap's flash), it
/// re-kicks aggressively (a few seconds) so a 30-hour backlog drains continuously instead of one
/// 15-min-spaced session at a time. The floor snaps back to 15 min the moment it's caught up.
enum BackfillPolicy {
    static let periodicFloorSeconds: TimeInterval = 900   // 15 min — steady state, caught up
    static let catchUpFloorSeconds: TimeInterval = 5      // backlog present — hammer until drained
    static let eventFloorSeconds: TimeInterval = 90       // absorbs reconnect-flaps / event bursts

    /// `caughtUp` = the last offload reached HISTORY_COMPLETE (nothing more pending). Defaults true so
    /// existing callers keep WHOOP's 15-min cadence until the BLE layer reports a pending backlog.
    static func shouldRun(trigger: BackfillTrigger, now: TimeInterval,
                          lastBackfillAt: TimeInterval?, caughtUp: Bool = true) -> Bool {
        guard let last = lastBackfillAt else { return true }
        let elapsed = now - last
        switch trigger {
        case .manual:                        return true
        case .connect, .foreground, .strap:  return elapsed >= eventFloorSeconds
        case .periodic:                      return elapsed >= (caughtUp ? periodicFloorSeconds : catchUpFloorSeconds)
        }
    }
}
