import Foundation

/// Serves canned responses keyed by a predicate on the request.
final class MockServer: URLProtocol {
    typealias Route = (match: (URLRequest) -> Bool, body: String, status: Int)
    // Keyed by host so suites running in parallel don't overwrite each other.
    nonisolated(unsafe) static var routes: [String: [Route]] = [:]
    nonisolated(unsafe) static var seen: [URLRequest] = []
    static let lock = NSLock()

    static func session(host: String, _ routes: [Route]) -> URLSession {
        lock.withLock { self.routes[host] = routes }
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockServer.self]
        return URLSession(configuration: config)
    }

    static func query(_ r: URLRequest, _ name: String) -> String? {
        URLComponents(url: r.url!, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == name }?.value
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}

    override func startLoading() {
        let route = Self.lock.withLock {
            Self.seen.append(request)
            return Self.routes[request.url!.host() ?? ""]?.first { $0.match(request) }
        }
        let response = HTTPURLResponse(url: request.url!, statusCode: route?.status ?? 404, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data((route?.body ?? "").utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
}
