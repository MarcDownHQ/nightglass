import Foundation

/// Xtream Codes `player_api.php`.
public struct XtreamClient: IPTVSource {
    public let server: URL
    public let username: String
    public let password: String
    let session: URLSession

    public init(server: URL, username: String, password: String, session: URLSession = .shared) {
        self.server = server; self.username = username; self.password = password; self.session = session
    }

    func api(_ action: String? = nil, _ extra: [URLQueryItem] = []) -> URL {
        var c = URLComponents(url: server.appending(path: "player_api.php"), resolvingAgainstBaseURL: false)!
        c.queryItems = [.init(name: "username", value: username), .init(name: "password", value: password)]
            + (action.map { [.init(name: "action", value: $0)] } ?? []) + extra
        return c.url!
    }

    func get(_ action: String? = nil, _ extra: [URLQueryItem] = []) async throws -> Any {
        try await HTTP.json(URLRequest(url: api(action, extra)), session: session)
    }

    func list(_ action: String) async throws -> [[String: Any]] {
        // Empty sections come back as `[]`, `{}` or `null` depending on the panel.
        (try await get(action) as? [[String: Any]]) ?? []
    }

    func categories(_ action: String) async throws -> [String: String] {
        var out: [String: String] = [:]
        for c in try await list(action) { if let id = str(c["category_id"]) { out[id] = str(c["category_name"]) } }
        return out
    }

    func media(_ kind: String, _ id: String, _ ext: String) -> URL {
        server.appending(path: "\(kind)/\(username)/\(password)/\(id).\(ext)")
    }

    public func authenticate() async throws {
        let root = try await get() as? [String: Any]
        guard let user = root?["user_info"] as? [String: Any], int(user["auth"]) == 1 else { throw IPTVError.authFailed }
    }

    public func catalog() async throws -> Catalog {
        try await authenticate()
        async let liveCats = categories("get_live_categories")
        async let vodCats = categories("get_vod_categories")
        async let seriesCats = categories("get_series_categories")
        async let live = list("get_live_streams")
        async let vod = list("get_vod_streams")
        async let series = list("get_series")

        let lc = try await liveCats, vc = try await vodCats, sc = try await seriesCats
        let channels = try await live.compactMap { s -> Channel? in
            guard let id = str(s["stream_id"]), let name = str(s["name"]) else { return nil }
            return Channel(id: "xt-live-\(id)", name: name, number: int(s["num"]),
                           group: str(s["category_id"]).flatMap { lc[$0] }, logo: url(s["stream_icon"]),
                           epgID: str(s["epg_channel_id"]), stream: media("live", id, "ts"))
        }
        let movies = try await vod.compactMap { s -> Movie? in
            guard let id = str(s["stream_id"]), let name = str(s["name"]) else { return nil }
            return Movie(id: "xt-vod-\(id)", name: name, group: str(s["category_id"]).flatMap { vc[$0] },
                         poster: url(s["stream_icon"]), stream: media("movie", id, str(s["container_extension"]) ?? "mp4"),
                         rating: double(s["rating"]), plot: str(s["plot"]))
        }
        let shows = try await series.compactMap { s -> Series? in
            guard let id = str(s["series_id"]), let name = str(s["name"]) else { return nil }
            return Series(id: id, name: name, group: str(s["category_id"]).flatMap { sc[$0] },
                          poster: url(s["cover"]), rating: double(s["rating"]), plot: str(s["plot"]))
        }
        var epg = URLComponents(url: server.appending(path: "xmltv.php"), resolvingAgainstBaseURL: false)!
        epg.queryItems = [.init(name: "username", value: username), .init(name: "password", value: password)]
        return Catalog(channels: channels, movies: movies, series: shows, epgURL: epg.url)
    }

    public func episodes(of series: Series) async throws -> [Episode] {
        let info = try await get("get_series_info", [.init(name: "series_id", value: series.id)]) as? [String: Any]
        // `episodes` is a dictionary keyed by season number on most panels, a list of lists on a few.
        let seasons: [[[String: Any]]] = switch info?["episodes"] {
        case let d as [String: [[String: Any]]]: d.sorted { (Int($0.key) ?? 0) < (Int($1.key) ?? 0) }.map(\.value)
        case let l as [[[String: Any]]]: l
        default: []
        }
        return seasons.flatMap { $0 }.compactMap { e in
            guard let id = str(e["id"]) else { return nil }
            let details = e["info"] as? [String: Any]
            return Episode(id: id, title: str(e["title"]) ?? "Episode \(int(e["episode_num"]) ?? 0)",
                           season: int(e["season"]) ?? 1, number: int(e["episode_num"]) ?? 0,
                           stream: media("series", id, str(e["container_extension"]) ?? "mp4"),
                           plot: str(details?["plot"]))
        }
    }
}
