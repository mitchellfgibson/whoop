import SwiftUI
import StrandDesign
import StrandAnalytics

// MARK: - Hydration detail (MVP, opt-in, local-only)
//
// Design-Reset compliant: a clean progress ring (GlowRing, blue accent, no bloom), the three quick-log
// buttons (Sip / Cup / Bottle) in the secondary NoopButton style, today's logged total as a single
// read-out, and a 7-day mini bar history. Flat cards (NoopCard), NoopMetrics spacing, tokens only, no
// gold. BYTE-PARITY twin of the Android `HydrationScreen`: the day total + history come from the
// local-only `HydrationStore` series (additive day total), and the goal is the pure `HydrationGoal`
// engine (profile sex + today's Effort bump). Per-tap rows aren't separately persisted on either
// platform — the day total is the source of truth, so the screen shows the honest day figure.
struct HydrationView: View {
    @EnvironmentObject var repo: Repository
    @EnvironmentObject var profile: ProfileStore

    /// Today's running total (ml) + the 7-day history (oldest→newest), loaded off the gesture path and
    /// refreshed after each log. A reload key the taps bump so the `.task` re-reads the store.
    @State private var totalML: Double = 0
    @State private var history: [(day: String, value: Double)] = []
    @State private var reloadTick = 0
    /// #798 - today's individual logged drinks (for swipe-to-delete + tap-to-edit), and the entry being
    /// edited in the amount sheet (nil when the sheet is closed).
    @State private var entries: [HydrationEntry] = []
    @State private var editingEntry: HydrationEntry?
    /// #798 - the user's custom container size (ml), editable from the custom-size sheet. Persisted local-only.
    @AppStorage(HydrationStore.customSizeKey) private var customSizeML = HydrationGoal.cupML
    @State private var showCustomSizeSheet = false

    private var goalML: Int { repo.hydrationGoalML(profileSex: profile.sex) }
    private var fraction: Double { HydrationGoal.fraction(totalML: totalML, goalML: goalML) }
    private var percent: Int { min(100, Int((fraction * 100).rounded(.towardZero))) }

