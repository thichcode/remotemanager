/// Strict whitelist validator for values that end up in shell command lines
/// (host, username, names). Rejects any control characters and any shell
/// metacharacter. This is a hard security boundary — never relax it.
pub fn validate_host(host: &str) -> Result<(), String> {
    validate_token(host, "Host", false)
}

/// Validate a username. Allows backslash for Windows `DOMAIN\user` form.
pub fn validate_username(username: &str) -> Result<(), String> {
    if username.is_empty() {
        return Ok(());
    }
    validate_token(username, "Username", true)
}

fn validate_token(value: &str, field: &str, allow_backslash: bool) -> Result<(), String> {
    if value.len() > 255 {
        return Err(format!("{} is too long (max 255)", field));
    }
    for c in value.chars() {
        if c.is_control() {
            return Err(format!("{} contains control characters (CR/LF etc.)", field));
        }
        let allowed = c.is_ascii_alphanumeric()
            || matches!(c, '.' | '-' | '_' | '@' | ':' | '[' | ']' | '+' | '=' | '/' | '(' | ')')
            || (allow_backslash && c == '\\');
        if !allowed {
            return Err(format!("{} contains invalid character '{}'", field, c));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_metacharacters() {
        for bad in [
            "a;b", "a&b", "a|b", "a`b", "a$b", "a\"b", "a b", "a\nb", "a\r\nb", "a^b", "a<b", "a>b", "a*b",
        ] {
            assert!(validate_host(bad).is_err(), "should reject: {:?}", bad);
        }
    }

    #[test]
    fn rejects_host_backslash() {
        assert!(validate_host("foo\\bar").is_err());
    }

    #[test]
    fn allows_valid_hosts() {
        for good in [
            "192.168.1.1",
            "host.example.com",
            "my-host_1",
            "2001:db8::1",
            "[::1]",
            "10.0.0.1",
        ] {
            assert!(validate_host(good).is_ok(), "should allow: {:?}", good);
        }
    }

    #[test]
    fn allows_windows_domain_username() {
        assert!(validate_username("DOMAIN\\user").is_ok());
        assert!(validate_username("root").is_ok());
        assert!(validate_username("").is_ok());
        assert!(validate_username("ro ot").is_err());
    }
}
