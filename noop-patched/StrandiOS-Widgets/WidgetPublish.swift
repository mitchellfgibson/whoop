#if os(iOS)
import Foundation
import WidgetKit

extension WidgetSnapshot {
    /// Build a glance snapshot from the live app state and publish it to the shared App Group, then
    /// ask WidgetKit to refresh. Called when the app becomes active and after a Health sync.
    @MainActor
    static func publish(from model: AppModel) {
        Task { await publishAsync(from: model) }
    }

    /// Async body — needs the offload-reach freshness for the sync-progress fields.
    @MainActor
    static func publishAsync(from model: AppModel) async {
        let recovery = model.repo.days.last(where: { $0.recovery != nil })?.recovery
        // Sync progress: how far the deep data (sleep/sensors) trails now, from the offload reach.
        let fresh = await model.repo.dataFreshness()
        let hoursBehind: Double? = fresh.health.map { max(0, Date().timeIntervalSince($0) / 3600.0) }
        let snap = WidgetSnapshot(
            recovery: recovery.map { Int($0.rounded()) },
            bpm: model.bpm ?? model.live.heartRate,
            batteryPct: model.live.batteryPct.map { Int($0.rounded()) },
            bonded: model.live.bonded,
            updated: Date(),
            syncHoursBehind: hoursBehind,
            syncing: model.live.backfilling
        )
        snap.save()
        WidgetCenter.shared.reloadAllTimelines()
    }
}
#endif
