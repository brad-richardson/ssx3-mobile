# ssx3-mobile

Local Android front end for [ssx3-decomp](https://github.com/andersfischernielsen/ssx3-decomp), pinned as the `ssx3/` submodule at PR #25 head `844b3ded5f2838d907a0ed55187dc363823cd5b8`.

`crates/ssx3-mobile-core` owns platform-neutral settings, storage paths, pacing, lifecycle, and the engine instance. `crates/ssx3-android` supplies the NativeActivity shell and renderer. `android/` is the Java/Gradle application, keeping the launcher and game in separate processes. `ios/` is reserved for a later front end.

The Android app expects `ssx3.pack` in its external app files directory. It writes settings and its own memory card in internal app storage. No game data or generated code belongs in this repository.

On a build host, place the SSX 3 USA ISO in a **build copy** of `ssx3/`, run `python3 tools/setup.py` there, then build the Rust library and Gradle app with the `game` profile. The build copy has to preserve the `ssx3/` path relative to this repository's Cargo workspace.

The Android front end uses `EngineConfig::audio = Audio::Pull`. It drains the public `ssx3_iop::audio::pull_into` ring from an app-owned cpal callback; the app closes the stream when inactive and reopens it when audio focus returns or after a stream error. The lane report records the measured underrun behavior.

## Pull audio queue experiment

`patches/audio-pull-queue.patch` is an optional engine patch for a **build copy** of `ssx3/`. It makes the Pull sink report its queued frames to the real-time mixer. Apply it from that build copy's `ssx3/` root with `git apply ../patches/audio-pull-queue.patch`; the pinned submodule in this repository stays unmodified. The lane report records the 120 Hz Android callback measurements before and after the patch.
