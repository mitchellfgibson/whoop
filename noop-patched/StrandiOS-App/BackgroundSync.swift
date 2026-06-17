#if os(iOS)
import Foundation
import BackgroundTasks

/// Periodic background-offload scheduler — the safety net for "sync in my pocket."
///
/// The live-HR notify drives the offload while data is streaming (see BLEManager's `.heartbeat`
/// trigger), but when the strap is idle/quiet the only way a backgrounded app gets CPU time is a
/// scheduled `BGProcessingTask`. iOS grants these opportunistically (typically when charging / on
/// Wi-Fi / at times it predicts you're idle — e.g. overnight). On each grant we kick a sync and
/// re-schedule the next one, so the offload keeps catching up across the night without the app being
/// foregrounded.
///
/// Requires (set in project.yml): the `processing` background mode + the identifier in
/// `BGTaskSchedulerPermittedIdentifiers`. Registration MUST happen before the app finishes launching.
enum BackgroundSync {
    static let taskIdentifier = "com.mitchygib.noop.offload"

    /// Register the launch handler. Call once, synchronously, during app `init` (before launch finishes).
    static func register(model: AppModel) {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: taskIdentifier, using: nil) { task in
            guard let task = task as? BGProcessingTask else { task.setTaskCompleted(success: false); return }
            handle(task: task, model: model)
        }
    }

    /// Ask iOS to schedule the next background offload window. Safe to call repeatedly (replaces the
    /// pending request). Call on background transition and after each grant.
    static func schedule() {
        let request = BGProcessingTaskRequest(identifier: taskIdentifier)
        request.requiresNetworkConnectivity = false      // BLE offload, no network
        request.requiresExternalPower = false            // allow on battery (best-effort either way)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        do { try BGTaskScheduler.shared.submit(request) }
        catch { /* too many pending / simulator — best-effort, no-op */ }
    }

    /// A granted background window: kick the offload, keep the task alive while it drains (with an
    /// expiration guard), then re-schedule the next window.
    private static func handle(task: BGProcessingTask, model: AppModel) {
        schedule()   // line up the next window first so the chain continues even if this run is cut short

        var finished = false
        let finish: (Bool) -> Void = { ok in
            guard !finished else { return }
            finished = true
            task.setTaskCompleted(success: ok)
        }
        task.expirationHandler = { finish(false) }   // iOS reclaiming our time — stop cleanly

        Task { @MainActor in
            model.ble.requestSync(.periodic)
            // Hold the task ~25s so an in-flight offload chunk can complete + commit.
            try? await Task.sleep(nanoseconds: 25 * 1_000_000_000)
            finish(true)
        }
    }
}
#endif
