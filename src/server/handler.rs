use crate::cgi;
use crate::chat::ChatHub;
use crate::config::{Config, Route, ServerBlock};
use crate::events::{emit, EventSender, ServerEvent};
use crate::http::{HttpRequest, HttpResponse, ParseError};
use crate::net::SocketFd;
use crate::session::{SessionStore, COOKIE_NAME};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub struct RequestHandler {
    pub sessions: SessionStore,
    pub chat: ChatHub,
    pub sse_clients: HashSet<SocketFd>,
    events: Option<EventSender>,
    pending_sse_broadcast: Option<String>,
}

impl RequestHandler {
    pub fn new(events: Option<EventSender>) -> Self {
        Self {
            sessions: SessionStore::new(),
            chat: ChatHub::new(200),
            sse_clients: HashSet::new(),
            events,
            pending_sse_broadcast: None,
        }
    }

    pub fn register_sse(&mut self, fd: SocketFd) {
        self.sse_clients.insert(fd);
    }

    pub fn unregister_sse(&mut self, fd: SocketFd) {
        self.sse_clients.remove(&fd);
    }

    pub fn take_pending_broadcast(&mut self) -> Option<String> {
        self.pending_sse_broadcast.take()
    }

    pub fn handle(
        &mut self,
        config: &Config,
        req: HttpRequest,
        peer_port: u16,
    ) -> HttpResponse {
        let host = req.host_name().unwrap_or_default();
        let server = config.resolve_server("0.0.0.0", peer_port, Some(&host));
        let path = req.uri.split('?').next().unwrap_or(&req.uri).trim();

        let result = if path.starts_with("/chat/api/") {
            self.handle_chat_api(&req)
        } else {
            self.dispatch(server, &req)
        };

        let session_cookie = req.cookie(COOKIE_NAME);
        let session = self.sessions.get_or_create(session_cookie.as_deref());
        let session_id = session.id.clone();

        let mut resp = result.unwrap_or_else(|e| error_response(server, e.0, e.1));

        emit(
            &self.events,
            ServerEvent::Request {
                method: req.method.clone(),
                path: path.to_string(),
                status: resp.status,
            },
        );

        if session_cookie.is_none() || session_cookie.as_deref() != Some(session_id.as_str()) {
            resp.set_cookie(COOKIE_NAME, &session_id);
        }
        resp
    }

