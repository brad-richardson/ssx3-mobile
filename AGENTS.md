# ssx3-mobile agent notes

Local Android + iOS front ends on the pinned `ssx3/` engine submodule
(upstream ssx3-decomp). No game data or generated code is ever committed;
builds need a private copy with `tools/setup.py` output and a local pack
(see `README.md`).

## Build / test

```sh
cd ssx3 && python3 tools/setup.py --iso /path/to/disc.iso   # once per pin
python3 ssx3/tools/bundle.py pack --iso /path/to/disc.iso ssx3.pack
cargo test --locked --profile game -p ssx3-android --lib
cd android && ./gradlew assembleDebug testDebugUnitTest
ios/build.sh sim /absolute/build-dir
```

## Rules

- Never commit game material: disc images, `ssx3.pack`, memory cards,
  `ssx3/orig`, `ssx3/generated`, screenshots of the game, keystores,
  provisioning profiles, team ids, or device ids.
- Never edit the `ssx3/` submodule in place; engine changes go in as
  `patches/` diffs applied to a private build copy, with
  `git apply --check` re-verified after every pin move.
- Verify a change with the tests above and one boot of each app touched.
- Keep diffs small and the tree building from `README.md` with no extra
  knowledge.
