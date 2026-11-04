import NightglassCore
import SwiftUI

/// ⌘K: terminal-style palette across channels, movies and series.
struct SearchPalette: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player
    @Environment(Nav.self) private var nav
    @State private var query = ""
    @State private var index = 0
    @FocusState private var focused: Bool

    enum Hit: Identifiable {
        case channel(LiveItem), movie(MovieItem), series(SeriesItem)
        var id: String {
            switch self { case .channel(let i): "c" + i.id; case .movie(let i): "m" + i.id; case .series(let i): "s" + i.id }
        }
        var title: String {
            switch self { case .channel(let i): displayName(i.channel.name); case .movie(let i): i.movie.name; case .series(let i): i.series.name }
        }
        var kind: (String, Color) {
            switch self { case .channel: ("LIVE", NG.magenta); case .movie: ("MOVIE", NG.cyan); case .series: ("SERIES", NG.acid) }
        }
    }

    var hits: [Hit] {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return [] }
        let c = store.channels.lazy.filter { $0.channel.name.localizedStandardContains(q) }.prefix(12).map(Hit.channel)
        let m = store.movies.lazy.filter { $0.movie.name.localizedStandardContains(q) }.prefix(10).map(Hit.movie)
        let s = store.series.lazy.filter { $0.series.name.localizedStandardContains(q) }.prefix(10).map(Hit.series)
        return Array(c) + Array(m) + Array(s)
    }

    var body: some View {
        let hits = hits
        ZStack(alignment: .top) {
            NG.ink.opacity(0.6).ignoresSafeArea().onTapGesture { nav.searching = false }
            HUDPanel(caption: "SEARCH THE CITY", trailing: "ESC") {
                HStack(spacing: 10) {
                    Text(">").foregroundStyle(NG.cyan)
                    TextField("", text: $query, prompt: Text("CHANNEL, MOVIE, SERIES…").foregroundStyle(NG.dim))
                        .textFieldStyle(.plain).focused($focused)
                }
                .font(NG.mono(18)).padding(18)
                if !hits.isEmpty {
                    Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1)
                    ScrollView {
                        VStack(spacing: 0) {
                            ForEach(Array(hits.enumerated()), id: \.element.id) { i, hit in
                                HStack(spacing: 14) {
                                    Tag(text: hit.kind.0, color: hit.kind.1).frame(width: 70, alignment: .leading)
                                    Text(hit.title.uppercased()).font(NG.title(18)).kerning(0.8).lineLimit(1)
                                    Spacer()
                                    if i == index { Text("↵").font(NG.mono(13)).foregroundStyle(NG.magenta) }
                                }
                                .padding(.horizontal, 18).padding(.vertical, 9)
                                .background(i == index ? NG.magenta.opacity(0.18) : .clear)
                                .contentShape(.rect)
                                .onTapGesture { open(hit) }
                            }
                        }
                    }
                    .frame(maxHeight: 420)
                }
            }
            .frame(width: 680)
            .padding(.top, 120)
            .shadow(color: NG.magenta.opacity(0.2), radius: 40)
        }
        .onAppear { focused = true }
        .onChange(of: query) { index = 0 }
        .onKeyPress(.escape) { nav.searching = false; return .handled }
        .onKeyPress(.downArrow) { index = min(index + 1, max(hits.count - 1, 0)); return .handled }
        .onKeyPress(.upArrow) { index = max(index - 1, 0); return .handled }
        .onKeyPress(.return) { if let hit = hits[safe: index] { open(hit) }; return .handled }
    }

    func open(_ hit: Hit) {
        nav.searching = false
        switch hit {
        case .channel(let item): nav.focusChannel = item.id; nav.section = .live
        case .movie(let item):
            player.play(Playable(id: item.id, title: item.movie.name, subtitle: item.movie.group ?? "MOVIE", url: item.movie.stream, headers: [:], isLive: false))
            player.expanded = true
        case .series: nav.section = .series
        }
    }
}

struct SourcesView: View {
    @Environment(Store.self) private var store

    var body: some View {
        HStack(alignment: .top, spacing: 32) {
            HUDPanel(caption: "UPLINKS", trailing: "\(store.sources.count)") {
                ScrollView {
                    VStack(spacing: 0) {
                        if store.sources.isEmpty {
                            Text("Aucune source. Ajoute-en une à droite.").font(NG.mono(12)).foregroundStyle(NG.dim).padding(24)
                        }
                        ForEach(store.sources) { s in SourceRow(source: s) }
                    }
                }
            }
            .frame(maxWidth: .infinity)
            AddSourcePanel().frame(width: 460)
        }
        .padding(.horizontal, 28).padding(.vertical, 16)
    }
}

struct SourceRow: View {
    @Environment(Store.self) private var store
    let source: Source

    var body: some View {
        let cat = store.catalogs[source.id]
        HStack(spacing: 14) {
            Tag(text: kind, color: NG.neon(for: kind))
            VStack(alignment: .leading, spacing: 3) {
                Text(source.name.uppercased()).font(NG.title(20)).kerning(1)
                Group {
                    if store.loading.contains(source.id) { Text("SYNCING…").foregroundStyle(NG.acid) }
                    else if let e = store.errors[source.id] { Text(e).foregroundStyle(NG.magenta) }
                    else { Text("\(cat?.channels.count ?? 0) LIVE · \(cat?.movies.count ?? 0) MOVIES · \(cat?.series.count ?? 0) SERIES") }
                }
                .font(NG.mono(11)).kerning(1).foregroundStyle(NG.dim)
            }
            Spacer()
            Button { Task { await store.refresh(source) } } label: { Image(systemName: "arrow.clockwise") }.buttonStyle(.plain).help("Rafraîchir")
            Button { store.remove(source) } label: { Image(systemName: "trash") }.buttonStyle(.plain).foregroundStyle(NG.magenta).help("Supprimer")
        }
        .padding(.horizontal, 18).padding(.vertical, 14)
        .overlay(alignment: .bottom) { Rectangle().fill(NG.cyan.opacity(0.08)).frame(height: 1) }
    }

