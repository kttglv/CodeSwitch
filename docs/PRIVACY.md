# Privacy

CodeSwitch performs language-layout detection locally. There are no accounts,
analytics or cloud models. Optional automatic update checks contact GitHub Releases
at startup and once per day. You can disable these checks in Preferences; manual
checks remain available. Packages download only after you choose Update and restart.
The updater validates the package signature and its signed version before installation.
GitHub and its download providers receive ordinary network metadata such as your
IP address, platform and request headers. Typed text, document contents, app rules
and settings are never included in these requests. No GitHub token is embedded.
An installer may download the Microsoft WebView2 runtime if it is not installed.

The native keyboard hook sees input events supplied by the operating system.
Events are passed through a bounded, temporary in-memory queue. Only text from
a verified editable, unprotected field enters the current-word buffer. Native
accessibility reads retrieve at most 192 UTF-16 units immediately before the
caret for verification; full documents and clipboard contents are not read.
No typed words, document contents, clipboard contents, or window titles are
written to logs or settings. Error messages describe a condition, never the text.

The current word and an immediately reversible correction are retained only in
memory. Typing, caret movement, focus changes, selections, paste shortcuts,
unsupported layouts and missing events invalidate the relevant state. Excluded
apps never enter the detection engine. Transient native snapshots are released
when processing finishes. Memory is not guaranteed to be securely zeroized.

Password/secure controls are skipped. macOS Secure Event Input blocks interception.
On Windows, the focused provider must explicitly expose a false password property
and an editable text range; unavailable properties result in no correction.
The app does not request administrator privileges, bypass secure input, or modify
elevated applications.

Settings contain only your enable/pause preference, onboarding/preview choice,
startup preference, update-check preference, undo shortcut, and application rules. They are stored in the
per-user application configuration directory as `settings.json`. Exit CodeSwitch
and remove this file to reset settings. Use Preferences to disable launch at login
before uninstalling.
