import WidgetKit
import SwiftUI
import StrandDesign

/// Timeline entry backed by the latest `WidgetSnapshot` the app published into the App Group.
struct NOOPEntry: TimelineEntry {
    let date: Date
    let snapshot: WidgetSnapshot
}

struct NOOPProvider: TimelineProvider {
    func placeholder(in context: Context) -> NOOPEntry {
        NOOPEntry(date: Date(), snapshot: .placeholder)
    }

    func getSnapshot(in context: Context, completion: @escaping (NOOPEntry) -> Void) {
        completion(NOOPEntry(date: Date(), snapshot: WidgetSnapshot.load() ?? .placeholder))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<NOOPEntry>) -> Void) {
        let snap = WidgetSnapshot.load() ?? .placeholder
        // Refresh roughly every 15 minutes; the app also forces a reload when it publishes fresh data.
        let next = Calendar.current.date(byAdding: .minute, value: 15, to: Date()) ?? Date().addingTimeInterval(900)
        completion(Timeline(entries: [NOOPEntry(date: Date(), snapshot: snap)], policy: .after(next)))
    }
}

/// The glanceable widget — the iOS analogue of the macOS menu-bar extra. Recovery, live/last HR,
/// and strap battery.
struct NOOPWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: NOOPEntry

    private var snap: WidgetSnapshot { entry.snapshot }

    var body: some View {
        switch family {
        case .accessoryCircular:
            recoveryGauge
        case .accessoryInline:
            Text(inlineText)
        case .accessoryRectangular:
            rectangular
        default:
            home
        }
    }

    private var recoveryColor: Color {
        guard let r = snap.recovery else { return StrandPalette.textTertiary }
        return r >= 67 ? StrandPalette.statusPositive : r >= 34 ? StrandPalette.statusWarning : StrandPalette.statusCritical
    }

    private var inlineText: String {
        // Lead with sync status so the lock-screen inline shows it while the phone is closed.
        var parts: [String] = [syncText]
        if let r = snap.recovery { parts.append("Rec \(r)%") }
        return parts.joined(separator: " · ")
    }

    /// One short sync-status string for the glance.
    private var syncText: String {
        if snap.syncCaughtUp { return "Synced ✓" }
        guard let h = snap.syncHoursBehind else { return "No sync yet" }
        let gap = h < 48 ? "\(Int(h.rounded()))h" : "\(Int((h/24).rounded()))d"
        return snap.syncing ? "Syncing · \(gap) behind" : "\(gap) behind"
    }

    private var syncColor: Color {
        snap.syncCaughtUp ? StrandPalette.statusPositive : StrandPalette.statusWarning
    }

    /// A compact sync row (icon + status + small progress bar) reused in the home/rectangular layouts.
    private var syncRow: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 4) {
                Image(systemName: snap.syncCaughtUp ? "checkmark.circle.fill"
                      : snap.syncing ? "arrow.triangle.2.circlepath" : "clock.badge.exclamationmark")
                    .foregroundStyle(syncColor)
                Text(syncText).foregroundStyle(StrandPalette.textSecondary)
            }
            .font(.caption2)
            if !snap.syncCaughtUp {
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        Capsule().fill(StrandPalette.hairline)
                        Capsule().fill(syncColor)
                            .frame(width: max(4, geo.size.width * snap.syncFraction))
                    }
                }
                .frame(height: 4)
            }
        }
    }

    private var recoveryGauge: some View {
        Gauge(value: Double(snap.recovery ?? 0), in: 0...100) {
            Image(systemName: "heart.fill")
        } currentValueLabel: {
            Text(snap.recovery.map { "\($0)" } ?? "–")
        }
        .gaugeStyle(.accessoryCircular)
        .tint(recoveryColor)
    }

    private var rectangular: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Image(systemName: "heart.fill").foregroundStyle(recoveryColor)
                Text("Recovery \(snap.recovery.map(String.init) ?? "–")%").font(.headline)
            }
            Text("\(snap.bpm.map(String.init) ?? "–") bpm · \(snap.batteryPct.map { "\($0)%" } ?? "–")")
                .font(.caption)
            // Sync progress — the whole point: visible on the lock screen while the phone is closed.
            syncRow
        }
    }

    private var home: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("NOOP").font(.system(size: 13, weight: .bold))
                    .foregroundStyle(StrandPalette.textSecondary)
                Spacer()
                Circle().fill(snap.bonded ? StrandPalette.statusPositive : StrandPalette.statusCritical)
                    .frame(width: 8, height: 8)
            }
            Spacer(minLength: 0)
            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(snap.recovery.map(String.init) ?? "–")
                    .font(.system(size: 40, weight: .bold, design: .rounded))
                    .foregroundStyle(recoveryColor)
                Text("%").font(.headline).foregroundStyle(StrandPalette.textTertiary)
            }
            Text("Recovery").font(.caption).foregroundStyle(StrandPalette.textTertiary)
            Spacer(minLength: 0)
            // Sync progress row — check the offload status without opening the app.
            syncRow
            HStack {
                Label("\(snap.bpm.map(String.init) ?? "–")", systemImage: "waveform.path.ecg")
                Spacer()
                Label("\(snap.batteryPct.map { "\($0)%" } ?? "–")", systemImage: "battery.50")
            }
            .font(.caption2).foregroundStyle(StrandPalette.textSecondary)
        }
        .padding(12)
    }
}

struct NOOPWidget: Widget {
    let kind = "NOOPWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: NOOPProvider()) { entry in
            if #available(iOS 17.0, *) {
                NOOPWidgetView(entry: entry)
                    .containerBackground(StrandPalette.surfaceBase, for: .widget)
            } else {
                NOOPWidgetView(entry: entry)
                    .padding()
                    .background(StrandPalette.surfaceBase)
            }
        }
        .configurationDisplayName("NOOP Recovery")
        .description("Recovery, live heart rate, and strap battery at a glance.")
        .supportedFamilies([
            .systemSmall, .systemMedium,
            .accessoryCircular, .accessoryInline, .accessoryRectangular
        ])
    }
}
