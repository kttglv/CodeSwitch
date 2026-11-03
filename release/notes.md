CodeSwitch provides offline English ↔ Russian keyboard-layout correction.

System-wide automatic correction stays disabled in this release until its typing
accuracy and platform validation are complete; a later update will enable it.

This release adds optional automatic update checks and signed in-app updates for
Apple Silicon Macs, Intel Macs and Windows 11 x64. Installation happens when you
choose Update and restart. Typed text is never included in update requests.

macOS input-source calls now run on the main queue to fix the reported HIToolbox
crash. See the repository's implementation status and compatibility documentation
for validation results and supported contexts.
