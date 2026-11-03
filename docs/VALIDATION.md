# Native validation checklist

These checks must be performed on actual systems. Compilation or simulated events
do not constitute end-to-end native validation. Record OS/app versions, build
source fingerprint and pass/fail results; never include private typing in reports.

Run on macOS Apple Silicon, macOS Intel, and Windows 11 x64. Start with a debug
build, acknowledge the development preview, complete setup, and enable correction.

## Application matrix

| Platform | Native editor | Browser | Desktop messaging |
| --- | --- | --- | --- |
| macOS | TextEdit, plain and rich text | Safari and Chrome input/textarea/contenteditable | Telegram Desktop |
| Windows 11 | Notepad | Edge and Chrome input/textarea/contenteditable | Telegram Desktop |

An app/control is supported only if it exposes an editable, unprotected range and
accepts synthetic Unicode input and a standard layout change. Mark unsupported
controls explicitly; never fall back to blind deletion or clipboard replacement.

## Required scenarios

- Type `ghbdtn ` under US: verify `привет ` and Russian layout; type the next word.
- Type `руддщ ` under Russian: verify `hello ` and US layout.
- Repeat with title case and all caps; check `k.,jdm ` → `любовь `, `ё` mapping,
  surrounding quotes/parentheses, and punctuation preserved exactly.
  English punctuation keys that could be Russian letters at a word's edge are
  deliberately left ambiguous rather than stripped (for example `ghbdtn.`).
- Ambiguous punctuation keys (comma/period/semicolon/quote/brackets) remain part
  of the possible Russian word; they do not prematurely split it. Space completes
  the run. Unambiguous punctuation may complete a word earlier.
- Valid/ambiguous words, short words, mixed case, identifiers, email and URLs stay
  unchanged. Enter and Tab do not trigger correction or cause a second submission.
- Press the configured undo immediately: verify original text and previous layout.
  Repeat while modifiers remain held. Verify undo is unavailable after new typing,
  selection, mouse click, navigation, focus change or manual layout change.
- Use Backspace mid-word; paste via keyboard/context menu; select then type; move
  the caret; switch apps or tabs mid-word. No stale text should be replaced.
- Type quickly while a correction is pending. No interleaved, duplicated or dropped
  characters should occur. Repeat with a mouse click during correction and with
  focus changed programmatically. Input must not be replayed into an unverified field.
- Check password fields, password managers, read-only fields, missing accessibility
  properties, IMEs, remote desktops and (Windows) elevated apps. Expect no correction.
- Remove permissions or a layout while running; restore them. Check hook recovery,
  pause status, setup guidance and lack of unintended capture/replacement.
- Check terminal/editor default exclusions, explicit allow overrides and explicit
  exclusions taking precedence. A custom unsupported editor still stays unsupported.
- Save/reload preferences, configure undo, enable/disable launch at login, close
  to tray, quit, relaunch, and reset settings. Check standard-user installation.
- Check automatic-update opt-out and manual checks, offline errors, package-signature
  rejection, progress, upgrade/restart, settings preservation and concurrent typing
  during download. Verify a pending correction completes before installation.
- Leave the utility idle and type normally for an extended session. Check memory,
  responsiveness, idle CPU and hook health. Verify no typed text is logged or sent
  over the network.

## Current limitations

Only standard US and Russian layouts are supported. The detector intentionally
misses short/rare/ambiguous words. Wordfreq reflects historical language usage.
Custom accessibility providers, secure input, administrator windows, composition,
games and remote input may be unsupported. Automatic app spellchecking can alter
the visible text before verification, in which case correction is skipped.

Keyboard hooks and accessibility changes cannot provide a transaction across
arbitrary application state. CodeSwitch revalidates the field/caret/suffix, queues
concurrent physical input briefly, and pauses on an unverifiable result. Review
these races on actual systems before public approval; the release gate starts closed.
