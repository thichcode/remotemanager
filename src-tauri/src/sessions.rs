use std::collections::HashMap;
use std::process::Child;
use std::sync::Mutex;

pub struct SessionManager {
    pub sessions: Mutex<HashMap<String, Child>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, id: String, child: Child) {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).insert(id, child);
    }

    pub fn remove(&self, id: &str) -> Option<Child> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(id)
    }

    /// Runs `f` with mutable access to the child for `id`. The internal lock
    /// is held for the duration of the closure: callers must not call other
    /// `SessionManager` methods (e.g. `remove`) inside `f`, since the lock is
    /// non-reentrant and that would deadlock.
    pub fn with_child<R>(&self, id: &str, f: impl FnOnce(&mut Child) -> R) -> Option<R> {
        let mut g = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        g.get_mut(id).map(f)
    }

    /// Number of live sessions. No production caller yet; exercised by the
    /// test suite and likely useful later (e.g. a session-count badge).
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn dummy_child() -> Child {
        Command::new("ping")
            .arg("127.0.0.1")
            .arg("-n")
            .arg("10")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn ping")
    }

    fn kill_wait(mut child: Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn insert_and_len() {
        let m = SessionManager::new();
        assert_eq!(m.len(), 0);
        m.insert("a".into(), dummy_child());
        assert_eq!(m.len(), 1);
        if let Some(child) = m.remove("a") {
            kill_wait(child);
        }
    }

    #[test]
    fn remove_returns_child() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_child());
        let child = m.remove("a");
        assert!(child.is_some());
        assert_eq!(m.len(), 0);
        if let Some(child) = child {
            kill_wait(child);
        }
    }

    #[test]
    fn remove_missing_returns_none() {
        let m = SessionManager::new();
        assert!(m.remove("nope").is_none());
    }

    #[test]
    fn write_and_close_lifecycle() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_child());
        {
            use std::io::Write;
            let mut guard = m.sessions.lock().unwrap();
            if let Some(c) = guard.get_mut("a") {
                if let Some(stdin) = c.stdin.as_mut() {
                    let _ = stdin.write_all(b"hello\n");
                }
            }
        }
        assert_eq!(m.len(), 1);
        let child = m.remove("a").unwrap();
        kill_wait(child);
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn with_child_writes_to_stdin() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_child());
        let wrote = m.with_child("a", |c| {
            use std::io::Write;
            match c.stdin.as_mut() {
                Some(stdin) => stdin.write_all(b"hello\n").is_ok(),
                None => false,
            }
        });
        assert_eq!(wrote, Some(true));
        if let Some(child) = m.remove("a") {
            kill_wait(child);
        }
    }

    #[test]
    fn with_child_missing_returns_none() {
        let m = SessionManager::new();
        assert!(m.with_child("nope", |_| ()).is_none());
    }
}
