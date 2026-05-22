use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn binary_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_localhost") {
        return PathBuf::from(p);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.join("target")
        .join("debug")
        .join(exe_name())
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "localhost.exe"
    } else {
        "localhost"
    }
}

fn http_get(port: u16, path: &str) -> String {
    let mut s =
        TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect to server");
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn spawn_server() -> (Child, u16) {
    let port = 8080u16;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let conf = root.join("configs/default.conf");
    let child = Command::new(binary_path())
        .arg(&conf)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");
    thread::sleep(Duration::from_millis(800));
    (child, port)
}

#[test]
fn server_http_smoke() {
    let (mut child, port) = spawn_server();

    let home = http_get(port, "/");
    assert!(
        home.contains("HTTP/1.1 200"),
        "expected 200 for /, got: {home}"
    );

    let missing = http_get(port, "/this-does-not-exist-xyz");
    assert!(
        missing.contains("404"),
        "expected 404 for unknown path, got: {missing}"
    );

    child.kill().ok();
    let _ = child.wait();
}
