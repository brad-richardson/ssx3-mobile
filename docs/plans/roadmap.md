# Roadmap

Mobile front ends for SSX 3: the pinned decomp engine plus a local
Android shell and a local iOS shell. Order matters: upstream first,
device-specific polish second.

Status legend: ✅ done · 🟡 in progress · ⬜ not started

## Where we are

| Layer | Today |
| --- | --- |
| Android | ✅ Play ride build: boots 120 Hz, panel-rate request, card import, finished Snow Jam race, save/cold-load verified, halt shows a fatal dialog instead of exiting silently. |
| iOS | ✅ Device build accepted (0.1(3)): title at 60/120 across restarts, ambient callback flat after startup, background/foreground PASS, synthetic interruption PASS. |
| Engine pin | Upstream main `7b665120`, submodule stays unmodified; mobile diffs live in `patches/`. |

## Upstreaming

Mobile-driven core changes belong in the decomp once proven on device,
so both front ends share them instead of carrying patches. Each needs
its device receipt first.

- ✅ **Front-end seam (upstream PR #25).** The engine library this repo builds
  on. The earlier Android PR (#21) was closed in its favour: the Android and
  iOS apps live here, on that seam, as agreed upstream.
- 🟡 **Save folder (upstream PR #32) and card-import contract.** The copy-only
  `imports/` + sha256 activation with no blank-card fallback. Why
  upstream: one verified import rule beats per-app save handling.
- 🟡 **Pipeline persistence and cache folder.** Remember the pipelines a run
  creates and keep the driver's compiled data, in an app cache folder
  (branch ready; follows PR #32). Why
  upstream: faster warm starts on every platform, one code path.
- ⬜ **Audio pull-queue reporting + lead floor.** Part of the queue
  accounting already rides upstream as `Engine::audio_buffered`;
  upstream the remainder plus a minimum audio lead in
  `Engine::pace` so the front end drops `patches/`. Why upstream:
  every pull-callback app needs the same accounting.

## Improvements

- ⬜ **120 Hz pacing.** The guest runs at 120; confirm the panel lock
  and pacing quality per device (guest vsyncs/s, missed callbacks).
- ⬜ **Presets.** Per-device defaults (resolution, rate) instead of
  one seeded setting.
- ⬜ **Controller.** Full race proof of the iOS GameController path;
  phone controller choice still open.
- ⬜ **iOS polish.** Purple boot screen (~3–5 s, cause unchecked),
  AirPods-handoff focus loss (accepted for now, low priority).

## Known issues and nits

- AirPods route handoff steals controller focus on iOS.
- Purple screen for ~3–5 s at iOS boot; cause not yet checked.
- Lifecycle checklist still open: silent switch, route change,
  interruption, backgrounding, controller rebind.
- iOS rate changes apply on process restart (one engine boot per
  process).

## Fast follow: GitHub Actions CI

Once the repo is public, add CI mirroring the upstream workflow
layout (this checkout of upstream main carries no `.github`
directory; confirm against upstream at the time). Without game data:

- `cargo fmt --check` and `clippy` / warnings gate on the workspace.
- `cargo test -p ssx3-mobile-core -p ssx3-android` (the pure unit
  tests; engine-dependent tests run only where generated code exists).
- Android `assembleDebug` of the no-disc stub path, if upstream's
  no-disc CI pattern allows it.
- iOS compile on a macOS runner (`ios/build.sh sim`).

Device boots, pack staging and sign-your-own-team stay out of CI:
they need a disc, generated code and Apple credentials.
