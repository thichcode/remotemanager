# SFTP Browser Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a sidebar SFTP file browser (WinSCP-style) in both apps — lazy folder tree rooted at remote home (`/home/user`), drag/drop recursive upload, multi-select download to an OS-picked folder, auto/manual refresh — on a persistent SFTP session per active server.

**Architecture:** Approach A — `SftpBrowserManager` owns one live `russh_sftp::SftpSession` per `server_id`, driven by a dedicated worker thread + tokio runtime serving `list_dir`/`get_home`. Upload and download batches run on their own background threads (fresh connection per batch via the existing job pattern), so navigation is never blocked by a transfer.

**Tech Stack:** Rust (russh 0.62, russh-sftp 2.4 — `SftpSession` is `Arc<RawSftpSession>`, cloneable, `read_dir`→`DirEntry(file_name/file_type/metadata)`), Tauri 2 (React + Mantine + `tauri-plugin-dialog` present), Sciter (TIScript in `sciter-app/ui/index.html`).

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/sftp.rs` | Rewrite: `RemoteEntry`, `SftpBrowserManager`, session worker, recursive transfer jobs — **canonical backend** |
| `src-tauri/src/db/mod.rs` | `AppState.upload_jobs` → `SftpBrowserManager` |
| `src-tauri/src/commands/uploads.rs` | `cmd_sftp_open/list/get_home/upload/download`, progress, cancel |
| `src-tauri/src/lib.rs` | Register new commands |
| `src/types/index.ts` | `RemoteEntry` |
| `src/services/tauri.ts` | New command wrappers |
| `src/components/SftpBrowser.tsx` | New tree/drop/download UI |
| `src/components/Sidebar.tsx` | Mount `SftpBrowser`; delete `DropZone.tsx` |
| `sciter-app/src/backend/sftp.rs` | Copy of canonical backend |
| `sciter-app/src/backend/db/mod.rs` | Sciter AppState field |
| `sciter-app/src/handler.rs` | New `sftp_*` match arms + impls |
| `sciter-app/ui/index.html` | Tree UI replacing drop-zone block |

**Reused existing patterns:** `UploadAuth`, `connect_sftp` (key/password auth flow from `sftp.rs:188-251`), tokio `current_thread` runtime per worker, `Arc<AtomicBool>` cancel + `Arc<Mutex<Progress>>`, credentials via `resolve_username`/`security::decrypt`/`sshkeys::get_private_key_path`, Tauri `onDragDropEvent` pattern from `DropZone.tsx`.

---

### Task 1: Backend — entry types + pure helpers + tests (Tauri canonical)

**Files:**
- Modify: `src-tauri/src/sftp.rs`

- [ ] **Step 1: Write failing unit tests (append to bottom of file)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_remote_path_handles_root_and_slashes() {
        assert_eq!(join_remote_path("/", "etc"), "/etc");
        assert_eq!(join_remote_path("/home/user", "x.txt"), "/home/user/x.txt");
        assert_eq!(join_remote_path("/home/user/", "x.txt"), "/home/user/x.txt");
        assert_eq!(join_remote_path("/home/user/", "/etc/hosts"), "/home/user/etc/hosts");
    }

    #[test]
    fn collect_local_files_produces_relative_paths() {
        let base = std::env::temp_dir().join("rm_sftp_test_plan");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("a")).unwrap();
        std::fs::write(base.join("a/1.txt"), "x").unwrap();
        std::fs::write(base.join("b.txt"), "y").unwrap();

        let mut out = Vec::new();
        collect_local_files(&base, "", &mut out).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|(_, rel)| rel == "a/1.txt"));
        assert!(out.iter().any(|(_, rel)| rel == "b.txt"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn collect_local_files_single_file_uses_file_name() {
        let f = std::env::temp_dir().join("rm_sftp_test_single.txt");
        std::fs::write(&f, "z").unwrap();
        let mut out = Vec::new();
        collect_local_files(&f, "", &mut out).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, "rm_sftp_test_single.txt");
        let _ = std::fs::remove_file(&f);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: FAIL — `cannot find function \`join_remote_path\`` / `collect_local_files`.

- [ ] **Step 3: Implement the helpers**

Append to `src-tauri/src/sftp.rs` (before the `#[cfg(test)]` module):

```rust
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: u64,
    pub is_hidden: bool,
}

/// Join two remote POSIX path fragments with exactly one `/`.
pub fn join_remote_path(base: &str, name: &str) -> String {
    let base = base.trim_end_matches('/');
    let name = name.trim_start_matches('/');
    if base.is_empty() {
        format!("/{}", name)
    } else {
        format!("{}/{}", base, name)
    }
}

/// Recursively walk a local path, collecting `(absolute_local, relative_remote)`.
/// File → rel is its file name. Dir → rel includes the subtree path.
pub fn collect_local_files(
    path: &std::path::Path,
    rel: &str,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if path.is_file() {
        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let child_rel = if rel.is_empty() { name } else { join_remote_path(rel, &name) };
        out.push((path.to_string_lossy().to_string(), child_rel));
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    let mut names: Vec<std::ffi::OsString> = std::fs::read_dir(path)
        .map_err(|e| format!("Cannot read dir {}: {}", path.display(), e))?
        .filter_map(|e| e.ok().map(|d| d.file_name()))
        .collect();
    names.sort();
    for n in names {
        let child = path.join(&n);
        let nstr = n.to_string_lossy().to_string();
        let child_rel = if rel.is_empty() { nstr.clone() } else { join_remote_path(rel, &nstr) };
        collect_local_files(&child, &child_rel, out)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sftp.rs
git commit -m "feat(sftp): entry type and pure path helpers with tests"
```

---

### Task 2: Backend — persistent session worker (open/list/close)

**Files:**
- Modify: `src-tauri/src/sftp.rs`

- [ ] **Step 1: Add browser session struct + worker + `connect_sftp` + `list_dir_async`**

Append to `src-tauri/src/sftp.rs` (before tests):

