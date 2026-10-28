#!/bin/sh
# Build the Android APK. The release keystore is NOT in the repo — cargo-apk
# reads these two env vars before it looks at Cargo.toml, so the secret stays in
# a 0600 file outside the tree:
#
#   export CARGO_APK_RELEASE_KEYSTORE=~/.config/torrentx/torrentx-release.jks
#   export CARGO_APK_RELEASE_KEYSTORE_PASSWORD=...
#
# Put them in ~/.config/torrentx/signing.env and this script sources it. Lose
# that keystore and the app can never be updated in place on a device that
# already installed it — back it up.
set -eu

cd "$(dirname "$0")/.."

ENVF=${TX_SIGNING_ENV:-$HOME/.config/torrentx/signing.env}
PROFILE=${1:-release}

if [ -f "$ENVF" ]; then
    # shellcheck disable=SC1090
    . "$ENVF"
fi

: "${ANDROID_HOME:=$HOME/Android/Sdk}"
export ANDROID_HOME
if [ -z "${ANDROID_NDK_ROOT:-}" ]; then
    ANDROID_NDK_ROOT=$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)
    export ANDROID_NDK_ROOT
fi

if [ "$PROFILE" = release ] && [ -z "${CARGO_APK_RELEASE_KEYSTORE:-}" ]; then
    echo "no release keystore: set CARGO_APK_RELEASE_KEYSTORE and _PASSWORD in $ENVF" >&2
    exit 1
fi

# --lib, not the bin: cargo-apk refuses a package with both a bin and a cdylib.
if [ "$PROFILE" = release ]; then
    cargo apk build --lib --release
else
    cargo apk build --lib
fi

APK="target/$PROFILE/apk/torrentx.apk"
[ -f "$APK" ] || { echo "no APK at $APK" >&2; exit 1; }
printf '\n%s  %s  %s\n' "$(sha256sum "$APK" | cut -c1-16)" \
    "$(du -h "$APK" | cut -f1)" "$APK"
