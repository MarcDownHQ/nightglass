import AetherEngine
import NightglassCore
import SwiftUI

struct LiveView: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @Environment(Nav.self) private var nav
    @State private var category = "ALL"
    @State private var selected: String?
    @State private var dial = Dial()
    @FocusState private var focused: Bool

    static let favorites = "★ FAVORITES"

    var categories: [String] {
        var seen = Set<String>(), out = ["ALL"]
        if !store.favorites.isEmpty { out.insert(Self.favorites, at: 0) }
        for c in store.channels { if let g = c.channel.group, seen.insert(g).inserted { out.append(g) } }
        return out
    }

    var list: [LiveItem] {
        switch category {
        case "ALL": store.channels
        case Self.favorites: store.channels.filter { store.favorites.contains($0.id) }
        default: store.channels.filter { $0.channel.group == category }
        }
    }

    var body: some View {
        let list = list
        HStack(alignment: .top, spacing: 32) {
            VStack(spacing: 14) {
                ScrollView(.horizontal) {
                    HStack(alignment: .top, spacing: 12) {
                        ForEach(Array(categories.enumerated()), id: \.element) { i, c in
                            NeonSign(title: c, selected: c == category, color: c == Self.favorites ? NG.acid : nil) { category = c }
                                .padding(.top, CGFloat([0, 26, 8, 40, 14, 32, 4, 22][i % 8]))
                        }
                    }
                    .padding(.horizontal, 4).padding(.bottom, 14)
                }
                .scrollIndicators(.hidden)
                .frame(height: 236)
                HUDPanel(caption: "CHANNELS · \(category.uppercased())", trailing: "\(list.count) LIVE") {
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
                    KeyHints(hints: [("↑↓", "CHANNEL"), ("←→", "DISTRICT"), ("↵", "WATCH"), ("0-9", "DIAL"), ("S", "FAVORITE"), ("⌘K", "SEARCH")])
                }
            }
            .frame(minWidth: 440)
            VStack(alignment: .trailing, spacing: 0) {
                Preview()
                    .overlay(alignment: .topTrailing) { DialOverlay(dial: dial).padding(20) }
                    .onTapGesture(count: 2) { if player.current != nil { player.expanded = true } }
                NowNextPanel(item: player.current?.live).padding(.top, 70)
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
                if let item = store.channels.first(where: { $0.channel.number == n }) { category = "ALL"; select(item) }
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
        category = "ALL"
        select(item)
    }
}

struct ChannelRow: View {
    @Environment(Store.self) private var store
    let item: LiveItem
    let selected: Bool
    @State private var hover = false
    @State private var flicks = 0

    var body: some View {
        let ch = item.channel
        let c = NG.neon(for: ch.name)
        let np = store.nowNext(for: ch)
        HStack(spacing: 14) {
            Text(ch.number.map { String(format: "%03d", $0) } ?? "---")
                .font(NG.mono(12)).foregroundStyle(selected ? NG.magenta : NG.cyan.opacity(0.6)).frame(width: 36, alignment: .leading)
            ChannelLogo(channel: ch, color: c).modifier(NeonFlicker(trigger: flicks))
            VStack(alignment: .leading, spacing: 3) {
                if selected { ChromaText(text: displayName(ch.name).uppercased(), size: 19, weight: .bold).fixedSize() }
                else { Text(displayName(ch.name).uppercased()).font(NG.title(19)).kerning(1).foregroundStyle(NG.text) }
                Text((np.now?.title ?? ch.group ?? "—").uppercased()).font(NG.mono(11)).kerning(0.6).foregroundStyle(NG.dim).lineLimit(1)
                ProgressLine(value: np.now.map(progress) ?? 0, color: c).padding(.top, 4)
            }
            Spacer(minLength: 8)
            if store.favorites.contains(item.id) { Text("★").foregroundStyle(NG.acid) }
            if let q = qualityTag(ch.name) { Tag(text: q) }
        }
        .padding(.horizontal, 16).padding(.vertical, 9)
        .background {
            if selected {
                LinearGradient(colors: [NG.magenta.opacity(0.24), NG.magenta.opacity(0.02)], startPoint: .leading, endPoint: .trailing)
            } else if hover {
                LinearGradient(colors: [NG.cyan.opacity(0.09), .clear], startPoint: .leading, endPoint: .trailing)
            }
        }
        .overlay(alignment: .leading) {
            if selected { Rectangle().fill(NG.magenta).frame(width: 3).padding(.vertical, 7).neonGlow(NG.magenta, radius: 8) }
        }
        .contentShape(.rect)
        .onHover { hover = $0; if $0 { flicks += 1 } }
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

struct KeyHints: View {
    let hints: [(String, String)]
    var body: some View {
        HStack(spacing: 14) {
            ForEach(hints, id: \.1) { k, label in
                HStack(spacing: 5) {
                    Text(k).foregroundStyle(NG.ink).padding(.horizontal, 5).background(NG.text.opacity(0.85))
                    Text(label)
                }
                .lineLimit(1).fixedSize()
            }
            Spacer()
        }
        .font(NG.mono(10)).kerning(1.5).foregroundStyle(NG.dim)
        .padding(.horizontal, 16).padding(.vertical, 10)
        .overlay(alignment: .top) { Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1) }
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
                    NoSignal(message: player.error ?? "TUNING · \(displayName(current.title).uppercased())")
                }
                GlitchOverlay(trigger: player.zapTick)
                liveBug.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading).padding(14)
            } else {
                NoSignal(message: "SELECT A CHANNEL")
            }
        }
        .aspectRatio(16 / 9, contentMode: .fit)
        .frame(maxWidth: 820)
        .overlay(Rectangle().stroke(NG.cyan.opacity(0.5), lineWidth: 1))
        .overlay(alignment: .topLeading) { Bracket().stroke(NG.magenta, lineWidth: 2).frame(width: 18, height: 18).offset(x: -7, y: -7).neonGlow(NG.magenta, radius: 4) }
        .overlay(alignment: .bottomTrailing) { Bracket().stroke(NG.magenta, lineWidth: 2).frame(width: 18, height: 18).rotationEffect(.degrees(180)).offset(x: 7, y: 7).neonGlow(NG.magenta, radius: 4) }
        .shadow(color: NG.cyan.opacity(0.16), radius: 40)
        .background(alignment: .bottom) {
            // Wet-floor reflection: a tinted, fading glow under the screen.
            LinearGradient(colors: [NG.cyan.opacity(0.22), NG.magenta.opacity(0.08), .clear], startPoint: .top, endPoint: .bottom)
                .frame(height: 70).offset(y: 74).blur(radius: 8)
        }
    }

    var liveBug: some View {
        HStack(spacing: 6) {
            TimelineView(.periodic(from: .now, by: 0.6)) { ctx in
                Circle().fill(NG.magenta).frame(width: 8).opacity(Int(ctx.date.timeIntervalSinceReferenceDate / 0.6) % 2 == 0 ? 1 : 0.35)
            }
            Text("LIVE").font(NG.mono(11, .semibold)).kerning(2)
        }
        .padding(.horizontal, 8).padding(.vertical, 4).background(NG.ink.opacity(0.7))
        .opacity(player.current?.isLive == true ? 1 : 0)
    }
}

