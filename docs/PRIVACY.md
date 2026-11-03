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
No document contents, clipboard contents, or window titles are written to logs or
settings. Typed words reach settings only through the never-correct list described
below. Error messages describe a condition, never the text; a message may name the
app identifier where an edit could not be verified.

The current word, an immediately reversible correction, the apps paused after an
unverified edit and a count of corrections since launch are retained only in
memory. The count is shown in the tray tooltip and the app window; it is not saved. Typing, caret movement, focus changes, selections, paste shortcuts,
unsupported layouts and missing events invalidate the relevant state. Excluded
apps never enter the detection engine. Transient native snapshots are released
when processing finishes. Memory is not guaranteed to be securely zeroized.

Password/secure controls are skipped. macOS Secure Event Input blocks interception.
On Windows, the focused provider must explicitly expose a false password property
and an editable text range; unavailable properties result in no correction.
The correction service does not request administrator privileges, bypass secure
input, or modify elevated applications. Updating a system-owned macOS installation
may ask the operating system for installation authorization.

Settings contain only your enable/pause preference, onboarding/preview choice,
startup preference, update-check preference, undo and convert shortcuts,
application rules, the learn-from-undo preference and your list of words to leave
alone. They are stored in the per-user application configuration directory as
`settings.json`.

The word list is the only typed text CodeSwitch writes to disk. It holds words you
add on the Words page and, while **Learn from undo** is on (the default), each word
whose automatic correction you undo, in lowercase. It never holds surrounding text,
is capped at 500 words (oldest dropped first) and is never sent over the network.
Remove words individually, clear the list, or turn off learning on the Words page. Exit CodeSwitch
and remove this file to reset settings. Use Preferences to disable launch at login
before uninstalling.
