use sciter::Value;
use crate::backend::db::AppState;
use std::sync::{Arc, Mutex};

pub struct AppHandler {
    state: Arc<Mutex<AppState>>,
}

impl AppHandler {
    pub fn new(conn: rusqlite::Connection) -> Self {
        AppHandler {
            state: Arc::new(Mutex::new(AppState {
                db: Mutex::new(conn),
                sessions: Arc::new(crate::backend::sessions::SessionManager::new()),
                rdp_sessions: Mutex::new(std::collections::HashMap::new()),
                terminal_sessions: Mutex::new(std::collections::HashMap::new()),
                upload_jobs: crate::backend::sftp::SftpBrowserManager::new(),
            })),
        }
    }
}

fn json_to_value(json: &str) -> Option<Value> {
    Value::parse(json).ok()
}

fn get_string(args: &[Value], index: usize) -> Option<String> {
    args.get(index)?.as_string()
}

impl sciter::EventHandler for AppHandler {
    fn attached(&mut self, _root: sciter::HELEMENT) {}

    fn document_complete(&mut self, _root: sciter::HELEMENT, _target: sciter::HELEMENT) {}

    fn on_script_call(&mut self, _root: sciter::HELEMENT, name: &str, args: &[Value]) -> Option<Value> {
        match name {
            "list_servers" => self.list_servers(args),
            "create_server" => self.create_server(args),
            "update_server" => self.update_server(args),
            "delete_server" => self.delete_server(args),
            "search_servers" => self.search_servers(args),
            "list_groups" => self.list_groups(args),
            "create_group" => self.create_group(args),
            "update_group" => self.update_group(args),
            "delete_group" => self.delete_group(args),
            "list_credentials" => self.list_credentials(args),
            "create_credential" => self.create_credential(args),
            "update_credential" => self.update_credential(args),
            "delete_credential" => self.delete_credential(args),
            "get_settings" => self.get_settings(args),
            "update_settings" => self.update_settings(args),
            "list_history" => self.list_history(args),
            "clear_history" => self.clear_history(args),
            "ping" => self.ping(args),
            "list_tags" => self.list_tags(args),
            "set_server_tags" => self.set_server_tags(args),
            "open_ssh_terminal" => self.open_ssh_terminal(args),
            "close_ssh_terminal" => self.close_ssh_terminal(args),
            "open_rdp_session" => self.open_rdp_session(args),
            "close_rdp_session" => self.close_rdp_session(args),
            "sftp_open" => self.sftp_open(args),
            "sftp_list" => self.sftp_list(args),
            "sftp_get_home" => self.sftp_get_home(args),
            "sftp_upload" => self.sftp_upload(args),
            "sftp_download" => self.sftp_download(args),
            "get_upload_progress" => self.get_download_progress(args),
            "cancel_upload" => self.cancel_download(args),
            _ => None,
        }
    }
}

impl AppHandler {
    fn list_servers(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let servers = crate::backend::db::operations::list_servers(&conn, None).ok()?;
        let json = serde_json::to_string(&servers).ok()?;
        json_to_value(&json)
    }

    fn create_server(&self, args: &[Value]) -> Option<Value> {
        let json = get_string(args, 0)?;
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let id = crate::backend::db::operations::create_server(
            &conn,
            v["name"].as_str().unwrap_or(""),
            v["host"].as_str().unwrap_or(""),
            v["port"].as_i64().unwrap_or(22) as i32,
            v["protocol"].as_str().unwrap_or("ssh"),
            v["username"].as_str().unwrap_or(""),
            v["group_id"].as_str(),
            v["tags"].as_str().unwrap_or(""),
            v["notes"].as_str().unwrap_or(""),
            v["description"].as_str().unwrap_or(""),
            v["credential_id"].as_str(),
            v["ssh_key_id"].as_str(),
        ).ok()?;
        Some(Value::from(id))
    }

    fn update_server(&self, args: &[Value]) -> Option<Value> {
        let id = get_string(args, 0)?;
        let json = get_string(args, 1)?;
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::update_server(
            &conn, &id,
            v["name"].as_str().unwrap_or(""),
            v["host"].as_str().unwrap_or(""),
            v["port"].as_i64().unwrap_or(22) as i32,
            v["protocol"].as_str().unwrap_or("ssh"),
            v["username"].as_str().unwrap_or(""),
            v["group_id"].as_str(),
            v["tags"].as_str().unwrap_or(""),
            v["notes"].as_str().unwrap_or(""),
            v["description"].as_str().unwrap_or(""),
            v["credential_id"].as_str(),
            v["ssh_key_id"].as_str(),
        ).ok()?;
        Some(Value::from(true))
    }

    fn delete_server(&self, args: &[Value]) -> Option<Value> {
        let id = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::delete_server(&conn, &id).ok()?;
        Some(Value::from(true))
    }

    fn search_servers(&self, args: &[Value]) -> Option<Value> {
        let query = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let servers = crate::backend::db::operations::search_servers(&conn, &query).ok()?;
        let json = serde_json::to_string(&servers).ok()?;
        json_to_value(&json)
    }

