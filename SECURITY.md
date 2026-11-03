# Security Policy

CodeSwitch observes keystrokes and edits text in other applications, so we take
security reports seriously.

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Instead, report them
privately through GitHub:
[Report a vulnerability](https://github.com/kttglv/CodeSwitch/security/advisories/new).

Include the affected version, platform (macOS or Windows), steps to reproduce,
and the impact you observed. You should receive an acknowledgement within a few
days.

## Scope

Of particular interest:

- Any path where typed text, clipboard contents or password-field input could be
  logged, persisted, or sent off the device (see [docs/PRIVACY.md](docs/PRIVACY.md)).
- Bypasses of the secure-input / password-field exclusions.
- Problems with update signature verification or the update feed.
