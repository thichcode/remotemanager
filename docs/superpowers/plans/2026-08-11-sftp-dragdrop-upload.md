# SFTP Drag/Drop File Upload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add drag-and-drop SFTP file upload to Linux servers from the left sidebar in both the Sciter and Tauri builds, following the active SSH tab.

**Architecture:** A shared pure-Rust SFTP uploader built on `russh` + `russh-sftp` sits in `backend/sftp.rs` in each app. The native command layer exposes `upload_files`, `get_upload_progress`, and `cancel_upload`. The UI replaces the "Recent" area (Tauri) / adds a drop zone at the bottom of the left panel (Sciter) that is enabled only when an SSH tab is active, and polls progress every 250ms.

**Tech Stack:** Rust, `russh` (0.62), `russh-sftp` (2.3), `tokio`, serde_json. Frontend: TIScript (Sciter) + React/Mantine (Tauri).

**Spec:** `docs/superpowers/specs/2026-08-11-sftp-dragdrop-upload-design.md`

---

## File Structure

**Sciter app (`sciter-app/`):**
- Create: `sciter-app/src/backend/sftp.rs` — russh SFTP uploader + job registry
- Modify: `sciter-app/src/backend/mod.rs` — add `pub mod sftp;`
- Modify: `sciter-app/src/backend/db/mod.rs` — add `upload_jobs` to `AppState`
- Modify: `sciter-app/src/handler.rs` — add `upload_files`, `get_upload_progress`, `cancel_upload` commands
- Modify: `sciter-app/ui/index.html` — drop zone + progress + cancel
- Modify: `sciter-app/Cargo.toml` — add `russh`, `russh-sftp`, `tokio` features

**Tauri app:**
- Create: `src-tauri/src/sftp.rs` — russh SFTP uploader + job registry (mirror of Sciter)
- Modify: `src-tauri/src/lib.rs` — `mod sftp;` + register commands
- Modify: `src-tauri/src/db/mod.rs` — add `upload_jobs` to `AppState`
- Create: `src-tauri/src/commands/uploads.rs` — `cmd_upload_files`, `cmd_get_upload_progress`, `cmd_cancel_upload`
- Modify: `src-tauri/Cargo.toml` — add `russh`, `russh-sftp`, `tokio` features
- Modify: `src/components/Sidebar.tsx` — replace Recent section with drop zone
- Modify: `src/components/DropZone.tsx` (create) — drop zone component
- Modify: `src/services/tauri.ts` — invoke wrappers
- Modify: `src/types/index.ts` — `UploadJob`, `UploadProgress` types
- Modify: `src/store/useStore.ts` — upload state if needed (or local component state)

**Release/CI:**
- Create: `sciter-app/.gitignore` — exclude `target/`, `*.dll`, logs
- Modify: `.github/workflows/release.yml` — add Sciter build + upload
- Modify: `sciter-app/Cargo.toml` — metadata version for release

---

### Task 1: Add russh dependencies to Sciter app

**Files:**
- Modify: `sciter-app/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Edit `sciter-app/Cargo.toml`, add to `[dependencies]`:

```toml
russh = "0.62"
russh-sftp = "2.3"
```

And update the `tokio` line to:

```toml
tokio = { version = "1", features = ["rt", "net", "io-util", "sync", "time", "macros"] }
```

- [ ] **Step 2: Verify it resolves**

Run: `cargo fetch`
Expected: downloads russh + russh-sftp + transitive deps with no version conflict.

- [ ] **Step 3: Commit**

```bash
git add sciter-app/Cargo.toml
git commit -m "chore(sciter): add russh and russh-sftp dependencies"
```

---

### Task 2: Sciter SFTP uploader module (`backend/sftp.rs`)

**Files:**
- Create: `sciter-app/src/backend/sftp.rs`

- [ ] **Step 1: Write the uploader module**

Create `sciter-app/src/backend/sftp.rs` with the following content. This provides:
- `UploadAuth` enum (password or SSH key path)
- `UploadJobState` / `UploadProgress` structs
- `UploadJob` registry (`start_upload`, `get_progress`, `cancel`)
- `upload_file` async core using russh + russh-sftp, with per-chunk progress + cancellation

```rust
//! SFTP file upload for the drag/drop feature.
//!
//! Uses russh + russh-sftp (pure Rust, async). Each upload batch opens a fresh
//! SSH connection, uploads each file into the remote home directory, reports
//! progress through a shared struct, and honours cancellation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use log::{info, warn};

#[derive(Clone)]
pub enum UploadAuth {
    Password(String),
    Key(String), // path to private key
}

