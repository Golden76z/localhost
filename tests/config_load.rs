use localhost::config::Config;
use std::path::PathBuf;

#[test]
fn loads_default_conf_from_repo() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("configs/default.conf");
    let cfg = Config::load(&path).expect("default.conf should parse");
    assert!(!cfg.servers.is_empty());
    assert!(cfg.servers.iter().any(|s| s.listen_port == 8080));
}

#[test]
fn skips_invalid_server_block() {
    let cfg = Config::load_from_str(
        r#"
        server {
            listen 127.0.0.1:7000;
            server_name ok.local;
            root www;
            route {
                path /;
                methods GET;
            }
        }
        server {
            listen NOT_A_PORT;
            root www;
        }
        "#,
    )
    .expect("parse");
    assert_eq!(cfg.servers.len(), 1);
    assert!(!cfg.errors.is_empty());
}

#[test]
fn duplicate_listen_records_warning() {
    let cfg = Config::load_from_str(
        r#"
        server {
            listen 127.0.0.1:7010;
            root www;
            route { path /; methods GET; }
        }
        server {
            listen 127.0.0.1:7010;
            root www;
            route { path /; methods GET; }
        }
        "#,
    )
    .unwrap();
    assert_eq!(cfg.servers.len(), 2);
    assert!(cfg.errors.iter().any(|e| e.contains("duplicate")));
}
