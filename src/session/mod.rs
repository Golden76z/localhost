use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub data: HashMap<String, String>,
    pub created: u64,
}

pub struct SessionStore {
    sessions: HashMap<String, Session>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn get_or_create(&mut self, cookie: Option<&str>) -> &Session {
        if let Some(id) = cookie {
            if self.sessions.contains_key(id) {
                return self.sessions.get(id).unwrap();
            }
        }
        let id = format!(
            "sess_{:x}_{}",
            now_secs(),
            SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let session = Session {
            id: id.clone(),
            data: HashMap::new(),
            created: now_secs(),
        };
        self.sessions.insert(id.clone(), session);
        self.sessions.get(&id).unwrap()
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub const COOKIE_NAME: &str = "localhost_session";
