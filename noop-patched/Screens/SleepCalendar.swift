import SwiftUI
import StrandDesign
import WhoopStore

// A month calendar for the Sleep tab. Days that have a sleep session get a green
// ring; tapping a day selects that night (drives which night SleepView shows).
// Pure presentation — the parent owns `selectedDay` and the set of days-with-data.

struct SleepCalendar: View {
    /// All sleep sessions (used to mark which days have data).
    let sessions: [CachedSleepSession]
    /// The currently-selected day (local midnight). nil = latest night.
    @Binding var selectedDay: Date?
    /// The month currently shown (local).
    @State private var visibleMonth: Date = Calendar.current.startOfDay(for: Date())

    private var cal: Calendar { Calendar.current }

    /// Local day-keys (yyyy-MM-dd) that have a sleep session, keyed off the session's
    /// END (wake) day — that's the day a "night" belongs to.
    private var daysWithData: Set<String> {
        var s = Set<String>()
        for sess in sessions {
            let end = Date(timeIntervalSince1970: TimeInterval(sess.endTs))
            s.insert(Self.dayKey(cal.startOfDay(for: end)))
        }
        return s
    }

    var body: some View {
        NoopCard {
            VStack(alignment: .leading, spacing: 12) {
                header
                weekdayRow
                grid
            }
        }
    }

    private var header: some View {
        HStack {
            Text(Self.monthFormatter.string(from: visibleMonth))
                .font(.headline)
                .foregroundStyle(StrandPalette.textPrimary)
            Spacer()
            Button { step(-1) } label: { Image(systemName: "chevron.left") }
                .buttonStyle(.plain)
                .foregroundStyle(StrandPalette.textSecondary)
            Button { step(1) } label: { Image(systemName: "chevron.right") }
                .buttonStyle(.plain)
                .foregroundStyle(StrandPalette.textSecondary)
                .padding(.leading, 8)
        }
    }

    private var weekdayRow: some View {
        HStack(spacing: 0) {
            ForEach(Self.weekdaySymbols, id: \.self) { d in
                Text(d)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(StrandPalette.textSecondary)
                    .frame(maxWidth: .infinity)
            }
        }
    }

    private var grid: some View {
        let days = monthGridDays()
        return LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 4), count: 7), spacing: 6) {
            ForEach(days.indices, id: \.self) { i in
                if let day = days[i] {
                    dayCell(day)
                } else {
                    Color.clear.frame(height: 38)
                }
            }
        }
    }

    @ViewBuilder
    private func dayCell(_ day: Date) -> some View {
        let key = Self.dayKey(day)
        let hasData = daysWithData.contains(key)
        let isSelected = selectedDay.map { cal.isDate($0, inSameDayAs: day) } ?? false
        let isToday = cal.isDateInToday(day)
        let isFuture = day > cal.startOfDay(for: Date())

        Button {
            guard hasData else { return }
            // Tapping the selected day again clears selection (→ latest night).
            selectedDay = isSelected ? nil : day
        } label: {
            ZStack {
                if isSelected {
                    Circle().fill(StrandPalette.accent.opacity(0.22))
                }
                Circle()
                    .strokeBorder(
                        hasData ? StrandPalette.accent : Color.clear,
                        lineWidth: isSelected ? 2 : 1.5
                    )
                Text("\(cal.component(.day, from: day))")
                    .font(.system(size: 13, weight: isToday ? .bold : .regular))
                    .foregroundStyle(
                        isFuture ? StrandPalette.textSecondary.opacity(0.4)
                        : (hasData ? StrandPalette.textPrimary : StrandPalette.textSecondary)
                    )
            }
            .frame(height: 38)
        }
        .buttonStyle(.plain)
        .disabled(!hasData)
    }

    // MARK: - Month math

    private func step(_ delta: Int) {
        if let m = cal.date(byAdding: .month, value: delta, to: visibleMonth) {
            visibleMonth = m
        }
    }

    /// The 7-col grid for `visibleMonth`: leading nils pad to the first weekday.
    private func monthGridDays() -> [Date?] {
        guard let monthStart = cal.date(from: cal.dateComponents([.year, .month], from: visibleMonth)),
              let range = cal.range(of: .day, in: .month, for: monthStart) else { return [] }
        // weekday of the 1st (1=Sun..7=Sat) → leading blanks (respect firstWeekday).
        let firstWeekday = cal.component(.weekday, from: monthStart)
        let lead = (firstWeekday - cal.firstWeekday + 7) % 7
        var cells: [Date?] = Array(repeating: nil, count: lead)
        for d in range {
            if let day = cal.date(byAdding: .day, value: d - 1, to: monthStart) {
                cells.append(cal.startOfDay(for: day))
            }
        }
        while cells.count % 7 != 0 { cells.append(nil) }
        return cells
    }

    // MARK: - Formatting

    static func dayKey(_ d: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        return f.string(from: d)
    }

    private static let monthFormatter: DateFormatter = {
        let f = DateFormatter(); f.dateFormat = "LLLL yyyy"; return f
    }()

    private static var weekdaySymbols: [String] {
        let cal = Calendar.current
        let syms = DateFormatter().veryShortStandaloneWeekdaySymbols ?? ["S","M","T","W","T","F","S"]
        // Rotate so index 0 = calendar's firstWeekday.
        let start = cal.firstWeekday - 1
        return Array(syms[start...] + syms[..<start])
    }
}
