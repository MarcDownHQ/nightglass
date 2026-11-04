import Foundation
import Testing
@testable import NightglassCore

/// Real servers, opt-in: `NG_XTREAM=http://127.0.0.1:9191 NG_XTREAM_USER=… NG_XTREAM_PASS=… swift test`.
@Suite struct IntegrationTests {
    static let env = ProcessInfo.processInfo.environment

    @Test(.enabled(if: env["NG_XTREAM"] != nil)) func xtreamDispatcharr() async throws {
        let c = XtreamClient(server: URL(string: Self.env["NG_XTREAM"]!)!, username: Self.env["NG_XTREAM_USER"] ?? "",
                             password: Self.env["NG_XTREAM_PASS"] ?? "")
        let cat = try await c.catalog()
        #expect(cat.channels.count > 10)
        let now = Date.now
        let guide = try await XMLTVParser.load(try #require(cat.epgURL), window: now...now.addingTimeInterval(86400))
        #expect(!guide.isEmpty)
        #expect(guide.nowNext(at: now).values.contains { $0.now != nil })
    }

    @Test(.enabled(if: env["NG_M3U"] != nil)) func iptvOrgPlaylist() async throws {
        let cat = try await M3USource(url: URL(string: Self.env["NG_M3U"]!)!).catalog()
        print("[NG] \(Self.env["NG_M3U"]!): \(cat.channels.count) channels")
        #expect(cat.channels.count > 50)
        #expect(cat.channels.allSatisfy { $0.stream.scheme != nil })
    }
}
