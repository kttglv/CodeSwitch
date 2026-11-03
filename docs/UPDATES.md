# App updates

CodeSwitch checks GitHub Releases 30 seconds after startup and then at most once
per 24 hours while running. Preferences can disable automatic checks. A manual
check is available in Preferences and the tray/menu bar. Closing the settings
window does not stop background checks.

An available release appears in the app. **Update and restart** downloads its
package, verifies its signature and signed version, waits for the input worker
to finish any pending correction, then installs and restarts. Download failures
leave correction running; installation failures release the update pause.
The updater does not install while the user is typing without an install action.

The webview cannot supply an endpoint, signing key, package URL or version to the
backend. Only HTTPS assets under `kttglv/CodeSwitch/releases/download/` are accepted.
The Tauri updater verifies the package before installation, rejects modified
packages, and requires the signature's version to match the feed. Lower/equal
versions are not installed. Release notes render as text, without HTML execution.

Detection stays offline. Update checks send no typed words, field contents or app
rules. See [privacy](PRIVACY.md) for request metadata and the opt-out setting.

## First installation and availability

Older builds without an updater must be replaced manually once. An updater-enabled
build can receive subsequent published releases with higher version numbers.

The feed is `https://github.com/kttglv/CodeSwitch/releases/latest/download/latest.json`.
It requires a publicly readable GitHub repository and a published release containing
all three signed packages and `latest.json`. A private repository or draft release
is unavailable to anonymous clients. The app reports an unavailable feed rather
than pretending it is up to date. No access token is embedded in the app.

The current local feature does not publish a release, upload private keys, change
repository visibility or bypass the existing public-release validation gate.

## Signing key

The public key is committed in `release/updater.pub` and `src-tauri/tauri.conf.json`.
The initial encrypted private key is stored locally at `.codeswitch-signing/updater.key`;
its password is in `.codeswitch-signing/updater-password.txt`. That directory is
ignored by Git, mode 0700, with secret files mode 0600. Back up both secrets in
a private password manager or equivalent secure storage. Never commit or publish
them. Losing the key prevents signing updates for already installed clients.

Configure the **release environment** in GitHub Actions with:

- `TAURI_SIGNING_PRIVATE_KEY`: contents of the encrypted private key file.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password.
- The macOS Developer ID/notarization and Windows publisher credentials listed in
  [the release guide](RELEASING.md).

Updater signatures authenticate packages; they do not replace Developer ID signing,
notarization or Windows Authenticode. Ad-hoc macOS development builds change identity
on rebuild and may require the Accessibility permission entry to be renewed.

## Release workflow

1. Bump the version consistently in `Cargo.toml`, `package.json`/lockfile and
   `src-tauri/tauri.conf.json`. Refresh `Cargo.lock` and release evidence.
2. Update `release/notes.md`, complete the validation/signing prerequisites and
   run **Signed release candidates** manually on the reviewed commit.
3. Each platform builds with `tauri.release.conf.json`, generating an updater
   package and a version-bound `.sig`. Mac archives are renamed with architecture
   suffixes, so Intel and Apple Silicon assets cannot overwrite each other.
4. `scripts/update_manifest.py` checks all three packages/signatures, generates
   platform URLs in `latest.json` and stages a draft release with the installers,
   update packages and dependency notices. An existing release tag causes failure
   rather than silently replacing it.
5. Review the draft, test a real upgrade from the preceding version on both Mac
   architectures and Windows 11, then publish. Keep versioned assets immutable.

The release gate remains closed until the original typing accuracy and native
compatibility requirements pass. Preview CI builds do not need the private signing
key and do not publish update feeds.

## Checks

Rust tests cover old-settings migration, check throttling, concurrent-operation
exclusion, pinned download origins and real signature verification using a disposable
test key. Valid fixtures pass; changed bytes and a falsified version fail, without
network or installation. Python tests cover manifest platforms, distinct asset names,
missing packages, invalid signatures and inconsistent versions.

Before public release, also verify real download progress, offline recovery,
interrupted installation, worker quiescence and settings/permissions after restart
on actual macOS and Windows systems. Automated fixture tests do not replace these.
