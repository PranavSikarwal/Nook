# Windows MSI package verification fix plan

## Purpose

Fix the Windows package inspection in the release workflow so it verifies the
files that an MSI installs instead of relying on an incomplete archive
extraction.

## Steps

1. Reproduce the failed Windows verification from Package and Release run #22.
2. Replace the MSI `7z` inspection with a Windows Installer administrative
   extraction.
3. Verify the extracted package contains `nookd.exe` and `release_entry.exe`.
4. Dispatch the package workflow for PR #29's current head and inspect all
   platform results before release.

## Done condition

The macOS DMG, Ubuntu `.deb`, and Windows MSI package checks pass, and their
artifacts are available from the workflow run.
