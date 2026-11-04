import Foundation
import Testing
@testable import NightglassCore

@Suite struct M3UTests {
    @Test func parsesIptvOrgStyle() {
        let text = """
        #EXTM3U x-tvg-url="https://epg.example/guide.xml.gz,https://other.example/g.xml"
        #EXTINF:-1 tvg-id="6ter.fr@SD" tvg-logo="https://i.imgur.com/2nd5Cox.png" group-title="Entertainment",6ter
        http://149.62.177.107:9999/6ter/index.m3u8
        #EXTINF:-1 tvg-id="Africa24Sport.fr@SD" group-title="News;Sports",Africa 24, Sport (1080p)
        #EXTVLCOPT:http-user-agent=Mozilla/5.0 (X11)
        #EXTVLCOPT:http-referrer=https://ref.example/
        https://example.com/a24.m3u8\r
        #EXTINF:-1,No attributes
        #EXTGRP:Misc
        https://example.com/plain.ts
        #EXTINF:-1 tvg-name="Orphan",
        not a url
        """
        let r = M3UParser.parse(text)
        #expect(r.epgURL == URL(string: "https://epg.example/guide.xml.gz"))
        #expect(r.channels.count == 3)
        #expect(r.channels[0].name == "6ter")
        #expect(r.channels[0].epgID == "6ter.fr@SD")
        #expect(r.channels[0].group == "Entertainment")
        #expect(r.channels[0].logo == URL(string: "https://i.imgur.com/2nd5Cox.png"))
        #expect(r.channels[1].name == "Africa 24, Sport (1080p)")
        #expect(r.channels[1].group == "News")
        #expect(r.channels[1].headers == ["User-Agent": "Mozilla/5.0 (X11)", "Referer": "https://ref.example/"])
        #expect(r.channels[2].group == "Misc")
        #expect(r.channels[2].headers.isEmpty)
    }

    @Test func titleIgnoresCommasInQuotes() {
        #expect(M3UParser.title(#"#EXTINF:-1 group-title="A, B",Name"#) == "Name")
    }
}

@Suite(.serialized) struct XtreamTests {
    static let server = URL(string: "http://panel.test:8080")!

