use std::fs;

fn main() {
    // Copy WebView2 bootstrapper next to the exe for portable builds
    let resources_dir = std::env::current_dir()
        .map(|d| d.join("resources"))
        .unwrap_or_default();
    let bootstrapper = resources_dir.join("MicrosoftEdgeWebview2Setup.exe");

    if bootstrapper.exists() {
        let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
        // Also copy to target/debug/release for cargo builds
        if let Some(target_dir) = std::path::Path::new(&out_dir).ancestors().find(|a| {
            a.join("debug").exists() || a.join("release").exists()
        }) {
            let _ = fs::copy(&bootstrapper, target_dir.join("MicrosoftEdgeWebview2Setup.exe"));
        }
    }

    tauri_build::build()
}
