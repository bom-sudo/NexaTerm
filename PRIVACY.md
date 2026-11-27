# Privacy policy

NexaTerm does not collect, store or send any personal data or usage data to the NexaTerm
project or anyone else. There is no account, no telemetry, no analytics, no crash reporting and
no automatic update check.

## What the application connects to

NexaTerm only opens network connections that you ask for:

* the servers you connect to (SSH, SFTP, Telnet, tunnels) and the hosts you test with the
  network tools (ping, traceroute, DNS, TCP port test);
* `api.ipify.org`, only when you click **Public IP** in the network tools, to show your public
  IP address;
* `api.github.com` and `github.com`, only when you click **Check for updates** (in Settings or on
  the version number in the status bar), to read the latest release and, if you choose to
  install it, download the installer and its SHA-256 checksum;
* the small Windows installer (`NexaTerm-Setup-x64-web.exe`) downloads the Microsoft WebView2
  Runtime from Microsoft if it is not already installed.

## What is stored on your computer

* Sessions, settings, snippets, tunnels and known host keys are stored in a local SQLite
  database (`%APPDATA%\NexaTerm` on Windows, `~/.config/NexaTerm` on Linux, or the
  `portable-data` folder of the portable edition).
* Passwords and key passphrases you choose to save are stored only in the operating system's
  credential store (Windows Credential Manager, or the Secret Service keyring on Linux). They are
  never written to the database, logs, exports or backups.
* The application log (`logs` folder) redacts passwords before writing.

All of this stays on your computer. You can delete it at any time; see
[docs/USER_GUIDE.md](docs/USER_GUIDE.md) or the uninstaller.

## Website

The website `nexaterm.pages.dev` is a static site hosted on Cloudflare Pages and loads fonts from
Google Fonts. It uses no cookies, analytics or tracking scripts. Cloudflare and Google may keep
standard server logs of requests under their own privacy policies. The language choice on the
site is remembered in your browser's local storage only.

## Code signing

Windows release files are code-signed through SignPath Foundation once the project's
application is approved. Signing happens on SignPath's servers from files built by GitHub
Actions; no user data is involved.

## Contact

Questions about this policy: open an issue at <https://github.com/bom-sudo/NexaTerm/issues>.
