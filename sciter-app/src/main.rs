mod backend;
mod handler;

fn main() {
    env_logger::init();

    // Initialize database
    let conn = match backend::db::init_connection() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Database init failed: {}", e);
            #[cfg(windows)]
            {
                use std::ptr::null_mut;
                extern "system" {
                    fn MessageBoxW(hWnd: *mut core::ffi::c_void, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
                }
                let text: Vec<u16> = format!("Database init failed:\n\n{}", e)
                    .encode_utf16().chain(std::iter::once(0)).collect();
                let caption: Vec<u16> = "Remote Manager".encode_utf16().chain(std::iter::once(0)).collect();
                unsafe { MessageBoxW(null_mut(), text.as_ptr(), caption.as_ptr(), 0x10); }
            }
            return;
        }
    };

    let app_handler = handler::AppHandler::new(conn);

    let mut frame = sciter::Window::new();
    frame.event_handler(app_handler);
    frame.load_file("ui/index.html");
    frame.run_app();
}