    fn list_groups(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let groups = crate::backend::db::operations::list_groups(&conn).ok()?;
        let json = serde_json::to_string(&groups).ok()?;
        json_to_value(&json)
    }

    fn create_group(&self, args: &[Value]) -> Option<Value> {
        let name = get_string(args, 0)?;
        let parent_id = get_string(args, 1);
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let id = crate::backend::db::operations::create_group(&conn, &name, parent_id.as_deref()).ok()?;
        Some(Value::from(id))
    }

    fn update_group(&self, args: &[Value]) -> Option<Value> {
        let id = get_string(args, 0)?;
        let name = get_string(args, 1)?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::update_group(&conn, &id, &name).ok()?;
        Some(Value::from(true))
    }

    fn delete_group(&self, args: &[Value]) -> Option<Value> {
        let id = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::delete_group(&conn, &id).ok()?;
        Some(Value::from(true))
    }

    fn list_credentials(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let creds = crate::backend::db::operations::list_credentials(&conn).ok()?;
        let json = serde_json::to_string(&creds).ok()?;
        json_to_value(&json)
    }

    fn create_credential(&self, args: &[Value]) -> Option<Value> {
        let json = get_string(args, 0)?;
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        let password = v["password"].as_str().unwrap_or("");
        let encrypted = crate::backend::security::encrypt(password).ok()?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let id = crate::backend::db::operations::create_credential(
            &conn,
            v["name"].as_str().unwrap_or(""),
            v["username"].as_str().unwrap_or(""),
            &encrypted,
        ).ok()?;
        Some(Value::from(id))
    }

    fn update_credential(&self, args: &[Value]) -> Option<Value> {
        let id = get_string(args, 0)?;
        let json = get_string(args, 1)?;
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        let password = v["password"].as_str();
        let encrypted = match password {
            Some(p) if !p.is_empty() => Some(crate::backend::security::encrypt(p).ok()?),
            _ => None,
        };
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::update_credential(
            &conn, &id,
            v["name"].as_str().unwrap_or(""),
            v["username"].as_str().unwrap_or(""),
            encrypted.as_deref(),
        ).ok()?;
        Some(Value::from(true))
    }

    fn delete_credential(&self, args: &[Value]) -> Option<Value> {
        let id = get_string(args, 0)?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::delete_credential(&conn, &id).ok()?;
        Some(Value::from(true))
    }

    fn get_settings(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let settings = crate::backend::db::operations::get_settings(&conn).ok()?;
        let json = serde_json::to_string(&settings).ok()?;
        json_to_value(&json)
    }

    fn update_settings(&self, args: &[Value]) -> Option<Value> {
        let json = get_string(args, 0)?;
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::update_settings(
            &conn,
            v["theme"].as_str().unwrap_or("dark"),
            v["font_size"].as_i64().unwrap_or(14) as i32,
            v["ssh_port"].as_i64().unwrap_or(22) as i32,
            v["rdp_fullscreen"].as_bool().unwrap_or(false),
            v["rdp_admin_mode"].as_bool().unwrap_or(false),
        ).ok()?;
        Some(Value::from(true))
    }

    fn list_history(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let history = crate::backend::db::operations::list_history(&conn).ok()?;
        let json = serde_json::to_string(&history).ok()?;
        json_to_value(&json)
    }

    fn clear_history(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::clear_history(&conn).ok()?;
        Some(Value::from(true))
    }

    fn ping(&self, args: &[Value]) -> Option<Value> {
        let host = get_string(args, 0)?;
        let output = std::process::Command::new("ping")
            .args(["-n", "1", "-w", "3000", &host])
            .output()
            .ok()?;
        if output.status.success() {
            Some(Value::from("Reachable".to_string()))
        } else {
            Some(Value::from("Unreachable".to_string()))
        }
    }

    fn list_tags(&self, _args: &[Value]) -> Option<Value> {
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let tags = crate::backend::db::operations::list_tags(&conn).ok()?;
        let json = serde_json::to_string(&tags).ok()?;
        json_to_value(&json)
    }