    func session(auth: Int = 1) -> URLSession {
        func action(_ a: String?) -> (URLRequest) -> Bool { { MockServer.query($0, "action") == a } }
        return MockServer.session(host: "panel.test", [
            (action(nil), #"{"user_info":{"auth":\#(auth),"status":"Active"},"server_info":{}}"#, 200),
            (action("get_live_categories"), #"[{"category_id":"1","category_name":"News"}]"#, 200),
            (action("get_live_streams"), #"[{"num":1,"name":"Wire 24","stream_id":42,"stream_icon":"http://logo.test/w.png","epg_channel_id":"wire.fr","category_id":"1"},{"name":"broken"}]"#, 200),
            (action("get_vod_categories"), "[]", 200),
            (action("get_vod_streams"), #"[{"stream_id":"7","name":"Blade","category_id":null,"rating":"7.5","container_extension":"mkv","stream_icon":""}]"#, 200),
            (action("get_series_categories"), "{}", 200),
            (action("get_series"), "null", 200),
            (action("get_series_info"), #"{"episodes":{"2":[{"id":"902","episode_num":1,"title":"S2E1","season":2,"container_extension":"mp4"}],"1":[{"id":"901","episode_num":"1","title":"S1E1","season":1,"container_extension":"mkv","info":{"plot":"Pilot"}}]}}"#, 200),
        ])
    }

    @Test func catalog() async throws {
        let c = XtreamClient(server: Self.server, username: "u", password: "p", session: session())
        let cat = try await c.catalog()
        #expect(cat.channels.count == 1)
        let ch = try #require(cat.channels.first)
        #expect(ch.name == "Wire 24" && ch.group == "News" && ch.epgID == "wire.fr" && ch.number == 1)
        #expect(ch.stream.absoluteString == "http://panel.test:8080/live/u/p/42.ts")
        #expect(cat.movies.first?.stream.absoluteString == "http://panel.test:8080/movie/u/p/7.mkv")
        #expect(cat.movies.first?.rating == 7.5)
        #expect(cat.movies.first?.poster == nil)
        #expect(cat.series.isEmpty)
        #expect(cat.epgURL?.absoluteString == "http://panel.test:8080/xmltv.php?username=u&password=p")
    }

    @Test func rejectsBadCredentials() async {
        let c = XtreamClient(server: Self.server, username: "u", password: "bad", session: session(auth: 0))
        await #expect(throws: IPTVError.authFailed) { try await c.catalog() }
    }

    @Test func episodesSortedBySeason() async throws {
        let c = XtreamClient(server: Self.server, username: "u", password: "p", session: session())
        let eps = try await c.episodes(of: Series(id: "5", name: "Show"))
        #expect(eps.map(\.title) == ["S1E1", "S2E1"])
        #expect(eps[0].stream.absoluteString == "http://panel.test:8080/series/u/p/901.mkv")
        #expect(eps[0].plot == "Pilot")
    }
}

@Suite(.serialized) struct StalkerTests {
    func session() -> URLSession {
        func action(_ a: String) -> (URLRequest) -> Bool { { MockServer.query($0, "action") == a } }
        return MockServer.session(host: "p.test", [
            (action("handshake"), #"{"js":{"token":"TOK"}}"#, 200),
            (action("get_profile"), #"{"js":{"id":1}}"#, 200),
            (action("get_genres"), #"{"js":[{"id":"*","title":"All"},{"id":"3","title":"Sport"}]}"#, 200),
            (action("get_all_channels"), #"{"js":{"data":[{"id":"10","name":"Orbit Sport","number":"3","cmd":"ffmpeg http://localhost/ch/10_","tv_genre_id":"3","logo":""}]}}"#, 200),
            (action("create_link"), #"{"js":{"cmd":"ffmpeg http://cdn.test/live/10.ts?token=abc"}}"#, 200),
        ])
    }

    @Test func endpointDiscovery() {
        #expect(StalkerClient(portal: URL(string: "http://p.test:8080/c/")!, mac: "x").endpoint.absoluteString == "http://p.test:8080/portal.php")
        #expect(StalkerClient(portal: URL(string: "http://p.test/stalker_portal/c/")!, mac: "x").endpoint.absoluteString == "http://p.test/stalker_portal/server/load.php")
        #expect(StalkerClient(portal: URL(string: "http://p.test/server/load.php")!, mac: "x").endpoint.absoluteString == "http://p.test/server/load.php")
    }

    @Test func catalogAndLink() async throws {
        let c = StalkerClient(portal: URL(string: "http://p.test/c/")!, mac: "00:1a:79:00:00:01", session: session())
        let cat = try await c.catalog()
        let ch = try #require(cat.channels.first)
        #expect(ch.name == "Orbit Sport" && ch.group == "Sport" && ch.number == 3)
        let link = try await c.streamURL(for: ch)
        #expect(link.absoluteString == "http://cdn.test/live/10.ts?token=abc")
        let last = try #require(MockServer.lock.withLock { MockServer.seen.last { $0.url?.host() == "p.test" } })
        #expect(MockServer.query(last, "cmd") == "ffmpeg http://localhost/ch/10_")
        #expect(last.value(forHTTPHeaderField: "Authorization") == "Bearer TOK")
        #expect(last.value(forHTTPHeaderField: "Cookie")?.contains("mac=00:1A:79:00:00:01") == true)
    }

    @Test func stripPlayer() {
        #expect(StalkerClient.stripPlayer("auto http://a/b") == "http://a/b")
        #expect(StalkerClient.stripPlayer("http://a/b") == "http://a/b")
    }
}

@Suite struct XMLTVTests {
    static let xml = """
    <?xml version="1.0" encoding="UTF-8"?>
    <tv>
      <channel id="wire.fr"><display-name>Wire</display-name></channel>
      <programme start="20261104190000 +0100" stop="20261104200000 +0100" channel="wire.fr"><title lang="fr">Le 19/20</title><desc>Infos &amp; météo</desc></programme>
      <programme start="20261104200000 +0100" stop="20261104210000 +0100" channel="wire.fr"><title>Journal</title></programme>
      <programme start="20261110200000 +0100" stop="20261110210000 +0100" channel="wire.fr"><title>Next week</title></programme>
      <programme start="20261104200000 +0100" stop="20261104210000 +0100" channel="other.fr"><title>Other</title></programme>
    </tv>
    """
    static let at = XMLTVParser.date("20261104193000 +0100")!

    @Test func parsesWindowAndChannels() {
        let p = XMLTVParser.parse(Data(Self.xml.utf8), window: Self.at...Self.at.addingTimeInterval(86400), channels: ["wire.fr"])
        #expect(p.map(\.title) == ["Le 19/20", "Journal"])
        #expect(p[0].desc == "Infos & météo")
        #expect(p[0].start == Date(timeIntervalSince1970: 1_793_815_200)) // 2026-11-04 18:00 UTC
        let nn = p.nowNext(at: Self.at)["wire.fr"]
        #expect(nn?.now?.title == "Le 19/20" && nn?.next?.title == "Journal")
    }

    @Test func gzip() throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let file = dir.appending(path: "g.xml")
        try Data(Self.xml.utf8).write(to: file)
        let p = Process()
        p.executableURL = URL(filePath: "/usr/bin/gzip")
        p.arguments = ["-k", file.path]
        try p.run(); p.waitUntilExit()
        let gz = try Data(contentsOf: dir.appending(path: "g.xml.gz"))
        #expect(XMLTVParser.gunzipIfNeeded(gz) == Data(Self.xml.utf8))
        #expect(XMLTVParser.parse(gz, window: Self.at...Self.at.addingTimeInterval(86400)).count == 3)
    }

    @Test func dates() {
        #expect(XMLTVParser.date("20261104180000") == Date(timeIntervalSince1970: 1_793_815_200))
        #expect(XMLTVParser.date("20261104190000 +0100") == Date(timeIntervalSince1970: 1_793_815_200))
        #expect(XMLTVParser.date("garbage") == nil)
    }
}