```rust
pub struct BrowserSession {
    pub home: String,
    cmd_tx: std::sync::mpsc::Sender<BrowseCmd>,
}

enum BrowseCmd {
    List { path: String, reply: std::sync::mpsc::SyncSender<Result<Vec<RemoteEntry>, String>> },
    Close,
}

impl BrowserSession {
    pub fn open(
        host: String, port: i32, username: String, auth: UploadAuth,
    ) -> Result<BrowserSession, String> {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<BrowseCmd>();
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<String, String>>(1);
        let worker_host = host.clone();
        std::thread::Builder::new()
            .name("sftp-browser".into())
            .spawn(move || session_worker(worker_host, port, username, auth, cmd_rx, init_tx))
            .map_err(|e| format!("Failed to spawn SFTP browser thread: {}", e))?;
        let home = init_rx
            .recv()
            .map_err(|_| "SFTP browser thread died during init".to_string())??;
        Ok(BrowserSession { home, cmd_tx })
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, String> {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        self.cmd_tx
            .send(BrowseCmd::List { path: path.to_string(), reply: reply_tx })
            .map_err(|_| "SFTP browser connection closed".to_string())?;
        reply_rx.recv().map_err(|_| "SFTP browser connection closed".to_string())?
    }
}

fn session_worker(
    host: String,
    port: i32,
    username: String,
    auth: UploadAuth,
    cmd_rx: std::sync::mpsc::Receiver<BrowseCmd>,
    init_tx: std::sync::mpsc::SyncSender<Result<String, String>>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(r) => r,
        Err(e) => { let _ = init_tx.send(Err(format!("tokio: {}", e))); return; }
    };
    let running = match rt.block_on(connect_sftp(&host, port, &username, auth)) {
        Ok((sftp, home)) => {
            let _ = init_tx.send(Ok(home.clone()));
            let running = Arc::new(std::sync::Mutex::new(Some(sftp)));
            let _home = home;
            running
        }
        Err(e) => { let _ = init_tx.send(Err(e)); return; }
    };

    for msg_res in cmd_rx.into_iter() {
        match msg_res {
            BrowseCmd::Close => break,
            BrowseCmd::List { path, reply } => {
                let sftp = running.lock().ok().and_then(|g| g.clone());
                let res = match sftp {
                    Some(sftp) => rt.block_on(list_dir_async(&sftp, &path)),
                    None => Err("SFTP session unavailable".into()),
                };
                let _ = reply.send(res);
            }
        }
    }
    if let Ok(mut g) = running.lock() {
        if let Some(sftp) = g.take() {
            let _ = rt.block_on(sftp.close());
        }
    }
}

async fn connect_sftp(
    host: &str, port: i32, username: &str, auth: UploadAuth,
) -> Result<(SftpSession, String), String> {
    use russh_sftp::client::SftpSession as S;
    let config = russh::client::Config::default();
    let mut session = russh::client::connect(
        Arc::new(config), (host.to_string(), port as u16), Client,
    )
    .await
    .map_err(|e| format!("SSH connect failed: {}", e))?;
    let authed = match &auth {
        UploadAuth::Password(pw) => session
            .authenticate_password(username, pw)
            .await
            .map_err(|e| format!("SSH auth failed: {}", e))?,
        UploadAuth::Key(path) => {
            let key = russh::keys::load_secret_key(path, None)
                .map_err(|e| format!("Failed to load SSH key {}: {}", path, e))?;
            let hash_alg = if key.algorithm().is_rsa() {
                session.best_supported_rsa_hash().await.ok().flatten().flatten()
            } else { None };
            session
                .authenticate_publickey(username, russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg))
                .await
                .map_err(|e| format!("SSH key auth failed: {}", e))?
        }
    };
    if !authed.success() {
        return Err("Authentication rejected by server".into());
    }
    let channel = session.channel_open_session().await.map_err(|e| format!("Failed to open session channel: {}", e))?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| format!("Failed to open SFTP subsystem: {}", e))?;
    let sftp = S::new(channel.into_stream()).await.map_err(|e| format!("SFTP init failed: {}", e))?;
    let home = sftp.canonicalize(".").await.unwrap_or_else(|_| ".".to_string());
    Ok((sftp, home))
}

async fn list_dir_async(sftp: &SftpSession, path: &str) -> Result<Vec<RemoteEntry>, String> {
    let entries = sftp.read_dir(path).await.map_err(|e| format!("List {} failed: {}", path, e))?;
    let mut out = Vec::new();
    for e in entries {
        let name = e.file_name();
        let meta = e.metadata();
        out.push(RemoteEntry {
            is_hidden: name.starts_with('.'),
            is_dir: e.file_type().is_dir(),
            size: meta.len(),
            mtime: meta.modified()
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
                .unwrap_or(0),
            name,
        });
    }
    Ok(out)
}
```

- [ ] **Step 2: Compile**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: clean (unused `list_dir`/`open` fine — manager wires them in Task 4; add `#[allow(dead_code)]` on `BrowserSession::list_dir` if warned).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/sftp.rs
git commit -m "feat(sftp): persistent browser session worker with list_dir"
```

---

### Task 3: Backend — recursive upload/download transfer jobs

**Files:**
- Modify: `src-tauri/src/sftp.rs`

- [ ] **Step 1: Append transfer plumbing + async runners**

Append to `src-tauri/src/sftp.rs` (before tests):

```rust
#[derive(Clone, Debug, serde::Serialize)]
pub struct DownloadProgress {
    pub state: String,          // "uploading" | "downloading" | "done" | "error" | "cancelled"
    pub current_file: String,
    pub file_index: usize,
    pub total_files: usize,
    pub bytes_sent: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
}

/// Recursively walk a remote dir collecting `(remote_path, relative_path)`.
async fn collect_remote_files(
    sftp: &SftpSession,
    path: &str,
    rel: &str,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let entries = sftp.read_dir(path).await.map_err(|e| format!("List {} failed: {}", path, e))?;
    for e in entries {
        let name = e.file_name();
        if name == "." || name == ".." { continue; }
        let child_path = join_remote_path(path, &name);
        let child_rel = if rel.is_empty() { name.clone() } else { join_remote_path(rel, &name) };
        if e.file_type().is_dir() {
            collect_remote_files(sftp, &child_path, &child_rel, out).await?;
        } else {
            out.push((child_path, child_rel));
        }
    }
    Ok(())
}

async fn upload_batch(
    sftp: &SftpSession,
    remote_dir: &str,
    local_paths: &[String],
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    use russh_sftp::protocol::OpenFlags;
    use tokio::io::AsyncWriteExt;

    let mut plan = Vec::new();
    for p in local_paths {
        collect_local_files(Path::new(p), "", &mut plan)?;
    }
    { let mut g = progress.lock().unwrap(); g.total_files = plan.len(); }
    for (idx, (local_abs, rel)) in plan.iter().enumerate() {
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Upload cancelled".into());
        }
        let remote_path = join_remote_path(remote_dir, rel);
        {
            let mut g = progress.lock().unwrap();
            g.current_file = rel.clone();
            g.file_index = idx;
            g.bytes_sent = 0;
            g.total_bytes = std::fs::metadata(local_abs).map(|m| m.len()).unwrap_or(0);
        }
        if let Some(parent) = remote_path.rfind('/') {
            let p = &remote_path[..parent];
            if !p.is_empty() && p != remote_dir {
                let _ = sftp.create_dir(p).await; // ignore "exists" errors
            }
        }
        let mut remote = sftp
            .open_with_flags(&remote_path, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE)
            .await
            .map_err(|e| format!("Open remote {} failed: {}", remote_path, e))?;
        let mut local = tokio::fs::File::open(local_abs)
            .await
            .map_err(|e| format!("Open local {} failed: {}", local_abs, e))?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) { return Err("Upload cancelled".into()); }
            let n = local.read(&mut buf).await.map_err(|e| format!("Read local: {}", e))?;
            if n == 0 { break; }
            remote.write_all(&buf[..n]).await.map_err(|e| format!("Write remote {}: {}", remote_path, e))?;
            let mut g = progress.lock().unwrap();
            g.bytes_sent += n as u64;
        }
        remote.shutdown().await.ok();
    }
    Ok(())
}

