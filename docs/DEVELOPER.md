# Developer Guide

## Toolchain

| Tool | Version used | Notes |
|---|---|---|
| Node.js | 22 | npm 10 |
| Rust | 1.100 stable | `x86_64-pc-windows-gnu` **or** `-msvc` |
| C compiler | WinLibs GCC 16 (MinGW-w64, msvcrt) or MSVC Build Tools | needed by `ring` and bundled SQLite |
| windres | from MinGW binutils | GNU builds only (resources + manifest) |

### Option A — standard (MSVC, needs admin)
Install rustup with the MSVC toolchain and *Visual Studio Build Tools* ("Desktop development with C++").

### Option B — no admin rights (used for the reference build)
Everything lives in one folder (default `D:\devtools`, override with `NEXATERM_DEVTOOLS`):

1. Rust standalone tarball `rust-<ver>-x86_64-pc-windows-gnu.tar.xz` from static.rust-lang.org,
   installed with `sh install.sh --prefix=/d/devtools/rust` (from Git Bash).
2. WinLibs `winlibs-x86_64-posix-seh-gcc-*-mingw-w64msvcrt-*.zip` unzipped to `D:\devtools\mingw64`.
3. `. .\scripts\devenv.ps1` (PowerShell) or `source scripts/devenv.sh` (Git Bash) puts both on PATH and
   redirects TEMP to the devtools drive.

## Commands

```powershell
. .\scripts\devenv.ps1           # only for Option B
npm install
npm run tauri dev                # run the app with hot reload
npm run typecheck                # TypeScript
npm test                         # Vitest (run from a path with an upper-case drive letter, see below)
npm run test:rust                # cargo test: unit + integration (mock SSH/SFTP server)
npm run build:installer          # release build + NSIS installer
npm run build:release            # full pipeline -> dist\release\v<version>\ (installers, portable zip, checksums)
npm run verify:release           # verify that folder, incl. self-test of the portable exe
powershell -File scripts\capture-screenshots.ps1   # regenerate docs\images (real app + mock server, throw-away data)
node scripts/gen-licenses.mjs    # regenerate THIRD_PARTY_LICENSES.md
cargo run --manifest-path src-tauri/Cargo.toml --example mock_sshd -- 2222   # local SSH/SFTP test server (tester/secret)
```

## Gotchas found while building

* **Memory:** the dependency graph is large. `src-tauri/.cargo/config.toml` limits cargo to 2 jobs so
  8 GB machines do not run out of commit memory. Raise it on bigger hosts.
* **GNU target + cdylib:** MinGW's linker cannot export the Tauri lib as a cdylib (export limit), so the
  library is built as `rlib` only (cdylib/staticlib are only needed for mobile).
* **Manifest:** `build.rs` disables Tauri's app manifest and embeds one (Common Controls v6, PerMonitorV2)
  into every artifact, including test binaries; otherwise tests fail with `STATUS_ENTRYPOINT_NOT_FOUND`.
  MinGW also links a `default-manifest.o`, so `ld` prints a harmless "multiple non-default manifests"
  warning; ours is the one that takes effect.
* **WebView2Loader.dll:** GNU builds load it dynamically; it must ship next to the exe (the NSIS bundle and
  `build-windows.ps1` include it).
* **Vitest on Windows:** if the shell's cwd uses a lower-case drive letter (`d:\`), Vitest may load two
  copies of itself ("Cannot read properties of undefined (reading 'config')"). Run from `D:\…`.
* **xterm.js** is incompatible with Tauri's `freezePrototype`, so it is off.
* **ConPTY** asks the terminal for the cursor position (`ESC[6n`) at startup and waits; xterm.js answers it.

## Code map

* `src-tauri/src/` — see [ARCHITECTURE.md](ARCHITECTURE.md). Entry: `lib.rs` (`run`, command registry).
* `src-tauri/migrations/` — append-only SQL migrations, registered in `database/mod.rs`.
* `src-tauri/tests/` — integration tests and the mock server (`common/mock_server.rs`).
* `src/services/api.ts` — every IPC call, typed. `src/store/` — zustand stores.
* `src/features/*` — one folder per module; `src/layouts/` — shell UI; `src/app/` — actions/shortcuts.

## Adding a protocol adapter (RDP/VNC/Telnet/Serial)

1. Implement `terminal::TerminalBackend` (text protocols) or a new tab kind (graphical protocols).
2. Add a `terminal_open_<proto>` command, an `api.ts` wrapper and a `PaneSpec` kind.
3. Enable the protocol in `SessionDialog` only once it works end to end.
4. For external components (FreeRDP, X servers), never bundle or auto-download GPL software; detect a
   user-installed copy and document licensing in THIRD_PARTY_LICENSES.md.
