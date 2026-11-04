import AetherEngine
import SwiftUI

/// Fullscreen playback: no ambient effect, a thin HUD that fades when the pointer rests.
struct PlayerScreen: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @State private var idle = false
    @State private var idleTask: Task<Void, Never>?
    @State private var dial = Dial()
    @FocusState private var focused: Bool

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            AetherPlayerSurface(engine: player.engine).ignoresSafeArea()
            if !player.firstFrame || player.error != nil {
                NoSignal(message: player.error ?? "TUNING · \(player.current.map { displayName($0.title) } ?? "")")
            }
            GlitchOverlay(trigger: player.zapTick)
            VStack(spacing: 0) {
                top
                Spacer()
                bottom
            }
            .opacity(idle ? 0 : 1)
            .animation(.easeOut(duration: 0.4), value: idle)
            DialOverlay(dial: dial).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing).padding(.top, 90).padding(.trailing, 40)
        }
        .onContinuousHover { _ in poke() }
        .onTapGesture(count: 2) { exit() }
        .onAppear { focused = true; poke() }
        .onDisappear { idleTask?.cancel(); NSCursor.unhide() }
        .focusable().focusEffectDisabled().focused($focused)
        .onKeyPress(.escape) { exit(); return .handled }
        .onKeyPress("f") { exit(); return .handled }
        .onKeyPress(.space) { player.togglePlayPause(); poke(); return .handled }
        .onKeyPress(.leftArrow) { player.seek(by: -10); poke(); return .handled }
        .onKeyPress(.rightArrow) { player.seek(by: 10); poke(); return .handled }
        .onKeyPress(.upArrow) { zap(-1); return .handled }
        .onKeyPress(.downArrow) { zap(1); return .handled }
        .onKeyPress(characters: .decimalDigits) { press in
            dial.type(press.characters) { n in
                if let item = store.channels.first(where: { $0.channel.number == n }) { tune(item) }
            }
            return .handled
        }
    }

    var top: some View {
        HStack(spacing: 20) {
            if let n = player.current?.live?.channel.number { ChromaText(text: String(format: "%03d", n), size: 34) }
            Text(player.current.map { displayName($0.title).uppercased() } ?? "").font(NG.title(34, .heavy)).kerning(2)
            Spacer()
            TimelineView(.periodic(from: .now, by: 1)) { ctx in Text(ctx.date.formatted(date: .omitted, time: .standard)) }
            HStack(spacing: 6) { Text("SIGNAL"); SignalBars(level: player.signal) }
            trackMenus
            Button { exit() } label: { Tag(text: "ESC", color: NG.cyan) }.buttonStyle(.plain)
        }
        .font(NG.mono(12)).kerning(2).foregroundStyle(NG.text)
        .padding(.horizontal, 34).padding(.top, 34).padding(.bottom, 40)
        .background(LinearGradient(colors: [NG.ink.opacity(0.85), .clear], startPoint: .top, endPoint: .bottom))
    }

    @ViewBuilder var bottom: some View {
        Group {
            if let live = player.current?.live {
                let np = store.nowNext(for: live.channel)
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("● NOW").font(NG.mono(10)).kerning(3).foregroundStyle(NG.acid)
                        Text((np.now?.title ?? displayName(live.channel.name)).uppercased()).font(NG.title(34, .heavy)).kerning(1)
                        if let now = np.now { ProgressLine(value: progress(now), height: 3).frame(width: 420) }
                    }
                    Spacer()
                    if let next = np.next {
                        VStack(alignment: .trailing, spacing: 6) {
                            Text("NEXT").font(NG.mono(10)).kerning(3).foregroundStyle(NG.cyan)
                            Text("\(hm(next.start)) · \(next.title.uppercased())").font(NG.mono(14))
                        }
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    Text(player.current?.subtitle.uppercased() ?? "").font(NG.mono(11)).kerning(3).foregroundStyle(NG.acid)
                    HStack(spacing: 14) {
                        Button { player.togglePlayPause() } label: {
                            Image(systemName: player.state == .playing ? "pause.fill" : "play.fill").font(.system(size: 20))
                        }
                        .buttonStyle(.plain)
                        Text(clock(player.time))
                        GeometryReader { g in
                            ProgressLine(value: player.duration > 0 ? player.time / player.duration : 0, color: NG.cyan, height: 3)
                                .frame(maxHeight: .infinity)
                                .contentShape(.rect)
                                .onTapGesture { loc in
                                    let target = player.duration * loc.x / g.size.width
                                    Task { await player.engine.seek(to: target) }
                                }
                        }
                        .frame(height: 20)
                        Text(clock(player.duration))
                    }
                }
            }
        }
        .font(NG.mono(12)).foregroundStyle(NG.text)
        .padding(.horizontal, 34).padding(.bottom, 34).padding(.top, 50)
        .background(LinearGradient(colors: [.clear, NG.ink.opacity(0.9)], startPoint: .top, endPoint: .bottom))
    }

    @ViewBuilder var trackMenus: some View {
        if player.audioTracks.count > 1 {
            Menu("AUDIO") {
                ForEach(player.audioTracks) { t in Button(trackName(t)) { player.engine.selectAudioTrack(index: t.id) } }
            }
            .menuStyle(.borderlessButton).fixedSize()
        }
        if !player.subtitleTracks.isEmpty {
            Menu("SUBS") {
                Button("Off") { player.engine.selectSubtitleTrack(index: -1) }
                ForEach(player.subtitleTracks) { t in Button(trackName(t)) { player.engine.selectSubtitleTrack(index: t.id) } }
            }
            .menuStyle(.borderlessButton).fixedSize()
        }
    }

    func trackName(_ t: TrackInfo) -> String {
        [t.language?.uppercased(), t.name.isEmpty ? nil : t.name, t.codec.uppercased(), t.channels > 2 ? "\(t.channels)ch" : nil]
            .compactMap { $0 }.joined(separator: " · ")
    }

    func clock(_ s: Double) -> String {
        guard s.isFinite, s >= 0 else { return "--:--" }
        let i = Int(s)
        return i >= 3600 ? String(format: "%d:%02d:%02d", i / 3600, i / 60 % 60, i % 60) : String(format: "%02d:%02d", i / 60, i % 60)
    }

    func poke() {
        idle = false; NSCursor.unhide()
        idleTask?.cancel()
        idleTask = Task {
            try? await Task.sleep(for: .seconds(2.5))
            guard !Task.isCancelled else { return }
            idle = true; NSCursor.hide()
        }
    }

    func exit() {
        player.expanded = false
        if player.current?.isLive == false { player.stop() }
    }

    func zap(_ delta: Int) {
        guard let live = player.current?.live, let i = store.channels.firstIndex(of: live) else { return }
        tune(store.channels[(i + delta + store.channels.count) % store.channels.count])
    }

    func tune(_ item: LiveItem) {
        poke()
        Task { if let p = try? await store.playable(item) { player.play(p) } }
    }
}
