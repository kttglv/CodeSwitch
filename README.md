# CodeSwitch

English and Russian, without the wrong keyboard layout.

CodeSwitch is an offline menu bar/system tray utility for macOS and Windows. Type
`ghbdtn ` using the US layout and it can replace it with `привет `, then switch
your active layout to Russian. It also handles the reverse direction.

**Status: development preview.** The correction engine, native adapters, Tauri
settings UI and validation tooling are implemented. Public automatic correction
is intentionally gated until independently annotated precision measurements and
real-app tests on both platforms pass. This repository does not claim a validated
99.5% precision result or completed native compatibility testing.

## Run

Install Node.js 22.12+ and stable Rust, plus the platform prerequisites described
in [the build guide](docs/RELEASING.md).

```sh
npm ci
npm run tauri dev
```

Add **U.S.** (or **ABC** on macOS) and **standard Russian** (**Russian — PC** on macOS)
keyboard layouts. On macOS, grant the app
Accessibility permission using its setup screen. A debug build offers an explicit
development-preview opt-in; correction starts paused. Use the test box to inspect
the detector without enabling system-wide changes.

## Behavior

- English ↔ Russian, with case and surrounding punctuation preserved.
- Conservative offline word-frequency detection; uncertain/short words are skipped.
- Correction at spaces and unambiguous punctuation; Enter/Tab pass through.
- Immediate undo: **Option–Command–Backspace** on macOS,
  **Ctrl–Alt–Backspace** on Windows; configurable in Preferences.
- Pause, launch at login, and per-app rules. Terminals/editors are excluded by default.
- Verified editable fields only. Secure/unsupported fields and elevated Windows
  apps are skipped; the clipboard is never used.
- No accounts, telemetry, cloud inference, typing history or automatic updater.

## Project structure

`crates/engine` contains detection, layout mapping, session/undo state and the
benchmark CLI. `crates/platform` contains the C ABI bridge and native macOS/Windows
integrations. `src-tauri` contains the application, worker and settings; `src`
contains its TypeScript/CSS interface. Original licensed resources live under
`resources/wordfreq`.

```sh
npm run build
cargo test --workspace --locked
python3 scripts/benchmark.py benchmarks/smoke.jsonl benchmarks/smoke-report.json
```

See [privacy](docs/PRIVACY.md), [native validation](docs/VALIDATION.md),
[benchmark methodology](benchmarks/README.md), and [releases](docs/RELEASING.md).
The [implementation status](docs/IMPLEMENTATION_STATUS.md) records the checks
actually performed and the remaining release requirements.

Original app code: [MIT](LICENSE). Frequency data and derived lookup implementation
retain their upstream licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).
