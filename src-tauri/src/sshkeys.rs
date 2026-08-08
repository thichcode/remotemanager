use std::fs;
use std::path::PathBuf;
use rusqlite::Connection;
use crate::db::operations;
use crate::paths;
use crate::security;

/// OpenSSH on Windows refuses private keys whose ACLs are too permissive
/// ("Permissions for ... are too open"), which silently falls back to password
/// auth and ends in "Permission denied". Replace the entire DACL so the only
/// permitted principal is the current user.
///
/// `icacls /inheritance:r` is NOT enough here: it *copies* the inherited ACEs
/// into explicit ones, so group access (e.g. "CodexSandboxUsers") survives and
/// OpenSSH still rejects the key. We instead reset DACL protection and re-add
/// a single allow rule for the current user only.
fn restrict_key_permissions(path: &std::path::Path) {
    #[cfg(windows)]
    {
        let p = path.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$p = '{0}'; $acl = Get-Acl -LiteralPath $p; $acl.SetAccessRuleProtection($true, $false); $id = $env:USERDOMAIN + '\\' + $env:USERNAME; $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(($id -as [string]).Trim(), 'FullControl', 'Allow'); $acl.AddAccessRule($rule); Set-Acl -LiteralPath $p -AclObject $acl",
            p
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .output();
    }
}

/// Re-apply the ACL restriction to an existing registered key (fixes keys that
/// were imported by older builds whose `/inheritance:r` left group access).
pub fn ensure_key_permissions(path: &str) {
    restrict_key_permissions(std::path::Path::new(path));
}

pub fn import_private_key(
    conn: &Connection,
    source_path: &str,
    name: &str,
    passphrase: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Key name is required".to_string());
    }
    let bytes = fs::read(source_path).map_err(|e| format!("Failed to read key file: {}", e))?;

    let key_dir = paths::keys_dir();
    let filename = format!("{}.key", uuid::Uuid::new_v4());
    let dest: PathBuf = key_dir.join(&filename);
    fs::write(&dest, &bytes).map_err(|e| format!("Failed to store key: {}", e))?;
    restrict_key_permissions(&dest);

    let public_key = extract_public_from_private(&bytes);

    let encrypted_pass = match passphrase {
        Some(p) if !p.trim().is_empty() => security::encrypt(&p)?,
        _ => String::new(),
    };

    operations::create_ssh_key(conn, name.trim(), &dest.to_string_lossy(), &public_key, &encrypted_pass)
        .map_err(|e| e.to_string())
}

fn extract_public_from_private(private_bytes: &[u8]) -> String {
    // Best effort: read corresponding .pub file if bytes are an OpenSSH key.
    // For simplicity, store empty string; real .pub import handled elsewhere.
    let _ = private_bytes;
    String::new()
}

pub fn list(conn: &Connection) -> Result<Vec<operations::SshKeyRow>, String> {
    operations::list_ssh_keys(conn).map_err(|e| e.to_string())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    // Remove file if present
    if let Ok(Some(path)) = operations::get_ssh_key_private_path(conn, id) {
        let _ = fs::remove_file(&path);
    }
    operations::delete_ssh_key(conn, id).map_err(|e| e.to_string())
}

pub fn get_private_key_path(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    operations::get_ssh_key_private_path(conn, id).map_err(|e| e.to_string())
}

pub fn attach(conn: &Connection, server_id: &str, ssh_key_id: Option<&str>) -> Result<(), String> {
    operations::attach_key_to_server(conn, server_id, ssh_key_id).map_err(|e| e.to_string())
}
