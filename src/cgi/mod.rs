use crate::http::HttpRequest;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn run_cgi(
    interpreter: &Path,
    script: &Path,
    req: &HttpRequest,
    cwd: &Path,
    path_info: &str,
) -> Result<Vec<u8>, String> {
    let mut child = Command::new(interpreter);
    child.arg(script);
    child.current_dir(cwd);
    child.stdin(Stdio::piped());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::null());

    child.env("REQUEST_METHOD", &req.method);
    child.env("REQUEST_URI", &req.uri);
    child.env("QUERY_STRING", query_string(&req.uri));
    child.env("CONTENT_LENGTH", &req.body.len().to_string());
    if let Some(ct) = req.header("content-type") {
        child.env("CONTENT_TYPE", ct);
    }
    child.env("PATH_INFO", path_info);
    if let Some(host) = req.header("host") {
        child.env("HTTP_HOST", host);
    }
    for (k, v) in &req.headers {
        let key = format!("HTTP_{}", k.replace('-', "_").to_uppercase());
        child.env(key, v);
    }

    let mut child = child.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        if !req.body.is_empty() {
            stdin
                .write_all(&req.body)
                .map_err(|e| e.to_string())?;
        }
    }

    let mut stdout = child
        .stdout
        .take()
        .ok_or("no stdout")?;
    let mut raw = Vec::new();
    stdout.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let _ = child.wait();

    Ok(cgi_stdout_to_http(&raw))
}

fn query_string(uri: &str) -> String {
    uri.split('?').nth(1).unwrap_or("").to_string()
}

fn cgi_stdout_to_http(raw: &[u8]) -> Vec<u8> {
    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        let header = std::str::from_utf8(&raw[..pos]).unwrap_or("");
        let body = &raw[pos + 4..];
        let mut status = 200u16;
        let mut headers = Vec::new();
        for line in header.lines() {
            if let Some(rest) = line.strip_prefix("Status:") {
                if let Some(code) = rest.trim().split_whitespace().next() {
                    status = code.parse().unwrap_or(200);
                }
            } else if let Some((k, v)) = line.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        let mut out = format!("HTTP/1.1 {status} OK\r\n");
        for (k, v) in headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        if !header.to_ascii_lowercase().contains("content-length") {
            out.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        out.push_str("Connection: close\r\n\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(body);
        return bytes;
    }
    let r = crate::http::HttpResponse::ok_html(raw);
    r.to_bytes()
}
