use std::collections::HashMap;

#[derive(Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: &'static str,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    /// Pre-serialized wire format (e.g. CGI output).
    pub wire: Option<Vec<u8>>,
    /// Keep socket open after the first write (SSE stream).
    pub sse_keep_alive: bool,
}

impl HttpResponse {
    pub fn new(status: u16, reason: &'static str) -> Self {
        Self {
            status,
            reason,
            headers: HashMap::new(),
            body: Vec::new(),
            wire: None,
            sse_keep_alive: false,
        }
    }

    pub fn sse_stream(initial_body: impl Into<Vec<u8>>) -> Self {
        let body = initial_body.into();
        let mut r = Self::new(200, "OK");
        r.header("Content-Type", "text/event-stream; charset=utf-8");
        r.header("Cache-Control", "no-cache");
        r.header("Connection", "keep-alive");
        r.body = body;
        r.sse_keep_alive = true;
        r
    }

    pub fn json_ok(body: impl Into<Vec<u8>>) -> Self {
        let body = body.into();
        let mut r = Self::new(200, "OK");
        r.set_body(body, "application/json; charset=utf-8");
        r
    }

    pub fn ok_html(body: impl Into<Vec<u8>>) -> Self {
        let body = body.into();
        let mut r = Self::new(200, "OK");
        r.header("Content-Type", "text/html; charset=utf-8");
        r.header("Content-Length", &body.len().to_string());
        r.body = body;
        r
    }

    pub fn redirect(location: &str, permanent: bool) -> Self {
        let (status, reason) = if permanent {
            (301, "Moved Permanently")
        } else {
            (302, "Found")
        };
        let mut r = Self::new(status, reason);
        r.header("Location", location);
        r.header("Content-Length", "0");
        r
    }

    pub fn error(status: u16) -> Self {
        let reason = match status {
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Payload Too Large",
            500 => "Internal Server Error",
            _ => "Error",
        };
        Self::new(status, reason)
    }

    pub fn header(&mut self, k: &str, v: &str) -> &mut Self {
        self.headers.insert(k.to_string(), v.to_string());
        self
    }

    pub fn set_body(&mut self, body: Vec<u8>, content_type: &str) {
        self.header("Content-Type", content_type);
        self.header("Content-Length", &body.len().to_string());
        self.body = body;
    }

    pub fn set_cookie(&mut self, name: &str, value: &str) {
        self.header(
            "Set-Cookie",
            &format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax"),
        );
    }

    pub fn from_wire(raw: Vec<u8>) -> Self {
        Self {
            status: 200,
            reason: "OK",
            headers: HashMap::new(),
            body: Vec::new(),
            wire: Some(raw),
            sse_keep_alive: false,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        if let Some(w) = &self.wire {
            return w.clone();
        }
        let mut out = format!("HTTP/1.1 {} {}\r\n", self.status, self.reason);
        for (k, v) in &self.headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        if !self.sse_keep_alive && !self.headers.contains_key("Content-Length") && !self.body.is_empty() {
            out.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }
        if !self.headers.contains_key("Connection") && !self.sse_keep_alive {
            out.push_str("Connection: close\r\n");
        }
        out.push_str("\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}
