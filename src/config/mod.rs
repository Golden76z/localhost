mod parser;
mod types;

pub use types::*;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub servers: Vec<ServerBlock>,
    /// host:port -> indices into `servers` (first = default)
    pub bindings: HashMap<String, Vec<usize>>,
    pub errors: Vec<String>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        parser::parse(&text)
    }

    pub fn load_from_str(text: &str) -> Result<Self, String> {
        parser::parse(text)
    }

    pub fn listen_sockets(&self) -> Vec<(String, u16, usize)> {
        let mut out = Vec::new();
        for (key, indices) in &self.bindings {
            if let Some((host, port)) = key.rsplit_once(':') {
                if let Ok(port) = port.parse() {
                    let default_idx = indices[0];
                    out.push((host.to_string(), port, default_idx));
                }
            }
        }
        out
    }

    pub fn resolve_server<'a>(&'a self, host: &str, port: u16, name: Option<&str>) -> &'a ServerBlock {
        let key = format!("{host}:{port}");
        let indices = self.bindings.get(&key).or_else(|| self.bindings.get(&format!("*:{port}")));
        if let Some(indices) = indices {
            if let Some(name) = name {
                for &i in indices {
                    if self.servers[i].names.iter().any(|n| n == name) {
                        return &self.servers[i];
                    }
                }
            }
            return &self.servers[indices[0]];
        }
        &self.servers[0]
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parses_server_block() {
        let cfg = Config::load_from_str(
            r#"
            server {
                listen 127.0.0.1:9000;
                server_name test.local;
                root www;
                route / { methods GET; }
            }
            "#,
        )
        .unwrap();
        assert_eq!(cfg.servers.len(), 1);
        assert_eq!(cfg.servers[0].listen_port, 9000);
    }
}
