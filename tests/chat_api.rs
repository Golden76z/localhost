use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn binary_path() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_localhost")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/debug/localhost")
                .with_extension(if cfg!(windows) { "exe" } else { "" })
        })
}

fn spawn() -> (Child, u16) {
    let port = 18090u16;
    let conf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("configs/test_chat.conf");
    let child = Command::new(binary_path())
        .arg(conf)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    thread::sleep(Duration::from_millis(800));
    (child, port)
}

fn post_json(port: u16, path: &str, json: &str) -> String {
    let mut s = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    let body = json.as_bytes();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(req.as_bytes()).unwrap();
    s.write_all(body).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
fn chat_send_returns_ok() {
    let (mut child, port) = spawn();
    let resp = post_json(port, "/chat/api/send", r#"{"user":"test","text":"hello"}"#);
    assert!(resp.contains("200"), "{resp}");
    assert!(resp.contains(r#""ok":true"#), "{resp}");
    child.kill().ok();
}
