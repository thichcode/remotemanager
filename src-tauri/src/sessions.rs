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
        self.sessions.lock().unwrap().insert(id, child);
    }

    pub fn remove(&self, id: &str) -> Option<Child> {
        self.sessions.lock().unwrap().remove(id)
    }

    pub fn get_mut(&self, id: &str) -> Option<std::sync::MutexGuard<'_, HashMap<String, Child>>> {
        let mut g = self.sessions.lock().unwrap();
        g.get_mut(id)?;
        Some(g)
    }

    pub fn len(&self) -> usize {
        self.sessions.lock().unwrap().len()
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

    #[test]
    fn insert_and_len() {
        let m = SessionManager::new();
        assert_eq!(m.len(), 0);
        m.insert("a".into(), dummy_child());
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn remove_returns_child() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_child());
        let child = m.remove("a");
        assert!(child.is_some());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn remove_missing_returns_none() {
        let m = SessionManager::new();
        assert!(m.remove("nope").is_none());
    }
}