struct Bracket: Shape {
    func path(in r: CGRect) -> Path { Path { p in p.move(to: CGPoint(x: 0, y: r.maxY)); p.addLine(to: .zero); p.addLine(to: CGPoint(x: r.maxX, y: 0)) } }
}

struct NoSignal: View {
    let message: String
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            TimelineView(.animation(minimumInterval: 1 / 15, paused: reduceMotion)) { _ in
                Canvas { g, size in
                    for _ in 0..<500 {
                        let r = CGRect(x: .random(in: 0...size.width), y: .random(in: 0...size.height), width: 3, height: 2)
                        g.fill(Path(r), with: .color(.white.opacity(.random(in: 0.02...0.12))))
                    }
                }
            }
            Text(message.uppercased()).font(NG.mono(12)).kerning(3).foregroundStyle(NG.dim).multilineTextAlignment(.center).padding()
        }
    }
}

/// 190 ms of CRT failure on every zap.
struct GlitchOverlay: View {
    let trigger: Int
    @State private var until = Date.distantPast
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(paused: until < .now)) { ctx in
            if ctx.date < until {
                Canvas { g, size in
                    for _ in 0..<1600 {
                        let r = CGRect(x: .random(in: 0...size.width), y: .random(in: 0...size.height), width: 4, height: 3)
                        g.fill(Path(r), with: .color(.white.opacity(.random(in: 0...0.55))))
                    }
                    for _ in 0..<7 {
                        let y = CGFloat.random(in: 0...size.height)
                        g.fill(Path(CGRect(x: .random(in: -40...40), y: y, width: size.width, height: .random(in: 3...24))),
                               with: .color(.white.opacity(0.08)))
                    }
                    g.blendMode = .plusLighter
                    g.fill(Path(CGRect(x: .random(in: 0...size.width), y: 0, width: .random(in: 10...60), height: size.height)), with: .color(NG.magenta.opacity(0.35)))
                    g.fill(Path(CGRect(x: .random(in: 0...size.width), y: 0, width: .random(in: 10...60), height: size.height)), with: .color(NG.cyan.opacity(0.35)))
                }
            }
        }
        .allowsHitTesting(false)
        .onChange(of: trigger) { if !reduceMotion { until = .now.addingTimeInterval(0.19) } }
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
                    Text("● NOW" + (np.now.map { " · \(hm($0.start))–\(hm($0.end))" } ?? "")).font(NG.mono(10)).kerning(3).foregroundStyle(NG.acid)
                    ChromaText(text: (np.now?.title ?? item.map { displayName($0.channel.name) } ?? "—").uppercased(), size: 30)
                        .lineLimit(2).frame(maxWidth: .infinity, alignment: .leading)
                    Text(np.now?.desc ?? (item == nil ? "Choisis une chaîne." : "Pas de guide des programmes pour cette chaîne."))
                        .font(NG.mono(12)).foregroundStyle(NG.dim).lineLimit(3).lineSpacing(3)
                    ProgressLine(value: np.now.map(progress) ?? 0, height: 3).padding(.top, 6)
                }
                .padding(18).frame(maxWidth: .infinity, alignment: .leading)
                Rectangle().fill(NG.cyan.opacity(0.14)).frame(width: 1)
                VStack(alignment: .leading, spacing: 6) {
                    Text("NEXT" + (np.next.map { " · \(hm($0.start))" } ?? "")).font(NG.mono(10)).kerning(3).foregroundStyle(NG.cyan)
                    Text((np.next?.title ?? "—").uppercased()).font(NG.title(20)).kerning(1).foregroundStyle(NG.dim).lineLimit(2)
                    if let now = np.now { Text("DANS \(max(1, Int(now.end.timeIntervalSinceNow / 60))) MIN").font(NG.mono(11)).foregroundStyle(NG.dim) }
                }
                .padding(18).frame(width: 240, alignment: .leading)
            }
            Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1)
            HStack(spacing: 18) {
                Text("CH ") + Text(item?.channel.number.map { String(format: "%03d", $0) } ?? "---").foregroundStyle(NG.text)
                Text(player.videoSize.height > 0 ? String(Int(player.videoSize.height)) + "P" : "—").foregroundStyle(NG.text)
                Text(item?.source.name.uppercased() ?? "")
                Spacer()
                Text("LINK"); SignalBars(level: item == nil ? 0 : player.signal)
            }
            .font(NG.mono(10)).kerning(2).foregroundStyle(NG.dim).padding(.horizontal, 18).padding(.vertical, 10)
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
        if !dial.digits.isEmpty { ChromaText(text: dial.digits, size: 90).transition(.opacity) }
    }
}
