#if os(iOS)
import SwiftUI
import StrandDesign

/// iOS navigation shell. macOS uses a `NavigationSplitView` sidebar (`RootView`); on iPhone the
/// natural analogue is a `TabView` with the most-used screens as tabs and everything else under a
/// "More" list. Every screen is the same `StrandDesign`-built view the macOS app uses.
struct RootTabView: View {
    @EnvironmentObject private var repo: Repository

    var body: some View {
        // Tabs mirror the Mac sidebar's curated top items (Today, Sleep, Workouts, Stress); the macOS
        // build folds Live into the bottom of Today, so it isn't a separate tab here either.
        TabView {
            tab(TodayView(), "Today", "circle.hexagongrid.fill")
            tab(SleepView(), "Sleep", "bed.double.fill")
            tab(WorkoutsView(), "Workouts", "figure.run")
            tab(StressView(), "Stress", "bolt.heart.fill")
            moreTab
        }
        .tint(StrandPalette.accent)
        .preferredColorScheme(.dark)
        .task { await repo.refresh() }
    }

    private func tab<V: View>(_ view: V, _ title: LocalizedStringKey, _ icon: String) -> some View {
        view
            .background(StrandPalette.surfaceBase.ignoresSafeArea())
            .tabItem { Label(title, systemImage: icon) }
    }

    private var moreTab: some View {
        // Mirrors the Mac sidebar curation: only the items you kept. Removed entirely (as on Mac):
        // Coach, Breathe, Intervals, Compare. Health is reachable here; Live folds into Today.
        NavigationStack {
            List {
                Section("Insights") {
                    link("Intelligence", "brain.head.profile") { IntelligenceView() }
                    link("Insights", "lightbulb.fill") { InsightsView() }
                    link("Explore", "square.grid.2x2.fill") { MetricExplorerView() }
                }
                Section("Body") {
                    link("Health", "heart.text.square.fill") { HealthView() }
                }
                Section("Data") {
                    link("Apple Health", "heart.fill") { AppleHealthView() }
                    link("Data Sources", "externaldrive.fill") { DataSourcesView() }
                    // #155: HealthKit-free Apple Health path for sideloaded installs (Siri Shortcut
                    // reads the opt-in Documents/noop_sync.txt drop file).
                    link("Shortcuts Export", "square.and.arrow.up.fill") { ShortcutExportSettingsView() }
                }
                Section("App") {
                    // (Notifications is macOS-only — NotificationSettingsView uses AppKit/NSWorkspace
                    //  for real app icons. DataSources, which you folded into it on Mac, is in the
                    //  Data section above, so the substance is still reachable on iPhone.)
                    link("Automations", "wand.and.stars") { AutomationsView() }
                    link("Smart alarm", "alarm.fill") { SmartAlarmView() }
                    link("Settings", "gearshape.fill") { SettingsView() }
                    link("Support", "hands.clap.fill") { SupportView() }
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(StrandPalette.surfaceBase.ignoresSafeArea())
            .navigationTitle("More")
        }
        .tabItem { Label("More", systemImage: "ellipsis.circle.fill") }
    }

    private func link<V: View>(_ title: LocalizedStringKey, _ icon: String, @ViewBuilder _ dest: @escaping () -> V) -> some View {
        NavigationLink {
            dest()
                .background(StrandPalette.surfaceBase.ignoresSafeArea())
                .navigationBarTitleDisplayMode(.inline)
                .toolbarBackground(StrandPalette.surfaceBase, for: .navigationBar)
        } label: {
            Label(title, systemImage: icon)
        }
        .listRowBackground(StrandPalette.surfaceRaised)
    }
}
#endif
