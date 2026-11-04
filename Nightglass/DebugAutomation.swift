#if DEBUG
import AppKit
import Foundation
import NightglassCore

/// Debug-only test hooks, driven by environment variables:
/// - `NG_SEED`: JSON `[Source]` added through `Store.add`, like the Sources screen does.
/// - `NG_SECTION`: LIVE / GUIDE / MOVIES / SERIES / SOURCES.
/// - `NG_PLAY`: substring of a channel name to tune; `NG_EXPAND=1` opens it fullscreen.
/// - `NG_PLAY_MOVIE`: substring of a movie name to play fullscreen (checked before `NG_PLAY`).
/// - `NG_SHOTS`: directory; the app then tours every screen and snapshots its own window there.
@MainActor
enum DebugAutomation {
    static let env = ProcessInfo.processInfo.environment

    static func run(store: Store, player: Player, nav: Nav) async {
        if let section = env["NG_SECTION"].flatMap(Section.init(rawValue:)) { nav.section = section }
        if let json = env["NG_SEED"], let sources = try? JSONDecoder().decode([Source].self, from: Data(json.utf8)) {
            for s in sources where !store.sources.contains(where: { $0.name == s.name }) {
                do { store.add(s, catalog: try await s.client().catalog()); log("seeded \(s.name): \(store.channels.count) channels") }
                catch { log("seed \(s.name) failed: \(error)") }
            }
        }
        if let name = env["NG_PLAY_MOVIE"] {
            for _ in 0..<30 where !store.movies.contains(where: { $0.movie.name.localizedStandardContains(name) }) { try? await Task.sleep(for: .seconds(1)) }
            if let m = store.movies.first(where: { $0.movie.name.localizedStandardContains(name) }) {
                player.play(Playable(id: m.id, title: m.movie.name, subtitle: "MOVIE", url: m.movie.stream, headers: [:], isLive: false))
                player.expanded = true
                for _ in 0..<30 {
                    try? await Task.sleep(for: .seconds(1))
                    log("movie \(m.movie.name) state=\(player.state) firstFrame=\(player.firstFrame) video=\(Int(player.videoSize.width))x\(Int(player.videoSize.height)) t=\(Int(player.time))/\(Int(player.duration)) audio=\(player.audioTracks.count)")
                    if player.firstFrame && player.time > 2 { break }
                }
                player.stop()
            }
        }
        if let name = env["NG_PLAY"] { await play(name, store: store, player: player, nav: nav) }
        if let dir = env["NG_SHOTS"] { await tour(dir, player: player, nav: nav) }
    }

    static func play(_ name: String, store: Store, player: Player, nav: Nav) async {
        for _ in 0..<60 where !store.channels.contains(where: { $0.channel.name.localizedStandardContains(name) }) {
            try? await Task.sleep(for: .seconds(1))
        }
        guard let item = store.channels.first(where: { $0.channel.name.localizedStandardContains(name) }) else { return log("no channel matching \(name)") }
        nav.focusChannel = item.id
        nav.section = .live
        if env["NG_EXPAND"] == "1" { try? await Task.sleep(for: .seconds(1)); player.expanded = true }
        for _ in 0..<30 {
            try? await Task.sleep(for: .seconds(1))
            log("play \(item.channel.name) state=\(player.state) firstFrame=\(player.firstFrame) video=\(Int(player.videoSize.width))x\(Int(player.videoSize.height)) epgNow=\(store.nowNext(for: item.channel).now?.title ?? "-")")
            if player.firstFrame { break }
        }
    }

    static func tour(_ dir: String, player: Player, nav: Nav) async {
        try? await Task.sleep(for: .seconds(4))
        for section in Section.allCases {
            nav.section = section
            try? await Task.sleep(for: .seconds(section == .live ? 2 : 4))
            snapshot("\(dir)/\(section.rawValue.lowercased()).png")
        }
        nav.section = .live; nav.searching = true
        try? await Task.sleep(for: .seconds(2))
        snapshot("\(dir)/search.png")
        nav.searching = false
        if player.current != nil {
            player.expanded = true
            try? await Task.sleep(for: .seconds(2))
            snapshot("\(dir)/player.png")
            player.expanded = false
        }
        log("shots done")
    }

    /// Own-window snapshot; needs no Screen Recording permission. Video layers may come out black.
    static func snapshot(_ path: String) {
        guard let view = NSApp.windows.first(where: \.isVisible)?.contentView,
              let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return log("snapshot failed") }
        view.cacheDisplay(in: view.bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(filePath: path))
        log("snapshot \(path)")
    }

    static func log(_ s: String) { print("[NG] \(s)"); fflush(stdout) }
}
#endif
