//! Build script.
//!
//! Tauri would embed its application manifest (Common Controls v6, DPI awareness) only into the
//! app binary. Test harnesses then lack the Common-Controls-v6 dependency and die at load time
//! with STATUS_ENTRYPOINT_NOT_FOUND (TaskDialogIndirect). We therefore disable Tauri's manifest
//! and embed one manifest into every linked artifact ourselves.

use std::path::PathBuf;

fn main() {
    let attrs = tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attrs).expect("failed to run tauri-build");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = embed_manifest::new_manifest("NexaTerm");
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
            embed_gnu(&manifest.to_string());
        } else {
            // MSVC: embed-manifest passes /MANIFEST:EMBED to the linker.
            embed_manifest::embed_manifest(manifest).expect("unable to embed manifest");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}

/// GNU toolchains: compile the manifest into a COFF resource object with windres (already a
/// requirement of tauri-build on GNU) and link it into bins *and* test harnesses exactly once.
fn embed_gnu(xml: &str) {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("app.manifest"), xml).unwrap();
    // 1 = CREATEPROCESS_MANIFEST_RESOURCE_ID, 24 = RT_MANIFEST
    std::fs::write(out.join("manifest.rc"), "1 24 \"app.manifest\"\n").unwrap();
    let obj = out.join("nexaterm-manifest.o");
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "windres".into());
    let status = std::process::Command::new(&windres)
        .current_dir(&out)
        .args(["manifest.rc", "-O", "coff", "-o"])
        .arg(&obj)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {windres} (install MinGW binutils or set WINDRES): {e}"));
    assert!(status.success(), "windres failed to compile the manifest");
    println!("cargo:rustc-link-arg={}", obj.display());
}
