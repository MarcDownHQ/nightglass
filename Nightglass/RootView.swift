import AetherEngine
import NightglassCore
import SwiftUI

struct RootView: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @Environment(Nav.self) private var nav

    var body: some View {
        ZStack {
            Background()
            VStack(spacing: 0) {
                Header()
                Group {
                    if store.sources.isEmpty && nav.section != .sources { EmptySources() } else {
                        switch nav.section {
                        case .live: LiveView()
                        case .guide: GuideView()
                        case .movies: MoviesView()
                        case .series: SeriesView()
                        case .sources: SourcesView()
                        }
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .transition(.opacity)
            }
            .opacity(player.expanded ? 0 : 1)
            if player.expanded { PlayerScreen().transition(.opacity) }
            if nav.searching { SearchPalette().transition(.opacity.combined(with: .scale(scale: 0.98))) }
        }
        .animation(.spring(duration: 0.35, bounce: 0.15), value: nav.section)
        .animation(.spring(duration: 0.35, bounce: 0.15), value: player.expanded)
        .animation(.spring(duration: 0.3, bounce: 0.1), value: nav.searching)
        .background(NG.ink)
        #if DEBUG
        .task { await DebugAutomation.run(store: store, player: player, nav: nav) }
        .overlay(alignment: .bottomTrailing) {
            // Read by the UI tests.
            Text("state=\(String(describing: player.state)) firstFrame=\(player.firstFrame) rendered=\(player.rendered) video=\(Int(player.videoSize.width))x\(Int(player.videoSize.height)) channels=\(store.channels.count) guide=\(store.guide.count)")
                .font(.system(size: 1)).opacity(0.01).accessibilityIdentifier("ng-status")
        }
        #endif
    }
}

struct Header: View {
    @Environment(Store.self) private var store
    @Environment(Nav.self) private var nav

    var body: some View {
        HStack(spacing: 28) {
            NeonText(text: "NIGHTGLASS", size: 30).fixedSize()
            HStack(spacing: 4) {
                ForEach(Section.allCases.prefix(4)) { s in navButton(s) }
            }
            Spacer()
            if !store.loading.isEmpty { ProgressView().controlSize(.small) }
            if store.sources.count > 1 { SourcePicker() }
            Button { nav.searching = true } label: {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                    Text("Search")
                    Text("⌘K").foregroundStyle(NG.dim)
                }
                .font(NG.title(15, .medium)).foregroundStyle(NG.text)
                .padding(.horizontal, 14).padding(.vertical, 7)
                .overlay(Capsule().strokeBorder(NG.text.opacity(0.15)))
                .contentShape(.capsule)
            }
            .buttonStyle(.plain)
            navButton(.sources)
        }
        .padding(.horizontal, 28).padding(.top, 34).padding(.bottom, 10)
    }

    func navButton(_ s: Section) -> some View {
        let on = nav.section == s
        return Button { nav.section = s } label: {
            Text(s.title).font(NG.title(18, .semibold)).lineLimit(1).fixedSize()
                .foregroundStyle(on ? .white : NG.dim)
                .neonGlow(on ? NG.magenta : .clear, radius: 8)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
    }
}

/// Which source the screens show. One click to switch, no trip to the Sources screen.
struct SourcePicker: View {
    @Environment(Store.self) private var store

    var body: some View {
        @Bindable var store = store
        let current = store.visibleSources.count == 1 ? store.visibleSources[0].name : "All sources"
        Menu {
            Picker("Source", selection: $store.activeSource) {
                Text("All sources").tag(UUID?.none)
                Divider()
                ForEach(store.sources) { s in Text(s.name).tag(Optional(s.id)) }
            }
            .pickerStyle(.inline)
        } label: {
            HStack(spacing: 6) {
                Circle().fill(NG.magenta).frame(width: 6, height: 6).neonGlow(NG.magenta, radius: 4)
                Text(current).lineLimit(1)
                Image(systemName: "chevron.down").font(.system(size: 10, weight: .bold)).foregroundStyle(NG.dim)
            }
            .font(NG.title(15, .medium)).foregroundStyle(NG.text)
            .padding(.horizontal, 14).padding(.vertical, 7)
            .overlay(Capsule().strokeBorder(NG.magenta.opacity(0.5)))
            .contentShape(.capsule)
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help("Choose the source to show")
        .accessibilityIdentifier("source-picker")
    }
}

struct EmptySources: View {
    @Environment(Nav.self) private var nav
    var body: some View {
        VStack(spacing: 18) {
            NeonText(text: "No source yet", size: 44)
            Text("Add an M3U playlist, an Xtream account or a Stalker portal.").font(NG.title(16, .regular)).foregroundStyle(NG.dim)
            Button("Add a source") { nav.section = .sources }
                .buttonStyle(NeonButtonStyle()).padding(.top, 8)
        }
    }
}

/// Filled neon button for the primary action of a screen.
struct NeonButtonStyle: ButtonStyle {
    var color: Color = NG.magenta
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(NG.title(18, .bold)).foregroundStyle(NG.ink)
            .padding(.horizontal, 22).padding(.vertical, 10)
            .background(color.opacity(configuration.isPressed ? 0.7 : 1), in: .capsule)
            .neonGlow(color, radius: 10)
    }
}

/// "Alpe d'Huez TV (720p) [Not 24/7]" → "Alpe d'Huez TV". The quality comes back as a tag via `qualityTag`.
func displayName(_ name: String) -> String {
    name.replacing(/\s*(\((\d{3,4}p|4K|HD|FHD|SD)\)|\[[^\]]*\])/.ignoresCase(), with: "").trimmingCharacters(in: .whitespaces)
}

func qualityTag(_ name: String) -> String? {
    guard let m = name.firstMatch(of: /\((\d{3,4})p\)/) else { return nil }
    return switch Int(m.1) ?? 0 { case 2160...: "4K"; case 1080...: "FHD"; case 720...: "HD"; default: "SD" }
}
