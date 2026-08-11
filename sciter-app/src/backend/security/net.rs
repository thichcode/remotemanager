use std::net::TcpStream;

/// Simple, dependency-light SSH auth probe.
///
/// Opens a TCP connection to the host:port and reads the SSH protocol banner
/// (e.g. "SSH-2.0-OpenSSH_9.6"). This verifies that the host is reachable and
/// speaking SSH without shipping a full SSH client. Full password/key
/// authentication is performed by the installed OpenSSH client at launch time.
pub fn test_ssh_auth(
    host: &str,
    port: i32,
    username: &str,
    _password: Option<&str>,
    _key_path: Option<&str>,
) -> Result<String, String> {
    use std::io::Read;
    use std::time::Duration;

    let address = format!("{}:{}", host, port);
    let mut stream = TcpStream::connect(&address)
        .map_err(|e| format!("Cannot reach {}: {}", address, e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;

    let mut banner = [0u8; 256];
    let read = stream
        .read(&mut banner)
        .map_err(|e| format!("No SSH banner from {}: {}", address, e))?;

    let banner = String::from_utf8_lossy(&banner[..read]).to_string();
    if banner.trim_start().starts_with("SSH-") {
        let auth_hint = if username.trim().is_empty() {
            String::new()
        } else {
            format!(" — will authenticate as {}", username)
        };
        Ok(format!(
            "Reachable at {} (SSH banner: {}){}",
            address,
            banner.trim().lines().next().unwrap_or(""),
            auth_hint
        ))
    } else {
        Err(format!(
            "{} responded but is not SSH (got: {})",
            address,
            banner.trim()
        ))
    }
}