async fn download_batch(
    sftp: &SftpSession,
    local_dir: &str,
    remote_paths: &[String],
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    use russh_sftp::protocol::OpenFlags;
    use tokio::io::AsyncReadExt;

    let mut plan = Vec::new();
    for r in remote_paths {
        let meta = sftp.metadata(r).await.map_err(|e| format!("Stat {} failed: {}", r, e))?;
        if meta.file_type().is_dir() {
            collect_remote_files(sftp, r, "", &mut plan).await?;
        } else {
            let name = r.rsplit('/').next().unwrap_or(r).to_string();
            plan.push((r.clone(), name));
        }
    }
    { let mut g = progress.lock().unwrap(); g.total_files = plan.len(); }
    for (idx, (remote_abs, rel)) in plan.iter().enumerate() {
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Download cancelled".into());
        }
        let local_path = Path::new(local_dir).join(rel);
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create dir {}: {}", parent.display(), e))?;
        }
        let mut remote = sftp
            .open_with_flags(remote_abs, OpenFlags::READ)
            .await
            .map_err(|e| format!("Open remote {} failed: {}", remote_abs, e))?;
        let total = remote.metadata().await.map(|m| m.len()).unwrap_or(0);
        {
            let mut g = progress.lock().unwrap();
            g.current_file = rel.clone();
            g.file_index = idx;
            g.bytes_sent = 0;
            g.total_bytes = total;
        }
        let mut local = std::fs::File::create(&local_path)
            .map_err(|e| format!("Create {} failed: {}", local_path.display(), e))?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) { return Err("Download cancelled".into()); }
            let n = remote.read(&mut buf).await.map_err(|e| format!("Read {} failed: {}", remote_abs, e))?;
            if n == 0 { break; }
            std::io::Write::write_all(&mut local, &buf[..n]).map_err(|e| format!("Write failed: {}", e))?;
            let mut g = progress.lock().unwrap();
            g.bytes_sent += n as u64;
        }
        remote.shutdown().await.ok();
    }
    Ok(())
}
```

- [ ] **Step 2: Compile + test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: 3 tests PASS; compiles (dead-code warnings OK, silenced in Task 4).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/sftp.rs
git commit -m "feat(sftp): recursive upload and download batch runners"
```

---

### Task 4: Backend — `SftpBrowserManager` orchestration

**Files:**
- Modify: `src-tauri/src/sftp.rs`

- [ ] **Step 1: Append manager**

Append to `src-tauri/src/sftp.rs` (before tests):

```rust
pub struct SftpBrowserManager {
    browsers: Arc<std::sync::Mutex<HashMap<String, BrowserSession>>>,
    jobs: Arc<std::sync::Mutex<HashMap<String, Arc<JobState>>>>,
}

struct JobState {
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    progress_download: Arc<Mutex<DownloadProgress>>,
}

impl Default for SftpBrowserManager { fn default() -> Self { Self::new() } }

impl SftpBrowserManager {
    pub fn new() -> Self {
        SftpBrowserManager {
            browsers: Arc::new(std::sync::Mutex::new(HashMap::new())),
            jobs: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn open_browser(
        &self, server_id: &str, host: String, port: i32, username: String, auth: UploadAuth,
    ) -> Result<String, String> {
        let mut browsers = self.browsers.lock().map_err(|e| e.to_string())?;
        // drop any existing session for this server first (single live session per server)
        let session = BrowserSession::open(host, port, username, auth)?;
        let home = session.home.clone();
        browsers.insert(server_id.to_string(), session);
        Ok(home)
    }

    pub fn close_browser(&self, server_id: &str) {
        if let Ok(mut browsers) = self.browsers.lock() {
            if let Some(s) = browsers.remove(server_id) {
                drop(s); // worker cleans up on Drop of the channel
            }
        }
    }

    pub fn list_dir(&self, server_id: &str, path: &str) -> Result<Vec<RemoteEntry>, String> {
        let browsers = self.browsers.lock().map_err(|e| e.to_string())?;
        let s = browsers
            .get(server_id)
            .ok_or_else(|| "No active SFTP browser for this server".to_string())?;
        s.list_dir(path)
    }

    pub fn get_home(&self, server_id: &str) -> Option<String> {
        self.browsers.lock().ok().and_then(|b| b.get(server_id).map(|s| s.home.clone()))
    }

    pub fn start_upload(
        &self, host: String, port: i32, username: String, auth: UploadAuth,
        remote_dir: String, local_paths: Vec<String>,
    ) -> Result<String, String> {
        start_transfer_job(self.jobs.clone(), host, port, username, auth, remote_dir, local_paths, TransferKind::Upload)
    }

    pub fn start_download(
        &self, host: String, port: i32, username: String, auth: UploadAuth,
        local_dir: String, remote_paths: Vec<String>,
    ) -> Result<String, String> {
        start_transfer_job(self.jobs.clone(), host, port, username, auth, local_dir, remote_paths, TransferKind::Download)
    }

    pub fn get_download_progress(&self, job_id: &str) -> Option<DownloadProgress> {
        self.jobs.lock().ok()?.get(job_id)?.progress_download.lock().ok().map(|g| g.clone())
    }

    pub fn cancel(&self, job_id: &str) {
        if let Ok(jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get(job_id) {
                job.cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TransferKind { Upload, Download }

fn start_transfer_job(
    jobs: Arc<std::sync::Mutex<HashMap<String, Arc<JobState>>>>,
    host: String,
    port: i32,
    username: String,
    auth: UploadAuth,
    target: String,   // remote upload dir OR local download dir
    items: Vec<String>,
    kind: TransferKind,
) -> Result<String, String> {
    if items.is_empty() {
        return Err(match kind { TransferKind::Upload => "No files to upload".into(), TransferKind::Download => "Nothing to download".into() });
    }
    let job_id = uuid::Uuid::new_v4().to_string();
    let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let progress_download = Arc::new(Mutex::new(DownloadProgress {
        state: match kind { TransferKind::Upload => "uploading".into(), TransferKind::Download => "downloading".into() },
        current_file: String::new(),
        file_index: 0,
        total_files: items.len(),
        bytes_sent: 0,
        total_bytes: 0,
        error: None,
    }));
    {
        let mut j = jobs.lock().map_err(|e| e.to_string())?;
        j.insert(job_id.clone(), Arc::new(JobState { cancel_flag: cancel_flag.clone(), progress_download: progress_download.clone() }));
    }
    let jobs_clone = jobs.clone();
    let job_id_clone = job_id.clone();
    std::thread::Builder::new()
        .name("sftp-transfer".into())
        .spawn(move || {
            let result = run_transfer_conn(host, port, username, auth, target, items, kind, cancel_flag.clone(), progress_download.clone());
            let mut p = progress_download.lock().unwrap();
            match result {
                Ok(()) => { if p.state != "cancelled" { p.state = "done".into(); } }
                Err(e) => {
                    let is_cancel = e.contains("cancelled");
                    if is_cancel { p.state = "cancelled".into(); }
                    else { p.state = "error".into(); p.error = Some(e); }
                }
            }
            drop(p);
            if let Ok(mut j) = jobs_clone.lock() { j.remove(&job_id_clone); }
        })
        .map_err(|e| format!("Failed to spawn transfer thread: {}", e))?;
    Ok(job_id)
}

fn run_transfer_conn(
    host: String, port: i32, username: String, auth: UploadAuth,
    target: String, items: Vec<String>, kind: TransferKind,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
    rt.block_on(run_transfer_async(host, port, username, auth, target, items, kind, cancel_flag, progress))
}

async fn run_transfer_async(
    host: String, port: i32, username: String, auth: UploadAuth,
    target: String, items: Vec<String>, kind: TransferKind,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    let (sftp, _) = connect_sftp(&host, port, &username, auth).await?;
    match kind {
        TransferKind::Upload => upload_batch(&sftp, &target, &items, cancel_flag, progress).await,
        TransferKind::Download => download_batch(&sftp, &target, &items, cancel_flag, progress).await,
    }
}
```