    var body: some View {
        ScreenScaffold(title: "Hydration",
                       subtitle: "Your fluid intake today, on \(Platform.deviceNounPhrase) only.",
                       onRefresh: { await reload() }) {
            VStack(alignment: .leading, spacing: NoopMetrics.sectionGap) {
                ringSection
                logSection
                entriesSection
                historySection
                todayTotalSection
                Text("A simple goal that adjusts to your effort. General wellness guidance, not medical advice.")
                    .font(StrandFont.footnote)
                    .foregroundStyle(StrandPalette.textTertiary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .task(id: reloadTick) { await reload() }
        // #798 - edit a logged drink's amount.
        .sheet(item: $editingEntry) { entry in
            HydrationAmountSheet(title: "Edit drink", initialML: entry.amountMl) { newML in
                editingEntry = nil
                Task { await updateEntry(entry, to: newML) }
            } onCancel: { editingEntry = nil }
        }
        // #798 - set the custom container size.
        .sheet(isPresented: $showCustomSizeSheet) {
            HydrationAmountSheet(title: "Custom size", initialML: customSizeML) { newML in
                customSizeML = newML
                showCustomSizeSheet = false
            } onCancel: { showCustomSizeSheet = false }
        }
    }

    // MARK: - Ring (total vs goal, in litres)

    private var ringSection: some View {
        NoopCard(padding: 20) {
            VStack(spacing: NoopMetrics.cardInnerSpacing) {
                ZStack {
                    GlowRing(fraction: fraction,
                             value: HydrationGoal.litres(fromML: totalML),
                             format: { _ in "" },   // centre text is the overlay below
                             color: StrandPalette.accent,
                             diameter: 184,
                             lineWidth: 14)
                    VStack(spacing: 2) {
                        Text(String(format: "%.1f", HydrationGoal.litres(fromML: totalML)))
                            .font(StrandFont.rounded(40, weight: .bold))
                            .foregroundStyle(StrandPalette.textPrimary)
                            .monospacedDigit()
                        Text(String(format: "of %.1f L", HydrationGoal.litres(fromML: Double(goalML))))
                            .font(StrandFont.subhead)
                            .foregroundStyle(StrandPalette.textSecondary)
                    }
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Hydration today")
                .accessibilityValue("\(String(format: "%.1f", HydrationGoal.litres(fromML: totalML))) of \(String(format: "%.1f", HydrationGoal.litres(fromML: Double(goalML)))) litres")

                Text("\(percent)% of today's goal")
                    .font(StrandFont.footnote)
                    .foregroundStyle(StrandPalette.textTertiary)
            }
            .frame(maxWidth: .infinity)
        }
    }

    // MARK: - Quick log (Sip / Cup / Bottle, secondary style)

    private var logSection: some View {
        VStack(alignment: .leading, spacing: NoopMetrics.gap) {
            HStack(spacing: NoopMetrics.gap) {
                logButton("Sip", systemImage: "drop", ml: HydrationGoal.sipML)
                logButton("Cup", systemImage: "cup.and.saucer.fill", ml: HydrationGoal.cupML)
                logButton("Bottle", systemImage: "drop.fill", ml: HydrationGoal.bottleML)
            }
            // #798 - a custom container the user sizes themselves. Tapping logs it; the pencil opens the
            // size editor so a one-off mug / flask / glass can be set once and reused.
            HStack(spacing: NoopMetrics.gap) {
                NoopButton("Custom \(customSizeML) ml", systemImage: "drop.circle", kind: .secondary, fullWidth: true) {
                    Task { await add(ml: customSizeML) }
                }
                .accessibilityLabel("Log custom \(customSizeML) millilitres")
                Button { showCustomSizeSheet = true } label: {
                    Image(systemName: "pencil")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(StrandPalette.accent)
                        .frame(width: 44, height: 44)
                        .background(RoundedRectangle(cornerRadius: NoopMetrics.cardRadius, style: .continuous)
                            .fill(StrandPalette.surfaceInset))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Set custom container size")
            }
            Text("Sip \(HydrationGoal.sipML) ml · Cup \(HydrationGoal.cupML) ml · Bottle \(HydrationGoal.bottleML) ml")
                .font(StrandFont.footnote)
                .foregroundStyle(StrandPalette.textTertiary)
        }
    }

    /// One quick-add button using the secondary (no-gold) NoopButton style. Logs the amount and refreshes.
    private func logButton(_ title: LocalizedStringKey, systemImage: String, ml: Int) -> some View {
        NoopButton(title, systemImage: systemImage, kind: .secondary, fullWidth: true) {
            Task { await add(ml: ml) }
        }
        .accessibilityLabel("Log \(title)")
    }

    // MARK: - Today's logged drinks (#798) - swipe to delete, tap to edit

    @ViewBuilder private var entriesSection: some View {
        if !entries.isEmpty {
            NoopCard(padding: 18) {
                VStack(alignment: .leading, spacing: NoopMetrics.gap) {
                    Text("Today's drinks").strandOverline()
                    // #842 — render rows in a plain VStack inside the page ScrollView. The previous nested,
                    // scroll-disabled List with a hardcoded `count * 44 + 8` height clipped every row past
                    // the third (real rows are taller than 44pt) and couldn't be scrolled to. Tap a row to
                    // edit; the trailing trash deletes (a VStack row can't host native swipe-to-delete).
                    VStack(spacing: 0) {
                        ForEach(Array(entries.enumerated()), id: \.element.id) { idx, entry in
                            entryRow(entry)
                                .padding(.vertical, 6)
                            if idx < entries.count - 1 {
                                Divider().opacity(0.4)
                            }
                        }
                    }
                    Text("Tap a drink to edit it, or use the trash to delete.")
                        .font(StrandFont.footnote)
                        .foregroundStyle(StrandPalette.textTertiary)
                }
            }
        }
    }

    /// One logged-drink row: the time it was logged + its amount (tap to edit) with a trailing trash.
    private func entryRow(_ entry: HydrationEntry) -> some View {
        HStack(spacing: 10) {
            Button { editingEntry = entry } label: {
                HStack(spacing: 10) {
                    Image(systemName: "drop.fill")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(StrandPalette.accent)
                        .accessibilityHidden(true)
                    Text(Self.entryTimeFmt.string(from: entry.loggedAt))
                        .font(StrandFont.subhead)
                        .foregroundStyle(StrandPalette.textSecondary)
                    Spacer(minLength: 8)
                    Text("\(entry.amountMl) ml")
                        .font(StrandFont.subhead.weight(.semibold))
                        .foregroundStyle(StrandPalette.textPrimary)
                        .monospacedDigit()
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Logged \(entry.amountMl) millilitres at \(Self.entryTimeFmt.string(from: entry.loggedAt))")
            .accessibilityHint("Tap to edit the amount")
            Button(role: .destructive) {
                Task { await deleteEntry(entry) }
            } label: {
                Image(systemName: "trash")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(StrandPalette.textTertiary)
                    .frame(width: 36, height: 36)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Delete the \(entry.amountMl) millilitre drink logged at \(Self.entryTimeFmt.string(from: entry.loggedAt))")
        }
    }

    private static let entryTimeFmt: DateFormatter = {
        let f = DateFormatter(); f.timeStyle = .short; f.dateStyle = .none; return f
    }()

    // MARK: - 7-day mini history (flat bars, today on the right)

    private var historySection: some View {
        NoopCard(padding: 18) {
            VStack(alignment: .leading, spacing: NoopMetrics.gap) {
                Text("Last 7 days").strandOverline()
                historyBars
            }
        }
    }

    @ViewBuilder private var historyBars: some View {
        if history.isEmpty {
            Text("No history yet.")
                .font(StrandFont.footnote)
                .foregroundStyle(StrandPalette.textTertiary)
        } else {
            // Scale the bars to the LARGER of the goal and the biggest day, so an over-goal day doesn't clip.
            let ceiling = max(Double(max(goalML, 1)), history.map(\.value).max() ?? 0, 1)
            let lastIndex = history.count - 1
            HStack(alignment: .bottom, spacing: 10) {
                ForEach(Array(history.enumerated()), id: \.element.day) { idx, bar in
                    VStack(spacing: 6) {
                        ZStack(alignment: .bottom) {
                            RoundedRectangle(cornerRadius: 6, style: .continuous)
                                .fill(StrandPalette.textPrimary.opacity(0.10))
                                .frame(height: 96)
                            let frac = min(1.0, max(0.0, bar.value / ceiling))
                            if frac > 0 {
                                RoundedRectangle(cornerRadius: 6, style: .continuous)
                                    .fill(idx == lastIndex ? StrandPalette.accent
                                                           : StrandPalette.accent.opacity(0.45))
                                    .frame(height: max(3, 96 * CGFloat(frac)))
                            }
                        }
                        Text(weekdayInitial(bar.day))
                            .font(StrandFont.overline)
                            .foregroundStyle(StrandPalette.textTertiary)
                    }
                    .frame(maxWidth: .infinity)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("\(weekdayInitial(bar.day)): \(String(format: "%.1f", HydrationGoal.litres(fromML: bar.value))) litres")
                }
            }
        }
    }

    // MARK: - Today's total (the honest day figure; per-tap rows aren't persisted)

    private var todayTotalSection: some View {
        NoopCard(padding: 18) {
            VStack(alignment: .leading, spacing: NoopMetrics.space2) {
                Text("Today").strandOverline()
                if totalML <= 0 {
                    Text("No drinks logged yet. Tap Sip, Cup or Bottle to start.")
                        .font(StrandFont.subhead)
                        .foregroundStyle(StrandPalette.textSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    HStack(spacing: 10) {
                        Image(systemName: "drop.fill")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(StrandPalette.accent)
                            .accessibilityHidden(true)
                        Text("Logged today")
                            .font(StrandFont.subhead)
                            .foregroundStyle(StrandPalette.textPrimary)
                        Spacer(minLength: 8)
                        Text("\(Int(totalML)) ml")
                            .font(StrandFont.headline.weight(.semibold))
                            .foregroundStyle(StrandPalette.textPrimary)
                            .monospacedDigit()
                    }
                }
            }
        }
    }

    // MARK: - Data

    /// The single-letter weekday for a yyyy-MM-dd key (M T W T F S S), or "·" when unparseable. Mirrors
    /// the Android `weekdayInitial` (EEE → first letter, US locale).
    private func weekdayInitial(_ dayKey: String) -> String {
        let parse = DateFormatter()
        parse.locale = Locale(identifier: "en_US_POSIX")
        parse.dateFormat = "yyyy-MM-dd"
        guard let date = parse.date(from: dayKey) else { return "·" }
        let out = DateFormatter()
        out.locale = Locale(identifier: "en_US")
        out.dateFormat = "EEE"
        return String(out.string(from: date).prefix(1))
    }

    /// Log `ml` (additive day total + a per-entry row, #798) and refresh.
    private func add(ml: Int) async {
        guard ml > 0 else { return }
        _ = await repo.logHydration(amountMl: ml)
        reloadTick &+= 1
    }

    /// #798 - delete a logged drink, re-deriving the day total, then refresh.
    private func deleteEntry(_ entry: HydrationEntry) async {
        _ = await repo.deleteHydrationEntry(id: entry.id)
        reloadTick &+= 1
    }

    /// #798 - set a logged drink's amount, re-deriving the day total, then refresh.
    private func updateEntry(_ entry: HydrationEntry, to ml: Int) async {
        _ = await repo.updateHydrationEntry(id: entry.id, amountMl: ml)
        reloadTick &+= 1
    }

    /// Load today's total + the 7-day history + today's per-entry list from the store.
    private func reload() async {
        totalML = await repo.hydrationTotal(day: Repository.localDayKey(Date()))
        history = await repo.hydrationHistory(days: 7)
        entries = repo.hydrationEntries()
    }
}

// MARK: - Amount sheet (#798) - edit a drink / set the custom size

/// A small stepper sheet for an ml amount. Reused by the edit-entry and custom-size flows. Tokens only,
/// NoopButton actions, ARIA labels. Clamps to a sane range so the value stays a real container size.
private struct HydrationAmountSheet: View {
    let title: LocalizedStringKey
    let initialML: Int
    let onSave: (Int) -> Void
    let onCancel: () -> Void

    @State private var ml: Int

    /// Bounds for a plausible single container (10 ml up to 3 L), stepping in 10 ml increments.
    private static let minML = 10
    private static let maxML = 3000
    private static let stepML = 10

    init(title: LocalizedStringKey, initialML: Int, onSave: @escaping (Int) -> Void,
         onCancel: @escaping () -> Void) {
        self.title = title
        self.initialML = initialML
        self.onSave = onSave
        self.onCancel = onCancel
        _ml = State(initialValue: Self.clamp(initialML))
    }

    static func clamp(_ value: Int) -> Int { min(maxML, max(minML, value)) }

    var body: some View {
        VStack(alignment: .leading, spacing: NoopMetrics.sectionGap) {
            Text(title)
                .font(StrandFont.title2)
                .foregroundStyle(StrandPalette.textPrimary)
            HStack {
                Text("Amount")
                    .font(StrandFont.subhead)
                    .foregroundStyle(StrandPalette.textSecondary)
                Spacer()
                Text("\(ml) ml")
                    .font(StrandFont.rounded(28, weight: .bold))
                    .foregroundStyle(StrandPalette.textPrimary)
                    .monospacedDigit()
            }
            Stepper(value: $ml, in: Self.minML...Self.maxML, step: Self.stepML) {
                Text("Adjust amount")
                    .font(StrandFont.footnote)
                    .foregroundStyle(StrandPalette.textTertiary)
            }
            .accessibilityLabel("Amount in millilitres")
            .accessibilityValue("\(ml) millilitres")
            HStack(spacing: NoopMetrics.gap) {
                NoopButton("Cancel", kind: .secondary, fullWidth: true) { onCancel() }
                NoopButton("Save", kind: .primary, fullWidth: true) { onSave(Self.clamp(ml)) }
            }
        }
        .padding(NoopMetrics.space5)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(StrandPalette.surfaceBase.ignoresSafeArea())
        // iOS-only sheet sizing - macOS sheets are free-floating windows and reject detents (see the
        // shared `noopSheetPresentation` note); the call site stays cross-platform via this guard.
        #if os(iOS)
        .presentationDetents([.height(300)])
        .presentationDragIndicator(.visible)
        #endif
    }
}