    fn set_server_tags(&self, args: &[Value]) -> Option<Value> {
        let server_id = get_string(args, 0)?;
        let tags_json = get_string(args, 1)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).ok()?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::db::operations::set_server_tags(&conn, &server_id, &tags).ok()?;
        Some(Value::from(true))
    }

    /// Resolve the username for a server, falling back to the attached credential.
    fn resolve_username(&self, username: &str, credential_id: Option<&str>) -> Option<String> {
        if !username.trim().is_empty() {
            return Some(username.to_string());
        }
        let cid = credential_id?;
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let meta = crate::backend::db::operations::get_credential_meta(&conn, cid).ok()??;
        Some(meta.username)
    }

    /// Resolve the decrypted password for a credential.
    fn resolve_password(&self, credential_id: Option<&str>) -> Option<String> {
        let cid = match credential_id {
            Some(c) if !c.is_empty() => c,
            _ => return None,
        };
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        let encrypted = crate::backend::db::operations::get_credential_password(&conn, cid).ok()??;
        crate::backend::security::decrypt(&encrypted).ok()
    }

    /// Resolve the SSH private key path for a server.
    fn resolve_ssh_key(&self, ssh_key_id: Option<&str>) -> Option<String> {
        let kid = match ssh_key_id {
            Some(k) if !k.is_empty() => k,
            _ => return None,
        };
        let state = self.state.lock().ok()?;
        let conn = state.db.lock().ok()?;
        crate::backend::sshkeys::get_private_key_path(&conn, kid).ok()?
    }

    /// Open an embedded SSH terminal session. Returns the WebSocket port.
    fn open_ssh_terminal(&self, args: &[Value]) -> Option<Value> {
        let host = get_string(args, 0)?;
        let port = args.get(1)?.to_int()? as i32;
        let username = get_string(args, 2).unwrap_or_default();
        let server_id = get_string(args, 3);
        let server_name = get_string(args, 4);
        let ssh_key_id = get_string(args, 5);
        let credential_id = get_string(args, 6);

        let username = self.resolve_username(&username, credential_id.as_deref())?;
        let ssh_key_path = self.resolve_ssh_key(ssh_key_id.as_deref());

        let result = crate::backend::terminal::start_session(
            crate::backend::terminal::TerminalSessionParams {
                host: host.clone(),
                port,
                username: username.clone(),
                ssh_key_path,
            },
        ).ok()?;

        {
            let state = self.state.lock().ok()?;
            let conn = state.db.lock().ok()?;
            let name = server_name.unwrap_or_else(|| host.clone());
            let _ = crate::backend::history::record(
                &conn,
                server_id.as_deref(),
                &name,
                &host,
                Some(port),
                "ssh",
                &username,
                ssh_key_id.as_deref(),
            );
            if let Some(sid) = server_id.as_deref() {
                let _ = crate::backend::db::operations::touch_last_connected(&conn, sid);
            }
        }

        {
            let state = self.state.lock().ok()?;
            let mut sessions = state.terminal_sessions.lock().ok()?;
            sessions.insert(result.ws_port, result.shutdown);
        }

        Some(Value::from(result.ws_port as i32))
    }

    /// Close an embedded SSH terminal session by WebSocket port.
    fn close_ssh_terminal(&self, args: &[Value]) -> Option<Value> {
        let ws_port = args.get(0)?.to_int()? as u16;
        let state = self.state.lock().ok()?;
        let mut sessions = state.terminal_sessions.lock().ok()?;
        if let Some(shutdown) = sessions.remove(&ws_port) {
            let _ = shutdown.send(());
        }
        Some(Value::from(true))
    }

    /// Open an embedded RDP session. Returns the WebSocket port.
    fn open_rdp_session(&self, args: &[Value]) -> Option<Value> {
        let host = get_string(args, 0)?;
        let username = get_string(args, 1).unwrap_or_default();
        let width = args.get(2).and_then(|v| v.to_int()).map(|i| i as u16).unwrap_or(1024);
        let height = args.get(3).and_then(|v| v.to_int()).map(|i| i as u16).unwrap_or(768);
        let server_id = get_string(args, 4);
        let server_name = get_string(args, 5);
        let credential_id = get_string(args, 6);

        let username = self.resolve_username(&username, credential_id.as_deref())?;
        let password = self.resolve_password(credential_id.as_deref()).unwrap_or_default();

        let result = crate::backend::rdp::start_session(
            crate::backend::rdp::RdpSessionParams {
                host: host.clone(),
                port: 3389,
                username: username.clone(),
                password,
                width,
                height,
            },
        ).ok()?;

        {
            let state = self.state.lock().ok()?;
            let conn = state.db.lock().ok()?;
            let name = server_name.unwrap_or_else(|| host.clone());
            let _ = crate::backend::history::record(
                &conn,
                server_id.as_deref(),
                &name,
                &host,
                Some(3389),
                "rdp",
                &username,
                None,
            );
            if let Some(sid) = server_id.as_deref() {
                let _ = crate::backend::db::operations::touch_last_connected(&conn, sid);
            }
        }

        {
            let state = self.state.lock().ok()?;
            let mut sessions = state.rdp_sessions.lock().ok()?;
            sessions.insert(result.ws_port, result.shutdown);
        }

        Some(Value::from(result.ws_port as i32))
    }

    /// Close an embedded RDP session by WebSocket port.
    fn close_rdp_session(&self, args: &[Value]) -> Option<Value> {
        let ws_port = args.get(0)?.to_int()? as u16;
        let state = self.state.lock().ok()?;
        let mut sessions = state.rdp_sessions.lock().ok()?;
        if let Some(shutdown) = sessions.remove(&ws_port) {
            let _ = shutdown.send(());
        }
        Some(Value::from(true))
    }

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
}
