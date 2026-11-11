import XCTest
import Vision

final class TouchRaceTests: XCTestCase {
    private let bundleID = "com.ssx3decomp.test.mobileios1"

    private func point(_ app: XCUIApplication, _ x: CGFloat, _ y: CGFloat) -> XCUICoordinate {
        app.coordinate(withNormalizedOffset: CGVector(dx: x, dy: y))
    }

    private func capture(_ name: String) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        print("UI_TRACE screenshot=\(name)")
    }

    private func words() -> String {
        let shot = XCUIScreen.main.screenshot().image
        guard let image = shot.cgImage else { return "" }
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .fast
        do {
            try VNImageRequestHandler(cgImage: image).perform([request])
            return (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
                .joined(separator: " ").uppercased()
        } catch {
            print("UI_TRACE ocr_error=\(error)")
            return ""
        }
    }

    private func waitForTitle(_ seconds: Int) -> Bool {
        for _ in 0..<(seconds / 5) {
            let seen = words()
            if seen.contains("START") && seen.contains("SSX") {
                print("UI_TRACE title_seen=true text=\(seen.prefix(180))")
                return true
            }
            sleep(5)
        }
        print("UI_TRACE title_seen=false text=\(words().prefix(180))")
        return false
    }

    private func raceVisible() -> Bool {
        let seen = words()
        print("UI_TRACE race_ocr=\(seen.prefix(220))")
        return seen.contains("MPH") || seen.contains("LAP") || seen.contains("SPEED")
    }

    private func launch(_ app: XCUIApplication, event: String? = nil) {
        app.launchEnvironment = [
            "SSX3_IOS_AUDIO_SESSION": "ambient",
            "SSX3_SKIP_MOVIES": "1",
        ]
        if let event { app.launchEnvironment["SSX3_EVENT"] = event }
        app.launch()
        print("UI_TRACE launched event=\(event ?? "none")")
    }

    private func attemptMenus(_ app: XCUIApplication, number: Int) -> Bool {
        launch(app)
        guard waitForTitle(80) else {
            capture("menu-attempt-\(number)-no-title")
            return false
        }
        point(app, 0.90, 0.08).tap() // Start zone.
        sleep(4)
        for step in 0..<12 {
            if raceVisible() {
                capture("menu-attempt-\(number)-race")
                return true
            }
            if number == 2 && step == 2 { point(app, 0.16, 0.78).tap() } // Down.
            if number == 3 && step == 2 { point(app, 0.16, 0.20).tap() } // Up.
            point(app, 0.90, 0.83).tap() // Cross zone.
            sleep(2)
        }
        capture("menu-attempt-\(number)-end")
        return raceVisible()
    }

    func testTouchRaceAndSiri() {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: bundleID)
        var menuRace = false
        for attempt in 1...3 {
            if attemptMenus(app, number: attempt) {
                menuRace = true
                print("UI_TRACE menu_route=pass attempt=\(attempt)")
                break
            }
            print("UI_TRACE menu_route=miss attempt=\(attempt)")
            app.terminate()
        }
        if !menuRace {
            print("UI_TRACE direct_event_fallback=ARA1")
            launch(app, event: "ARA1")
            for _ in 0..<24 {
                if raceVisible() { break }
                sleep(5)
            }
        }
        capture("race-start")
        print("UI_TRACE race_start_visible=\(raceVisible())")

        let accelerate = point(app, 0.90, 0.84) // Cross, the lower-right face zone.
        let steer = point(app, 0.16, 0.53) // Left direction zone.
        accelerate.press(forDuration: 20)
        print("UI_TRACE race_ride_accel_seconds=20 siri_request=begin")
        let before = testRun?.failureCount ?? 0
        XCUIDevice.shared.siriService.activate(voiceRecognitionText: "What time is it")
        sleep(6)
        print("UI_TRACE siri_failure_delta=\((testRun?.failureCount ?? 0) - before)")
        app.activate()
        accelerate.press(forDuration: 30)
        steer.press(forDuration: 6)
        print("UI_TRACE race_ride_accel_seconds=50 steer_seconds=6")
        capture("race-end")
        print("UI_TRACE race_end_visible=\(raceVisible())")
    }
}