    var kind: String {
        switch source.kind { case .m3u: "M3U"; case .xtream: "XTREAM"; case .stalker: "STALKER" }
    }
}

struct AddSourcePanel: View {
    @Environment(Store.self) private var store
    enum Kind: String, CaseIterable { case m3u = "M3U", xtream = "XTREAM", stalker = "STALKER" }
    @State private var kind = Kind.m3u
    @State private var name = ""
    @State private var url = ""
    @State private var epg = ""
    @State private var user = ""
    @State private var pass = ""
    @State private var mac = "00:1A:79:"
    @State private var log: [(String, Color)] = []
    @State private var busy = false

    var body: some View {
        HUDPanel(caption: "JACK IN") {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 8) {
                    ForEach(Kind.allCases, id: \.self) { k in
                        Button { kind = k } label: {
                            Text(k.rawValue).font(NG.title(17, .bold)).kerning(1.5).padding(.horizontal, 12).padding(.vertical, 5)
                                .foregroundStyle(k == kind ? .white : NG.dim)
                                .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(k == kind ? NG.magenta : NG.text.opacity(0.2), lineWidth: 1.5))
                                .shadow(color: k == kind ? NG.magenta : .clear, radius: 6)
                        }
                        .buttonStyle(.plain)
                    }
                }
                field("NAME", $name, "My provider")
                switch kind {
                case .m3u:
                    field("PLAYLIST URL", $url, "https://… .m3u")
                    field("XMLTV GUIDE (OPTIONAL)", $epg, "https://… .xml")
                    Button {
                        name = "iptv-org · France"; url = "https://iptv-org.github.io/iptv/countries/fr.m3u"; epg = "https://xmltvfr.fr/xmltv/xmltv.xml"
                    } label: { Tag(text: "PRESET · IPTV-ORG FRANCE", color: NG.cyan) }.buttonStyle(.plain)
                case .xtream:
                    field("SERVER", $url, "http://provider:8080")
                    field("USERNAME", $user, "")
                    field("PASSWORD", $pass, "", secure: true)
                case .stalker:
                    field("PORTAL", $url, "http://portal/c/")
                    field("MAC ADDRESS", $mac, "00:1A:79:XX:XX:XX")
                }
                Button(action: connect) {
                    Text(busy ? "CONNECTING…" : "CONNECT →").font(NG.title(20, .heavy)).kerning(2)
                        .frame(maxWidth: .infinity).padding(.vertical, 10)
                        .background(NG.magenta.opacity(busy ? 0.3 : 0.9)).foregroundStyle(NG.ink)
                        .neonGlow(NG.magenta, radius: 6)
                }
                .buttonStyle(.plain).disabled(busy || url.isEmpty)
                if !log.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(log.enumerated()), id: \.offset) { Text($0.element.0).foregroundStyle($0.element.1) }
                    }
                    .font(NG.mono(11)).kerning(1.5)
                }
            }
            .padding(18)
        }
    }

    func field(_ label: String, _ value: Binding<String>, _ prompt: String, secure: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(label).font(NG.mono(10)).kerning(2.5).foregroundStyle(NG.cyan)
            Group {
                if secure { SecureField("", text: value, prompt: Text(prompt)) } else { TextField("", text: value, prompt: Text(prompt)) }
            }
            .textFieldStyle(.plain).font(NG.mono(13)).padding(9)
            .background(NG.ink.opacity(0.6)).overlay(Rectangle().stroke(NG.cyan.opacity(0.25)))
        }
    }

    func connect() {
        let trimmed = url.trimmingCharacters(in: .whitespaces)
        guard let u = URL(string: trimmed), u.scheme?.hasPrefix("http") == true || u.isFileURL else {
            log = [("✕ INVALID URL", NG.magenta)]; return
        }
        let k: SourceKind = switch kind {
        case .m3u: .m3u(url: u, epg: URL(string: epg.trimmingCharacters(in: .whitespaces)).flatMap { $0.scheme == nil ? nil : $0 })
        case .xtream: .xtream(server: u, username: user, password: pass)
        case .stalker: .stalker(portal: u, mac: mac.trimmingCharacters(in: .whitespaces))
        }
        let source = Source(name: name.isEmpty ? (u.host() ?? kind.rawValue) : name, kind: k)
        busy = true
        log = [("› HANDSHAKE \(u.host() ?? "")…", NG.dim)]
        Task {
            defer { busy = false }
            do {
                log.append(("› AUTH…", NG.dim))
                let catalog = try await source.client().catalog()
                log.append(("› \(catalog.channels.count) CHANNELS · \(catalog.movies.count) MOVIES · \(catalog.series.count) SERIES", NG.text))
                log.append(("● LINK STABLE", NG.cyan))
                store.add(source, catalog: catalog)
                name = ""; url = ""; epg = ""; user = ""; pass = ""
            } catch {
                log.append(("✕ \(error.localizedDescription.uppercased())", NG.magenta))
            }
        }
    }
}
