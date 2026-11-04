import AetherEngine
import NightglassCore
import SwiftUI

struct RootView: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @Environment(Nav.self) private var nav

    var body: some View {
        ZStack {
            BokehBackground(tint: player.current?.live.map { NG.neon(for: $0.channel.group ?? $0.channel.name) } ?? NG.magenta,
                            paused: player.expanded)
            VStack(spacing: 0) {
                Header()
                Group {
                    if store.sources.isEmpty && nav.section != .sources { EmptyUplink() } else {
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
                Wire(items: wireItems) { id in
                    if let item = store.channels.first(where: { $0.id == id }) { nav.focusChannel = item.id; nav.section = .live }
                }
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

    var wireItems: [Wire.Item] {
        let favs = store.channels.filter { store.favorites.contains($0.id) }
        return (favs.isEmpty ? Array(store.channels.prefix(40)) : favs).compactMap { item in
            guard let now = store.nowNext(for: item.channel).now else { return nil }
            let left = max(1, Int(now.end.timeIntervalSinceNow / 60))
            return .init(id: item.id, name: displayName(item.channel.name).uppercased(),
                         text: "\(now.title.uppercased()) · \(left) MIN LEFT", color: NG.neon(for: item.channel.name))
        }
    }
}

struct Header: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @Environment(Nav.self) private var nav
    @State private var boot = 0
    @State private var sound = Sound.enabled

    var body: some View {
        @Bindable var nav = nav
        HStack(spacing: 22) {
            VStack(alignment: .leading, spacing: 6) {
                ChromaText(text: "NIGHTGLASS", size: 36).modifier(NeonFlicker(trigger: boot))
                HStack(spacing: 0) {
                    Text("\(store.channels.count.formatted()) CHANNELS · ")
                    Text(store.loading.isEmpty ? "LINK STABLE" : "SYNCING…").foregroundStyle(store.loading.isEmpty ? NG.cyan : NG.acid)
                }
                .font(NG.mono(10)).kerning(3).foregroundStyle(NG.dim).lineLimit(1).fixedSize()
            }
            .fixedSize()
            HStack(spacing: 2) {
                ForEach(Section.allCases.prefix(4)) { s in
                    Button { nav.section = s } label: {
                        VStack(spacing: 4) {
                            Text(s.rawValue).font(NG.title(20, .bold)).kerning(2.5).lineLimit(1).fixedSize()
                                .foregroundStyle(nav.section == s ? .white : NG.dim)
                            Rectangle().fill(NG.magenta).frame(height: 2).neonGlow(NG.magenta, radius: 5).opacity(nav.section == s ? 1 : 0)
                        }
                        .padding(.horizontal, 12).contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                }
            }
            Button { nav.searching = true } label: {
                HStack(spacing: 10) {
                    Text(">").foregroundStyle(NG.cyan)
                    Text("SEARCH THE CITY").lineLimit(1).truncationMode(.tail)
                    Spacer(minLength: 0)
                    Tag(text: "⌘K").fixedSize()
                }
                .font(NG.mono(11)).kerning(2).foregroundStyle(NG.dim)
                .padding(.horizontal, 12).padding(.vertical, 9).frame(minWidth: 90, maxWidth: 320)
                .background(NG.ink.opacity(0.55)).overlay(Rectangle().stroke(NG.cyan.opacity(0.25)))
            }
            .buttonStyle(.plain)
            Spacer()
            // Narrow windows drop the labels first, then the uplinks button.
            ViewThatFits(in: .horizontal) {
                hud(labels: true, uplinks: true)
                hud(labels: false, uplinks: true)
                hud(labels: false, uplinks: false)
            }
            .font(NG.mono(11)).kerning(2).foregroundStyle(NG.dim)
        }
        .padding(.horizontal, 28).padding(.top, 30).padding(.bottom, 8)
        .onAppear { boot += 1 }
    }

    func hud(labels: Bool, uplinks: Bool) -> some View {
        HStack(spacing: 16) {
            TimelineView(.periodic(from: .now, by: 1)) { ctx in
                Text(ctx.date.formatted(date: .omitted, time: labels ? .standard : .shortened)).font(NG.mono(13)).foregroundStyle(NG.text).fixedSize()
            }
            HStack(spacing: 6) { if labels { Text("SIGNAL").fixedSize() }; SignalBars(level: player.current == nil ? 0 : player.signal) }
            if uplinks { Button { nav.section = .sources } label: { hudButton("UPLINKS", "\(store.sources.count)", hot: nav.section == .sources) }.buttonStyle(.plain) }
            Button { sound.toggle(); Sound.enabled = sound } label: { hudButton("SOUND", sound ? "ON" : "OFF", hot: sound) }.buttonStyle(.plain)
        }
        .fixedSize()
    }

    func hudButton(_ label: String, _ value: String, hot: Bool) -> some View {
        (Text(label + " ").foregroundStyle(NG.text) + Text(value).foregroundStyle(hot ? NG.magenta : NG.cyan))
            .font(NG.mono(11, .semibold)).kerning(2).lineLimit(1).fixedSize()
            .padding(.horizontal, 12).padding(.vertical, 8)
            .background(NG.ink.opacity(0.55))
            .overlay(Rectangle().stroke((hot ? NG.magenta : NG.cyan).opacity(0.4)))
    }
}

struct EmptyUplink: View {
    @Environment(Nav.self) private var nav
    var body: some View {
        VStack(spacing: 18) {
            ChromaText(text: "NO UPLINK", size: 64)
            Text("Ajoute une playlist M3U, un compte Xtream ou un portail Stalker.").font(NG.mono(13)).foregroundStyle(NG.dim)
            Button { nav.section = .sources } label: { Tag(text: "JACK IN  →", color: NG.cyan).scaleEffect(1.4) }.buttonStyle(.plain).padding(.top, 8)
        }
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
