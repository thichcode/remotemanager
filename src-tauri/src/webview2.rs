/// WebView2 detection and installation helper.
///
/// Checks if WebView2 Runtime is installed. If not, attempts to run the
/// bundled bootstrapper or shows a dialog with instructions.

#[cfg(windows)]
pub fn check_and_install_webview2() -> bool {
    if is_webview2_installed() {
        return true;
    }

    eprintln!("[webview2] Runtime not found, attempting install...");

    // Try to run the bundled bootstrapper silently
    if try_install_webview2() {
        // Re-check after install
        return is_webview2_installed();
    }

    // Show dialog explaining what to do
    show_webview2_dialog();
    false
}

#[cfg(windows)]
fn is_webview2_installed() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Check both 64-bit and 32-bit registry paths
    let paths = [
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-23456918E9FD}",
        r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-23456918E9FD}",
    ];

    for path in &paths {
        if let Ok(key) = hklm.open_subkey_with_flags(path, KEY_READ) {
            if let Ok(value) = key.get_value::<String, _>("pv") {
                if !value.is_empty() {
                    eprintln!("[webview2] Found version: {}", value);
                    return true;
                }
            }
        }
    }

    // Also check via file system — WebView2 runtime DLL
    let edge_dirs = [
        std::env::var("PROGRAMFILES(X86)")
            .map(|p| format!(r"{}\Microsoft\EdgeWebView\Application", p))
            .unwrap_or_default(),
        std::env::var("PROGRAMFILES")
            .map(|p| format!(r"{}\Microsoft\EdgeWebView\Application", p))
            .unwrap_or_default(),
    ];

    for dir in &edge_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().join("msedgewebview2.exe").exists() {
                    eprintln!("[webview2] Found at: {}", entry.path().display());
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(windows)]
fn try_install_webview2() -> bool {
    use std::process::Command;

    // Find the bootstrapper next to the exe
    let exe_path = std::env::current_exe().ok();
    let bootstrapper = exe_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|d| d.join("MicrosoftEdgeWebview2Setup.exe"))
        .filter(|p| p.exists());

    let bootstrapper = match bootstrapper {
        Some(b) => b,
        None => {
            eprintln!("[webview2] Bootstrapper not found next to exe");
            return false;
        }
    };

    eprintln!("[webview2] Running bootstrapper: {}", bootstrapper.display());

    let result = Command::new(&bootstrapper)
        .args(["/silent", "/install"])
        .spawn();

    match result {
        Ok(mut child) => {
            // Wait up to 60 seconds for install
            let _ = child.wait();
            eprintln!("[webview2] Bootstrapper completed");
            true
        }
        Err(e) => {
            eprintln!("[webview2] Failed to run bootstrapper: {}", e);
            false
        }
    }
}

#[cfg(windows)]
fn show_webview2_dialog() {
    use std::ptr::null_mut;
    extern "system" {
        fn MessageBoxW(hWnd: *mut core::ffi::c_void, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
    }

    let text = "\
Remote Manager requires Microsoft Edge WebView2 Runtime.

Please download and install it from:
https://go.microsoft.com/fwlink/p/?LinkId=2124703

After installation, restart Remote Manager.

(This is a one-time setup, no admin rights required for Evergreen Bootstrapper)";
    let caption = "Remote Manager - WebView2 Required";

    let text_utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption_utf16: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        MessageBoxW(null_mut(), text_utf16.as_ptr(), caption_utf16.as_ptr(), 0x30);
    }
}

#[cfg(not(windows))]
pub fn check_and_install_webview2() -> bool {
    true
}
