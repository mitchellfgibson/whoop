import Foundation

/// Small, Codable glance snapshot shared between the iOS app and its widget/Live-Activity extension
/// via an App Group. The app writes it; the widget reads it. Keeping it tiny avoids any cross-process
/// database access — the widget never opens SQLite.
public struct WidgetSnapshot: Codable, Equatable {
    public var recovery: Int?
    public var bpm: Int?
    public var batteryPct: Int?
    public var bonded: Bool
    public var updated: Date
    /// Sync progress for the lock-screen glance — so you can watch the offload catch up without
    /// opening the app. `syncHoursBehind` = how far the deep data (sleep/sensors) trails now;
    /// `syncing` = a backfill is actively running. Optional/defaulted so older payloads still decode.
    public var syncHoursBehind: Double?
    public var syncing: Bool

    public init(recovery: Int?, bpm: Int?, batteryPct: Int?, bonded: Bool, updated: Date,
                syncHoursBehind: Double? = nil, syncing: Bool = false) {
        self.recovery = recovery
        self.bpm = bpm
        self.batteryPct = batteryPct
        self.bonded = bonded
        self.updated = updated
        self.syncHoursBehind = syncHoursBehind
        self.syncing = syncing
    }

    /// Sync fraction (0…1) for a ring, derived from the backlog. Caught up within 12h → 1.0.
    /// Without a known starting backlog we can't show true %, so this maps the current gap onto a
    /// soft 0–48h scale purely for the ring fill — the textual "Nh behind" is the honest number.
    public var syncFraction: Double {
        guard let h = syncHoursBehind else { return 0 }
        if h <= 12 { return 1.0 }
        return max(0.04, min(1.0, 1.0 - (h - 12) / 48.0))
    }

    /// "Synced" when the deep data is within ~12h of now.
    public var syncCaughtUp: Bool { (syncHoursBehind ?? .infinity) <= 12 }

    /// App Group suite the app and widget both use. Must match the `com.apple.security.application-groups`
    /// entitlement on both targets. If the entitlement is missing on either side, `UserDefaults(suiteName:)`
    /// returns nil and every consumer (PendingIntents, WidgetSnapshot.publish, Live Activity) silently
    /// no-ops — see `assertGroupProvisioned` for the debug-time canary.
    public static let suiteName = "group.com.mitchygib.noop"
    public static let storageKey = "noop.widget.snapshot"

    /// Debug-only canary: trips on the first run after a misprovisioning so the silent no-op gets
    /// caught immediately rather than masquerading as "widget shows nothing yet." Release builds do
    /// nothing — App Store apps can't crash on a missing entitlement.
    public static func assertGroupProvisioned() {
        assert(UserDefaults(suiteName: suiteName) != nil,
               "App Group '\(suiteName)' not provisioned on this target — check the entitlement.")
    }

    public static var placeholder: WidgetSnapshot {
        WidgetSnapshot(recovery: 72, bpm: 58, batteryPct: 84, bonded: true, updated: Date(),
                       syncHoursBehind: 6, syncing: false)
    }

    /// Read the last-published snapshot from the shared suite, if any.
    public static func load() -> WidgetSnapshot? {
        guard let defaults = UserDefaults(suiteName: suiteName),
              let data = defaults.data(forKey: storageKey),
              let snap = try? JSONDecoder().decode(WidgetSnapshot.self, from: data) else { return nil }
        return snap
    }

    /// Persist this snapshot into the shared suite.
    public func save() {
        guard let defaults = UserDefaults(suiteName: WidgetSnapshot.suiteName),
              let data = try? JSONEncoder().encode(self) else { return }
        defaults.set(data, forKey: WidgetSnapshot.storageKey)
    }
}
