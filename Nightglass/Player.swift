import AetherEngine
import AVFoundation
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
    /// Bumped on every zap; drives the CRT glitch.
    var zapTick = 0
    var time: Double = 0
    var duration: Double = 0
    var audioTracks: [TrackInfo] = []
    var subtitleTracks: [TrackInfo] = []
    var videoSize = CGSize.zero

    private var bag = Set<AnyCancellable>()
    private var retunes = 0

    init() {
        do { engine = try AetherEngine() } catch { fatalError("AetherEngine init failed: \(error)") }
        engine.$state.receive(on: DispatchQueue.main).sink { [weak self] s in
            self?.state = s
            if case .error(let message) = s { self?.error = message }
        }.store(in: &bag)
        engine.$hasFirstFrameReadyForDisplay.receive(on: DispatchQueue.main).sink { [weak self] in self?.firstFrame = $0 }.store(in: &bag)
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
        current = p; error = nil; retunes = 0; zapTick += 1
        Sound.tick()
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

/// Neon hum and selection tick. Off by default, never during fullscreen playback.
@MainActor
enum Sound {
    static var enabled = false
    private static let engine = AVAudioEngine()
    private static var started = false

    static func tick() { tone(from: 2200, to: 500, duration: 0.05, volume: 0.05) }
    static func buzz() { tone(from: 120, to: 110, duration: 0.18, volume: 0.04) }

    private static func tone(from f0: Double, to f1: Double, duration: Double, volume: Float) {
        guard enabled else { return }
        let format = engine.outputNode.inputFormat(forBus: 0)
        let rate = format.sampleRate > 0 ? format.sampleRate : 44_100
        let frames = AVAudioFrameCount(rate * duration)
        guard let mono = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1),
              let buffer = AVAudioPCMBuffer(pcmFormat: mono, frameCapacity: frames) else { return }
        buffer.frameLength = frames
        var phase = 0.0
        for i in 0..<Int(frames) {
            let p = Double(i) / Double(frames)
            phase += (f0 * pow(f1 / f0, p)) / rate
            let saw = Float(2 * (phase - floor(phase)) - 1)
            buffer.floatChannelData![0][i] = saw * volume * Float(1 - p)
        }
        let node = AVAudioPlayerNode()
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: mono)
        if !started { try? engine.start(); started = true }
        node.scheduleBuffer(buffer) { DispatchQueue.main.async { engine.detach(node) } }
        node.play()
    }
}
