# NexaTerm developer environment (PowerShell).
# Dot-source before building:  . .\scripts\devenv.ps1
#
# Defaults assume the portable per-user toolchain layout used on the reference
# build machine (no admin rights needed):
#   D:\devtools\rust     Rust stable, x86_64-pc-windows-gnu host
#   D:\devtools\mingw64  WinLibs MinGW-w64 GCC (msvcrt) - C compiler for ring / SQLite
# Override with NEXATERM_DEVTOOLS if your toolchain lives elsewhere.
# If you use rustup + MSVC Build Tools instead, you do not need this script.

$nxDevtools = if ($env:NEXATERM_DEVTOOLS) { $env:NEXATERM_DEVTOOLS } else { 'D:\devtools' }
$env:PATH = "$nxDevtools\rust\bin;$nxDevtools\mingw64\bin;$env:PATH"
if (-not $env:CARGO_HOME) { $env:CARGO_HOME = "$nxDevtools\cargo" }
# Keep large temporary build artifacts off small system drives.
New-Item -ItemType Directory -Force "$nxDevtools\tmp" | Out-Null
$env:TEMP = "$nxDevtools\tmp"
$env:TMP = "$nxDevtools\tmp"
Write-Host "NexaTerm dev env: rustc $((rustc -V) -replace 'rustc ','') | gcc $((gcc -dumpversion))"
