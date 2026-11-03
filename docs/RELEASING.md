# Building and releasing

## Local development

Install Node.js 22.12+ (or 24+) and stable Rust, Xcode Command Line Tools on macOS,
or Visual Studio Build Tools (Desktop development with C++, Windows SDK) on Windows.
Use a current Rust compiler; the lockfile is tested with Rust 1.99.1. Tauri 2 uses
the system WebKit on macOS and WebView2 on Windows.

```sh
npm ci
npm run tauri dev
```

On macOS use **U.S. or ABC** and **Russian — PC**. Apple's "Russian" and phonetic
variants are not identical to the standard PC map and are not supported in v1.

The native permission prompt is only opened by **Grant access**. Enable CodeSwitch
in macOS Accessibility, install the supported layouts, and restart the app if macOS
requests it. Finish setup and explicitly opt into the development preview to test
global correction. The test box works independently of global correction.

```sh
npm run build
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 scripts/benchmark.py benchmarks/smoke.jsonl benchmarks/smoke-report.json
npm run check:release
```

The last command deliberately fails until release evidence exists. `npm run dev`
alone previews the interface in a browser; it does not monitor your keyboard.

To bundle an unsigned/ad-hoc **development preview** on macOS:

```sh
npm run bundle:preview -- --bundles app,dmg
```

The preview configuration explicitly ad-hoc-signs the macOS app bundle so its
identity and Info.plist are bound for permission checks. A rebuilt ad-hoc bundle
may need its Accessibility permission granted again. This is not a Developer ID
signature and does not notarize the app.

Do not describe these as signed/notarized public releases. Debug builds have an
explicit experimental opt-in and may behave differently from optimized builds.

## CI

CI builds/tests macOS arm64, macOS x64 and Windows x64, checks resource hashes,
exports dependency notices, and uploads unsigned preview artifacts. The separate
manual release workflow requires the benchmark/manual release gate to pass before
building signed artifacts. Source, corpus and metadata fingerprints must match.
The signed release workflow collects all three platforms, creates `latest.json`
and stages a **draft** GitHub Release. A maintainer reviews and publishes that
draft; clients only see published releases. See [updater setup](UPDATES.md).

## Signing prerequisites

For a public release, configure environment-protected repository secrets:

- macOS: `APPLE_CERTIFICATE` (base64 Developer ID Application p12),
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
  `APPLE_PASSWORD` (app-specific), and `APPLE_TEAM_ID`.
- Windows: `WINDOWS_CERTIFICATE` (base64 code-signing pfx),
  `WINDOWS_CERTIFICATE_PASSWORD`. Certificates must permit the chosen CI signing
  mechanism. For a hardware-backed key use a suitable signing service instead.
- Both platforms: `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, matching the committed updater public key.
  These are separate from OS signing certificates. See [key management](UPDATES.md).

Never commit keys/certificates or bypass OS verification. The macOS workflow uses
Tauri signing, notarization and stapling; the Windows workflow signs the app and
NSIS installer with SHA-256 and a timestamp. Verify Authenticode/Gatekeeper results
on clean machines. Windows reputation warnings may remain despite valid signatures.

Ship the dependency license inventory and bundled wordfreq notices with installers.
Test installation, first-run permission guidance, launch at login and uninstall.
The first public build targets macOS 13+ (arm64/x64) and Windows 11 x64.

## Resource provenance

Original wordfreq files are committed so ordinary builds work offline after
dependencies are cached. `python3 scripts/fetch_wordfreq.py` reproduces the download
from the pinned upstream commit. Review a resource update as a detector change;
it invalidates release evidence. Data remains CC BY-SA 4.0; original/derived lookup
code retains Apache-2.0 notices. Original app code is MIT.
