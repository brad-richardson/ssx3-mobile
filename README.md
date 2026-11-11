# ssx3-mobile

Local Android and iOS front ends for
[ssx3-decomp](https://github.com/andersfischernielsen/ssx3-decomp),
pinned as the `ssx3/` submodule at upstream main
`7b665120e09c0bb3c3f6b5c008d7ab4646f14132`.

`crates/ssx3-mobile-core` owns platform-neutral settings, storage paths,
pacing, lifecycle, and the engine instance. `crates/ssx3-android` supplies
the NativeActivity shell and renderer. `android/` is the Java/Gradle
application, keeping the launcher and game in separate processes.
`crates/ssx3-ios` and `ios/` provide a local iOS static library and Xcode
app; see `ios/README.md`.

## What you need

- An SSX 3 USA disc image (`SLUS-20772`). Make it from a disc you own.
  Other regions are not supported.
- Rust (pinned by `rust-toolchain.toml`; `rustup` installs it on first use).
- Python 3 (for the engine's `setup.py` / `bundle.py`).
- Android: JDK 17, the Android SDK with platform 34 and
  NDK `28.2.13676358`, and `cargo-ndk`. Or use `android/gradlew`
  (Gradle 8.9, matching AGP 8.7.3), which fetches its own distribution.
- iOS: Xcode and [xcodegen](https://github.com/yonaskolb/XcodeGen)
  (`brew install xcodegen`), plus your own Apple team for device installs.

## Set up (once)

```sh
git clone --recurse-submodules https://github.com/brad-richardson/ssx3-mobile.git
cd ssx3-mobile

# Generated game code (local only, never committed). `SSX3_ISO` must be
# exported: the recompiler's IOP pass reads it directly.
export SSX3_ISO=/path/to/your/disc.iso
cd ssx3
python3 tools/setup.py --iso "$SSX3_ISO"
cd ..

# Data pack (local only, never committed, about 2.4 GB):
python3 ssx3/tools/bundle.py pack ssx3.pack --iso "$SSX3_ISO"
```

The build copy must keep the `ssx3/` path: the workspace excludes `ssx3`
but the crates reference it by path. The `game` profile (workspace
`Cargo.toml`) is the optimized profile both apps build.

## Android build

```sh
cargo install cargo-ndk
cd android && ./gradlew assembleDebug
```

The APK lands at
`android/app/build/outputs/apk/debug/app-debug.apk`. Install it, then copy
the pack to the app's external files directory and restart the app:

```sh
adb push ssx3.pack /storage/emulated/0/Android/data/<applicationId>/files/ssx3.pack
```

The default id is `com.ssx3decomp.test.mobileplay`
(`android/app/build.gradle`, overridable with `-PapplicationId=`).
A fresh install seeds 120 Hz in its settings; changing the rate
restarts the game process. To carry over an existing memory card, copy
`card.ps2` plus its `card.sha256` into the `imports/` folder next to the
pack before first launch; a failed import stops boot rather than
starting a blank card. Saves live in the app's internal storage.

## iOS build

```sh
ios/build.sh sim /absolute/build-dir     # simulator .app (unsigned)
ios/build.sh device /absolute/build-dir  # device .app (unsigned)
```

Copy `ssx3.pack` into the app's `Documents/data/` (e.g. in the
Simulator, or via the Files app on device: the app shares its
`Documents`), then launch. Saves and `ssx3-settings.cfg` live in
`Documents/saves/`, visible in Files. The build is unsigned
(`CODE_SIGNING_ALLOWED=NO`, and the test target carries no team):
to run on a device, open the generated project
(`ios/SSX3MobileIOS.xcodeproj`, gitignored) in Xcode, assign your own
team under Signing & Capabilities, and build there. Never commit the
team id or provisioning files. See `ios/README.md` for controls and
audio-session options.

## Optional engine patches

`patches/` holds engine diffs applied only to a private build copy of
`ssx3/` (the pinned submodule stays unmodified):

- `audio-pull-queue.patch`: the Pull sink reports its queued frames to
  the real-time mixer (Android `Audio::Pull` callback accounting).
- `audio-lead-floor.patch`: keeps at least 84 ms queued while the
  app-owned callback drains (Android play build).

Apply from the build copy's `ssx3/` root, e.g.
`git apply ../patches/audio-lead-floor.patch`.

## Licence

MIT — see `LICENSE` (Anders Fischer-Nielsen for the `ssx3/` engine,
Brad Richardson for this front end). This repository carries no game
data and no generated code: the disc image, `ssx3.pack`, memory cards,
`ssx3/orig`, `ssx3/generated` and build outputs are all gitignored.
Never commit them.

## Known limits

- iOS: an AirPods-route handoff banner can steal controller focus
  (accepted for now); boot shows a purple screen for ~3–5 s (cause not
  yet checked). The game rate applies when iOS starts a new process.
- Android: audio-only interruptions (headphone unplug, focus loss,
  stream errors) silence the game but never pause it; losing the
  window or surface still pauses.
- Both: USA disc only; the ELF hash is checked at boot.
