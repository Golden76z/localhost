use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub user: String,
    pub text: String,
    pub ts: u64,
}

pub struct ChatHub {
    messages: Vec<ChatMessage>,
    max_messages: usize,
}

impl ChatHub {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    pub fn push(&mut self, user: impl Into<String>, text: impl Into<String>) -> String {
        let msg = ChatMessage {
            user: user.into(),
            text: text.into(),
            ts: now_secs(),
        };
        let event = format!("data: {}\n\n", msg.to_json());
        self.messages.push(msg);
        if self.messages.len() > self.max_messages {
            let drop = self.messages.len() - self.max_messages;
            self.messages.drain(0..drop);
        }
        event
    }

    pub fn replay_events(&self) -> String {
        self.messages
            .iter()
            .map(|m| format!("data: {}\n\n", m.to_json()))
            .collect()
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
}

impl ChatMessage {
    fn to_json(&self) -> String {
        format!(
            r#"{{"user":{},"text":{},"ts":{}}}"#,
            json_str(&self.user),
            json_str(&self.text),
            self.ts
        )
    }
}

fn json_str(s: &str) -> String {
    let escaped: String = s
        .chars()
        .map(|c| match c {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            c if c.is_control() => String::new(),
            c => c.to_string(),
        })
        .collect();
    format!("\"{escaped}\"")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
