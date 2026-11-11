# Development

## Layout

| Path | What |
| --- | --- |
| `ssx3/` | Engine submodule (upstream ssx3-decomp, pinned commit below). Read-only: never edit, never commit generated code in it. |
| `crates/ssx3-mobile-core/` | Shared core: settings, data/save roots, pacing, lifecycle, engine instance. |
| `crates/ssx3-android/` | Android shell: NativeActivity entry, renderer, gamepad, pull-audio owner, card import. |
| `crates/ssx3-ios/` | iOS static library driven from `ios/`. |
| `android/` | Gradle app (launcher + game processes, Java unit tests). |
| `ios/` | Xcode app sources, `project.yml` (xcodegen), `build.sh`. |
| `patches/` | Engine diffs for private build copies only (see below). |
| `docs/plans/roadmap.md` | Where this is going. |

## Submodule and patches

The engine pin is the `ssx3` gitlink. Update it with:

```sh
cd ssx3
git fetch origin
git checkout <new-upstream-main-sha>
cd ..
git add ssx3
```

Then regenerate in a private build copy (never in this checkout):

```sh
cd ssx3
python3 tools/setup.py --iso /path/to/disc.iso
```

`patches/` applies only inside that build copy, from its `ssx3/` root:

```sh
cd <build-copy>/ssx3
git apply <mobile-repo>/patches/audio-pull-queue.patch
```

Check a patch still applies after every pin move
(`git apply --check ../patches/*.patch`). The committed submodule stays
unmodified.

## Tests

Rust tests for the shared core (need generated code present from the
setup step above; the core has no OS gate, so this runs on the host —
currently a compile check, the core holds no unit tests yet):

```sh
cargo test --locked --profile game -p ssx3-mobile-core
```

The `ssx3-android` / `ssx3-ios` crates are whole-crate OS-gated
(`#![cfg(target_os = ...)]`), so their unit tests compile only for
their device target and ride with the app builds below — there is no
host `cargo test` for them.

Android Java unit tests (need JDK 17 + SDK; run under `android/`):

```sh
./gradlew testDebugUnitTest
```

Engine-side audio check for the pull-queue patch (inside the build
copy's `ssx3/`, after applying the patch):

```sh
cargo test --locked --profile game -p ssx3-iop --lib \
  audio::tests::pull_output_hands_the_samples_to_the_front_end -- --exact
```

## Debugging on device

- Android: `adb logcat | grep -E 'mobile|NeedsData'` shows bootstrap,
  lifecycle and halt lines. A fatal halt/panic raises the app's fatal
  dialog and stops the loop; the card is left untouched.
- iOS: stderr goes to `Documents/ssx3.log` (readable in Files);
  Simulator logs stream from `simctl spawn booted log stream`.
- Both: settings are a plain `ssx3-settings.cfg` of `SSX3_*` lines;
  delete it for a fresh seed (Android reseeds 120 Hz on first launch).

## Rules

- Never commit game data (`*.iso`, `ssx3.pack`, `*.ps2`), generated
  code (`ssx3/orig`, `ssx3/generated`), keystores, provisioning
  profiles, team ids, or device ids. `.gitignore` covers the paths;
  keep it that way.
- Verify a change with the tests above plus one boot of each app you
  touched (pack staged, title reached). UI text stays short enough
  for the smallest supported screen.
