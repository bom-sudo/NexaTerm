# NexaTerm developer environment (Git Bash). Usage:  source scripts/devenv.sh
ROOT="${NEXATERM_DEVTOOLS:-/d/devtools}"
export PATH="$ROOT/rust/bin:$ROOT/mingw64/bin:$PATH"
export CARGO_HOME="${CARGO_HOME:-$ROOT/cargo}"
mkdir -p "$ROOT/tmp"
export TEMP="$ROOT/tmp" TMP="$ROOT/tmp"
