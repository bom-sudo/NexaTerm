# winget manifests

`manifests/n/NexaTerm/NexaTerm/<version>/` is the package as submitted to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) (`winget install NexaTerm.NexaTerm`).
The installer is the small web installer from the GitHub release; it is per-user (`Scope: user`)
and installs silently with `/S`, which winget uses automatically for `nullsoft` installers.

## First submission (once)

1. Create a classic GitHub token with the `public_repo` scope (GitHub, Settings, Developer settings,
   Personal access tokens, Tokens (classic)).
2. Download wingetcreate: <https://aka.ms/wingetcreate/latest> (`wingetcreate.exe`).
3. From the repository root:

   ```powershell
   .\wingetcreate.exe submit --prtitle "New package: NexaTerm.NexaTerm version 1.1.0" --token <TOKEN> packaging\winget\manifests\n\NexaTerm\NexaTerm\1.1.0
   ```

   This forks winget-pkgs under your account and opens the pull request. Microsoft's bots install
   the package in a sandbox and scan it; a moderator merges it, usually within a few days.

## Later versions

Add the same token as the repository secret `WINGET_TOKEN`. The `winget` job in
`.github/workflows/release.yml` then opens the update pull request for every new release tag.
