# iOS front end

`crates/ssx3-ios` is a static library driven by the Objective-C app in this directory. `ssx3-mobile-core` owns the engine, settings, data/save roots, pacing and lifecycle for both mobile platforms. The iOS shell owns its UIKit scene, Metal surface, GameController and touch input, AVAudioSession, and a cpal/CoreAudio callback that drains `Audio::Pull`.

Use a private build copy with `ssx3/` at the pinned submodule commit. Put the USA disc image in that copy and run `python3 tools/setup.py --iso /path/to/disc.iso` from its `ssx3/` directory; this generates the EE and IOP code needed by the matching engine revision. Build with `ios/build.sh sim /absolute/build-dir` or `ios/build.sh device /absolute/build-dir`. The script limits the build directory to 16 GB, uses eight Rust jobs and nice priority, and prints the unsigned `.app` path. `xcodegen` and Xcode are required. The repo carries no team id: to run
on a device, open the generated `ios/SSX3MobileIOS.xcodeproj`
(gitignored) in Xcode, pick your own team under Signing &
Capabilities for the app target, and build there — or sign the
unsigned `.app` the script prints with a provisioning profile for
your own bundle id. Never commit the team id or profiles.

The app reads `Documents/data/ssx3.pack`, creates `Documents/saves/`, and keeps its own `ssx3-mc0.ps2` and `ssx3-settings.cfg` there. No game data or generated source belongs in this repository. Use a new bundle id when testing beside another installation; never uninstall an app to upgrade it, because that deletes its Documents.

Tap the upper left corner to open settings, then tap to change the game rate. The rate applies when iOS starts a new process after the current app has ended. Touch zones provide Start (upper right), directions (left), and face buttons (right); an extended GameController also works. `SSX3_IOS_AUDIO_SESSION=off` disables session activation for automated tests; `ambient` mixes with other audio, and the default is `playback`.