#[derive(Clone, Debug)]
pub struct UploadProgress {
    pub state: String,          // "uploading" | "done" | "error" | "cancelled"
    pub current_file: String,
    pub file_index: usize,
    pub total_files: usize,
    pub bytes_sent: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
}

struct UploadJob {
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<UploadProgress>>,
}

pub struct UploadManager {
    jobs: Mutex<HashMap<String, UploadJob>>,
}

impl Default for UploadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UploadManager {
    pub fn new() -> Self {
        UploadManager {
            jobs: Mutex::new(HashMap::new()),
        }
    }

    /// Spawn an upload batch on a background thread. Returns a job id.
    pub fn start_upload(
        &self,
        host: String,
        port: i32,
        username: String,
        auth: UploadAuth,
        local_paths: Vec<String>,
    ) -> Result<String, String> {
        if local_paths.is_empty() {
            return Err("No files to upload".into());
        }
        for p in &local_paths {
            if !Path::new(p).is_file() {
                return Err(format!("Not a file: {}", p));
            }
        }

        let job_id = uuid::Uuid::new_v4().to_string();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(UploadProgress {
            state: "uploading".into(),
            current_file: String::new(),
            file_index: 0,
            total_files: local_paths.len(),
            bytes_sent: 0,
            total_bytes: 0,
            error: None,
        }));

        {
            let mut jobs = self.jobs.lock().map_err(|e| e.to_string())?;
            jobs.insert(
                job_id.clone(),
                UploadJob {
                    cancel_flag: cancel_flag.clone(),
                    progress: progress.clone(),
                },
            );
        }

        let jobs_handle = self.jobs.clone();
        std::thread::Builder::new()
            .name("sftp-upload".into())
            .spawn(move || {
                let result = run_upload(
                    &host, port, &username, auth, &local_paths,
                    cancel_flag.clone(), progress.clone(),
                );
                let mut p = progress.lock().unwrap();
                match result {
                    Ok(()) => {
                        if p.state != "cancelled" {
                            p.state = "done".into();
                        }
                    }
                    Err(e) => {
                        p.state = "error".into();
                        p.error = Some(e);
                    }
                }
                drop(p);
                if let Ok(mut jobs) = jobs_handle.lock() {
                    jobs.remove(&job_id);
                }
            })
            .map_err(|e| format!("Failed to spawn upload thread: {}", e))?;

        Ok(job_id)
    }

    pub fn get_progress(&self, job_id: &str) -> Option<UploadProgress> {
        let jobs = self.jobs.lock().ok()?;
        let job = jobs.get(job_id)?;
        Some(job.progress.lock().unwrap().clone())
    }

    pub fn cancel(&self, job_id: &str) {
        if let Ok(jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get(job_id) {
                job.cancel_flag.store(true, Ordering::SeqCst);
            }
        }
    }
}

struct Client;

impl russh::client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Upload a list of files into the remote home directory, streaming progress.
fn run_upload(
    host: &str,
    port: i32,
    username: &str,
    auth: UploadAuth,
    local_paths: &[String],
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<UploadProgress>>,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    rt.block_on(upload_async(
        host, port, username, auth, local_paths, cancel_flag, progress,
    ))
}

