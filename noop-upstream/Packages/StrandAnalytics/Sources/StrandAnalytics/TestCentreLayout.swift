import Foundation

/// Pure projection of the registry for the Test Centre screen's section 1 (domain test modes).
///
/// Shipped modes only (the registry is already Phase 1 only), priority-ordered high then med then low
/// with registry order stable inside a band, and requires5MG modes hidden off a non-5/MG strap (spec
/// section 12, the #22 gating question). No app import, so both platforms render the same order. The
/// status helper formats each row's status string identically across iOS and Android. The Kotlin twin
/// is TestCentreLayout.kt, kept aligned by a parity test.
public enum TestCentreLayout {

    /// Rank a priority so high sorts before med before low; ties keep their input order (stable sort).
    static func rank(_ p: TestPriority) -> Int {
        switch p {
        case .high: return 0
        case .med: return 1
        case .low: return 2
        }
    }

    /// Order an arbitrary mode list (the registry, or a test fixture) for the screen. Stable within a
    /// priority band so registry order decides ties.
    public static func order(_ modes: [TestMode], is5MG: Bool) -> [TestMode] {
        modes
            .filter { is5MG || !$0.requires5MG }
            .enumerated()
            .sorted { a, b in
                let ra = rank(a.element.priority), rb = rank(b.element.priority)
                return ra == rb ? a.offset < b.offset : ra < rb
            }
            .map { $0.element }
    }

    /// The shipped registry projected for the current strap. Section 1 of the screen binds this.
    public static func visibleModes(is5MG: Bool) -> [TestMode] {
        order(TestModeRegistry.all, is5MG: is5MG)
    }
}

public extension TestCentreLayout {

    /// The row status string. "Off" when inactive; "On" for an active toggle mode; "Capturing K of N
    /// <unit>" for an active guided mode, where K is the elapsed-day count (1-based, ceil), clamped to
    /// the target so a long-running capture never reads past its window (spec section 12). `unit` is the
    /// mode's own word ("nights" / "days"), so Sleep and Battery read naturally. No em-dash.
    static func statusText(for mode: TestMode, active: Bool, elapsedSeconds: Double?) -> String {
        guard active else { return "Off" }
        switch mode.capture {
        case .toggle:
            return "On"
        case let .guided(unit, defaultCount):
            let elapsed = max(0, elapsedSeconds ?? 0)
            let dayIndex = Int(ceil(elapsed / 86_400.0))
            let k = min(max(dayIndex, 1), defaultCount)
            return "Capturing \(k) of \(defaultCount) \(unit.rawValue)"
        }
    }
}
