# Android build

The same source builds as an `.so` that Android's `NativeActivity` loads — no fork, no wrapper app.

## Prerequisites

- Android SDK with **NDK r27**, and `cargo-apk`
- Rust target `aarch64-linux-android`
- A JDK (`keytool`) only for a release-signed APK

```bash
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/27.0.12077973
rustup target add aarch64-linux-android
cargo install cargo-apk
```

## Build

```bash
./scripts/build-apk.sh debug    # debug-signed, ~85 MB
./scripts/build-apk.sh          # release, ~13 MB, needs a keystore (below)
```

The script default `ANDROID_HOME` to `$HOME/Android/Sdk`, picks the newest NDK under it when
`ANDROID_NDK_ROOT` is unset, sources `~/.config/torrentx/signing.env`, and passes `--lib` for you.
`--lib` is required because the package has both a `cdylib` (what Android loads) and the desktop
`bin`, and cargo-apk refuses a package that mixes the two.

## Release signing

The keystore is read from the environment, never from the repo (`cargo-apk` checks these variables
before it looks at `Cargo.toml`):

```bash
# one-time; back the keystore up — losing it means no in-place app updates
keytool -genkeypair -keystore ~/.config/torrentx/torrentx-release.jks \
  -storetype PKCS12 -alias torrentx -keyalg RSA -keysize 4096 -validity 10000 \
  -dname "CN=TorrentX, O=chethan62, C=IN"

read -rsp 'keystore password: ' KEYSTORE_PASS; echo
printf 'export CARGO_APK_RELEASE_KEYSTORE=%s\nexport CARGO_APK_RELEASE_KEYSTORE_PASSWORD=%s\n' \
  "$HOME/.config/torrentx/torrentx-release.jks" "$KEYSTORE_PASS" > ~/.config/torrentx/signing.env
chmod 600 ~/.config/torrentx/signing.env ~/.config/torrentx/torrentx-release.jks
```

`./scripts/build-apk.sh` refuses a release build when `CARGO_APK_RELEASE_KEYSTORE` is unset.

## Verify the artefact

```bash
BT=$HOME/Android/Sdk/build-tools/34.0.0
$BT/apksigner verify --verbose --print-certs target/release/apk/torrentx.apk
$BT/aapt2 dump badging target/release/apk/torrentx.apk | head -3
```

Expected: `package: name='org.chethan.torrentx' versionName='<version>'`, `sdkVersion:'26'`,
`targetSdkVersion:'34'`, `native-code: 'arm64-v8a'`, `uses-permission: android.permission.INTERNET`,
signer `CN=TorrentX`, and `debuggable=false` for a release build. The certificate SHA-256 must match
`keytool -list -v` on the same keystore.

## What works on a phone

- Native sources (`apibay`, `knaben`) and user indexer definition files
- Jackett over the network, search, filters, favourites, RSS
- **Not available:** the player (it shells out to mpv/vlc/webtorrent), the tray, desktop
  notifications and the single-instance socket

The window reflows under 720 px wide — header, pickers, filters and the detail panel all stack — and
`ui_scale` is the knob for text size. The status bar and gesture strip are reserved via
`android_inset_top` / `android_inset_bottom` in the config (points, default 40 and 16, measured on a
1080x2376 density-480 device); tune them if the header still sits under your clock.

## Verified so far

Installed and launched on a CPH2691 (Android 16, SDK 36): `am start -W` returned `Status: ok`,
`topResumedActivity` was the activity, the process stayed alive, and the UI rendered at phone width.
Interactive flows (typing a query, opening Settings on the device) have not been driven by hand.
