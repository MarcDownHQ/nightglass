import Foundation

/// Stalker / MAG middleware portal, authenticated by MAC address.
public struct StalkerClient: IPTVSource {
    public let portal: URL
    public let mac: String
    let session: URLSession

    public init(portal: URL, mac: String, session: URLSession = .shared) {
        self.portal = portal; self.mac = mac.uppercased(); self.session = session
    }

    /// Users paste `http://host/c/`, `http://host/stalker_portal/c/` or the API itself; find the API.
    var endpoint: URL {
        if portal.path.hasSuffix(".php") { return portal }
        var c = URLComponents(url: portal, resolvingAgainstBaseURL: false)!
        c.path = portal.path.contains("stalker_portal") ? "/stalker_portal/server/load.php" : "/portal.php"
        c.query = nil
        return c.url!
    }

    func request(_ type: String, _ action: String, _ extra: [URLQueryItem] = [], token: String?) -> URLRequest {
        var c = URLComponents(url: endpoint, resolvingAgainstBaseURL: false)!
        c.queryItems = [.init(name: "type", value: type), .init(name: "action", value: action)] + extra
            + [.init(name: "JsHttpRequest", value: "1-xml")]
        var r = URLRequest(url: c.url!)
        r.setValue("MAG200 stbapp ver: 2 rev: 250 Safari/533.3", forHTTPHeaderField: "User-Agent")
        r.setValue("mac=\(mac); stb_lang=en; timezone=Europe/Paris", forHTTPHeaderField: "Cookie")
        if let token { r.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        return r
    }

    func js(_ type: String, _ action: String, _ extra: [URLQueryItem] = [], token: String?) async throws -> Any? {
        let root = try await HTTP.json(request(type, action, extra, token: token), session: session) as? [String: Any]
        return root?["js"]
    }

    public func handshake() async throws -> String {
        guard let js = try await js("stb", "handshake", token: nil) as? [String: Any], let token = str(js["token"]) else {
            throw IPTVError.authFailed
        }
        _ = try await self.js("stb", "get_profile", token: token)
        return token
    }

    public func catalog() async throws -> Catalog {
        let token = try await handshake()
        var genres: [String: String] = [:]
        for g in (try await js("itv", "get_genres", token: token) as? [[String: Any]]) ?? [] {
            if let id = str(g["id"]) { genres[id] = str(g["title"]) }
        }
        let data = ((try await js("itv", "get_all_channels", token: token) as? [String: Any])?["data"] as? [[String: Any]]) ?? []
        let channels = data.compactMap { c -> Channel? in
            guard let id = str(c["id"]), let name = str(c["name"]), let cmd = str(c["cmd"]),
                  let stream = URL(string: "stalker:" + (cmd.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? cmd))
            else { return nil }
            return Channel(id: "stk-\(id)", name: name, number: int(c["number"]),
                           group: str(c["tv_genre_id"]).flatMap { genres[$0] }, logo: url(c["logo"]),
                           epgID: str(c["xmltv_id"]), stream: stream)
        }
        // shortcut: Stalker VOD and EPG are not fetched; add `vod/get_ordered_list` and `itv/get_short_epg` when a portal needs them.
        return Catalog(channels: channels)
    }

    /// Portal links are short-lived: ask for a fresh one at play time.
    public func streamURL(for channel: Channel) async throws -> URL {
        let cmd = String(channel.stream.absoluteString.dropFirst("stalker:".count)).removingPercentEncoding ?? ""
        let token = try await handshake()
        let js = try await js("itv", "create_link", [.init(name: "cmd", value: cmd)], token: token) as? [String: Any]
        guard let link = str(js?["cmd"]).map(Self.stripPlayer), let url = URL(string: link) else {
            throw IPTVError.badResponse("create_link")
        }
        return url
    }

    /// `cmd` values look like `ffmpeg http://…` or `auto http://…`.
    static func stripPlayer(_ cmd: String) -> String {
        let parts = cmd.split(separator: " ", maxSplits: 1)
        return parts.count == 2 && !parts[0].contains(":") ? String(parts[1]) : cmd
    }
}
