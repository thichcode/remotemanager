//! SFTP file upload for the drag/drop feature.
//!
//! Uses russh + russh-sftp (pure Rust, async). Each upload batch opens a fresh
//! SSH connection, uploads each file into the remote home directory, reports
//! progress through a shared struct, and honours cancellation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use log::info;

#[derive(Clone)]
pub enum UploadAuth {
    Password(String),
    Key(String), // path to private key
}

#[derive(Clone, Debug, serde::Serialize)]
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
    jobs: Arc<Mutex<HashMap<String, UploadJob>>>,
}

impl Default for UploadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UploadManager {
    pub fn new() -> Self {
        UploadManager {
            jobs: Arc::new(Mutex::new(HashMap::new())),
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
        let job_id_clone = job_id.clone();
        std::thread::Builder::new()
            .name("sftp-upload".into())
            .spawn(move || {
                let result = run_upload(
                    &host,
                    port,
                    &username,
                    auth,
                    &local_paths,
                    cancel_flag.clone(),
                    progress.clone(),
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
                    jobs.remove(&job_id_clone);
                }
            })
            .map_err(|e| format!("Failed to spawn upload thread: {}", e))?;

        Ok(job_id)
    }

    pub fn get_progress(&self, job_id: &str) -> Option<UploadProgress> {
        let progress = {
            let jobs = self.jobs.lock().ok()?;
            let job = jobs.get(job_id)?;
            job.progress.clone()
        };
        let guard = progress.lock().ok()?;
        Some(guard.clone())
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
        host,
        port,
        username,
        auth,
        local_paths,
        cancel_flag,
        progress,
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
        UploadAuth::Key(path) => {
            let key = russh::keys::load_secret_key(path, None)
                .map_err(|e| format!("Failed to load SSH key {}: {}", path, e))?;
            let hash_alg = if key.algorithm().is_rsa() {
                session
                    .best_supported_rsa_hash()
                    .await
                    .ok()
                    .flatten()
                    .flatten()
            } else {
                None
            };
            session
                .authenticate_publickey(
                    username,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg),
                )
                .await
                .map_err(|e| format!("SSH key auth failed: {}", e))?
        }
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

        {
            let mut p = progress.lock().unwrap();
            p.current_file = file_name.clone();
            p.file_index = idx;
        }

        let mut local = tokio::fs::File::open(local_path)
            .await
            .map_err(|e| format!("Failed to open local file {}: {}", local_path, e))?;
        let total = local.metadata().await.map(|m| m.len()).unwrap_or(0);

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
            {
                let mut p = progress.lock().unwrap();
                p.bytes_sent += n as u64;
            }
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