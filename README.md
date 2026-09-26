# Singboard for Android

`Singboard for Android` is the Android edition of [Singboard](https://github.com/Yuu518/singboard), a sing-box dashboard built with `Tauri 2 + Vue 3`. It works as a remote panel for any reachable Clash API, and on rooted devices it can also run and manage a local sing-box core.

## Features

- Floating bottom bar: Home, Proxies, Connections, Logs, Rules, Settings
- Home (opens first)
  - Running state, core version and uptime
  - Core on/off switch and restart (root mode only)
  - Realtime upload/download speed with totals
  - Memory usage, plus CPU usage of the core process (root mode only)
  - Network info: IP lookups and connectivity checks
- Proxies: group switching, latency tests, per-group test URLs, providers
- Rules: rule list, rule providers, provider content search (root mode)
- Connections: active/closed tabs, details, disconnect
- Logs: realtime stream with level/keyword filters
- Settings: Clash API endpoints, DNS query tool, themes, glass styles, custom wallpapers, updates
- Remote mode (no root required)
  - Connects to sing-box running on a router, a PC or another app
  - Connection state and core version come from the Clash API
- Root mode (Magisk / KernelSU / APatch)
  - Point the app at a sing-box working directory; it searches the directory and its subdirectories (4 levels) for:
    - the core: ELF executables in a `bin` folder first, otherwise files named `sing-box*` / `singbox*`
    - the configuration: JSON files containing `"outbounds"`, preferring `config.json`
    - rule sets: a `rules` / `ruleset` / `rule-set` style folder, otherwise the folder holding the most `.srs` files
  - Start, stop and restart the core; it keeps running after the panel is closed
  - Stopping sends SIGTERM and waits for the core to exit on its own (up to 60 s) so TUN `auto_redirect` rules are cleaned up; the core is never force-killed
  - Core logs follow `log.output` in the configuration; singboard keeps its own log in `/data/adb/singboard/logs`
  - Start on boot through `/data/adb/service.d/singboard.sh` (Magisk / KernelSU / APatch run it after boot completes)
  - Core updates from GitHub releases (`android-*.tar.gz`), verified by SHA-256 and rolled back if the new core fails to start; without a core, it installs to `bin/sing-box` (or `sing-box` in the working directory)
  - Every root operation goes through one long-lived `su` shell, so the root manager prompts once
- In-app updates
  - Checks `Yuu518/singboard-android` releases on startup (can be disabled)
  - Downloads the APK for the device ABI, verifies its SHA-256 digest, then opens the system installer

## Root layout

The working directory stays yours; singboard only keeps its service files in a fixed place so the boot hook can find them:

```text
/data/adb/singboard/
├─ singboard.sh      # service script: start | stop | restart | status | log
├─ service.env       # detected core, config and working directory
├─ logs/
│  └─ singboard.log  # service events plus core stdout/stderr, trimmed at 1 MB
└─ run/
   └─ sing-box.pid
/data/adb/service.d/singboard.sh   # boot hook
```

The service script is refreshed with the detected paths every time the core is started from the app. It can also be used from a root shell, for example `sh /data/adb/singboard/singboard.sh restart`.
## Requirements

- Node.js 18+ and `pnpm`
- Rust stable with Android targets:
  `rustup target add aarch64-linux-android armv7-linux-androideabi`
- JDK 17+
- Android SDK with platform 36, build-tools 36 and NDK r30 (`ANDROID_HOME`, `NDK_HOME`)

## Development

```bash
pnpm install
pnpm dev                    # frontend only
pnpm tauri android dev      # run on a connected device or emulator
```

## Build

```bash
pnpm tauri android build --apk --split-per-abi --target aarch64 armv7
```

APKs are written to `src-tauri/gen/android/app/build/outputs/apk/<abi>/release/`.

Release builds are signed when `src-tauri/gen/android/keystore.properties` exists:

```properties
storeFile=release.jks
storePassword=...
keyAlias=...
keyPassword=...
```

## Releases

The `Build` workflow runs on pushes to `master`, reads the latest tag as the version, and publishes `singboard-arm64-v8a.apk` and `singboard-armeabi-v7a.apk` to that release. Configure these repository secrets first:

| Secret | Value |
| --- | --- |
| `ANDROID_KEYSTORE_B64` | Base64 of the release keystore |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Key alias |
| `ANDROID_KEY_PASSWORD` | Key password |

Every release must be signed with the same key, otherwise Android refuses to install the update over the existing app.

## Notes

- The panel talks to the Clash API over HTTP, so cleartext traffic is allowed.
- Rule provider content search reads `cache.db` and `.srs` files from the working directory, so it needs root mode.

## License

AGPL-3.0, same as the desktop project.
