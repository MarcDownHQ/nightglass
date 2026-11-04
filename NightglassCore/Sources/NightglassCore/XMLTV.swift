import Foundation
import Compression

/// Streaming XMLTV parser. Guides run to hundreds of MB, so only programmes inside `window` are kept.
public final class XMLTVParser: NSObject, XMLParserDelegate {
    private let window: ClosedRange<Date>
    private let wanted: Set<String>?
    private var programmes: [Programme] = []
    private var current: (channel: String, start: Date, end: Date)?
    private var title = "", desc = "", field: String?

    public static func parse(_ data: Data, window: ClosedRange<Date>, channels: Set<String>? = nil) -> [Programme] {
        let me = XMLTVParser(window: window, channels: channels)
        let parser = XMLParser(data: gunzipIfNeeded(data))
        parser.delegate = me
        parser.parse()
        return me.programmes
    }

    public static func load(_ url: URL, window: ClosedRange<Date>, channels: Set<String>? = nil,
                            session: URLSession = .shared) async throws -> [Programme] {
        let data = url.isFileURL ? try Data(contentsOf: url) : try await HTTP.data(URLRequest(url: url), session: session)
        return parse(data, window: window, channels: channels)
    }

    init(window: ClosedRange<Date>, channels: Set<String>?) {
        self.window = window; self.wanted = channels
    }

    public func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?,
                       qualifiedName: String?, attributes a: [String: String]) {
        switch name {
        case "programme":
            guard let ch = a["channel"], wanted?.contains(ch) ?? true,
                  let start = a["start"].flatMap(Self.date), let end = a["stop"].flatMap(Self.date),
                  end >= window.lowerBound, start <= window.upperBound else { current = nil; return }
            current = (ch, start, end); title = ""; desc = ""
        case "title", "desc": field = current == nil ? nil : name
        default: break
        }
    }

    public func parser(_ parser: XMLParser, foundCharacters string: String) {
        if field == "title" { title += string } else if field == "desc" { desc += string }
    }

    public func parser(_ parser: XMLParser, didEndElement name: String, namespaceURI: String?, qualifiedName: String?) {
        if name == "title" || name == "desc" { field = nil }
        if name == "programme", let c = current {
            programmes.append(Programme(channel: c.channel, start: c.start, end: c.end,
                                        title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                                        desc: desc.isEmpty ? nil : desc.trimmingCharacters(in: .whitespacesAndNewlines)))
            current = nil
        }
    }

    /// `20261104200000 +0100`, offset optional (then UTC).
    static func date(_ s: String) -> Date? {
        let parts = s.split(separator: " ")
        let digits = parts.first.map { String($0.prefix(14)) } ?? ""
        guard digits.count == 14, let n = Int(digits) else { return nil }
        var c = DateComponents()
        c.year = n / 10_000_000_000; c.month = n / 100_000_000 % 100; c.day = n / 1_000_000 % 100
        c.hour = n / 10_000 % 100; c.minute = n / 100 % 100; c.second = n % 100
        var offset = 0
        if parts.count > 1, let o = Int(parts[1].replacingOccurrences(of: "+", with: "")) {
            offset = (o / 100 * 3600) + (o % 100 * 60)
        }
        c.timeZone = TimeZone(secondsFromGMT: offset)
        return Calendar(identifier: .gregorian).date(from: c)
    }

    /// `.xml.gz` guides: skip the gzip header and inflate the raw deflate body.
    static func gunzipIfNeeded(_ data: Data) -> Data {
        guard data.count > 18, data[data.startIndex] == 0x1f, data[data.startIndex + 1] == 0x8b else { return data }
        let bytes = [UInt8](data)
        let flags = bytes[3]
        var i = 10
        if flags & 4 != 0 { i += 2 + Int(bytes[i]) + Int(bytes[i + 1]) << 8 }
        if flags & 8 != 0 { while i < bytes.count, bytes[i] != 0 { i += 1 }; i += 1 }
        if flags & 16 != 0 { while i < bytes.count, bytes[i] != 0 { i += 1 }; i += 1 }
        if flags & 2 != 0 { i += 2 }
        guard i < bytes.count - 8 else { return data }
        let size = Int(bytes[bytes.count - 4]) | Int(bytes[bytes.count - 3]) << 8 | Int(bytes[bytes.count - 2]) << 16 | Int(bytes[bytes.count - 1]) << 24
        // ISIZE is mod 2^32; a guide over 4 GB is not a real case.
        var out = [UInt8](repeating: 0, count: max(size, 1))
        let n = bytes[i..<(bytes.count - 8)].withUnsafeBufferPointer { src in
            compression_decode_buffer(&out, out.count, src.baseAddress!, src.count, nil, COMPRESSION_ZLIB)
        }
        return n > 0 ? Data(out.prefix(n)) : data
    }
}

public extension Array where Element == Programme {
    /// Now and next per channel ID.
    func nowNext(at date: Date = .now) -> [String: (now: Programme?, next: Programme?)] {
        var out: [String: (now: Programme?, next: Programme?)] = [:]
        for p in self.sorted(by: { $0.start < $1.start }) {
            var e = out[p.channel] ?? (nil, nil)
            if p.start <= date, p.end > date { e.now = p }
            else if p.start > date, e.next == nil { e.next = p }
            out[p.channel] = e
        }
        return out
    }
}
