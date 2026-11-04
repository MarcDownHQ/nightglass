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
            switch self { case .channel: ("Live", NG.magenta); case .movie: ("Movie", NG.cyan); case .series: ("Series", NG.acid) }
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
            HUDPanel {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass").foregroundStyle(NG.cyan)
                    TextField("", text: $query, prompt: Text("Channels, movies, series").foregroundStyle(NG.dim))
                        .textFieldStyle(.plain).focused($focused)
                }
                .font(NG.title(20, .medium)).padding(18)
                if !hits.isEmpty {
                    Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1)
                    ScrollView {
                        VStack(spacing: 0) {
                            ForEach(Array(hits.enumerated()), id: \.element.id) { i, hit in
                                HStack(spacing: 14) {
                                    Tag(text: hit.kind.0, color: hit.kind.1).frame(width: 70, alignment: .leading)
                                    Text(hit.title).font(NG.title(17, .semibold)).lineLimit(1)
                                    Spacer()
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
            player.play(Playable(id: item.id, title: item.movie.name, subtitle: item.movie.group ?? "", url: item.movie.stream, headers: [:], isLive: false))
            player.expanded = true
        case .series: nav.section = .series
        }
    }
}

struct SourcesView: View {
    @Environment(Store.self) private var store

    var body: some View {
        HStack(alignment: .top, spacing: 32) {
            HUDPanel(caption: "Sources", trailing: "\(store.sources.count)") {
                ScrollView {
                    VStack(spacing: 0) {
                        if store.sources.isEmpty {
                            Text("No source yet.").font(NG.title(15, .regular)).foregroundStyle(NG.dim).padding(24)
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
                Text(source.name).font(NG.title(19, .semibold))
                Group {
                    if store.loading.contains(source.id) { Text("Loading…").foregroundStyle(NG.acid) }
                    else if let e = store.errors[source.id] { Text(e).foregroundStyle(NG.magenta) }
                    else { Text("\(cat?.channels.count ?? 0) channels · \(cat?.movies.count ?? 0) movies · \(cat?.series.count ?? 0) series") }
                }
                .font(NG.title(14, .regular)).foregroundStyle(NG.dim)
            }
            Spacer()
            Button { Task { await store.refresh(source) } } label: { Image(systemName: "arrow.clockwise") }.buttonStyle(.plain).help("Rafraîchir")
            Button { store.remove(source) } label: { Image(systemName: "trash") }.buttonStyle(.plain).foregroundStyle(NG.magenta).help("Supprimer")
        }
        .padding(.horizontal, 18).padding(.vertical, 14)
        .overlay(alignment: .bottom) { Rectangle().fill(NG.cyan.opacity(0.08)).frame(height: 1) }
    }

    var kind: String {
        switch source.kind { case .m3u: "M3U"; case .xtream: "Xtream"; case .stalker: "Stalker" }
    }
}

struct AddSourcePanel: View {
    @Environment(Store.self) private var store
    enum Kind: String, CaseIterable { case m3u = "M3U", xtream = "Xtream", stalker = "Stalker" }
    @State private var kind = Kind.m3u
    @State private var name = ""
    @State private var url = ""
    @State private var epg = ""
    @State private var user = ""
    @State private var pass = ""
    @State private var mac = "00:1A:79:"
    @State private var message: (String, Color)?
    @State private var busy = false

    var body: some View {
        HUDPanel(caption: "Add a source") {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 8) {
                    ForEach(Kind.allCases, id: \.self) { k in
                        NeonTab(title: k.rawValue, selected: k == kind) { kind = k }
                    }
                }
                field("Name", $name, "My provider")
                switch kind {
                case .m3u:
                    field("Playlist URL", $url, "https://… .m3u")
                    field("Guide URL (XMLTV, optional)", $epg, "https://… .xml")
                    Button("Use iptv-org France") {
                        name = "iptv-org · France"; url = "https://iptv-org.github.io/iptv/countries/fr.m3u"; epg = "https://xmltvfr.fr/xmltv/xmltv.xml"
                    }
                    .buttonStyle(.plain).font(NG.title(14, .medium)).foregroundStyle(NG.cyan)
                case .xtream:
                    field("Server", $url, "http://provider:8080")
                    field("Username", $user, "")
                    field("Password", $pass, "", secure: true)
                case .stalker:
                    field("Portal URL", $url, "http://portal/c/")
                    field("MAC address", $mac, "00:1A:79:XX:XX:XX")
                }
                Button(busy ? "Connecting…" : "Connect", action: connect)
                    .buttonStyle(NeonButtonStyle()).disabled(busy || url.isEmpty).opacity(url.isEmpty ? 0.5 : 1)
                if let message { Text(message.0).font(NG.title(14, .medium)).foregroundStyle(message.1).fixedSize(horizontal: false, vertical: true) }
            }
            .padding(18)
        }
    }

    func field(_ label: String, _ value: Binding<String>, _ prompt: String, secure: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(label).font(NG.title(13, .medium)).foregroundStyle(NG.dim)
            Group {
                if secure { SecureField("", text: value, prompt: Text(prompt)) } else { TextField("", text: value, prompt: Text(prompt)) }
            }
            .accessibilityIdentifier(label)
            .textFieldStyle(.plain).font(NG.mono(13)).padding(9)
            .background(NG.ink.opacity(0.6), in: .rect(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(NG.cyan.opacity(0.25)))
        }
    }

    func connect() {
        let trimmed = url.trimmingCharacters(in: .whitespaces)
        guard let u = URL(string: trimmed), u.scheme?.hasPrefix("http") == true || u.isFileURL else {
            message = ("Invalid URL", NG.magenta); return
        }
        let k: SourceKind = switch kind {
        case .m3u: .m3u(url: u, epg: URL(string: epg.trimmingCharacters(in: .whitespaces)).flatMap { $0.scheme == nil ? nil : $0 })
        case .xtream: .xtream(server: u, username: user, password: pass)
        case .stalker: .stalker(portal: u, mac: mac.trimmingCharacters(in: .whitespaces))
        }
        let source = Source(name: name.isEmpty ? (u.host() ?? kind.rawValue) : name, kind: k)
        busy = true
        message = nil
        Task {
            defer { busy = false }
            do {
                let catalog = try await source.client().catalog()
                message = ("Added · \(catalog.channels.count) channels, \(catalog.movies.count) movies, \(catalog.series.count) series", NG.cyan)
                store.add(source, catalog: catalog)
                name = ""; url = ""; epg = ""; user = ""; pass = ""
            } catch {
                message = (error.localizedDescription, NG.magenta)
            }
        }
    }
}
