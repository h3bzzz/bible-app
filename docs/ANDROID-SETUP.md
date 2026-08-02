# Android build setup (Arch Linux)

Everything needed to build the Bible app for Android and sideload it to a phone
for testing. Written for this machine: Arch Linux, JDK 17 and `adb` already
present. iOS is not covered — it needs macOS + Xcode and cannot be built here.

Run the steps in order. Where a step is a one-time download it says so; the
environment variables in step 4 must be present in every shell that builds.

---

## 1. Rust Android targets

Tauri cross-compiles the Rust core to four Android ABIs. Add all four:

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

`aarch64` is every modern phone. The other three cover older devices and the
emulator; `cargo tauri android build` expects all four unless you pass
`--target`.

---

## 2. Android SDK command-line tools

The SDK lives under `~/Android/Sdk`. Download the standalone command-line tools
(no Android Studio required) and place them at the path `sdkmanager` expects
(`cmdline-tools/latest`):

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
mkdir -p "$ANDROID_HOME/cmdline-tools"

# One-time download (~150 MB). Check the current URL at
# https://developer.android.com/studio#command-line-tools-only — the number in
# the filename changes; the layout does not.
cd /tmp
curl -O https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
unzip -q commandlinetools-linux-11076708_latest.zip
mv cmdline-tools "$ANDROID_HOME/cmdline-tools/latest"
rm commandlinetools-linux-11076708_latest.zip
```

After this, `$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager` exists.

---

## 3. SDK packages + NDK

Install platform tools, one platform, matching build-tools, and the NDK. The
NDK is what compiles the Rust `cdylib` and the bundled SQLite C code for the
phone.

```bash
SDKM="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"

# Accept licenses first (interactive — type `y` at each prompt).
"$SDKM" --licenses

"$SDKM" \
  "platform-tools" \
  "platforms;android-34" \
  "build-tools;34.0.0" \
  "ndk;26.3.11579264"
```

Notes:
- `platform-tools` provides its own `adb`; the system `adb` already in `$PATH`
  works too, either is fine.
- NDK `26.3.11579264` is a known-good r26 for Tauri 2. A newer r27 also works;
  if you change it, update `NDK_HOME` in step 4 to match.

---

## 4. Environment variables

Tauri and Gradle read these. Add to `~/.zshrc` (this machine's shell) so every
future shell has them, then re-source:

```bash
cat >> ~/.zshrc <<'EOF'

# Android / Tauri mobile
export ANDROID_HOME="$HOME/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/26.3.11579264"
export JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.12-tem"
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
EOF

source ~/.zshrc
```

`NDK_HOME` must point at the exact NDK version installed in step 3. Tauri also
accepts `ANDROID_NDK_ROOT`; `NDK_HOME` is enough.

---

## 5. Verify

```bash
sdkmanager --list_installed          # platform-tools, platform-34, build-tools, ndk all present
echo "$ANDROID_HOME"                 # /home/<you>/Android/Sdk
echo "$NDK_HOME"                     # .../ndk/26.3.11579264
ls "$NDK_HOME/toolchains/llvm"       # NDK really there
java -version                        # 17.x
rustup target list --installed | grep android   # four targets
```

If any line is empty or missing, fix that step before continuing.

---

## 6. Initialize the Android project

From the repo root. This generates `apps/desktop/gen/android` (a Gradle
project) from `tauri.conf.json`:

```bash
cd apps/desktop
cargo tauri android init
```

`gen/android` is generated output — it can be regenerated and is normally
gitignored. Re-run `init` after changing the bundle identifier or app name.

---

## 7. Build + install to a phone

Enable **USB debugging** on the phone (Settings → Developer options), plug it
in, accept the RSA prompt, then:

```bash
adb devices                          # phone shows as "device", not "unauthorized"

cd apps/desktop
# --debug = debug-signed, which adb can install. Plain `--apk` (release) is
# UNSIGNED and `adb install` will reject it with INSTALL_PARSE_FAILED_NO_CERT.
cargo tauri android build --debug --apk --target aarch64
```

The APK lands at:

```
apps/desktop/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Install it:

