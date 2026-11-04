import Foundation
import NightglassCore
import Security
import SwiftUI

/// A channel and the source it came from (Stalker needs the source to resolve its link).
struct LiveItem: Identifiable, Hashable {
    let source: Source
    let channel: Channel
    var id: String { source.id.uuidString + channel.id }
}

struct MovieItem: Identifiable, Hashable { let source: Source; let movie: Movie; var id: String { source.id.uuidString + movie.id } }
struct SeriesItem: Identifiable, Hashable { let source: Source; let series: Series; var id: String { source.id.uuidString + series.id } }

/// What the player is showing.
struct Playable: Equatable {
    let id: String
    let title: String
    let subtitle: String
    let url: URL
    let headers: [String: String]
    let isLive: Bool
    var live: LiveItem?
}

@MainActor @Observable
final class Store {
    var sources: [Source] = []
    var catalogs: [UUID: Catalog] = [:]
    var loading: Set<UUID> = []
    var errors: [UUID: String] = [:]
    /// Programmes per normalised EPG id, sorted by start.
    var guide: [String: [Programme]] = [:]
    var favorites: Set<String> = Set(UserDefaults.standard.stringArray(forKey: "favorites") ?? [])
    /// Source the screens show; nil means every source.
    var activeSource: UUID? = UserDefaults.standard.string(forKey: "activeSource").flatMap(UUID.init) {
        didSet { UserDefaults.standard.set(activeSource?.uuidString, forKey: "activeSource"); rebuild() }
    }

    var visibleSources: [Source] {
        guard let id = activeSource, sources.contains(where: { $0.id == id }) else { return sources }
        return sources.filter { $0.id == id }
    }

    var channels: [LiveItem] = []
    var movies: [MovieItem] = []
    var series: [SeriesItem] = []

    init() {
        sources = Keychain.loadSources()
        Task { await refreshAll() }
    }

    func refreshAll() async {
        await withTaskGroup(of: Void.self) { group in
            for s in sources { group.addTask { await self.refresh(s) } }
        }
    }

    func refresh(_ source: Source) async {
        loading.insert(source.id); errors[source.id] = nil
        defer { loading.remove(source.id) }
        do {
            let catalog = try await source.client().catalog()
            catalogs[source.id] = catalog
            rebuild()
            if let epg = catalog.epgURL { await loadGuide(epg) }
        } catch {
            errors[source.id] = error.localizedDescription
        }
    }

    func loadGuide(_ url: URL) async {
        let now = Date.now
        // Parse and group off the main thread.
        guard let incoming = try? await Task.detached(operation: { () async throws -> [String: [Programme]] in
            let programmes = try await XMLTVParser.load(url, window: now.addingTimeInterval(-3 * 3600)...now.addingTimeInterval(24 * 3600))
            return Dictionary(grouping: programmes, by: { Store.epgKey($0.channel) }).mapValues { $0.sorted { $0.start < $1.start } }
        }).value else { return }
        // Merge into the current guide, not a snapshot: several guides load at once.
        var merged = guide
        for (key, list) in incoming {
            merged[key] = merged[key].map { ($0 + list).sorted { $0.start < $1.start } } ?? list
        }
        guide = merged
    }

    /// iptv-org ids carry a feed suffix (`6ter.fr@SD`) that guides don't.
    nonisolated static func epgKey(_ id: String) -> String { (id.split(separator: "@").first.map(String.init) ?? id).lowercased() }

    func programmes(for channel: Channel) -> [Programme] {
        channel.epgID.flatMap { guide[Self.epgKey($0)] } ?? []
    }

    func nowNext(for channel: Channel, at date: Date = .now) -> (now: Programme?, next: Programme?) {
        let list = programmes(for: channel)
        let i = list.firstIndex { $0.end > date }
        guard let i else { return (nil, nil) }
        return list[i].start <= date ? (list[i], list[safe: i + 1]) : (nil, list[i])
    }

    private func rebuild() {
        // Playlists without tvg-chno get their position as number, so every channel can be dialled.
        var n = 0
        channels = visibleSources.flatMap { s in
            (catalogs[s.id]?.channels ?? []).map { ch in
                n += 1
                var ch = ch
                if ch.number == nil { ch.number = n }
                return LiveItem(source: s, channel: ch)
            }
        }
        movies = visibleSources.flatMap { s in (catalogs[s.id]?.movies ?? []).map { MovieItem(source: s, movie: $0) } }
        series = visibleSources.flatMap { s in (catalogs[s.id]?.series ?? []).map { SeriesItem(source: s, series: $0) } }
    }

    func add(_ source: Source, catalog: Catalog) {
        sources.append(source)
        catalogs[source.id] = catalog
        Keychain.saveSources(sources)
        rebuild()
        if let epg = catalog.epgURL { Task { await loadGuide(epg) } }
    }

    func remove(_ source: Source) {
        sources.removeAll { $0.id == source.id }
        catalogs[source.id] = nil
        if activeSource == source.id { activeSource = nil }
        Keychain.saveSources(sources)
        rebuild()
    }

    func toggleFavorite(_ item: LiveItem) {
        if favorites.remove(item.id) == nil { favorites.insert(item.id) }
        UserDefaults.standard.set(Array(favorites), forKey: "favorites")
    }

    func playable(_ item: LiveItem) async throws -> Playable {
        let url = try await item.source.client().streamURL(for: item.channel)
        let np = nowNext(for: item.channel)
        return Playable(id: item.id, title: item.channel.name, subtitle: np.now?.title ?? item.channel.group ?? "",
                        url: url, headers: item.channel.headers, isLive: true, live: item)
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}

/// Sources hold provider passwords: the whole list lives in one Keychain item, never in a plain file.
enum Keychain {
    // UI tests pass their own account so they start empty and never touch the user's sources.
    nonisolated(unsafe) static let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                       kSecAttrService as String: "com.marcdownhq.nightglass",
                                       kSecAttrAccount as String: ProcessInfo.processInfo.environment["NG_KEYCHAIN_ACCOUNT"] ?? "sources"]

    static func loadSources() -> [Source] {
        var q = query; q[kSecReturnData as String] = true
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return [] }
        return (try? JSONDecoder().decode([Source].self, from: data)) ?? []
    }

    static func saveSources(_ sources: [Source]) {
        guard let data = try? JSONEncoder().encode(sources) else { return }
        let status = SecItemUpdate(query as CFDictionary, [kSecValueData as String: data] as CFDictionary)
        if status == errSecItemNotFound {
            var q = query; q[kSecValueData as String] = data
            SecItemAdd(q as CFDictionary, nil)
        }
    }
}
