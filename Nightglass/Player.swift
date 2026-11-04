import AetherEngine
import Combine
import SwiftUI

@MainActor @Observable
final class Player {
    let engine: AetherEngine
    var current: Playable?
    var state: PlaybackState = .idle
    var firstFrame = false
    var expanded = false
    var error: String?
    var time: Double = 0
    var duration: Double = 0
    var audioTracks: [TrackInfo] = []
    var subtitleTracks: [TrackInfo] = []
    var videoSize = CGSize.zero
    /// Confirmed by the engine that a picture reached the screen (not just decoded).
    var rendered = false

    private var bag = Set<AnyCancellable>()
    private var retunes = 0

    init() {
        do { engine = try AetherEngine() } catch { fatalError("AetherEngine init failed: \(error)") }
        engine.$state.receive(on: DispatchQueue.main).sink { [weak self] s in
            self?.state = s
            if case .error(let message) = s { self?.error = message }
        }.store(in: &bag)
        engine.$hasFirstFrameReadyForDisplay.receive(on: DispatchQueue.main).sink { [weak self] ready in
            guard let self else { return }
            firstFrame = ready
            rendered = false
            if ready {
                Task {
                    for _ in 0..<20 where !self.rendered && self.firstFrame {
                        self.rendered = await self.engine.hasRenderedVideoFrame()
                        if !self.rendered { try? await Task.sleep(for: .milliseconds(500)) }
                    }
                }
            }
        }.store(in: &bag)
        engine.clock.$currentTime.receive(on: DispatchQueue.main).sink { [weak self] in self?.time = $0 }.store(in: &bag)
        engine.$duration.receive(on: DispatchQueue.main).sink { [weak self] in self?.duration = $0 }.store(in: &bag)
        engine.$audioTracks.receive(on: DispatchQueue.main).sink { [weak self] in self?.audioTracks = $0 }.store(in: &bag)
        engine.$subtitleTracks.receive(on: DispatchQueue.main).sink { [weak self] in self?.subtitleTracks = $0 }.store(in: &bag)
        engine.$sourceVideoWidth.combineLatest(engine.$sourceVideoHeight).receive(on: DispatchQueue.main)
            .sink { [weak self] w, h in self?.videoSize = CGSize(width: Int(w), height: Int(h)) }.store(in: &bag)
        // The engine gave up on a live session and wants a fresh load; reload the same URL, a bounded number of times.
        engine.liveSourceReset.receive(on: DispatchQueue.main).sink { [weak self] in
            guard let self, let p = current, retunes < 3 else { return }
            retunes += 1
            Task { await self.load(p) }
        }.store(in: &bag)
    }

    var signal: Int {
        switch state {
        case .error: 0
        case .loading, .seeking: 2
        case .paused: 4
        case .playing: firstFrame ? 5 : 3
        default: 1
        }
    }

    func play(_ p: Playable) {
        guard p.id != current?.id || state == .idle || error != nil else { return }
        current = p; error = nil; retunes = 0
        Task { await load(p) }
    }

    private func load(_ p: Playable) async {
        do {
            _ = try await engine.load(url: p.url, options: LoadOptions(httpHeaders: p.headers, isLive: p.isLive, liveJoinProfile: .fastZap))
        } catch is CancellationError {
            // A newer zap superseded this load.
        } catch {
            if current?.id == p.id { self.error = error.localizedDescription }
        }
    }

    func stop() {
        engine.stop(); current = nil; expanded = false
    }

    func togglePlayPause() { engine.togglePlayPause() }

    func seek(by seconds: Double) {
        guard current?.isLive == false else { return }
        let target = max(0, min(duration, time + seconds))
        Task { await engine.seek(to: target) }
    }
}
