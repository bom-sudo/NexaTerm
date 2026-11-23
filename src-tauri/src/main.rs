// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Headless verification of an installed / portable build: `NexaTerm.exe --self-test > report.txt`
    if std::env::args().any(|a| a == "--self-test") {
        std::process::exit(nexaterm_lib::selftest::run());
    }
    nexaterm_lib::run()
}