async fn upload_async(
    host: &str,
    port: i32,
    username: &str,
    auth: UploadAuth,
    local_paths: &[String],
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<UploadProgress>>,
) -> Result<(), String> {
    use russh_sftp::client::SftpSession;
    use russh_sftp::protocol::OpenFlags;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let config = russh::client::Config::default();
    let mut session = russh::client::connect(
        Arc::new(config),
        (host.to_string(), port as u16),
        Client,
    )
    .await
    .map_err(|e| format!("SSH connect failed: {}", e))?;

    let authed = match &auth {
        UploadAuth::Password(pw) => session
            .authenticate_password(username, pw)
            .await
            .map_err(|e| format!("SSH auth failed: {}", e))?,
        UploadAuth::Key(path) => session
            .authenticate_publickey_files(username, &[path.as_str()])
            .await
            .map_err(|e| format!("SSH key auth failed: {}", e))?,
    };
    if !authed.success() {
        return Err("Authentication rejected by server".into());
    }

    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("Failed to open session channel: {}", e))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| format!("Failed to open SFTP subsystem: {}", e))?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP init failed: {}", e))?;

    // Remote home directory
    let home = sftp
        .canonicalize(".")
        .await
        .unwrap_or_else(|_| ".".to_string());

    for (idx, local_path) in local_paths.iter().enumerate() {
        if cancel_flag.load(Ordering::SeqCst) {
            {
                let mut p = progress.lock().unwrap();
                p.state = "cancelled".into();
            }
            return Err("Upload cancelled".into());
        }

        let file_name = Path::new(local_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let remote_path = format!("{}/{}", home, file_name);

        let mut p = progress.lock().unwrap();
        p.current_file = file_name.clone();
        p.file_index = idx;
        drop(p);

        let mut local = tokio::fs::File::open(local_path)
            .await
            .map_err(|e| format!("Failed to open local file {}: {}", local_path, e))?;
        let total = local
            .metadata()
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        {
            let mut p = progress.lock().unwrap();
            p.bytes_sent = 0;
            p.total_bytes = total;
        }

        let mut remote = sftp
            .open_with_flags(
                &remote_path,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await
            .map_err(|e| format!("Failed to open remote file {}: {}", remote_path, e))?;

        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel_flag.load(Ordering::SeqCst) {
                {
                    let mut p = progress.lock().unwrap();
                    p.state = "cancelled".into();
                }
                return Err("Upload cancelled".into());
            }
            let n = local
                .read(&mut buf)
                .await
                .map_err(|e| format!("Failed to read local file: {}", e))?;
            if n == 0 {
                break;
            }
            remote
                .write_all(&buf[..n])
                .await
                .map_err(|e| format!("Failed to write remote file: {}", e))?;
            let mut p = progress.lock().unwrap();
            p.bytes_sent += n as u64;
        }
        remote
            .flush()
            .await
            .map_err(|e| format!("Failed to flush remote file: {}", e))?;
        remote.shutdown().await.ok();

        info!("Uploaded {} ({}/{} files)", file_name, idx + 1, local_paths.len());
    }

    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p remote-manager` (workdir `sciter-app`), after adding `pub mod sftp;` to `sciter-app/src/backend/mod.rs`:

```rust
pub mod db;
pub mod paths;
pub mod history;
pub mod sshkeys;
pub mod sessions;
pub mod security;
pub mod rdp;
pub mod terminal;
pub mod sftp;
```

Expected: compiles (warnings ok). If `authenticate_publickey_files` signature differs in russh 0.62, adjust to the exact method name — check `docs.rs/russh/latest/russh/client/struct.Handle.html` and use `authenticate_publickey` with a loaded key if needed.

- [ ] **Step 3: Commit**

```bash
git add sciter-app/src/backend/sftp.rs sciter-app/src/backend/mod.rs
git commit -m "feat(sciter): add SFTP upload manager"
```

---

### Task 3: Wire uploads into Sciter AppState + handler

**Files:**
- Modify: `sciter-app/src/backend/db/mod.rs`
- Modify: `sciter-app/src/handler.rs`

- [ ] **Step 1: Add `upload_jobs` to AppState**

In `sciter-app/src/backend/db/mod.rs`, add the field to `AppState`:

```rust
pub struct AppState {
    pub db: Mutex<Connection>,
    pub sessions: Arc<crate::backend::sessions::SessionManager>,
    pub rdp_sessions: Mutex<std::collections::HashMap<u16, tokio::sync::oneshot::Sender<()>>>,
    pub terminal_sessions: Mutex<std::collections::HashMap<u16, tokio::sync::oneshot::Sender<()>>>,
    pub upload_jobs: crate::backend::sftp::UploadManager,
}
```

Check the existing field declarations in the file first — `rdp_sessions` and `terminal_sessions` may have slightly different declared types; keep their existing types and just add `upload_jobs`.

- [ ] **Step 2: Initialize it in `AppHandler::new`**

In `sciter-app/src/handler.rs`, update `AppHandler::new`:

```rust
let state = AppState {
    db: Mutex::new(conn),
    sessions: Arc::new(crate::backend::sessions::SessionManager::new()),
    rdp_sessions: Mutex::new(std::collections::HashMap::new()),
    terminal_sessions: Mutex::new(std::collections::HashMap::new()),
    upload_jobs: crate::backend::sftp::UploadManager::new(),
};
```

- [ ] **Step 3: Register command names in `on_script_call`**

In `sciter-app/src/handler.rs`, add to the `match name` block:

```rust
"upload_files" => self.upload_files(args),
"get_upload_progress" => self.get_upload_progress(args),
"cancel_upload" => self.cancel_upload(args),
```

- [ ] **Step 4: Implement the three command methods**

Add these methods to `impl AppHandler` in `sciter-app/src/handler.rs` (after `open_rdp_session`/`close_rdp_session`):

```rust
fn upload_files(&self, args: &[Value]) -> Option<Value> {
    let server_id = get_string(args, 0)?;
    let local_paths_json = get_string(args, 1)?;
    let paths: Vec<String> = serde_json::from_str(&local_paths_json).ok()?;

    // Resolve server connection info
    let state = self.state.lock().ok()?;
    let conn = state.db.lock().ok()?;
    let server = crate::backend::db::operations::get_server(&conn, &server_id).ok()??;

    let username = self.resolve_username(&server.username, server.credential_id.as_deref())?;
    let password = self.resolve_password(server.credential_id.as_deref());
    let key_path = self.resolve_ssh_key(server.ssh_key_id.as_deref());

    let auth = match (password, key_path) {
        (Some(pw), _) => crate::backend::sftp::UploadAuth::Password(pw),
        (None, Some(kp)) => crate::backend::sftp::UploadAuth::Key(kp),
        (None, None) => return None,
    };

    let job_id = state.upload_jobs.start_upload(
        server.host.clone(),
        server.port,
        username,
        auth,
        paths,
    ).ok()?;

    Some(Value::from(job_id))
}

fn get_upload_progress(&self, args: &[Value]) -> Option<Value> {
    let job_id = get_string(args, 0)?;
    let state = self.state.lock().ok()?;
    let p = state.upload_jobs.get_progress(&job_id)?;
    let json = serde_json::to_string(&p).ok()?;
    json_to_value(&json)
}

fn cancel_upload(&self, args: &[Value]) -> Option<Value> {
    let job_id = get_string(args, 0)?;
    let state = self.state.lock().ok()?;
    state.upload_jobs.cancel(&job_id);
    Some(Value::from(true))
}
```

**Note:** verify `ServerRow` field names in `sciter-app/src/backend/db/operations.rs` — the server struct may use `credential_id`/`ssh_key_id` (match what `open_ssh_terminal` uses at handler.rs:345-360).

- [ ] **Step 5: Compile check**

Run: `cargo check` (workdir `sciter-app`)
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add sciter-app/src/backend/db/mod.rs sciter-app/src/handler.rs
git commit -m "feat(sciter): wire SFTP upload commands into handler"
```

---

### Task 4: Sciter UI drop zone

**Files:**
- Modify: `sciter-app/ui/index.html`

- [ ] **Step 1: Add drop zone to the sidebar**

In `sciter-app/ui/index.html`, after the `#server-list` div (inside `.sidebar`, line ~87), add:

```html
<div class="drop-zone" id="drop-zone">
  <div class="drop-zone-body" id="drop-zone-body">
    <strong>Drop files to upload</strong>
    <div class="drop-zone-hint">to the active SSH server</div>
  </div>
  <div class="drop-zone-progress" id="drop-zone-progress" style="display:none">
    <div class="dz-file" id="dz-file"></div>
    <div class="dz-bar"><div class="dz-fill" id="dz-fill"></div></div>
    <div class="dz-actions">
      <button class="btn btn-danger" onclick="cancelUpload()" id="dz-cancel">Cancel</button>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Add CSS for the drop zone**

In the `<style>` block (before the closing `</style>`), add:

```css
.drop-zone { margin: 8px; padding: 16px 8px; border: 2px dashed #3b4261; border-radius: 8px; text-align: center; color: #565f89; cursor: pointer; }
.drop-zone.disabled { opacity: 0.4; cursor: not-allowed; }
.drop-zone.dragover { border-color: #7aa2f7; background: #1a1b26; color: #7aa2f7; }
.drop-zone-body strong { display: block; font-size: 12px; }
.drop-zone-hint { font-size: 11px; margin-top: 4px; }
.drop-zone-progress { margin-top: 8px; text-align: left; }
.dz-file { font-size: 11px; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dz-bar { height: 6px; background: #292e42; border-radius: 3px; overflow: hidden; }
.dz-fill { height: 100%; width: 0%; background: #7aa2f7; transition: width 0.15s; }
.dz-actions { margin-top: 6px; text-align: center; }
```

- [ ] **Step 3: Add upload state + JS logic**

In the script section, add near the state vars:

```javascript
var uploads = {};    // jobId -> { interval, lastState }
var currentUpload = null;  // jobId
```

Add these functions before the `// ---------- Init ----------` block:

```javascript
function uploadServerForZone() {
  // Server of the active SSH tab, or null
  if (!activeSessionId) return null;
  var s = sessions[activeSessionId];
  if (!s || s.type !== "ssh" || !s.serverId) return null;
  for (var i = 0; i < servers.length; i++) if (servers[i].id === s.serverId) return servers[i];
  return null;
}

function refreshDropZone() {
  var zone = document.getElementById("drop-zone");
  var server = uploadServerForZone();
  if (!server) {
    zone.classList.add("disabled");
    document.getElementById("drop-zone-body").innerHTML =
      "<strong>Drop files to upload</strong><div class='drop-zone-hint'>Open an SSH terminal to upload</div>";
  } else {
    zone.classList.remove("disabled");
    document.getElementById("drop-zone-body").innerHTML =
      "<strong>Drop files to upload</strong><div class='drop-zone-hint'>to " + escapeHtml(server.host) + "</div>";
  }
}
```

- [ ] **Step 4: Hook zone enable/disable into session changes**

Call `refreshDropZone()` inside `activateSession` and `closeSession` (after `renderTabs()`), and once at init.

- [ ] **Step 5: Handle drop events**

Sciter/TIScript supports DOM drag events. Add event listeners at init:

```javascript
(function () {
  var zone = document.getElementById("drop-zone");
  zone.on("dragover", function () {
    if (!uploadServerForZone()) return;
    this.classList.add("dragover");
    return true; // allow drop
  });
  zone.on("dragleave", function () {
    this.classList.remove("dragover");
  });
  zone.on("drop", function (evt) {
    this.classList.remove("dragover");
    var server = uploadServerForZone();
    if (!server) return;
    var files = evt.dataTransfer ? evt.dataTransfer.files : null;
    if (!files || files.length === 0) return;
    var paths = [];
    for (var i = 0; i < files.length; i++) {
      if (files[i].path) paths.push(files[i].path);
    }
    if (paths.length === 0) return;
    startUpload(server.id, paths);
  });
  refreshDropZone();
})();
```

**Verification note:** Sciter's `dataTransfer.files[i].path` support must be confirmed. If `files[i].path` is unavailable, fall back to `files[i].name` + a file dialog via `view.selectFile(#open, ...)` per file. Adjust in implementation if the runtime differs.

- [ ] **Step 6: Implement `startUpload`, poll, and `cancelUpload`**

```javascript
function startUpload(serverId, paths) {
  var json = JSON.stringify(paths);
  var jobId = view.upload_files(serverId, json);
  if (!jobId) { toast("Upload failed to start"); return; }
  currentUpload = jobId;
  uploads[jobId] = {};
  showUploadProgress();
  uploads[jobId].interval = setInterval(function () {
    var p = view.get_upload_progress(jobId);
    if (!p) { stopUploadPoll(jobId); return; }
    p = JSON.parse(p);
    renderUploadProgress(p);
    if (p.state === "done" || p.state === "error" || p.state === "cancelled") {
      stopUploadPoll(jobId);
      if (p.state === "done") { toast("Upload complete"); }
      else if (p.state === "error") { toast("Upload failed: " + (p.error || "")); }
      else { toast("Upload cancelled"); }
      hideUploadProgress();
    }
  }, 250);
}

function stopUploadPoll(jobId) {
  var u = uploads[jobId];
  if (u && u.interval) { clearInterval(u.interval); }
  delete uploads[jobId];
}

function renderUploadProgress(p) {
  document.getElementById("dz-file").textContent =
    "(" + (p.file_index + 1) + "/" + p.total_files + ") " + p.current_file;
  var pct = p.total_bytes > 0 ? Math.round(p.bytes_sent * 100 / p.total_bytes) : 0;
  document.getElementById("dz-fill").style.width = pct + "%";
}

function showUploadProgress() {
  document.getElementById("drop-zone-body").style.display = "none";
  document.getElementById("drop-zone-progress").style.display = "block";
}

function hideUploadProgress() {
  currentUpload = null;
  document.getElementById("drop-zone-progress").style.display = "none";
  document.getElementById("drop-zone-body").style.display = "block";
  refreshDropZone();
}

function cancelUpload() {
  if (!currentUpload) return;
  view.cancel_upload(currentUpload);
}
```

- [ ] **Step 7: Manual smoke test**

Run the Sciter app, open an SSH terminal to a Linux box, drop a small file on the zone. Expected: progress appears, file lands in `~`, "Upload complete" toast.

- [ ] **Step 8: Commit**

```bash
git add sciter-app/ui/index.html
git commit -m "feat(sciter): add drag/drop SFTP upload zone to sidebar"
```

---

### Task 5: Tauri SFTP uploader module + commands

**Files:**
- Create: `src-tauri/src/sftp.rs`
- Create: `src-tauri/src/commands/uploads.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/db/mod.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add dependencies to Tauri Cargo.toml**

Add to `[dependencies]`:

```toml
russh = "0.62"
russh-sftp = "2.3"
```

And change tokio to `features = ["full"]` (already full in this project — verify; if so, no change needed).

- [ ] **Step 2: Create `src-tauri/src/sftp.rs`**

Copy `sciter-app/src/backend/sftp.rs` (Task 2) verbatim — it is app-agnostic. No changes needed to the module itself.

- [ ] **Step 3: Add `upload_jobs` to Tauri AppState**

In `src-tauri/src/db/mod.rs`, add to `AppState`:

```rust
pub struct AppState {
    pub db: std::sync::Mutex<rusqlite::Connection>,
    pub sessions: std::sync::Arc<crate::sessions::SessionManager>,
    pub rdp_sessions: std::sync::Mutex<std::collections::HashMap<u16, tokio::sync::oneshot::Sender<()>>>,
    pub upload_jobs: crate::sftp::UploadManager,
}
```

Match existing field types (check the file). In `src-tauri/src/lib.rs`, initialize it:

```rust
let state = AppState {
    db: std::sync::Mutex::new(conn),
    sessions: std::sync::Arc::new(crate::sessions::SessionManager::new()),
    rdp_sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
    upload_jobs: crate::sftp::UploadManager::new(),
};
```

- [ ] **Step 4: Create `src-tauri/src/commands/uploads.rs`**

```rust
use tauri::State;

use crate::db::AppState;
use crate::sftp::UploadAuth;

#[tauri::command]
pub fn cmd_upload_files(
    state: State<AppState>,
    server_id: String,
    local_paths: Vec<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let server = crate::db::operations::get_server(&conn, &server_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Server not found".to_string())?;
    drop(conn);

    let username = crate::commands::ssh::resolve_username(
        &state,
        server.username.clone(),
        server.credential_id.as_deref(),
    )?;
    let password = crate::commands::ssh::resolve_credential_password(&state, server.credential_id.as_deref())
        .ok()
        .flatten();
    let key_path = if let Some(kid) = server.ssh_key_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::sshkeys::get_private_key_path(&conn, kid).ok().flatten()
    } else {
        None
    };

    let auth = match (password, key_path) {
        (Some(pw), _) => UploadAuth::Password(pw),
        (None, Some(kp)) => UploadAuth::Key(kp),
        (None, None) => return Err("No password or SSH key available for this server".into()),
    };

    state.upload_jobs.start_upload(
        server.host,
        server.port,
        username,
        auth,
        local_paths,
    )
}

#[tauri::command]
pub fn cmd_get_upload_progress(
    state: State<AppState>,
    job_id: String,
) -> Result<Option<crate::sftp::UploadProgress>, String> {
    Ok(state.upload_jobs.get_progress(&job_id))
}

#[tauri::command]
pub fn cmd_cancel_upload(state: State<AppState>, job_id: String) -> Result<(), String> {
    state.upload_jobs.cancel(&job_id);
    Ok(())
}
```

**Note:** verify `ServerRow` field names in `src-tauri/src/db/operations.rs` (`credential_id`, `ssh_key_id`, `username`, `host`, `port`). Also confirm `resolve_credential_password` returns `Option<String>`; if it returns the encrypted blob that needs `security::decrypt`, call `crate::security::dpapi::decrypt` on it (check how `cmd_launch_ssh`/`cmd_open_ssh_session` obtain the plaintext password — mirror that exact pattern).

- [ ] **Step 5: Register module + commands**

In `src-tauri/src/lib.rs`:
- Add `mod sftp;` and `mod commands;` is already there; add `pub mod sftp;` (or `mod sftp;`) near other mods.
- Add to `generate_handler![...]`:

```rust
commands::uploads::cmd_upload_files,
commands::uploads::cmd_get_upload_progress,
commands::uploads::cmd_cancel_upload,
```

- [ ] **Step 6: Compile check**

Run: `cargo check` (workdir `src-tauri`)
Expected: compiles.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/sftp.rs src-tauri/src/commands/uploads.rs src-tauri/src/lib.rs src-tauri/src/db/mod.rs
git commit -m "feat(tauri): add SFTP upload commands"
```

---

### Task 6: Tauri UI drop zone (replace Recent)

**Files:**
- Create: `src/components/DropZone.tsx`
- Modify: `src/components/Sidebar.tsx`
- Modify: `src/services/tauri.ts`
- Modify: `src/types/index.ts`

- [ ] **Step 1: Add types**

In `src/types/index.ts`, add:

```typescript
export interface UploadProgress {
  state: 'uploading' | 'done' | 'error' | 'cancelled';
  current_file: string;
  file_index: number;
  total_files: number;
  bytes_sent: number;
  total_bytes: number;
  error: string | null;
}
```

- [ ] **Step 2: Add invoke wrappers**

In `src/services/tauri.ts`, add:

```typescript
export function uploadFiles(serverId: string, localPaths: string[]): Promise<string> {
  return invoke<string>('upload_files', { serverId, localPaths });
}

export function getUploadProgress(jobId: string): Promise<UploadProgress | null> {
  return invoke<UploadProgress | null>('get_upload_progress', { jobId });
}

export function cancelUpload(jobId: string): Promise<void> {
  return invoke('cancel_upload', { jobId });
}
```

Import `UploadProgress` from `../types`.

- [ ] **Step 3: Create `DropZone.tsx`**

```tsx
import { Box, Text, Progress, Button, Group, ActionIcon, Tooltip } from '@mantine/core';
import { IconUpload, IconTrash } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';
import { uploadFiles, getUploadProgress, cancelUpload } from '../services/tauri';
import { clearHistory } from '../services/tauri';
import type { UploadProgress } from '../types';

interface Props {
  activeServerId: string | null;
  activeServerHost: string | null;
  onClearHistory: () => void;
}

export function DropZone({ activeServerId, activeServerHost, onClearHistory }: Props) {
  const [dragging, setDragging] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<UploadProgress | null>(null);
  const timerRef = useRef<number | null>(null);

  const enabled = activeServerId !== null;

  const stopPolling = () => {
    if (timerRef.current !== null) { window.clearInterval(timerRef.current); timerRef.current = null; }
  };

  useEffect(() => () => { stopPolling(); }, []);

  const onDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    if (!enabled || !activeServerId) return;
    const files = Array.from(e.dataTransfer.files).map(f => f.path);
    if (files.length === 0) return;
    try {
      const id = await uploadFiles(activeServerId, files);
      setJobId(id);
      timerRef.current = window.setInterval(async () => {
        try {
          const p = await getUploadProgress(id);
          if (!p) { stopPolling(); setJobId(null); return; }
          setProgress(p);
          if (p.state === 'done' || p.state === 'error' || p.state === 'cancelled') {
            stopPolling();
            setTimeout(() => { setJobId(null); setProgress(null); }, 2500);
          }
        } catch { stopPolling(); }
      }, 250);
    } catch (err) {
      notifications.show({ title: 'Upload failed', message: String(err), color: 'red' });
    }
  };

  const pct = progress && progress.total_bytes > 0
    ? Math.round(progress.bytes_sent * 100 / progress.total_bytes)
    : 0;

  return (
    <Stack gap={4} mb="md">
      <Group justify="space-between" align="center">
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">Upload</Text>
        <Tooltip label="Clear history">
          <ActionIcon size="sm" variant="subtle" onClick={onClearHistory}>
            <IconTrash size={14} />
          </ActionIcon>
        </Tooltip>
      </Group>
      <Box
        p="xs"
        style={{
          border: dragging ? '2px dashed var(--mantine-color-blue-5)' : '2px dashed var(--mantine-color-dark-4)',
          borderRadius: 6,
          textAlign: 'center',
          opacity: enabled ? 1 : 0.4,
          cursor: enabled ? 'copy' : 'not-allowed',
          transition: 'border-color 0.15s',
        }}
        onDragOver={(e) => { if (enabled) { e.preventDefault(); setDragging(true); } }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
      >
        {jobId && progress ? (
          <>
            <Text size="xs" mb={4}>{(progress.file_index + 1)}/{progress.total_files} {progress.current_file}</Text>
            <Progress value={pct} size="sm" />
            <Group justify="center" mt={4}>
              <Button size="xs" variant="light" color="red" onClick={async () => { if (jobId) { await cancelUpload(jobId); } }}>
                Cancel
              </Button>
            </Group>
          </>
        ) : (
          <>
            <IconUpload size={18} style={{ marginBottom: 4 }} />
            <Text size="xs">{enabled ? `Drop files to upload to ${activeServerHost}` : 'Open an SSH terminal to upload'}</Text>
          </>
        )}
      </Box>
    </Stack>
  );
}
```

Note: this snippet assumes `notifications` and `Stack` are imported. Fix imports (`Stack` from `@mantine/core`, `notifications` from `@mantine/notifications`, `clearHistory` from tauri services). Also `file.path` requires Tauri v2 drag-drop path support (`e.dataTransfer.files` items expose `path` in Tauri). Confirm at implementation.

- [ ] **Step 4: Wire into `Sidebar.tsx`**

In `Sidebar.tsx`:
- Import `DropZone` and `useStore` selectors for the active session tab.
- Compute `activeServerId` / `activeServerHost` from the active session tab:

```tsx
const activeSessionTabId = useStore((s) => s.activeSessionTabId);
const sessionTabs = useStore((s) => s.sessionTabs);
const activeTab = sessionTabs.find((t) => t.id === activeSessionTabId && t.protocol === 'ssh');
const activeServer = activeTab?.serverId ? servers.find((s) => s.id === activeTab!.serverId) ?? null : null;
```

- **Replace** the `{history.length > 0 && (...)}` "Recent" block with:

```tsx
<DropZone
  activeServerId={activeServer?.id ?? null}
  activeServerHost={activeServer?.host ?? null}
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

Keep `history` loading in the store (history may still be used elsewhere, e.g. dashboard). If history is only used here, leave the store methods intact.

- [ ] **Step 5: Build frontend**

Run: `npm run build` (workdir repo root)
Expected: TypeScript compiles clean.

- [ ] **Step 6: Manual test in Tauri**

Run `npm run tauri dev`, open SSH tab, drop a file on the zone. Expected: progress + cancel work, file lands in `~`.

- [ ] **Step 7: Commit**

```bash
git add src/components/DropZone.tsx src/components/Sidebar.tsx src/services/tauri.ts src/types/index.ts
git commit -m "feat(tauri): replace Recent sidebar area with SFTP drop zone"
```

---

### Task 7: Track sciter-app in git + CI build for Sciter

**Files:**
- Create: `sciter-app/.gitignore`
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Create `sciter-app/.gitignore`**

```gitignore
target/
*.dll
*.exe
*.log
Cargo.lock
```

Note: `Cargo.lock` for a binary app is normally committed, but sciter-app is a side build; excluding avoids vendored-path drift. If you prefer locking deps, keep `Cargo.lock` — decide and be consistent.

- [ ] **Step 2: Add sciter-app build job to release.yml**

Add a second job `sciter-release` to `.github/workflows/release.yml` (after the existing `release` job) that builds the Sciter binary on Windows and attaches it to the same release. Key steps:

```yaml
  sciter-release:
    runs-on: windows-latest
    needs: release
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ env.RUST_TARGET }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: sciter-app

      - name: Build Sciter app
        run: cargo build --release --manifest-path sciter-app/Cargo.toml

      - name: Download sciter.dll
        shell: bash
        run: |
          curl -L -o sciter-sdk.zip "https://gitlab.com/sciter-engine/sciter-js-sdk/-/archive/main/sciter-js-sdk-main.zip"
          powershell -NoProfile -Command "Expand-Archive -Path sciter-sdk.zip -DestinationPath sdk"
          find sdk -name "sciter.dll" -path "*windows/x64*" -exec cp {} sciter-app/target/release/ \;

      - name: Package portable zip
        shell: bash
        run: |
          mkdir -p portable-sciter
          cp sciter-app/target/release/remote-manager.exe "portable-sciter/Remote Manager Sciter.exe"
          cp sciter-app/target/release/sciter.dll "portable-sciter/"
          cd portable-sciter
          powershell -NoProfile -Command "Compress-Archive -Path * -DestinationPath ../sciter-portable.zip -Force"

      - name: Upload to Release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release upload "${{ steps.version.outputs.tag }}" sciter-portable.zip --clobber
```

Note: `steps.version.outputs.tag` belongs to the `release` job's context — the `needs: release` dependency does NOT expose its step outputs directly. To share the tag, add to the `release` job a final step that writes the tag to an artifact or repo variable, or recompute the tag in `sciter-release` using the same bash logic (copy the "Check version changed" step). Recommend: recompute the tag with an identical `Check version changed` step in `sciter-release`.

- [ ] **Step 3: Verify workflow YAML parses**

Run: `node -e "require('yaml')"` is not available; instead open the file and confirm no obvious YAML indentation errors. (Optionally run `npx actionlint` if available.)

- [ ] **Step 4: Commit**

```bash
git add sciter-app/.gitignore .github/workflows/release.yml
git commit -m "ci: track sciter-app and build it in release workflow"
```

---

### Task 8: Version bump + push + release verification

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/tauri.conf.json` (version synced automatically by workflow)

- [ ] **Step 1: Bump version**

Edit `package.json`, bump `version` from `0.4.1` to `0.5.0` (feature release).

- [ ] **Step 2: Commit**

```bash
git add package.json
git commit -m "chore: bump version to 0.5.0"
```

- [ ] **Step 3: Push and verify release**

```bash
git push origin main
```

Expected: `.github/workflows/release.yml` triggers on the push; both `release` (Tauri MSI + portable ZIP) and `sciter-release` (Sciter portable ZIP) jobs run; GitHub Release `v0.5.0` is created with all artifacts.

**Note:** the previous push (`12853d7` docs commit) already triggered a release run that will fail at the version-check (no version change) or succeed and re-run — harmless. Confirm the new tag appears in GitHub Releases at `https://github.com/thichcode/remotemanager/releases`.

---

## Self-Review

- **Spec coverage:** upload files (Tasks 2/5), progress + cancel (Tasks 2/4/6), drop zone follows active SSH tab (Tasks 4/6), disabled without SSH tab (Tasks 4/6), files only (validated in `start_upload`), destination `~` (Task 2 `canonicalize(".")`), both apps (Tasks 2-6), push + release (Task 8).
- **Placeholders:** none — every step has concrete code or commands.
- **Type consistency:** `UploadProgress`/`UploadAuth`/`UploadManager` used identically across Tasks 2-6; command names match JS/TS invoke wrappers.
