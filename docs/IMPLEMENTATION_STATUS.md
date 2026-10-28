# Implementation status — October 28, 2026

CodeSwitch 0.1.0 is a **development preview**, not a validated public release.

## Completed and checked locally

- Rust detector, standard US/ABC ↔ Russian-PC mapping, conservative boundary
  handling, typing state, stale-state rejection and immediate undo logic.
- Native macOS and Windows adapters, bounded input serialization, verification,
  protected-field checks, app exclusions and local settings.
- Tauri settings/onboarding, tray controls, startup preference, shortcut validation
  and an isolated word test box. No accounts, cloud inference or typing logs.
- macOS arm64 app/DMG builds; macOS x64 cross-compilation check passes.
- 11 Rust tests and 6 Python release-gate tests pass; Clippy and TypeScript/Vite
  production build pass. Bundled original wordfreq resource checksums pass.
- Desktop UI tested on this Mac: both conversion directions in the word test box,
  uppercase preservation, layout recognition, invalid-shortcut rejection, valid
  preference saving, launch and quit. Browser interface visually inspected.
- Explicit macOS preview app-bundle ad-hoc signature passes
  `codesign --verify --deep --strict`. This is not Developer ID signing/notarization.
- macOS arm64/x64 and Windows x64 build/test workflows, release approval gate,
  credential-based signing workflows and dependency license inventory are supplied.

## Engineering evaluation

The frozen `conservative-v2` detector was evaluated on 9,000 previously unused,
deterministically selected cases, excluding smoke/development vocabulary:

| Measure | Result |
| --- | ---: |
| Correct corrections | 1,130 |
| Incorrect corrections | 0 |
| Correctly unchanged negative cases | 6,000 |
| Missed synthetic corrections | 1,870 |
| Precision in this sample | 100% |
| Recall in this sample | 37.67% |

This vocabulary-weighted evaluation deliberately includes unfamiliar words.
Positive wrong-layout inputs are synthetic, derived from published UD tokens.
It is **not** a real-world precision guarantee or an independently reviewed
natural bilingual typing study. See `benchmarks/report.json`, corpus provenance,
sampling script, licenses and methodology. The original sample exposed a word-end
punctuation defect; it is now development data with a retained historical baseline.

## Remaining before public release

- Live system-wide macOS correction/undo, secure-field and concurrency validation.
  Accessibility access was requested, but the app continued to report it unavailable.
  Computer-use access to System Settings was not approved. The user chose to finish
  without further live keyboard testing. Global correction remains paused.
- Actual Intel Mac and Windows 11 runtime validation, including the documented
  application matrix. Windows adapter compilation is configured in CI but was not
  executed in this Mac-only environment.
- Independent review/evaluation of natural wrong-layout typing, platform approvals,
  Developer ID notarization and Windows publisher signing using maintainer credentials.
- Attach reviewed, signed installers to a GitHub Release. No repository or release
  was created or published during this implementation.

`release/readiness.json` remains unapproved. Production builds cannot enable global
automatic correction until the release gate passes. Debug builds offer an explicit
development-preview opt-in after permissions and onboarding are complete.