    fn handle_chat_api(&mut self, req: &HttpRequest) -> Result<HttpResponse, (u16, &'static str)> {
        let path = req.uri.split('?').next().unwrap_or(&req.uri).trim();
        match (req.method.as_str(), path) {
            ("GET", "/chat/api/events") => {
                let replay = self.chat.replay_events();
                let body = format!(": ok\n\n{replay}");
                Ok(HttpResponse::sse_stream(body))
            }
            ("POST", "/chat/api/send") => {
                let (user, text) = parse_chat_post(req)?;
                let event = self.chat.push(&user, &text);
                emit(
                    &self.events,
                    ServerEvent::Chat {
                        user,
                        text,
                    },
                );
                self.pending_sse_broadcast = Some(event);
                Ok(HttpResponse::json_ok(r#"{"ok":true}"#))
            }
            ("GET", "/chat/api/history") => {
                let mut json = String::from("[");
                for (i, m) in self.chat.messages().iter().enumerate() {
                    if i > 0 {
                        json.push(',');
                    }
                    json.push_str(&format!(
                        r#"{{"user":{},"text":{},"ts":{}}}"#,
                        quote_json(&m.user),
                        quote_json(&m.text),
                        m.ts
                    ));
                }
                json.push(']');
                Ok(HttpResponse::json_ok(json))
            }
            _ => Err((404, "not found")),
        }
    }

    fn dispatch(&self, server: &ServerBlock, req: &HttpRequest) -> Result<HttpResponse, (u16, &'static str)> {
        let (route, suffix) = server.match_route(&req.uri);

        if let Some(route) = route {
            if !route.methods.is_empty() && !route.methods.iter().any(|m| m == &req.method) {
                let mut r = HttpResponse::error(405);
                r.header("Allow", &route.methods.join(", "));
                return Ok(r);
            }
            if let Some(loc) = &route.redirect {
                return Ok(HttpResponse::redirect(loc, true));
            }
            return self.handle_route(server, route, req, &suffix);
        }

        self.serve_path(server, &server.root, &req.uri.trim_start_matches('/'), req)
    }

    fn handle_route(
        &self,
        server: &ServerBlock,
        route: &Route,
        req: &HttpRequest,
        suffix: &str,
    ) -> Result<HttpResponse, (u16, &'static str)> {
        if let Some(loc) = &route.redirect {
            return Ok(HttpResponse::redirect(loc, false));
        }

        let root = route.root.as_ref().unwrap_or(&server.root);
        let rel = if suffix.is_empty() { "" } else { suffix };

        if req.method == "DELETE" {
            let path = safe_join(root, rel)?;
            if path.is_file() {
                fs::remove_file(&path).map_err(|_| (500, "delete failed"))?;
                return Ok(HttpResponse::new(204, "No Content"));
            }
            return Err((404, "not found"));
        }

        if req.method == "POST" {
            if let Some(upload) = route.upload_dir.as_ref().or(server.upload_dir.as_ref()) {
                return self.save_upload(server, upload, req);
            }
        }

        let index = route.index.as_deref().unwrap_or(server.index.as_str());
        let autoindex = route.autoindex.unwrap_or(server.autoindex);

        let path = safe_join(root, rel)?;
        if path.is_dir() {
            let index_path = path.join(index);
            if index_path.is_file() {
                return self.file_response(&index_path);
            }
            if autoindex {
                return Ok(directory_listing(&path, &req.uri));
            }
            return Err((403, "directory forbidden"));
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_key = format!(".{ext}");
            if let Some(interp) = route.cgi.get(&ext_key) {
                let req_owned = HttpRequest {
                    method: req.method.clone(),
                    uri: req.uri.clone(),
                    version: req.version.clone(),
                    headers: req.headers.clone(),
                    body: req.body.clone(),
                };
                let raw = cgi::run_cgi(interp, &path, &req_owned, root, &req.uri)
                    .map_err(|_| (500, "cgi failed"))?;
                return Ok(HttpResponse::from_wire(raw));
            }
        }

        if path.is_file() {
            return self.file_response(&path);
        }

        Err((404, "not found"))
    }

    fn serve_path(
        &self,
        server: &ServerBlock,
        root: &Path,
        rel: &str,
        req: &HttpRequest,
    ) -> Result<HttpResponse, (u16, &'static str)> {
        let path = safe_join(root, rel)?;
        if path.is_dir() {
            let index_path = path.join(&server.index);
            if index_path.is_file() {
                return self.file_response(&index_path);
            }
            if server.autoindex {
                return Ok(directory_listing(&path, &req.uri));
            }
            return Err((403, "forbidden"));
        }
        if path.is_file() {
            return self.file_response(&path);
        }
        Err((404, "not found"))
    }

    fn file_response(&self, path: &Path) -> Result<HttpResponse, (u16, &'static str)> {
        let data = fs::read(path).map_err(|_| (500, "read"))?;
        let mut r = HttpResponse::new(200, "OK");
        r.set_body(data, content_type(path));
        Ok(r)
    }

    fn save_upload(
        &self,
        server: &ServerBlock,
        dir: &Path,
        req: &HttpRequest,
    ) -> Result<HttpResponse, (u16, &'static str)> {
        if req.body.len() > server.client_max_body_size {
            return Err((413, "too large"));
        }
        fs::create_dir_all(dir).map_err(|_| (500, "mkdir"))?;
        let name = req.header("x-filename").unwrap_or("upload.bin");
        let path = safe_join(dir, name)?;
        fs::write(&path, &req.body).map_err(|_| (500, "write"))?;
        Ok(HttpResponse::ok_html(format!(
            "<!DOCTYPE html><html><body><h1>Uploaded</h1><p>Saved to {}</p></body></html>",
            path.display()
        )))
    }
}

fn parse_chat_post(req: &HttpRequest) -> Result<(String, String), (u16, &'static str)> {
    let body = std::str::from_utf8(&req.body).map_err(|_| (400, "bad json"))?;
    let user = json_field(body, "user").unwrap_or_else(|| "guest".to_string());
    let text = json_field(body, "text").filter(|t| !t.is_empty()).ok_or((400, "missing text"))?;
    if text.len() > 2000 {
        return Err((413, "too large"));
    }
    Ok((user, text))
}

fn json_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let start = json.find(&pattern)? + pattern.len();
    let rest = json.get(start..)?.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = rest[1..].chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            break;
        }
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn quote_json(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn error_response(server: &ServerBlock, status: u16, default_reason: &'static str) -> HttpResponse {
    let mut r = HttpResponse::error(status);
    if status == 405 {
        return r;
    }
    if let Some(page) = server.error_pages.get(&status) {
        if let Ok(data) = fs::read(page) {
            r.set_body(data, "text/html; charset=utf-8");
            return r;
        }
    }
    let body = format!(
        "<!DOCTYPE html><html><head><title>{status}</title></head><body><h1>{status}</h1><p>{default_reason}</p></body></html>"
    );
    r.set_body(body.into_bytes(), "text/html; charset=utf-8");
    r
}

pub fn parse_error_response(server: &ServerBlock, err: ParseError) -> HttpResponse {
    match err {
        ParseError::BodyTooLarge => error_response(server, 413, "Payload Too Large"),
        ParseError::BadRequest(m) => {
            let mut r = error_response(server, 400, "Bad Request");
            r.body = format!("<p>{m}</p>").into_bytes();
            r
        }
        ParseError::Incomplete => error_response(server, 400, "Incomplete"),
    }
}

fn safe_join(base: &Path, rel: &str) -> Result<PathBuf, (u16, &'static str)> {
    let mut p = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(s) => p.push(s),
            Component::ParentDir => return Err((403, "path traversal")),
            _ => {}
        }
    }
    Ok(p)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn directory_listing(dir: &Path, uri: &str) -> HttpResponse {
    let mut entries = Vec::new();
    if let Ok(read) = fs::read_dir(dir) {
        for e in read.flatten() {
            entries.push(e.file_name().to_string_lossy().to_string());
        }
    }
    entries.sort();
    let base = if uri.ends_with('/') { uri } else { &format!("{uri}/") };
    let links: String = entries
        .iter()
        .map(|n| format!(r#"<li><a href="{base}{n}">{n}</a></li>"#))
        .collect();
    let html = format!(
        r#"<!DOCTYPE html><html><head><title>Index of {uri}</title>
        <style>body{{font-family:system-ui;background:#0f172a;color:#e2e8f0;padding:2rem}}</style>
        </head><body><h1>Index of {uri}</h1><ul>{links}</ul></body></html>"#
    );
    HttpResponse::ok_html(html)
}
