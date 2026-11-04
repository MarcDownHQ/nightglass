import AetherEngine
import NightglassCore
import SwiftUI

struct LiveView: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @Environment(Nav.self) private var nav
    @State private var category = "All"
    @State private var selected: String?
    @State private var dial = Dial()
    @FocusState private var focused: Bool

    static let favorites = "★ Favorites"

    var categories: [String] {
        var seen = Set<String>(), out = ["All"]
        if !store.favorites.isEmpty { out.insert(Self.favorites, at: 0) }
        for c in store.channels { if let g = c.channel.group, seen.insert(g).inserted { out.append(g) } }
        return out
    }

    var list: [LiveItem] {
        switch category {
        case "All": store.channels
        case Self.favorites: store.channels.filter { store.favorites.contains($0.id) }
        default: store.channels.filter { $0.channel.group == category }
        }
    }

    var body: some View {
        let list = list
        HStack(alignment: .top, spacing: 32) {
            VStack(spacing: 14) {
                TabRow(items: categories, selection: $category)
                HUDPanel(caption: category == "All" ? "All channels" : category, trailing: "\(list.count)") {
                    ScrollViewReader { proxy in
                        ScrollView {
                            LazyVStack(spacing: 0) {
                                ForEach(list) { item in
                                    ChannelRow(item: item, selected: item.id == selected)
                                        .id(item.id)
                                        .onTapGesture(count: 2) { select(item); player.expanded = true }
                                        .onTapGesture { select(item) }
                                        .contextMenu {
                                            Button(store.favorites.contains(item.id) ? "Retirer des favoris" : "Ajouter aux favoris") { store.toggleFavorite(item) }
                                        }
                                }
                            }
                            .padding(.vertical, 4)
                        }
                        .scrollIndicators(.never)
                        .onChange(of: selected) { _, id in withAnimation(.spring(duration: 0.25)) { proxy.scrollTo(id) } }
                    }
                }
            }
            .frame(minWidth: 440)
            VStack(alignment: .trailing, spacing: 0) {
                Preview()
                    .overlay(alignment: .topTrailing) { DialOverlay(dial: dial).padding(20) }
                    .onTapGesture(count: 2) { if player.current != nil { player.expanded = true } }
                NowNextPanel(item: player.current?.live).padding(.top, 24)
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity)
        }
        .padding(.horizontal, 28).padding(.top, 4).padding(.bottom, 16)
        .focusable().focusEffectDisabled().focused($focused)
        .onAppear { focused = true; consumeFocus() }
        .onChange(of: nav.focusChannel) { consumeFocus() }
        .onKeyPress(.downArrow) { move(1, in: list); return .handled }
        .onKeyPress(.upArrow) { move(-1, in: list); return .handled }
        .onKeyPress(.rightArrow) { shiftCategory(1); return .handled }
        .onKeyPress(.leftArrow) { shiftCategory(-1); return .handled }
        .onKeyPress(.return) { if player.current != nil { player.expanded = true }; return .handled }
        .onKeyPress(characters: .decimalDigits) { press in
            dial.type(press.characters) { n in
                if let item = store.channels.first(where: { $0.channel.number == n }) { category = "All"; select(item) }
            }
            return .handled
        }
        .onKeyPress("s") { if let item = list.first(where: { $0.id == selected }) { store.toggleFavorite(item) }; return .handled }
    }

    func select(_ item: LiveItem) {
        selected = item.id
        Task {
            do { player.play(try await store.playable(item)) }
            catch { player.error = error.localizedDescription }
        }
    }

    func move(_ delta: Int, in list: [LiveItem]) {
        guard !list.isEmpty else { return }
        let i = list.firstIndex { $0.id == selected }.map { ($0 + delta + list.count) % list.count } ?? 0
        select(list[i])
    }

    func shiftCategory(_ delta: Int) {
        let c = categories
        guard let i = c.firstIndex(of: category) else { return }
        category = c[(i + delta + c.count) % c.count]
    }

    func consumeFocus() {
        guard let id = nav.focusChannel, let item = store.channels.first(where: { $0.id == id }) else { return }
        nav.focusChannel = nil
        category = "All"
        select(item)
    }
}

struct ChannelRow: View {
    @Environment(Store.self) private var store
    let item: LiveItem
    let selected: Bool
    @State private var hover = false

    var body: some View {
        let ch = item.channel
        let c = NG.neon(for: ch.name)
        let np = store.nowNext(for: ch)
        HStack(spacing: 14) {
            Text(ch.number.map(String.init) ?? "")
                .font(NG.mono(12)).foregroundStyle(NG.dim).frame(width: 32, alignment: .leading)
            ChannelLogo(channel: ch, color: c)
            VStack(alignment: .leading, spacing: 3) {
                Text(displayName(ch.name)).font(NG.title(18, .semibold)).foregroundStyle(selected ? .white : NG.text)
                    .neonGlow(selected ? c : .clear, radius: 6).lineLimit(1)
                Text(np.now?.title ?? ch.group ?? "").font(NG.title(14, .regular)).foregroundStyle(NG.dim).lineLimit(1)
                ProgressLine(value: np.now.map(progress) ?? 0, color: c).padding(.top, 4)
            }
            Spacer(minLength: 8)
            if store.favorites.contains(item.id) { Text("★").foregroundStyle(NG.acid) }
            if let q = qualityTag(ch.name) { Tag(text: q) }
        }
        .padding(.horizontal, 16).padding(.vertical, 9)
        .background {
            if selected {
                c.opacity(0.14)
            } else if hover {
                NG.text.opacity(0.04)
            }
        }
        .overlay(alignment: .leading) {
            if selected { Capsule().fill(c).frame(width: 3).padding(.vertical, 10).neonGlow(c, radius: 6) }
        }
        .contentShape(.rect)
        .onHover { hover = $0 }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
        .accessibilityIdentifier("channel-" + ch.name)
    }
}

