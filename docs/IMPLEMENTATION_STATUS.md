# Implementation status — November 3, 2026

CodeSwitch 0.1.0 is a **development preview**, not a validated public release.

## Completed and checked locally

- Rust detector, standard US/ABC ↔ Russian-PC mapping, conservative boundary
  handling, typing state, stale-state rejection and immediate undo logic.
- Native macOS and Windows adapters, bounded input serialization, verification,
  protected-field checks, app exclusions and local settings.
- Tauri settings/onboarding, tray controls, startup preference, shortcut validation
  and an isolated word test box. No accounts, cloud inference or typing logs.
- macOS arm64 and Intel app/DMG builds pass in GitHub Actions. An updated arm64
  preview is also built locally.
- 16 Rust tests and 12 Python checks pass, including a native macOS regression;
  Clippy and TypeScript/Vite
  production build pass. Bundled original wordfreq resource checksums pass.
- Desktop UI tested on this Mac: both conversion directions in the word test box,
  uppercase preservation, layout recognition, invalid-shortcut rejection, valid
  preference saving, launch and quit. Browser interface visually inspected.
- Explicit macOS preview app-bundle ad-hoc signature passes
  `codesign --verify --deep --strict`. This is not Developer ID signing/notarization.
- macOS arm64/x64 and Windows x64 build/test workflows, release approval gate,
  credential-based signing workflows and dependency license inventory are supplied.
- Windows CI builds/tests the native adapter after fixes to COM-header ordering
  and macOS-only launch-agent configuration. Inspection of the first successful
  installer identified an unbundled `MSVCP140.dll` dependency; Windows packages now
  enable Tauri's C++ runtime bundling.
- Undo shortcuts pass through when no immediately reversible correction exists.
  Shared native tests cover shortcut repeats, modifier release, configuration
  changes and pause. Replayed typing goes through normal buffer invalidation.
- Fixed the user-reported macOS 27.2 crash in `currentLayout()`: HIToolbox input
  source queries, enumeration and selection now execute on the main queue.
  The regression exercises main-thread and repeated worker-thread calls against
  real HIToolbox APIs without starting hooks, reading fields or switching layouts.
- Optional automatic update checks, manual checks in Preferences/tray, progress,
  verified packages and user-initiated install/restart. Old settings migrate without
  losing preferences. Installation waits for the correction worker to pause.
- Version-bound updater signatures, public-key pinning and three-platform manifest
  staging are implemented. The release workflow prepares a reviewable draft release.
  Private updater keys remain local; no keys were uploaded and no release published.

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

- Comprehensive live system-wide macOS correction/undo, secure-field and concurrency
  validation. The user originally skipped further live tests, then supplied the
  macOS 27.2 crash above after enabling the preview. The targeted regression passes;
  the complete real-app validation matrix remains unverified.
- Actual Intel Mac and Windows 11 runtime validation, including the documented
  application matrix. CI builds do not establish real-app typing compatibility.
- Independent review/evaluation of natural wrong-layout typing, platform approvals,
  Developer ID notarization and Windows publisher signing using maintainer credentials.
- Attach reviewed, signed installers to a GitHub Release. The private repository
  has a draft [implementation fix PR](https://github.com/kttglv/CodeSwitch/pull/1);
  no public release has been published.
- Configure updater signing secrets, make the feed publicly readable, publish the
  first reviewed signed release, and validate real upgrades on all target systems.
  See [updater setup](UPDATES.md). The updater cannot serve unpublished private assets.

`release/readiness.json` remains unapproved. Production builds cannot enable global
automatic correction until the release gate passes. Debug builds offer an explicit
development-preview opt-in after permissions and onboarding are complete.
