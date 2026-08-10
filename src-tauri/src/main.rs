use std::panic;

fn main() {
    // Redirect panics to a visible file so "disappears silently" is debuggable
    panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        let log = format!("PANIC at {}: {}\n", location, msg);
        eprintln!("{}", log);

        // Also try to write to a log file next to the exe
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let _ = std::fs::write(dir.join("crash.log"), &log);
            }
        }

        // Show a message box so the user sees the error
        #[cfg(windows)]
        {
            use std::ptr::null_mut;
            extern "system" {
                fn MessageBoxW(hWnd: *mut core::ffi::c_void, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
            }
            let text: Vec<u16> = format!("{}\n\nLocation: {}\n\nCheck crash.log next to the exe.", msg, location)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let caption: Vec<u16> = "Remote Manager - Crash".encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                MessageBoxW(null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
            }
        }
    }));

    remote_manager_lib::run()
}
