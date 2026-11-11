#!/bin/sh
# Build from a private source copy with generated code and a local SSX 3 pack.
set -eu
if [ "$#" -ne 2 ]; then
    echo "usage: ios/build.sh sim|device /absolute/build-dir" >&2
    exit 2
fi
mode=$1
outdir=$2
case "$mode" in
    sim) target=aarch64-apple-ios-sim; sdk=iphonesimulator; destination='generic/platform=iOS Simulator' ;;
    device) target=aarch64-apple-ios; sdk=iphoneos; destination='generic/platform=iOS' ;;
    *) echo "mode must be sim or device" >&2; exit 2 ;;
esac
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$outdir" "$outdir/tmp"
export RUSTC="${RUSTC:-$(rustup which rustc)}"
export CARGO_TARGET_DIR="$outdir/target"
export TMPDIR="$outdir/tmp"
cap=16000000000
check_cap() {
    used=$(du -sk "$outdir" | awk '{print $1 * 1024}')
    if [ "$used" -ge "$cap" ]; then
        echo "iOS build-dir byte cap exceeded: $used >= $cap" >&2
        exit 1
    fi
}
check_cap
cd "$root"
nice -n 10 cargo rustc --locked --profile game -j8 -p ssx3-ios --target "$target" -- --crate-type=staticlib > "$outdir/rust-$mode.log" 2>&1
check_cap
xcodegen generate --spec "$root/ios/project.yml" --project "$root/ios" > "$outdir/xcodegen-$mode.log" 2>&1
nice -n 10 xcodebuild -project "$root/ios/SSX3MobileIOS.xcodeproj" -scheme SSX3MobileIOS \
    -configuration Release -sdk "$sdk" -destination "$destination" \
    -derivedDataPath "$outdir/derived-$mode" CODE_SIGNING_ALLOWED=NO \
    "SSX3_IOS_LIB_DIR=$outdir/target/$target/game" build > "$outdir/xcode-$mode.log" 2>&1
check_cap
printf '%s\n' "$outdir/derived-$mode/Build/Products/Release-$sdk/SSX3MobileIOS.app"
