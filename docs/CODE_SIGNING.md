# Code signing

Windows shows "Unknown publisher" and SmartScreen warnings for unsigned programs. NexaTerm
releases are meant to be Authenticode-signed. There are two supported ways to do it; both sign
the application exe, the NSIS uninstaller and both installers, and both are checked by
`scripts/verify-release.ps1 -RequireSignature` (status must be `Valid` and timestamped).

Releases built before signing was set up (including the first 1.0.0 build) are unsigned.

## Option 1: SignPath Foundation (free for open-source projects, recommended)

[SignPath Foundation](https://signpath.org/) provides free code signing certificates to open
source projects. The certificate is issued to SignPath Foundation and the private key lives in
their HSM; signing happens from GitHub Actions on GitHub-hosted runners, so the signed binaries
are provably built from this repository's source. No USB token has to be bought.

### 1. Apply

1. Read the conditions at <https://signpath.org/terms> (OSI license: MIT is fine; the project
   must be actively maintained, released and publicly built from source; no malware/PUP
   behavior).
2. Apply at <https://signpath.org/apply> with the repository URL
   `https://github.com/bom-sudo/NexaTerm`.
3. Wait for approval. SignPath creates an organization and a project for NexaTerm.

### 2. Configure SignPath (in the SignPath web UI)

1. Install the SignPath GitHub App / add GitHub as a trusted build system for the project.
2. Create two **artifact configurations** with these slugs, pasting the XML from the repository:
   - `exe` - [.signpath/artifact-configurations/exe.xml](../.signpath/artifact-configurations/exe.xml)
   - `installers` - [.signpath/artifact-configurations/installers.xml](../.signpath/artifact-configurations/installers.xml)
3. Signing policies: SignPath creates `test-signing` (self-signed test certificate, for trying
   the pipeline) and `release-signing` (the real certificate; each request needs manual approval
   by an approver of the project).
4. Create a CI user / API token for the project with submitter permission.

### 3. Configure the GitHub repository

Settings > Secrets and variables > Actions:

| Kind     | Name                       | Value                                  |
|----------|----------------------------|----------------------------------------|
| Secret   | `SIGNPATH_API_TOKEN`       | API token of the SignPath CI user       |
| Variable | `SIGNPATH_ORGANIZATION_ID` | Organization ID (GUID) from SignPath    |
| Variable | `SIGNPATH_PROJECT_SLUG`    | Project slug, if not `NexaTerm`         |

### 4. Release

- Push a tag (`git tag v1.0.1 && git push origin v1.0.1`) or run the **Release** workflow
  manually (choose `test-signing` first to try the pipeline).
- The workflow builds and tests from source, sends `nexaterm.exe` to SignPath, packages the
  installers and portable zip with the signed exe, sends both installers to SignPath, verifies
  every signature and drafts a GitHub release with Windows and Linux files plus `SHA256SUMS.txt`.
- With `release-signing`, approve each signing request in SignPath (you get an e-mail).

Without the secret/variable the workflow still runs and produces an unsigned release, with a
warning on the run page.

### Code signing policy (required by SignPath Foundation)

The README carries the policy text. In short: free code signing is provided by
[SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/); only binaries built by the Release workflow from
this repository are signed; maintainers of the repository are the committers, reviewers and
approvers; NexaTerm does not transfer any data to other systems unless the user explicitly
asks for it (see [SECURITY.md](../SECURITY.md)).

## Option 2: your own certificate (local build)

If you own a code signing certificate (OV/EV from a CA, often on a USB token or in a cloud
HSM):

1. Install the Windows SDK "Signing Tools for Desktop Apps" to get `signtool.exe`
   (or set `NEXATERM_SIGNTOOL` to its path).
2. Make the certificate available:
   - Certificate store / token: set `NEXATERM_SIGN_THUMBPRINT` to the certificate's SHA-1
     thumbprint (`Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert`).
   - PFX file: set `NEXATERM_SIGN_PFX` and `NEXATERM_SIGN_PFX_PASSWORD`.
3. Build with signing:

   ```powershell
   $env:NEXATERM_SIGN_THUMBPRINT = "<thumbprint>"
   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Sign
   ```

   The Tauri bundler calls `scripts\sign-file.ps1` (through `bundle.windows.signCommand`) for the
   exe, the uninstaller and the installers; timestamps come from `http://timestamp.digicert.com`
   (override with `NEXATERM_SIGN_TIMESTAMP_URL`). The build fails if anything ends up unsigned.

Nothing secret is written to `tauri.conf.json` or the repository: the signing configuration is
generated at build time into `build\sign.conf.json` (ignored by git) and the certificate comes
only from environment variables.

## What signing does and does not fix

- Signed files show the publisher name instead of "Unknown publisher", and Windows can tell the
  file was not modified after signing.
- SmartScreen reputation is earned per certificate over time and downloads; a new certificate can
  still show "Windows protected your PC" for the first releases. It goes away as the signed
  releases are downloaded.
- A self-signed certificate does not help other users; Windows only trusts certificates from
  public CAs (SignPath Foundation's is one).
