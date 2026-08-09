use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

use portable_pty::{MasterPty, PtySize};

/// A live terminal session. `child` is the ssh process attached to a real
/// pseudo-terminal (ConPTY on Windows via portable-pty); `master` is kept for
/// resizing and `writer` carries keystrokes from the frontend into the pty.
pub struct Session {
    pub child: Box<dyn portable_pty::Child>,
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
}

impl Session {
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        self.master
            .resize(PtySize { rows, cols, ..PtySize::default() })
            .map_err(|e| e.to_string())
    }
}

pub struct SessionManager {
    pub sessions: Mutex<HashMap<String, Session>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, id: String, session: Session) {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).insert(id, session);
    }

    pub fn remove(&self, id: &str) -> Option<Session> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(id)
    }

    /// Runs `f` with mutable access to the session for `id`. The internal lock
    /// is held for the duration of the closure: callers must not call other
    /// `SessionManager` methods (e.g. `remove`) inside `f`, since the lock is
    /// non-reentrant and that would deadlock.
    pub fn with_session<R>(&self, id: &str, f: impl FnOnce(&mut Session) -> R) -> Option<R> {
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
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    fn dummy_session() -> Session {
        let pty = native_pty_system()
            .openpty(PtySize::default())
            .expect("openpty");
        let mut cmd = CommandBuilder::new("ping");
        cmd.arg("127.0.0.1");
        cmd.arg("-n");
        cmd.arg("10");
        let child = pty.slave.spawn_command(cmd).expect("spawn ping");
        drop(pty.slave);
        let writer = pty.master.take_writer().expect("take writer");
        Session { child, master: pty.master, writer }
    }

    fn kill_wait(mut session: Session) {
        let _ = session.child.kill();
        let _ = session.child.wait();
    }

    #[test]
    fn insert_and_len() {
        let m = SessionManager::new();
        assert_eq!(m.len(), 0);
        m.insert("a".into(), dummy_session());
        assert_eq!(m.len(), 1);
        if let Some(session) = m.remove("a") {
            kill_wait(session);
        }
    }

    #[test]
    fn remove_returns_session() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_session());
        let session = m.remove("a");
        assert!(session.is_some());
        assert_eq!(m.len(), 0);
        if let Some(session) = session {
            kill_wait(session);
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
        m.insert("a".into(), dummy_session());
        {
            let mut guard = m.sessions.lock().unwrap();
            if let Some(session) = guard.get_mut("a") {
                let _ = session.writer.write_all(b"hello\n");
            }
        }
        assert_eq!(m.len(), 1);
        let session = m.remove("a").unwrap();
        kill_wait(session);
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn with_session_writes_to_writer() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_session());
        let wrote = m.with_session("a", |s| s.writer.write_all(b"hello\n").is_ok());
        assert_eq!(wrote, Some(true));
        if let Some(session) = m.remove("a") {
            kill_wait(session);
        }
    }

    #[test]
    fn with_session_missing_returns_none() {
        let m = SessionManager::new();
        assert!(m.with_session("nope", |_| ()).is_none());
    }

    #[test]
    fn resize_updates_pty_size() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_session());
        let resized = m.with_session("a", |s| s.resize(30, 100));
        assert_eq!(resized, Some(Ok(())));
        if let Some(session) = m.remove("a") {
            kill_wait(session);
        }
    }
}
