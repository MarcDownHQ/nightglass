import XCTest

/// Drives the app the way a person would: Sources form by click, then a live channel.
/// Needs `TEST_RUNNER_NG_XC_PASS` (Dispatcharr XC password) to also add the local Xtream source.
final class NightglassUITests: XCTestCase {
    var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchEnvironment["NG_KEYCHAIN_ACCOUNT"] = "uitest-\(UUID().uuidString)"
        // A freshly signed build sometimes misses XCTest's launch deadline ("does not have a process ID"):
        // the first attempt may fail without failing the test, the second one may not.
        app.terminate()
        let lenient = XCTExpectedFailure.Options()
        lenient.isStrict = false
        XCTExpectFailure("first launch of a fresh build can time out", options: lenient) { app.launch() }
        if !app.wait(for: .runningForeground, timeout: 20) {
            print("[NG] first launch failed, retrying")
            app.terminate()
            app.launch()
        }
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20), "app did not start")
        continueAfterFailure = false
    }

    override func tearDown() { app.terminate() }

    func status() -> String { app.staticTexts["ng-status"].value as? String ?? app.staticTexts["ng-status"].label }

    func waitStatus(_ fragment: String, timeout: TimeInterval) -> Bool {
        let end = Date().addingTimeInterval(timeout)
        while Date() < end {
            if status().contains(fragment) { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        return false
    }

    func shot(_ name: String) {
        let a = XCTAttachment(screenshot: app.windows.firstMatch.screenshot())
        a.name = name; a.lifetime = .keepAlways
        add(a)
    }

    func type(_ field: String, _ text: String) {
        let f = app.textFields[field].exists ? app.textFields[field] : app.secureTextFields[field]
        f.click()
        f.typeText(text)
    }

    func testAddSourcesBrowseGuideAndWatch() throws {
        // Empty keychain account: the app starts on "No source yet".
        app.buttons["Add a source"].click()

        // M3U via the preset (iptv-org France + xmltvfr guide).
        app.buttons["Use iptv-org France"].click()
        app.buttons["Connect"].click()
        let added = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Added' OR value BEGINSWITH 'Added'")).firstMatch
        XCTAssertTrue(added.waitForExistence(timeout: 90), "M3U source did not connect")
        shot("sources-m3u")

        // Local Xtream (Dispatcharr), typed into the form.
        if let pass = ProcessInfo.processInfo.environment["NG_XC_PASS"] {
            app.buttons["Xtream"].click()
            type("Name", "Dispatcharr")
            type("Server", "http://127.0.0.1:9191")
            type("Username", "nightglass")
            type("Password", pass)
            app.buttons["Connect"].click()
            XCTAssertTrue(waitStatus("channels=400", timeout: 90), "Xtream source did not add its channels: \(status())")
            shot("sources-both")
        }

        // Guide: wait for the XMLTV guide, then look at it.
        XCTAssertTrue(waitStatus("guide=", timeout: 5))
        let guideLoaded = waitStatus("guide=1", timeout: 1) || {
            let end = Date().addingTimeInterval(120)
            while Date() < end {
                if let n = Int(status().components(separatedBy: "guide=").last ?? ""), n > 50 { return true }
                RunLoop.current.run(until: Date().addingTimeInterval(1))
            }
            return false
        }()
        XCTAssertTrue(guideLoaded, "guide never loaded: \(status())")
        app.buttons["Guide"].click()
        XCTAssertTrue(app.buttons["Now"].waitForExistence(timeout: 5))
        shot("guide")

        // Switch source from the header: only the M3U playlist's 200 channels remain.
        if ProcessInfo.processInfo.environment["NG_XC_PASS"] != nil {
            app.descendants(matching: .any)["source-picker"].firstMatch.click()
            app.menuItems["iptv-org · France"].click()
            XCTAssertTrue(waitStatus("channels=200", timeout: 5), "source switch: \(status())")
            shot("source-switched")
        }

        // Live: click a channel, wait for a picture the engine confirms on screen.
        // Public streams go down now and then, so try a few before calling it a failure.
        app.buttons["Live"].click()
        var row = app.descendants(matching: .any)["channel-arte (France) (1080p)"].firstMatch
        var playing = false
        for name in ["arte (France) (1080p)", "BFM TV (1080p)", "20 Minutes TV (1080p)"] {
            row = app.descendants(matching: .any)["channel-" + name].firstMatch
            guard row.waitForExistence(timeout: 10) else { continue }
            row.click()
            if waitStatus("rendered=true", timeout: 30) { playing = true; break }
            print("[NG] \(name) did not play: \(status())")
        }
        XCTAssertTrue(playing, "no channel rendered a frame: \(status())")
        sleep(2)
        shot("live-preview")

        // Fullscreen player.
        row.doubleClick()
        sleep(3)
        XCTAssertTrue(status().contains("rendered=true"), status())
        shot("player-fullscreen")

        // Stop from fullscreen: move the pointer to reveal the controls, like a person would.
        let window = app.windows.firstMatch
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.4)).hover()
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.5)).hover()
        XCTAssertTrue(app.buttons["stop-playback"].firstMatch.waitForExistence(timeout: 3), "fullscreen controls did not appear")
        app.buttons["stop-playback"].firstMatch.click()
        XCTAssertTrue(waitStatus("state=idle", timeout: 5), "stop: \(status())")
        XCTAssertTrue(row.waitForExistence(timeout: 5))

        // Play again, then stop from the preview's button.
        row.click()
        XCTAssertTrue(waitStatus("rendered=true", timeout: 45), "replay: \(status())")
        shot("live-with-stop")
        app.buttons["stop-playback"].firstMatch.click()
        XCTAssertTrue(waitStatus("state=idle", timeout: 5), "stop from preview: \(status())")
        shot("stopped")
    }
}
