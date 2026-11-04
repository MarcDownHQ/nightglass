import SwiftUI

enum Section: String, CaseIterable, Identifiable {
    case live = "LIVE", guide = "GUIDE", movies = "MOVIES", series = "SERIES", sources = "SOURCES"
    var id: String { rawValue }
    var title: String { rawValue.capitalized }
}

@MainActor @Observable
final class Nav {
    var section: Section = .live
    var searching = false
    /// Channel the guide or search asked the Live screen to select.
    var focusChannel: String?
}

@main
struct NightglassApp: App {
    @State private var store = Store()
    @State private var player = Player()
    @State private var nav = Nav()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(store).environment(player).environment(nav)
                .frame(minWidth: 1280, minHeight: 780)
                .preferredColorScheme(.dark)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1440, height: 900)
        .commands {
            CommandMenu("Navigate") {
                ForEach(Array(Section.allCases.prefix(4).enumerated()), id: \.offset) { i, s in
                    Button(s.title) { nav.section = s; player.expanded = false }
                        .keyboardShortcut(KeyEquivalent(Character("\(i + 1)")), modifiers: .command)
                }
                Button("Sources") { nav.section = .sources; player.expanded = false }.keyboardShortcut("5", modifiers: .command)
                Divider()
                Button("Search") { nav.searching = true }.keyboardShortcut("k", modifiers: .command)
            }
        }
    }
}
