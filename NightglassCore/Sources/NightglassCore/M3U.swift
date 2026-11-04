import Foundation

public enum M3UParser {
    public struct Result: Sendable {
        public var channels: [Channel]
        public var epgURL: URL?
    }

    public static func parse(_ text: String) -> Result {
        var channels: [Channel] = []
        var epg: URL?
        var info: (attrs: [String: String], title: String)?
        var headers: [String: String] = [:]
        var extgrp: String?

        for raw in text.split(whereSeparator: \.isNewline) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty { continue }
            if line.hasPrefix("#EXTM3U") {
                let a = attributes(line)
                epg = (a["url-tvg"] ?? a["x-tvg-url"])?.split(separator: ",").first.flatMap { URL(string: String($0)) }
            } else if line.hasPrefix("#EXTINF") {
                info = (attributes(line), title(line))
                headers = [:]; extgrp = nil
            } else if line.hasPrefix("#EXTVLCOPT:") {
                let opt = line.dropFirst("#EXTVLCOPT:".count)
                guard let eq = opt.firstIndex(of: "=") else { continue }
                let key = opt[..<eq], value = String(opt[opt.index(after: eq)...])
                if key == "http-user-agent" { headers["User-Agent"] = value }
                if key == "http-referrer" || key == "http-referer" { headers["Referer"] = value }
            } else if line.hasPrefix("#EXTGRP:") {
                extgrp = String(line.dropFirst("#EXTGRP:".count))
            } else if !line.hasPrefix("#"), let current = info, let stream = URL(string: line), stream.scheme != nil {
                let a = current.attrs
                let group = (a["group-title"] ?? extgrp)?.split(separator: ";").first.map(String.init)
                channels.append(Channel(
                    id: stream.absoluteString,
                    name: current.title.isEmpty ? (a["tvg-name"] ?? stream.lastPathComponent) : current.title,
                    number: a["tvg-chno"].flatMap { Int($0) },
                    group: group?.isEmpty == false ? group : nil,
                    logo: a["tvg-logo"].flatMap { URL(string: $0) },
                    epgID: a["tvg-id"].flatMap { $0.isEmpty ? nil : $0 },
                    stream: stream,
                    headers: headers))
                info = nil
            }
        }
        return Result(channels: channels, epgURL: epg)
    }

    /// `key="value"` pairs of an `#EXTINF` / `#EXTM3U` line.
    static func attributes(_ line: String) -> [String: String] {
        var out: [String: String] = [:]
        for m in line.matches(of: /([A-Za-z0-9_-]+)="([^"]*)"/) { out[String(m.1).lowercased()] = String(m.2) }
        return out
    }

    /// Display name: everything after the first comma that sits outside quotes.
    static func title(_ line: String) -> String {
        var inQuotes = false
        for i in line.indices {
            if line[i] == "\"" { inQuotes.toggle() }
            else if line[i] == ",", !inQuotes { return line[line.index(after: i)...].trimmingCharacters(in: .whitespaces) }
        }
        return ""
    }
}

public struct M3USource: IPTVSource {
    public let url: URL
    public let epg: URL?
    let session: URLSession

    public init(url: URL, epg: URL? = nil, session: URLSession = .shared) {
        self.url = url; self.epg = epg; self.session = session
    }

    public func catalog() async throws -> Catalog {
        let data = url.isFileURL ? try Data(contentsOf: url) : try await HTTP.data(URLRequest(url: url), session: session)
        let result = M3UParser.parse(String(decoding: data, as: UTF8.self))
        // shortcut: every M3U entry is treated as live TV; VOD inside M3U playlists is not split out.
        return Catalog(channels: result.channels, epgURL: epg ?? result.epgURL)
    }
}