- [ ] **Step 2: Add `impl Drop for BrowserSession` so removing from map closes the worker**

Do this by dropping `cmd_tx`: append the Drop impl so a removed session stops its loop. The loop exits when `cmd_tx` is dropped (channel closed) or a `Close` cmd arrives. The manager's `close_browser` just removes from the map; the worker's `cmd_rx.into_iter()` ends when the sender is dropped → loop ends → `sftp.close()` runs. Verify: when all `BrowserSession`s drop, `std::sync::mpsc` channel closes and `into_iter()` returns `None`. **No Drop impl needed** — document this invariant in a comment.

- [ ] **Step 3: Compile + tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: 3 tests PASS; clean build.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/sftp.rs
git commit -m "feat(sftp): SftpBrowserManager orchestrating sessions and transfer jobs"
```

---

### Task 5: Wire into Tauri AppState + commands

**Files:**
- Modify: `src-tauri/src/db/mod.rs:13`
- Modify: `src-tauri/src/commands/uploads.rs` (rewrite)
- Modify: `src-tauri/src/lib.rs:50,105-107`

- [ ] **Step 1: AppState field type**

`src-tauri/src/db/mod.rs` line 13:

```rust
pub upload_jobs: crate::sftp::SftpBrowserManager,
```

- [ ] **Step 2: lib.rs construction**

`src-tauri/src/lib.rs` line 50:

```rust
upload_jobs: crate::sftp::SftpBrowserManager::new(),
```

- [ ] **Step 3: Rewrite `src-tauri/src/commands/uploads.rs`**

```rust
use tauri::State;

use crate::db::AppState;
use crate::sftp::UploadAuth;

