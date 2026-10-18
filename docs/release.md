# Releasing and installing POS Heart

## Building installers

| Platform | Command | Output (`target/dist/`) |
|---|---|---|
| macOS (universal) | `sh packaging/macos/build-dmg.sh` | `POS Heart.app`, `POS-Heart-<version>-macOS.dmg` |
| Windows (x64) | `cargo build --release --locked -p pos-desktop --bin pos-heart` then `iscc /DVersion=<version> packaging\windows\pos-heart.iss` (Inno Setup 6) | `POS-Heart-<version>-Windows-Setup.exe` |

`TARGETS=aarch64-apple-darwin sh packaging/macos/build-dmg.sh` builds Apple silicon only (faster, for local checks).

The **Release** workflow (`.github/workflows/release.yml`) builds both on GitHub: run it by hand for test builds (artifacts on the run), or push a tag `vX.Y.Z` equal to the workspace `version` in `Cargo.toml` to also create a **draft** GitHub release with both files. Steps:

1. Bump `version` in the root `Cargo.toml`, run the required checks, commit.
2. `git tag v0.2.0 && git push origin v0.2.0`
3. Check the draft release (notes, files), then publish it.

The icon is generated: `python3 packaging/icon/make_icon.py` writes `packaging/icon/` (`icon-1024.png`, `AppIcon.icns`, `icon.ico`) and `apps/pos-desktop/assets/icon-256.png` (window icon). `apps/pos-desktop/build.rs` embeds the icon and product name in `pos-heart.exe`.

## Signing (not set up yet)

Unsigned builds work but warn on first launch:

- **macOS:** the app is signed ad hoc. The first time, right-click (or Control-click) **POS Heart** in Applications → **Open** → **Open**. To remove the warning, get an Apple Developer ID (Apple Developer Program, about US$99/year), put the certificate in the runner keychain and set `MACOS_SIGN_IDENTITY`; with `APPLE_ID`, `APPLE_TEAM_ID` and `APPLE_APP_PASSWORD` the script also notarizes and staples the dmg.
- **Windows:** SmartScreen shows "Windows protected your PC" → **More info** → **Run anyway**. A code-signing certificate (OV/EV) removes it once it has reputation; sign `pos-heart.exe` and the setup with `signtool` before publishing.

## Computer requirements

- macOS 11 or later (Apple silicon or Intel); Windows 10/11 64-bit.
- 2 CPU cores and 4 GB RAM are enough (measured: a 2 vCPU / 4 GB Windows VM idles at a few percent CPU; on an Apple-silicon Mac the app used at most about 20% CPU and under 70 MB of memory).
- **A graphics adapter is needed for smooth use:** any integrated GPU (Intel UHD/Iris, AMD Radeon, Apple) with DirectX 12, Vulkan or Metal. Without one (cloud VMs, remote desktops) Windows draws the app on the CPU: it works, but CPU jumps while the screen changes.
- Screen at least 1000×700 points (the app scales down to fit, e.g. 1366×768 at 125%).

## Installing and updating

- macOS: open the dmg, drag **POS Heart** to **Applications**. Windows: run the setup (per user, no administrator needed; an "all users" install is offered).
- Shop data is not inside the app: it lives in the app-data folder (Settings → Data & backup shows where), so reinstalling or updating keeps it. Uninstalling on Windows also keeps it.
- To update, install the newer version over the old one (on Windows the installer closes a running POS Heart). When an update changes the database schema, the first launch keeps the old database as `pos-heart.sqlite3.pre-v<old schema>.sqlite3` next to it before migrating, so the update can be undone by restoring that file with the older app. An older app refuses a newer database rather than damaging it.
- macOS asks once for **Local Network** access when shop devices are used (Settings → Devices & cloud); allow it.

## Not yet

- Automatic update checks inside the app (the repository is private; a download location such as the cloud server is needed first).
- Linux packages.
