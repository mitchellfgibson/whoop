import SwiftUI
import StrandDesign

// PATCH (consolidated nav): Explore is now the hub for Explore + Health + Trends +
// Compare, selected by a segmented control. Each sub-view keeps its own ScreenScaffold
// (scroll + header), so exactly one is shown at a time — no nested scrolls.
struct ExploreHubView: View {
    enum Tab: String, CaseIterable, Identifiable {
        case explore = "Explore", health = "Health", trends = "Trends", compare = "Compare"
        var id: String { rawValue }
    }
    @State private var tab: Tab = .explore

    var body: some View {
        VStack(spacing: 0) {
            Picker("", selection: $tab) {
                ForEach(Tab.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, 28)
            .padding(.top, 14)
            .padding(.bottom, 6)
            .background(StrandPalette.surfaceBase)

            Divider().overlay(StrandPalette.hairline)

            switch tab {
            case .explore: MetricExplorerView()
            case .health: HealthView()
            case .trends: TrendsView()
            case .compare: CompareView()
            }
        }
        .background(StrandPalette.surfaceBase.ignoresSafeArea())
    }
}
