import NightglassCore
import SwiftUI

struct MoviesView: View {
    @Environment(Store.self) private var store
    @Environment(Player.self) private var player

    var body: some View {
        PosterBrowser(items: store.movies, group: \.movie.group, empty: "No movies. M3U playlists don't carry any; add an Xtream account.") { item in
            Poster(title: item.movie.name, url: item.movie.poster, rating: item.movie.rating)
        } action: { item in
            player.play(Playable(id: item.id, title: item.movie.name, subtitle: item.movie.group ?? "",
                                 url: item.movie.stream, headers: [:], isLive: false))
            player.expanded = true
        }
    }
}

struct SeriesView: View {
    @Environment(Store.self) private var store
    @State private var open: SeriesItem?

    var body: some View {
        ZStack {
            PosterBrowser(items: store.series, group: \.series.group, empty: "No series. Add an Xtream account that has some.") { item in
                Poster(title: item.series.name, url: item.series.poster, rating: item.series.rating)
            } action: { open = $0 }
            if let open { SeriesDetail(item: open) { self.open = nil }.transition(.opacity) }
        }
        .animation(.spring(duration: 0.35, bounce: 0.15), value: open)
    }
}

struct PosterBrowser<Item: Identifiable & Hashable, Card: View>: View {
    let items: [Item]
    let group: KeyPath<Item, String?>
    let empty: String
    @ViewBuilder let card: (Item) -> Card
    let action: (Item) -> Void
    @State private var category = "All"

    var body: some View {
        var seen = Set<String>(), cats = ["All"]
        for i in items { if let g = i[keyPath: group], seen.insert(g).inserted { cats.append(g) } }
        let shown = category == "All" ? items : items.filter { $0[keyPath: group] == category }
        return VStack(spacing: 10) {
            if items.isEmpty {
                Text(empty).font(NG.title(16, .regular)).foregroundStyle(NG.dim).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                TabRow(items: cats, selection: $category)
                ScrollView {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 200), spacing: 22)], spacing: 26) {
                        ForEach(shown) { item in Button { action(item) } label: { card(item) }.buttonStyle(.plain) }
                    }
                    .padding(.horizontal, 6).padding(.vertical, 12)
                }
                .scrollIndicators(.hidden)
            }
        }
        .padding(.horizontal, 28).padding(.bottom, 16)
    }
}

/// Poster card; lights up in its neon on hover.
struct Poster: View {
    let title: String
    let url: URL?
    let rating: Double?
    @State private var hover = false

    var body: some View {
        let c = NG.neon(for: title)
        VStack(alignment: .leading, spacing: 8) {
            // The 2:3 box sets the size; the art fills it and is clipped, so a wide image can't push the grid.
            LinearGradient(colors: [NG.night, NG.ink], startPoint: .top, endPoint: .bottom)
            .aspectRatio(2 / 3, contentMode: .fit)
            .overlay {
                AsyncImage(url: url) { phase in
                    if let img = phase.image { img.resizable().scaledToFill() }
                    else { Text(title).font(NG.title(20, .bold)).multilineTextAlignment(.center).padding() }
                }
            }
            .clipShape(.rect(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(hover ? c : NG.text.opacity(0.12), lineWidth: hover ? 2 : 1))
            .shadow(color: hover ? c.opacity(0.55) : .clear, radius: 18)
            .scaleEffect(hover ? 1.03 : 1)
            HStack(alignment: .top) {
                Text(title).font(NG.title(15, .semibold)).lineLimit(2)
                Spacer(minLength: 4)
                if let rating, rating > 0 { Tag(text: String(format: "%.1f", rating)) }
            }
        }
        .onHover { hover = $0 }
        .animation(.spring(duration: 0.35, bounce: 0.2), value: hover)
    }
}

struct SeriesDetail: View {
    @Environment(Player.self) private var player
    let item: SeriesItem
    let close: () -> Void
    @State private var episodes: [Episode] = []
    @State private var season = 1
    @State private var error: String?

    var body: some View {
        let seasons = Array(Set(episodes.map(\.season))).sorted()
        ZStack(alignment: .topLeading) {
            AsyncImage(url: item.series.poster) { $0.resizable().scaledToFill() } placeholder: { NG.night }
                .blur(radius: 40).opacity(0.45).ignoresSafeArea().clipped()
            NG.ink.opacity(0.55)
            HStack(alignment: .top, spacing: 40) {
                AsyncImage(url: item.series.poster) { $0.resizable().scaledToFit() } placeholder: { NG.night }
                    .frame(width: 260).clipShape(.rect(cornerRadius: 8)).shadow(color: NG.magenta.opacity(0.3), radius: 30)
                VStack(alignment: .leading, spacing: 16) {
                    Button(action: close) { Label("Back", systemImage: "chevron.left").font(NG.title(16, .medium)) }.buttonStyle(.plain).foregroundStyle(NG.cyan)
                    NeonText(text: item.series.name, size: 48).lineLimit(2)
                    if let plot = item.series.plot { Text(plot).font(NG.title(16, .regular)).foregroundStyle(NG.dim).lineSpacing(4).frame(maxWidth: 700, alignment: .leading) }
                    HStack(spacing: 8) {
                        ForEach(seasons, id: \.self) { s in
                            Button { season = s } label: {
                                Text("Season \(s)").font(NG.title(16, .semibold)).padding(.horizontal, 12).padding(.vertical, 4)
                                    .foregroundStyle(s == season ? .white : NG.dim)
                                    .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(s == season ? NG.magenta : NG.text.opacity(0.2), lineWidth: 1.5))
                                    .shadow(color: s == season ? NG.magenta : .clear, radius: 6)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    HUDPanel(caption: "Episodes", trailing: "\(episodes.filter { $0.season == season }.count)") {
                        ScrollView {
                            LazyVStack(alignment: .leading, spacing: 0) {
                                if let error { Text(error).font(NG.mono(12)).foregroundStyle(NG.magenta).padding() }
                                ForEach(episodes.filter { $0.season == season }) { e in
                                    Button {
                                        player.play(Playable(id: e.id, title: e.title, subtitle: "\(item.series.name) · S\(e.season)E\(e.number)",
                                                             url: e.stream, headers: [:], isLive: false))
                                        player.expanded = true
                                    } label: {
                                        HStack(spacing: 14) {
                                            Text(String(e.number)).font(NG.mono(12)).foregroundStyle(NG.dim).frame(width: 24, alignment: .leading)
                                            VStack(alignment: .leading, spacing: 2) {
                                                Text(e.title).font(NG.title(17, .semibold))
                                                if let plot = e.plot { Text(plot).font(NG.title(14, .regular)).foregroundStyle(NG.dim).lineLimit(1) }
                                            }
                                            Spacer()
                                            Image(systemName: "play.fill").foregroundStyle(NG.magenta)
                                        }
                                        .padding(.horizontal, 16).padding(.vertical, 10).contentShape(.rect)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        .frame(maxHeight: 360)
                    }
                }
            }
            .padding(40)
        }
        .task {
            do {
                episodes = try await item.source.client().episodes(of: item.series)
                season = episodes.map(\.season).min() ?? 1
            } catch { self.error = error.localizedDescription }
        }
        .onKeyPress(.escape) { close(); return .handled }
    }
}