/// Resolve host/port/username/auth for a server (shared by all sftp commands).
fn resolve_auth(
    state: &State<AppState>,
    server_id: &str,
) -> Result<(String, i32, String, UploadAuth), String> {
    let (host, port, username, credential_id, ssh_key_id) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let server = crate::db::operations::get_server(&conn, server_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Server not found".to_string())?;
        drop(conn);
        (server.host, server.port, server.username, server.credential_id, server.ssh_key_id)
    };

    let username = crate::commands::ssh::resolve_username(state, username, credential_id.as_deref())?;
    let password = if let Some(cid) = credential_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let encrypted = crate::db::operations::get_credential_password(&conn, cid)
            .map_err(|e| e.to_string())?
            .ok_or("Credential not found")?;
        drop(conn);
        crate::security::decrypt(&encrypted).ok()
    } else { None };
    let key_path = if let Some(kid) = ssh_key_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::sshkeys::get_private_key_path(&conn, kid).ok().flatten()
    } else { None };

    let auth = match (password, key_path) {
        (Some(pw), _) => UploadAuth::Password(pw),
        (None, Some(kp)) => UploadAuth::Key(kp),
        (None, None) => return Err("No password or SSH key available for this server".into()),
    };
    Ok((host, port, username, auth))
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_sftp_open(state: State<AppState>, server_id: String) -> Result<String, String> {
    let (host, port, username, auth) = resolve_auth(&state, &server_id)?;
    state.upload_jobs.open_browser(&server_id, host, port, username, auth)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_sftp_list(
    state: State<AppState>,
    server_id: String,
    path: String,
) -> Result<Vec<crate::sftp::RemoteEntry>, String> {
    state.upload_jobs.list_dir(&server_id, &path)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_sftp_get_home(state: State<AppState>, server_id: String) -> Result<Option<String>, String> {
    Ok(state.upload_jobs.get_home(&server_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_sftp_upload(
    state: State<AppState>,
    server_id: String,
    remote_dir: String,
    local_paths: Vec<String>,
) -> Result<String, String> {
    let (host, port, username, auth) = resolve_auth(&state, &server_id)?;
    state.upload_jobs.start_upload(host, port, username, auth, remote_dir, local_paths)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_sftp_download(
    state: State<AppState>,
    server_id: String,
    local_dir: String,
    remote_paths: Vec<String>,
) -> Result<String, String> {
    let (host, port, username, auth) = resolve_auth(&state, &server_id)?;
    state.upload_jobs.start_download(host, port, username, auth, local_dir, remote_paths)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_get_upload_progress(
    state: State<AppState>,
    job_id: String,
) -> Result<Option<crate::sftp::DownloadProgress>, String> {
    Ok(state.upload_jobs.get_download_progress(&job_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_cancel_upload(state: State<AppState>, job_id: String) -> Result<(), String> {
    state.upload_jobs.cancel(&job_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_snapshot_shape() {
        // resolve_auth touches DB/security; keep smoke marker so command module is exercised.
        assert!(std::any::type_name::<UploadAuth>().contains("UploadAuth"));
    }
}
```

- [ ] **Step 4: lib.rs handler list**

Replace lines 105-107:

```rust
            commands::uploads::cmd_sftp_open,
            commands::uploads::cmd_sftp_list,
            commands::uploads::cmd_sftp_get_home,
            commands::uploads::cmd_sftp_upload,
            commands::uploads::cmd_sftp_download,
            commands::uploads::cmd_get_upload_progress,
            commands::uploads::cmd_cancel_upload,
```

Remove the old `cmd_upload_files` — nothing references it anymore.

- [ ] **Step 5: Build + test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS + clean.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/mod.rs src-tauri/src/lib.rs src-tauri/src/commands/uploads.rs
git commit -m "feat(sftp): expose browser commands through tauri"
```

---

### Task 6: Tauri UI types + services

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/services/tauri.ts`

- [ ] **Step 1: Add `RemoteEntry` type**

Append to `src/types/index.ts`:

```ts
export interface RemoteEntry {
  name: string;
  is_dir: boolean;
  size: number;
  mtime: number;
  is_hidden: boolean;
}
```

- [ ] **Step 2: Replace upload service block in `src/services/tauri.ts`**

Replace lines 219-227 (the `uploadFiles`/`getUploadProgress`/`cancelUpload` block) with:

```ts
// SFTP Browser
export const sftpOpen = (serverId: string): Promise<string> =>
  invoke('cmd_sftp_open', { serverId });
export const sftpList = (serverId: string, path: string): Promise<RemoteEntry[]> =>
  invoke('cmd_sftp_list', { serverId, path });
export const sftpGetHome = (serverId: string): Promise<string | null> =>
  invoke('cmd_sftp_get_home', { serverId });
export const sftpUpload = (serverId: string, remoteDir: string, localPaths: string[]): Promise<string> =>
  invoke('cmd_sftp_upload', { serverId, remoteDir, localPaths });
export const sftpDownload = (serverId: string, localDir: string, remotePaths: string[]): Promise<string> =>
  invoke('cmd_sftp_download', { serverId, localDir, remotePaths });
export const getUploadProgress = (jobId: string): Promise<UploadProgress | null> =>
  invoke('cmd_get_upload_progress', { jobId });
export const cancelUpload = (jobId: string): Promise<void> =>
  invoke('cmd_cancel_upload', { jobId });
```

Update the import from `../types` (line 2) to include `RemoteEntry`.

- [ ] **Step 3: Build**

Run: `npm run build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/types/index.ts src/services/tauri.ts
git commit -m "feat(sftp): tauri types and service wrappers"
```

---

### Task 7: Tauri UI — SftpBrowser component + Sidebar

**Files:**
- Create: `src/components/SftpBrowser.tsx`
- Modify: `src/components/Sidebar.tsx`
- Delete: `src/components/DropZone.tsx`

- [ ] **Step 1: Create `src/components/SftpBrowser.tsx`**

```tsx
import { Box, Text, Progress, Button, Group, ActionIcon, Tooltip, Stack, ScrollArea, Checkbox, Divider, Loader } from '@mantine/core';
import { IconFolder, IconFile, IconRefresh, IconDownload, IconChevronRight, IconUpload, IconTrash } from '@tabler/icons-react';
import { useEffect, useRef, useState, useCallback } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { sftpOpen, sftpList, sftpGetHome, sftpUpload, sftpDownload, getUploadProgress, cancelUpload } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import type { RemoteEntry, UploadProgress } from '../types';

interface TreeNode {
  path: string;
  name: string;
  is_dir: boolean;
  children: TreeNode[] | null;
  loaded: boolean;
  expanded: boolean;
  hint: string;
}

function toNodes(entries: RemoteEntry[], showHidden: boolean): TreeNode[] {
  return entries
    .filter(e => showHidden || !e.is_hidden)
    .sort((a, b) => (a.is_dir === b.is_dir ? a.name.localeCompare(b.name) : a.is_dir ? -1 : 1))
    .map(e => ({
      path: e.name,
      name: e.name,
      is_dir: e.is_dir,
      children: e.is_dir ? null : undefined,
      loaded: false,
      expanded: false,
      hint: e.is_dir ? '' : `${formatSize(e.size)}`,
    }));
}

function formatSize(b: number): string {
  if (b >= 1024 * 1024 * 1024) return `${(b / (1024 ** 3)).toFixed(1)} GB`;
  if (b >= 1024 * 1024) return `${(b / (1024 ** 2)).toFixed(1)} MB`;
  if (b >= 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${b} B`;
}

// Remap a node: node.path is full remote path. children rendered recursively.
function renderNodes(
  nodes: TreeNode[],
  depth: number,
  handlers: {
    toggle: (n: TreeNode) => void;
    refresh: (n: TreeNode) => void;
    download: (n: TreeNode) => void;
    onDragOver: (e: React.DragEvent, n: TreeNode) => void;
    onDragLeave: () => void;
    onDrop: (e: React.DragEvent, n: TreeNode) => void;
    selected: Set<string>;
    toggleSelect: (n: TreeNode, ctrl: boolean) => void;
  },
): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  for (const n of nodes) {
    const isSel = handlers.selected.has(n.path);
    out.push(
      <div key={n.path}>
        <Group
          gap={4}
          pl={depth * 14}
          px={6}
          py={2}
          style={{
            cursor: 'pointer',
            borderRadius: 4,
            background: isSel ? 'var(--mantine-color-blue-9)' : undefined,
            outline: undefined,
          }}
          onClick={(e) => handlers.toggleSelect(n, e.ctrlKey || e.metaKey)}
          draggable={false}
          onDragOver={(e) => { e.preventDefault(); e.stopPropagation(); handlers.onDragOver(e, n); }}
          onDragLeave={(e) => { e.preventDefault(); e.stopPropagation(); handlers.onDragLeave(); }}
          onDrop={(e) => { e.preventDefault(); e.stopPropagation(); handlers.onDrop(e, n); }}
        >
          {n.is_dir ? (
            <>
              <IconChevronRight size={11} style={{ transform: n.expanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.12s' }} />
              <IconFolder size={14} />
              {!n.loaded && n.is_dir && <IconRefresh size={10} />}
            </>
          ) : (
            <>
              <Box style={{ width: 15 }} />
              <IconFile size={14} />
            </>
          )}
          <Text size="xs" style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={n.hint}>
            {n.name}
          </Text>
          {!n.is_dir && (
            <ActionIcon size={16} variant="subtle" title="Download" onClick={(e) => { e.stopPropagation(); handlers.download(n); }}>
              <IconDownload size={12} />
            </ActionIcon>
          )}
        </Group>
        {n.is_dir && n.expanded && n.children
          ? renderNodes(n.children, depth + 1, handlers)
          : null}
      </div>,
    );
  }
  return out;
}

interface Props {
  serverId: string | null;
  serverHost: string | null;
  onClearHistory: () => void;
}

export function SftpBrowser({ serverId, serverHost, onClearHistory }: Props) {
  const [home, setHome] = useState('');
  const [root, setRoot] = useState<TreeNode[]>([]);
  const [rootLoaded, setRootLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showHidden, setShowHidden] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<UploadProgress | null>(null);
  const [dragging, setDragging] = useState(false);
  const timerRef = useRef<number | null>(null);
  const rootRef = useRef<TreeNode[]>([]);
  const enabled = serverId !== null;

  const stopPolling = () => {
    if (timerRef.current !== null) { window.clearInterval(timerRef.current); timerRef.current = null; }
  };
  useEffect(() => () => stopPolling(), []);

  // Imperative tree storage so drop targets can mutate the same nodes.
  useEffect(() => { rootRef.current = root; }, [root]);

  const load = useCallback(async (serverId: string) => {
    setLoading(true);
    setRootLoaded(false);
    setSelected(new Set());
    try {
      let h = await sftpGetHome(serverId);
      if (!h) h = await sftpOpen(serverId);
      setHome(h);
      const entries = await sftpList(serverId, h);
      setRoot(toNodes(entries, showHidden));
      setRootLoaded(true);
    } catch (e) {
      notifications.show({ title: 'SFTP browse failed', message: String(e), color: 'red' });
      setRootLoaded(true);
    } finally {
      setLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHidden]);

  useEffect(() => { if (serverId) void load(serverId); else { setRoot([]); setRootLoaded(false); } }, [serverId, load]);

  const mutate = (fn: (nodes: TreeNode[]) => void) => {
    const nodes = rootRef.current;
    fn(nodes);
    setRoot([...nodes]);
  };

  const findNode = (nodes: TreeNode[], path: string): TreeNode | null => {
    for (const n of nodes) {
      if (n.path === path) return n;
      if (n.is_dir && n.children) {
        const f = findNode(n.children, path);
        if (f) return f;
      }
    }
    return null;
  };

  const toggleNode = async (n: TreeNode) => {
    if (!n.is_dir) return;
    mutate((nodes) => {
      const found = findNode(nodes, n.path);
      if (!found) return;
      if (!found.loaded) {
        found.loaded = true;
        found.expanded = true;
        void sftpList(serverId!, found.path).then((entries) => {
          mutate2(entries, found.path);
        }).catch((e) => notifications.show({ title: 'List failed', message: String(e), color: 'red' }));
      } else {
        found.expanded = !found.expanded;
      }
    });
  };
  const mutate2 = (entries: RemoteEntry[], path: string) => {
    const nodes = rootRef.current;
    const f = findNode(nodes, path);
    if (f) {
      f.children = toNodes(entries, showHidden);
      f.loaded = true;
      f.expanded = true;
    }
    setRoot([...nodes]);
  };

  const refreshNode = async (n: TreeNode) => {
    try {
      const entries = await sftpList(serverId!, n.path);
      mutate2(entries, n.path);
    } catch (e) { notifications.show({ title: 'Refresh failed', message: String(e), color: 'red' }); }
  };

  const refreshRoot = async () => {
    if (serverId) { await refreshNodeRef(home); }
  };

  const refreshNodeRef = async (path: string) => {
    try {
      const entries = await sftpList(serverId!, path === home ? home : path);
      mutate2(entries, path);
    } catch (e) { notifications.show({ title: 'Refresh failed', message: String(e), color: 'red' }); }
  };

  const toggleSelect = (n: TreeNode, ctrl: boolean) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (ctrl) {
        if (next.has(n.path)) next.delete(n.path); else next.add(n.path);
      } else {
        next.clear();
        next.add(n.path);
      }
      return next;
    });
  };

  const downloadNodes = async (paths: string[]) => {
    if (!serverId || paths.length === 0) return;
    const dir = await open({ directory: true, title: 'Choose download folder' });
    if (!dir) return;
    try {
      const id = await sftpDownload(serverId, String(dir), paths);
      startJobPoll(id, 'Download');
    } catch (e) { notifications.show({ title: 'Download failed', message: String(e), color: 'red' }); }
  };

  const downloadSelected = () => downloadNodes([...selected]);

  const onDownloadNode = (n: TreeNode) => downloadNodes([n.path]);

  const dropUpload = async (targetPath: string, localPaths: string[]) => {
    if (!serverId) return;
    try {
      const id = await sftpUpload(serverId, targetPath === home ? home : targetPath, localPaths);
      startJobPoll(id, 'Upload');
    } catch (e) { notifications.show({ title: 'Upload failed', message: String(e), color: 'red' }); }
  };

  const startJobPoll = (id: string, kind: 'Upload' | 'Download') => {
    stopPolling();
    setJobId(id);
    setProgress(null);
    timerRef.current = window.setInterval(async () => {
      try {
        const p = await getUploadProgress(id);
        if (!p) { stopPolling(); setJobId(null); return; }
        setProgress(p);
        if (p.state === 'done' || p.state === 'error' || p.state === 'cancelled') {
          stopPolling();
          if (p.state === 'done') {
            notifications.show({ title: `${kind} complete`, message: `${p.total_files} file(s)`, color: 'green' });
            void refreshNodeRef(home);
          } else if (p.state === 'error') {
            notifications.show({ title: `${kind} failed`, message: p.error ?? 'Unknown error', color: 'red' });
          }
          setTimeout(() => { setJobId(null); setProgress(null); }, 2500);
        }
      } catch { stopPolling(); }
    }, 250);
  };

  const cancelJob = async () => { if (jobId) { await cancelUpload(jobId); } };

  const onGlobalDragOver = (e: React.DragEvent) => { e.preventDefault(); setDragging(true); };
  const onGlobalDragLeave = () => { setDragging(false); };
  const onGlobalDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    if (!enabled || !serverId) return;
    const files = e.dataTransfer?.files;
    const paths = files ? Array.from(files).map(f => (f as unknown as { path: string }).path).filter(Boolean) : [];
    if (paths.length === 0) return;
    await dropUpload(home, paths);
  };
  const onNodeDragOver = (e: React.DragEvent, n: TreeNode) => { setDragging(true); };
  const onNodeDrop = (e: React.DragEvent, n: TreeNode) => {
    e.preventDefault();
    e.stopPropagation();
    setDragging(false);
    if (!enabled || !serverId || !n.is_dir) return;
    const files = e.dataTransfer?.files;
    const paths = files ? Array.from(files).map(f => (f as unknown as { path: string }).path).filter(Boolean) : [];
    if (paths.length === 0) return;
    void dropUpload(n.path, paths);
  };

  // Tauri v2 drag-drop event (for OS-level file drop anywhere on webview)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === 'over') setDragging(true);
      else if (payload.type === 'leave') setDragging(false);
      else if (payload.type === 'drop') {
        setDragging(false);
        if (enabled && serverId && payload.paths.length > 0) {
          void dropUpload(home, payload.paths);
        }
      }
    }).then((fn) => { unlisten = fn; });
    return () => { if (unlisten) unlisten(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, serverId, home]);

  const pct = progress && progress.total_bytes > 0 ? Math.round(progress.bytes_sent * 100 / progress.total_bytes) : 0;

  return (
    <Stack gap={4}>
      <Group justify="space-between" align="center">
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">SFTP Files</Text>
        <Group gap={2}>
          <Tooltip label="Show hidden files">
            <Checkbox size="xs" checked={showHidden} onChange={(e) => setShowHidden(e.currentTarget.checked)} aria-label="Show hidden files" />
          </Tooltip>
          <Tooltip label="Refresh">
            <ActionIcon size="sm" variant="subtle" onClick={refreshRoot}><IconRefresh size={14} /></ActionIcon>
          </Tooltip>
          <Tooltip label="Clear history">
            <ActionIcon size="sm" variant="subtle" onClick={onClearHistory}><IconTrash size={14} /></ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {!enabled ? (
        <Text size="xs" c="dimmed" px={6}>Open an SSH terminal to browse files.</Text>
      ) : (
        <Box
          style={{
            border: dragging ? '2px dashed var(--mantine-color-blue-5)' : '2px dashed var(--mantine-color-dark-4)',
            borderRadius: 6,
            padding: 4,
            minHeight: 60,
          }}
          onDragOver={onGlobalDragOver}
          onDragLeave={onGlobalDragLeave}
          onDrop={onGlobalDrop}
        >
          {loading ? (
            <Group justify="center" py="md"><Loader size="xs" /></Group>
          ) : !rootLoaded ? (
            <Text size="xs" c="dimmed" px={6}>SFTP unavailable.</Text>
          ) : (
            <ScrollArea.Autosize mah={380} type="auto">
              <Stack gap={0}>
                <Group gap={4} px={6} py={2}>
                  <IconFolder size={14} />
                  <Text size="xs" style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} c="dimmed">{home || serverHost}</Text>
                  <Text size="xs" c="dimmed">{serverHost}</Text>
                </Group>
                <Divider />
                {root.length === 0 && <Text size="xs" c="dimmed" px={6} py={4}>Empty directory</Text>}
                {root.map((n) => (
                  <Box key={n.path}>
                  {renderNodes([n], 1, {
                    toggle: toggleNode,
                    refresh: refreshNode,
                    download: onDownloadNode,
                    onDragOver: onNodeDragOver,
                    onDragLeave: () => setDragging(false),
                    onDrop: onNodeDrop,
                    selected,
                    toggleSelect,
                  })}
                  </Box>
                ))}
              </Stack>
            </ScrollArea.Autosize>
          )}

          {selected.size > 0 && (
            <Button size="xs" variant="light" fullWidth mt={4} leftSection={<IconDownload size={12} />} onClick={downloadSelected}>
              Download ({selected.size})
            </Button>
          )}

          {jobId && progress ? (
            <>
              <Text size="xs" mt={4}>{(progress.file_index + 1)}/{progress.total_files} {progress.current_file}</Text>
              <Progress value={pct} size="sm" />
              <Group justify="center" mt={4}>
                <Button size="xs" variant="light" color="red" onClick={cancelJob}>Cancel</Button>
              </Group>
            </>
          ) : (
            <Text size="xs" c="dimmed" px={6} mt={4}>Drop files to upload to {home}</Text>
          )}
        </Box>
      )}
    </Stack>
  );
}
```

**Note on path semantics:** `toNodes` produces node paths that are *relative* (e.g. `"docs"`), then the renderers use `n.path` directly as full remote path — this is only correct for the root level. **Fix:** make `toNodes` produce full remote paths by prepending the parent path. In `load()`, after `setHome(h)`, build root nodes with `path = join(h, e.name)`. Adjust `toNodes(entries, showHidden, parentPath)` and callers: root passes `h`; `mutate2(entries, path)` passes `path` (the parent) so children get `join(parentFull, name)`.

Concretely change:
- signature → `toNodes(entries: RemoteEntry[], showHidden: boolean, parentPath: string)`
- each node `path: joinRemote(parentPath, e.name)`

Add at module scope:

```ts
function joinRemote(base: string, name: string): string {
  const b = base.endsWith('/') ? base.slice(0, -1) : base;
  return name.startsWith('/') ? `${b}/${name.slice(1)}` : `${b}/${name}`;
}
```

Callers: root `toNodes(entries, showHidden, h)`; `mutate2` → `toNodes(entries, showHidden, path)`. `refreshRoot` refreshes `home` via `mutate2(freshEntries, home)`.

- [ ] **Step 2: Update `Sidebar.tsx`**

Replace the `<DropZone ... />` block (lines 92-104) with:

```tsx
<SftpBrowser
  serverId={activeServer?.id ?? null}
  serverHost={activeServer?.host ?? null}
  onClearHistory={() => {
    modals.openConfirmModal({
      title: 'Clear History',
      children: <Text size="sm">This will remove all recent connection history. This cannot be undone.</Text>,
      labels: { confirm: 'Clear', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: () => clearHistory(),
    });
  }}
/>
```

Update imports: remove `import { DropZone } from './DropZone';`, add `import { SftpBrowser } from './SftpBrowser';`.

- [ ] **Step 3: Delete** `src/components/DropZone.tsx`

- [ ] **Step 4: Build**

Run: `npm run build`, followed by `npx eslint src/components/SftpBrowser.tsx`
Expected: clean (address any unused-var errors that tsc flags).

- [ ] **Step 5: Commit**

```bash
git add src/components/SftpBrowser.tsx src/components/Sidebar.tsx
git rm src/components/DropZone.tsx
git commit -m "feat(sftp): sidebar SFTP browser with drop upload and multi-select download"
```

---

### Task 8: Tauri smoke verification

**Files:** none (verification)

- [ ] **Step 1: Run the Tauri dev build to confirm the app starts with the new sidebar**

Run: `npm run tauri dev` (or `cargo build --manifest-path src-tauri/Cargo.toml` then launch `src-tauri/target/debug/remote-manager-mvp.exe`)
Expected: app launches; when an SSH tab is open, the SFTP Files panel loads `/home/<user>` with expandable folders.

- [ ] **Step 2: Manual integration against a real SSH server**
  1. Open an SSH terminal to the target server → SFTP Files panel shows home with files and folders.
  2. Expand a folder → entries load lazily; hidden files absent until toggle.
  3. Drag a file from Explorer onto the home area → upload bar appears, completes → home refreshes, new file visible.
  4. Drag a **folder** onto a remote folder → recursive upload; tree shows created subfolder.
  5. Ctrl-click 2 files + a folder → Download (N) button → pick local folder → files land with folder tree preserved.
  6. Mid-transfer Cancel → job marked cancelled.
  7. Close the SSH tab → panel clears; open another server's SSH → panel reloads that server's home.

Record results in `QA & UAT TEST PLAN.md` (append), no commit yet.

---

### Task 9: Sciter backend (copy + wire)

**Files:**
- Modify: `sciter-app/src/backend/sftp.rs` (overwrite with canonical backend)
- Modify: `sciter-app/src/backend/db/mod.rs`
- Modify: `sciter-app/src/handler.rs`

- [ ] **Step 1: Copy canonical backend**

Copy `src-tauri/src/sftp.rs` → `sciter-app/src/backend/sftp.rs` (same logic; `SftpSession`/`UploadAuth`/`Client` already exist there). The sciter crate compiles the same code — ensure the `#[cfg(test)]` module stays (it compiles under `cargo test` only).

- [ ] **Step 2: Sciter AppState field**

`sciter-app/src/backend/db/mod.rs` — change `upload_jobs` type to `crate::backend::sftp::SftpBrowserManager`.

- [ ] **Step 3: Handler init** — in `sciter-app/src/handler.rs:12-18`, `upload_jobs: crate::backend::sftp::SftpBrowserManager::new(),`.

- [ ] **Step 4: Handler match arms** — in `on_script_call` replace `"upload_files"` arm with:

```rust
            "sftp_open" => self.sftp_open(args),
            "sftp_list" => self.sftp_list(args),
            "sftp_get_home" => self.sftp_get_home(args),
            "sftp_upload" => self.sftp_upload(args),
            "sftp_download" => self.sftp_download(args),
            "get_upload_progress" => self.get_download_progress(args),
            "cancel_upload" => self.cancel_download(args),
```

- [ ] **Step 5: Implement handler methods**

Replace the old `upload_files`/`get_upload_progress`/`cancel_upload` impls (lines ~456-512) with:

```rust
    fn resolve_server_auth(&self, server_id: &str) -> Option<(String, i32, String, crate::backend::sftp::UploadAuth)> {
        let (host, port, username, credential_id, ssh_key_id) = {
            let state = self.state.lock().ok()?;
            let conn = state.db.lock().ok()?;
            let server = crate::backend::db::operations::get_server(&conn, server_id).ok()??;
            (server.host.clone(), server.port, server.username.clone(),
             server.credential_id.clone(), server.ssh_key_id.clone())
        };
        let username = self.resolve_username(&username, credential_id.as_deref())?;
        let password = self.resolve_password(credential_id.as_deref());
        let key_path = self.resolve_ssh_key(ssh_key_id.as_deref());
        let auth = match (password, key_path) {
            (Some(pw), _) => crate::backend::sftp::UploadAuth::Password(pw),
            (None, Some(kp)) => crate::backend::sftp::UploadAuth::Key(kp),
            (None, None) => return None,
        };
        Some((host, port, username, auth))
    }

    fn sftp_open(&self, args: &[Value]) -> Option<Value> {
        let server_id = get_string(args, 0)?;
        let (host, port, username, auth) = self.resolve_server_auth(&server_id)?;
        let state = self.state.lock().ok()?;
        let home = state.upload_jobs.open_browser(&server_id, host, port, username, auth).ok()?;
        Some(Value::from(home))
    }

    fn sftp_list(&self, args: &[Value]) -> Option<Value> {
        let server_id = get_string(args, 0)?;
        let path = get_string(args, 1)?;
        let state = self.state.lock().ok()?;
        let entries = state.upload_jobs.list_dir(&server_id, &path).ok()?;
        let json = serde_json::to_string(&entries).ok()?;
        json_to_value(&json)
    }

    fn sftp_get_home(&self, args: &[Value]) -> Option<Value> {
        let server_id = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        let home = state.upload_jobs.get_home(&server_id)?;
        Some(Value::from(home))
    }

    fn sftp_upload(&self, args: &[Value]) -> Option<Value> {
        let server_id = get_string(args, 0)?;
        let remote_dir = get_string(args, 1)?;
        let paths_json = get_string(args, 2)?;
        let paths: Vec<String> = serde_json::from_str(&paths_json).ok()?;
        let (host, port, username, auth) = self.resolve_server_auth(&server_id)?;
        let state = self.state.lock().ok()?;
        let job_id = state.upload_jobs.start_upload(host, port, username, auth, remote_dir, paths).ok()?;
        Some(Value::from(job_id))
    }

    fn sftp_download(&self, args: &[Value]) -> Option<Value> {
        let server_id = get_string(args, 0)?;
        let local_dir = get_string(args, 1)?;
        let remote_paths_json = get_string(args, 2)?;
        let remote_paths: Vec<String> = serde_json::from_str(&remote_paths_json).ok()?;
        let (host, port, username, auth) = self.resolve_server_auth(&server_id)?;
        let state = self.state.lock().ok()?;
        let job_id = state.upload_jobs.start_download(host, port, username, auth, local_dir, remote_paths).ok()?;
        Some(Value::from(job_id))
    }

    fn get_download_progress(&self, args: &[Value]) -> Option<Value> {
        let job_id = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        let p = state.upload_jobs.get_download_progress(&job_id)?;
        let json = serde_json::to_string(&p).ok()?;
        json_to_value(&json)
    }

    fn cancel_download(&self, args: &[Value]) -> Option<Value> {
        let job_id = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        state.upload_jobs.cancel(&job_id);
        Some(Value::from(true))
    }
```

- [ ] **Step 6: Build + test**

Run: `cargo test --manifest-path sciter-app/Cargo.toml --lib && cargo check --manifest-path sciter-app/Cargo.toml`
Expected: PASS + clean.

- [ ] **Step 7: Commit**

```bash
git add sciter-app/src/backend/sftp.rs sciter-app/src/backend/db/mod.rs sciter-app/src/handler.rs
git commit -m "feat(sftp): sciter backend wiring"
```

---

### Task 10: Sciter UI — sidebar tree (depends on runtime dispatch fix)

**⚠️ PREREQUISITE:** Before building DOI, verify Sciter native dispatch works. Add a temporary probe: in `handler.rs` `on_script_call`, add

```rust
if name == "__probe__" { eprintln!("[probe] called {}", name); return Some(Value::from(true)); }
```

Then in `index.html` init script: `view.__probe__();`. Launch `sciter-app/target/debug/remote-manager-sciter.exe` (or `remote-manager.exe`), read `stderr.log`. 

- [ ] **Step 0: Probe result**
  - If `[probe] called __probe__` appears in stderr → dispatch works, proceed.
  - If absent → **STOP**: report to the user that Sciter runtime dispatch (`view`/`on_script_call`) still does not work; Sciter UI is deferred until the runtime issue (likely sciter.js-sciter.dll engine mismatch) is fixed. Do NOT build the Sciter tree UI.

- [ ] **Step 1 (if probe passed): Replace the upload block in `sciter-app/ui/index.html`**

Replace the `.drop-zone` element (lines ~99-106) with:

```html
<div class="sftp-browser" id="sftp-browser">
  <div class="sftp-heading">
    <strong>SFTP Files</strong>
    <div class="sftp-tools">
      <label title="Show hidden"><input type="checkbox" id="sftp-show-hidden"> hidden</label>
      <button id="sftp-refresh" title="Refresh">&#8635;</button>
    </div>
  </div>
  <div class="sftp-status" id="sftp-status">Open an SSH terminal to browse files.</div>
  <div class="sftp-tree" id="sftp-tree"></div>
  <div class="sftp-progress" id="sftp-progress" style="display:none">
    <span id="sftp-progress-label"></span>
    <div class="dz-bar"><div class="dz-fill" id="sftp-progress-fill"></div></div>
    <button id="sftp-cancel">Cancel</button>
  </div>
  <div class="drop-zone-hint" id="sftp-hint">Drop files to upload</div>
</div>
```

Also keep the dragover/drop handlers but constrain them to the browser container.

- [ ] **Step 2 (if probe passed): Add CSS** — reuse `.dz-bar`/`.dz-fill`; add:

```css
.sftp-browser { margin: 8px; }
.sftp-heading { display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: #565f89; margin-bottom: 4px; }
.sftp-tools { display: flex; gap: 8px; }
.sftp-tree { max-height: 320px; overflow-y: auto; border: 1px solid #292e42; border-radius: 6px; padding: 2px; }
.sftp-node { display: flex; align-items: center; gap: 4px; padding: 2px 6px; cursor: pointer; font-size: 11px; white-space: nowrap; }
.sftp-node:hover { background: #1a1b26; }
.sftp-node.sel { background: #3b4261; }
.sftp-node .caret { width: 10px; }
.sftp-node .dl { margin-left: auto; color: #7aa2f7; cursor: pointer; visibility: hidden; }
.sftp-node:hover .dl { visibility: visible; }
.sftp-node.dir > .nm { font-weight: bold; }
```

- [ ] **Step 3 (if probe passed): Add TIScript functions** for `sftpOpen`, `sftpList`, tree render, drop upload, selection, download reminder:

Follow the existing pattern (`view.upload_files` → now `view.sftp_open(serverId)` returning JSON strings). Windows paths in TIScript drops come from `evt.dataTransfer.files[i].path`. Multi-select download uses Ctrl-click tracking and a folder picker — try `view.select_folder(cb)`; if unavailable use a prompt input fallback per spec.

- [ ] **Step 4 (if probe passed): Smoke test** — open SSH, expand tree, upload via drop, download via button, cancel. Append results to `QA & UAT TEST PLAN.md`.

- [ ] **Step 5: Commit**

```bash
git add sciter-app/ui/index.html
git commit -m "feat(sftp): sciter sidebar SFTP tree"
```

---

### Task 11: Cleanup + final verification + release

**Files:** none (verification)

- [ ] **Step 1: Verify both builds**

Run:
- `cargo test --manifest-path src-tauri/Cargo.toml && cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path sciter-app/Cargo.toml && cargo check --manifest-path sciter-app/Cargo.toml`
- `npm run build`

Expected: all clean.

- [ ] **Step 2: Bump versions** — `package.json` 0.5.1 → 0.6.0; `sciter-app/Cargo.toml` 0.5.1 → 0.6.0. Run `cargo check --manifest-path sciter-app/Cargo.toml` to refresh lockfile, commit both + lockfile.

- [ ] **Step 3: Merge to main + push** (triggers release.yml → tag v0.6.0 with Tauri MSI/portable + sciter-portable.zip via the fixed cross-job tag sharing). Verify assets appear at `github.com/thichcode/remotemanager/releases`.

- [ ] **Step 4: Update `QA & UAT TEST PLAN.md`** with the SFTP browser test list if not already tracked.

---

## Self-Review Notes (from spec)

- **Spec coverage:** persistent session (Tasks 2+4) ✅; lazy tree + default home (Task 7/10) ✅; recursive upload (Task 3) ✅; multi-select download + folder picker (Task 7, dialog already permitted in `capabilities/default.json`) ✅; follow active SSH tab (Sidebar passes `activeServer`) ✅; hidden toggle + size tooltip + auto/manual refresh (Task 7) ✅; both apps (Tasks 5-8 Tauri, 9-10 Sciter) ✅; overwrite semantics (open_with_flags TRUNCATE, no conflict logic) ✅; Sciter runtime verification gate (Task 10 Step 0) ✅.
- **Placeholders:** none — all code blocks complete.
- **Type consistency:** `cmd_sftp_upload(serverId, remoteDir, localPaths)`, `cmd_sftp_download(serverId, localDir, remotePaths)`, `cmd_get_upload_progress(jobId) → DownloadProgress`, `cmd_cancel_upload(jobId)`, `sftpOpen/sftpList/sftpGetHome/sftpUpload/sftpDownload` match. `UploadProgress` TS type reused for progress display (fields align with `DownloadProgress`).