func progress(_ p: Programme) -> Double {
    min(1, max(0, Date.now.timeIntervalSince(p.start) / p.end.timeIntervalSince(p.start)))
}

struct ProgressLine: View {
    let value: Double
    var color: Color = NG.magenta
    var height: CGFloat = 2
    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Rectangle().fill(NG.text.opacity(0.1))
                Rectangle().fill(color).frame(width: g.size.width * value).shadow(color: color, radius: 3)
            }
        }
        .frame(height: height)
    }
}

struct ChannelLogo: View {
    let channel: Channel
    let color: Color
    var size: CGFloat = 42

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 7).fill(NG.ink.opacity(0.75))
            AsyncImage(url: channel.logo) { phase in
                if let image = phase.image { image.resizable().scaledToFit().padding(5) }
                else {
                    Text(channel.name.split(separator: " ").compactMap(\.first).prefix(2).map(String.init).joined().uppercased())
                        .font(NG.title(size * 0.36, .heavy)).foregroundStyle(.white).neonGlow(color, radius: 5)
                }
            }
            RoundedRectangle(cornerRadius: 7).strokeBorder(color, lineWidth: 1.5).shadow(color: color.opacity(0.6), radius: 5)
        }
        .frame(width: size, height: size)
    }
}

/// The live preview, with its floor reflection and the corner brackets.
struct Preview: View {
    @Environment(Player.self) private var player

    var body: some View {
        ZStack {
            Color.black
            if let current = player.current, !player.expanded {
                AetherPlayerSurface(engine: player.engine)
                if !player.firstFrame || player.error != nil {
                    StatusText(text: player.error ?? "Loading \(displayName(current.title))…", isError: player.error != nil)
                }
            } else {
                StatusText(text: "Select a channel")
            }
        }
        .aspectRatio(16 / 9, contentMode: .fit)
        .frame(maxWidth: 820)
        .clipShape(.rect(cornerRadius: 14))
        // Glow on the frame only: a shadow on the video itself would force an offscreen pass every frame.
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(NG.cyan.opacity(0.7), lineWidth: 1.5).neonGlow(NG.cyan, radius: 12))
    }
}

struct StatusText: View {
    let text: String
    var isError = false
    var body: some View {
        Text(text).font(NG.title(16, .medium)).foregroundStyle(isError ? NG.magenta : NG.dim)
            .multilineTextAlignment(.center).padding()
    }
}

struct NowNextPanel: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    let item: LiveItem?

    var body: some View {
        let np: (now: Programme?, next: Programme?) = item.map { store.nowNext(for: $0.channel) } ?? (nil, nil)
        HUDPanel {
            HStack(alignment: .top, spacing: 0) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Now" + (np.now.map { " · \(hm($0.start))–\(hm($0.end))" } ?? "")).font(NG.mono(11, .medium)).foregroundStyle(NG.acid)
                    NeonText(text: np.now?.title ?? item.map { displayName($0.channel.name) } ?? "—", size: 28, weight: .bold)
                        .lineLimit(2).frame(maxWidth: .infinity, alignment: .leading)
                    if let desc = np.now?.desc {
                        Text(desc).font(NG.title(15, .regular)).foregroundStyle(NG.dim).lineLimit(3).lineSpacing(2)
                    }
                    ProgressLine(value: np.now.map(progress) ?? 0, height: 3).padding(.top, 6)
                }
                .padding(18).frame(maxWidth: .infinity, alignment: .leading)
                Rectangle().fill(NG.cyan.opacity(0.14)).frame(width: 1)
                VStack(alignment: .leading, spacing: 6) {
                    Text("Next" + (np.next.map { " · \(hm($0.start))" } ?? "")).font(NG.mono(11, .medium)).foregroundStyle(NG.cyan)
                    Text(np.next?.title ?? "—").font(NG.title(19, .semibold)).foregroundStyle(NG.text).lineLimit(2)
                }
                .padding(18).frame(width: 240, alignment: .leading)
            }
            Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1)
            HStack(spacing: 14) {
                if let n = item?.channel.number { Text("Ch \(n)") }
                if player.videoSize.height > 0 { Text(String(Int(player.videoSize.height)) + "p") }
                Text(item?.source.name ?? "")
                Spacer()
                if item != nil { SignalBars(level: player.signal) }
            }
            .font(NG.mono(11)).foregroundStyle(NG.dim).padding(.horizontal, 18).padding(.vertical, 10)
        }
        .frame(maxWidth: 820)
    }
}

func hm(_ d: Date) -> String { d.formatted(date: .omitted, time: .shortened) }

/// Typing a channel number shows big chromatic digits, then tunes after a pause.
@MainActor @Observable
final class Dial {
    var digits = ""
    private var task: Task<Void, Never>?

    func type(_ d: String, then tune: @escaping (Int) -> Void) {
        digits = String((digits + d).suffix(4))
        task?.cancel()
        task = Task {
            try? await Task.sleep(for: .seconds(1.1))
            guard !Task.isCancelled else { return }
            if let n = Int(digits) { tune(n) }
            digits = ""
        }
    }
}

struct DialOverlay: View {
    let dial: Dial
    var body: some View {
        if !dial.digits.isEmpty { NeonText(text: dial.digits, size: 90).transition(.opacity) }
    }
}
