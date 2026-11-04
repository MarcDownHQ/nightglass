import Foundation

/// A live channel, whatever protocol it came from.
public struct Channel: Identifiable, Hashable, Codable, Sendable {
    public var id: String
    public var name: String
    public var number: Int?
    public var group: String?
    public var logo: URL?
    public var epgID: String?
    /// Direct stream for M3U/Xtream. For Stalker this is the portal `cmd`, resolved by `IPTVSource.streamURL(for:)`.
    public var stream: URL
    public var headers: [String: String]

    public init(id: String, name: String, number: Int? = nil, group: String? = nil, logo: URL? = nil,
                epgID: String? = nil, stream: URL, headers: [String: String] = [:]) {
        self.id = id; self.name = name; self.number = number; self.group = group
        self.logo = logo; self.epgID = epgID; self.stream = stream; self.headers = headers
    }
}

public struct Movie: Identifiable, Hashable, Codable, Sendable {
    public var id: String
    public var name: String
    public var group: String?
    public var poster: URL?
    public var stream: URL
    public var rating: Double?
    public var plot: String?
}

public struct Series: Identifiable, Hashable, Codable, Sendable {
    public var id: String
    public var name: String
    public var group: String?
    public var poster: URL?
    public var rating: Double?
    public var plot: String?
}

public struct Episode: Identifiable, Hashable, Codable, Sendable {
    public var id: String
    public var title: String
    public var season: Int
    public var number: Int
    public var stream: URL
    public var plot: String?
}

public struct Programme: Hashable, Codable, Sendable {
    public var channel: String
    public var start: Date
    public var end: Date
    public var title: String
    public var desc: String?

    public init(channel: String, start: Date, end: Date, title: String, desc: String? = nil) {
        self.channel = channel; self.start = start; self.end = end; self.title = title; self.desc = desc
    }
}

public struct Catalog: Sendable {
    public var channels: [Channel] = []
    public var movies: [Movie] = []
    public var series: [Series] = []
    /// XMLTV guide advertised by the source (M3U header, Xtream `xmltv.php`).
    public var epgURL: URL?

    public init(channels: [Channel] = [], movies: [Movie] = [], series: [Series] = [], epgURL: URL? = nil) {
        self.channels = channels; self.movies = movies; self.series = series; self.epgURL = epgURL
    }
}

public enum SourceKind: Hashable, Codable, Sendable {
    case m3u(url: URL, epg: URL?)
    case xtream(server: URL, username: String, password: String)
    case stalker(portal: URL, mac: String)
}

public struct Source: Identifiable, Hashable, Codable, Sendable {
    public var id: UUID
    public var name: String
    public var kind: SourceKind

    public init(id: UUID = UUID(), name: String, kind: SourceKind) {
        self.id = id; self.name = name; self.kind = kind
    }

    public func client(session: URLSession = .shared) -> any IPTVSource {
        switch kind {
        case let .m3u(url, epg): M3USource(url: url, epg: epg, session: session)
        case let .xtream(server, user, pass): XtreamClient(server: server, username: user, password: pass, session: session)
        case let .stalker(portal, mac): StalkerClient(portal: portal, mac: mac, session: session)
        }
    }
}

/// What every protocol client provides to the app.
public protocol IPTVSource: Sendable {
    func catalog() async throws -> Catalog
    func streamURL(for channel: Channel) async throws -> URL
    func episodes(of series: Series) async throws -> [Episode]
}

public extension IPTVSource {
    func streamURL(for channel: Channel) async throws -> URL { channel.stream }
    func episodes(of series: Series) async throws -> [Episode] { [] }
}

public enum IPTVError: Error, LocalizedError, Equatable {
    case http(Int)
    case authFailed
    case badResponse(String)

    public var errorDescription: String? {
        switch self {
        case .http(let code): "Le serveur a répondu \(code)."
        case .authFailed: "Identifiants refusés par le serveur."
        case .badResponse(let what): "Réponse inattendue : \(what)."
        }
    }
}

enum HTTP {
    static func data(_ request: URLRequest, session: URLSession) async throws -> Data {
        let (data, response) = try await session.data(for: request)
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw IPTVError.http(http.statusCode)
        }
        return data
    }

    static func json(_ request: URLRequest, session: URLSession) async throws -> Any {
        let data = try await data(request, session: session)
        do { return try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) }
        catch { throw IPTVError.badResponse(String(decoding: data.prefix(120), as: UTF8.self)) }
    }
}

// Panels send numbers as strings and strings as numbers; read both.
func str(_ v: Any?) -> String? {
    switch v {
    case let s as String: s.isEmpty ? nil : s
    case let n as NSNumber: n.stringValue
    default: nil
    }
}

func int(_ v: Any?) -> Int? { str(v).flatMap { Int($0) ?? Double($0).map { Int($0) } } }
func double(_ v: Any?) -> Double? { str(v).flatMap(Double.init) }
func url(_ v: Any?) -> URL? { str(v).flatMap { URL(string: $0.trimmingCharacters(in: .whitespaces)) }.flatMap { $0.scheme == nil ? nil : $0 } }
