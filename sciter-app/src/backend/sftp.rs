//! SFTP file upload for the drag/drop feature.
//!
//! Uses russh + russh-sftp (pure Rust, async). Each upload batch opens a fresh
//! SSH connection, uploads each file into the remote home directory, reports
//! progress through a shared struct, and honours cancellation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use russh_sftp::client::SftpSession;

#[derive(Clone)]
pub enum UploadAuth {
    Password(String),
    Key(String), // path to private key
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
/// File â†’ rel is its file name. Dir â†’ rel includes the subtree path.
pub fn collect_local_files(
    path: &std::path::Path,
    rel: &str,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if path.is_file() {
        // rel is already the fully-computed relative path from the caller
        // (empty only when a single file is passed at the top level).
        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let child_rel = if rel.is_empty() { name } else { rel.to_string() };
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

pub struct BrowserSession {
    pub home: String,
    cmd_tx: std::sync::mpsc::Sender<BrowseCmd>,
}

enum BrowseCmd {
    #[allow(dead_code)]
    List { path: String, reply: std::sync::mpsc::SyncSender<Result<Vec<RemoteEntry>, String>> },
    #[allow(dead_code)]
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
                let session = running.lock().ok().and_then(|mut g| g.take());
                let res = match session {
                    Some(sftp) => {
                        let res = rt.block_on(list_dir_async(&sftp, &path));
                        if let Ok(mut g) = running.lock() {
                            *g = Some(sftp);
                        }
                        res
                    }
                    None => Err("SFTP session unavailable".into()),
                };
                let _ = reply.send(res);
            }
        }
    }
    let mut guard = running.lock().ok();
    if let Some(g) = guard.as_mut() {
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
    root: &str,
    rel: &str,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut stack = vec![(root.to_string(), rel.to_string())];
    while let Some((path, rel)) = stack.pop() {
        let entries = sftp.read_dir(&path).await.map_err(|e| format!("List {} failed: {}", path, e))?;
        for e in entries {
            let name = e.file_name();
            if name == "." || name == ".." { continue; }
            let child_path = join_remote_path(&path, &name);
            let child_rel = if rel.is_empty() { name.clone() } else { join_remote_path(&rel, &name) };
            if e.file_type().is_dir() {
                stack.push((child_path, child_rel));
            } else {
                out.push((child_path, child_rel));
            }
        }
    }
    Ok(())
}

async fn upload_batch(
    sftp: &SftpSession,
    remote_dir: &str,
    local_paths: &[String],
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    use russh_sftp::protocol::OpenFlags;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut plan = Vec::new();
    for p in local_paths {
        collect_local_files(Path::new(p), "", &mut plan)?;
    }
    { let mut g = progress.lock().unwrap(); g.total_files = plan.len(); }
    for (idx, (local_abs, rel)) in plan.iter().enumerate() {
        if cancel_flag.load(Ordering::SeqCst) {
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
            if cancel_flag.load(Ordering::SeqCst) { return Err("Upload cancelled".into()); }
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
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    use russh_sftp::protocol::OpenFlags;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        if cancel_flag.load(Ordering::SeqCst) {
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
            if cancel_flag.load(Ordering::SeqCst) { return Err("Download cancelled".into()); }
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

pub struct SftpBrowserManager {
    browsers: Arc<std::sync::Mutex<HashMap<String, BrowserSession>>>,
    jobs: Arc<std::sync::Mutex<HashMap<String, Arc<JobState>>>>,
}

struct JobState {
    cancel_flag: Arc<AtomicBool>,
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

    #[allow(dead_code)]
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
                job.cancel_flag.store(true, Ordering::SeqCst);
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
    let cancel_flag = Arc::new(AtomicBool::new(false));
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
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
    rt.block_on(run_transfer_async(host, port, username, auth, target, items, kind, cancel_flag, progress))
}

async fn run_transfer_async(
    host: String, port: i32, username: String, auth: UploadAuth,
    target: String, items: Vec<String>, kind: TransferKind,
    cancel_flag: Arc<AtomicBool>,
    progress: Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    let (sftp, _) = connect_sftp(&host, port, &username, auth).await?;
    match kind {
        TransferKind::Upload => upload_batch(&sftp, &target, &items, cancel_flag, progress).await,
        TransferKind::Download => download_batch(&sftp, &target, &items, cancel_flag, progress).await,
    }
}

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
