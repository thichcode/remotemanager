use std::path::PathBuf;

pub fn is_portable() -> bool {
    let marker = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("portable")))
        .unwrap_or_default();
    marker.exists()
}

pub fn data_dir() -> PathBuf {
    if is_portable() {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = exe_dir.join("data");
        std::fs::create_dir_all(&dir).ok();
        dir
    } else {
        let mut path = dirs::data_dir().expect("Failed to get data directory");
        path.push("remote-manager");
        std::fs::create_dir_all(&path).ok();
        path
    }
}

pub fn keys_dir() -> PathBuf {
    let dir = data_dir().join("keys");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn db_path() -> PathBuf {
    data_dir().join("data.db")
}