```bash
adb install -r apps/desktop/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

The app launches offline. On first launch the ~159 MB `corpus.db` is copied
out of the APK assets into private app storage (see corpus staging below), so
the first cold start is noticeably slower than every launch after.

> **Disk:** a debug build needs ~10 GB free — the debug `.so` carries symbols
> (~84 MB) and the corpus is duplicated across the staged asset, the APK, and
> the on-device copy. `cargo clean` or removing `target/debug` reclaims it.

### Corpus staging (why the extra pieces exist)

Getting the 159 MB SQLite corpus onto the phone took three fixes, all already
in the repo:

1. **`apps/desktop/corpus.db` symlink → `build/corpus.db`.** `tauri.conf.json`
   references the corpus as `"corpus.db"`, not `"../../build/corpus.db"`.
   Tauri encodes a `../` in a resource path as an `_up_` directory under
   `assets/`, and Android's asset packager silently drops any directory whose
   name starts with `_` (its default `ignoreAssetsPattern` includes
   `<dir>_*`). The symlink keeps the resource path parent-free so it lands at
   `assets/corpus.db` and actually ships in the APK. The symlink is
   gitignored; recreate it with `ln -sf ../../build/corpus.db apps/desktop/corpus.db`.

2. **`MainActivity.stageCorpus` (Kotlin).** An APK asset is not a real file —
   it lives inside the APK zip and is reachable only through the Java
   `AssetManager`, so `rusqlite` (and `std::fs`) cannot open it. `MainActivity`
   copies it to `applicationInfo.dataDir/corpus.db` before the Rust runtime
   starts. This lives in the *user-editable* `MainActivity.kt` (only the
   `generated/` package is overwritten by re-init), but a fresh
   `cargo tauri android init` on a clean checkout writes a stock `MainActivity`
   without it — re-add `stageCorpus` if you ever regenerate from scratch.

3. **`mobile_corpus_path` (Rust, `apps/desktop/src/lib.rs`).** Opens the staged
   copy on Android; on iOS it copies from the (real, read-only) bundle path
   itself. A writable path is mandatory: `CorpusDb::open` puts the database in
   WAL mode and runs migrations, both of which write.

### Live-reload alternative

For iterating on the UI against the phone:

```bash
cargo tauri android dev
```

---

## Known constraints

- **159 MB corpus.** Fine for `adb install` sideloading. For the Play Store the
  raw asset is near the AAB size ceiling — Play Asset Delivery or an
  install-time asset pack is the real fix, tracked separately.
- **Signing.** The `--debug` build above is debug-signed, good for sideload
  testing only. Plain `cargo tauri android build --apk` produces an *unsigned*
  release APK. Play Store upload needs an `--aab` built and signed with a
  release keystore (`keytool -genkey`, wired into `signingConfigs` in
  `gen/android/app/build.gradle.kts`).
- **On-device runtime is unverified here.** The build pipeline is proven end to
  end (APK builds, contains `assets/corpus.db`, Kotlin + Rust compile), but no
  phone was attached to confirm first-launch corpus staging and reading. Do
  that install + smoke test on a real device.
- **iOS.** Not buildable on Linux. Needs macOS + Xcode, `cargo tauri ios init`,
  an Apple Developer account, and TestFlight for device testing.

---

## Arch AUR shortcut (alternative to steps 2–3)

Instead of the manual download you can use AUR packages:

```bash
# with an AUR helper such as yay
yay -S android-sdk-cmdline-tools-latest android-platform android-sdk-build-tools android-ndk
```

These install under `/opt/android-sdk`. If you take this route, set
`ANDROID_HOME=/opt/android-sdk` and `NDK_HOME=/opt/android-sdk/ndk/<version>`
in step 4 instead, and ensure your user owns the SDK dir (the AUR packages need
group `android-sdk` write access, or `chown`).
```
