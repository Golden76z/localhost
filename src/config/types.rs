use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ServerBlock {
    pub listen_host: String,
    pub listen_port: u16,
    pub names: Vec<String>,
    pub root: PathBuf,
    pub index: String,
    pub client_max_body_size: usize,
    pub error_pages: HashMap<u16, PathBuf>,
    pub routes: Vec<Route>,
    pub upload_dir: Option<PathBuf>,
    pub autoindex: bool,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub path: String,
    pub methods: Vec<String>,
    pub root: Option<PathBuf>,
    pub index: Option<String>,
    pub redirect: Option<String>,
    pub upload_dir: Option<PathBuf>,
    pub autoindex: Option<bool>,
    pub cgi: HashMap<String, PathBuf>,
}

impl ServerBlock {
    pub fn match_route(&self, uri: &str) -> (Option<&Route>, String) {
        let path = uri.split('?').next().unwrap_or(uri);
        let mut best: Option<&Route> = None;
        let mut best_len = 0usize;
        for r in &self.routes {
            if path == r.path || path.starts_with(&format!("{}/", r.path.trim_end_matches('/'))) {
                let len = r.path.len();
                if len >= best_len {
                    best_len = len;
                    best = Some(r);
                }
            }
        }
        let suffix = if let Some(r) = best {
            let base = r.path.trim_end_matches('/');
            if path.len() > base.len() {
                path[base.len()..].trim_start_matches('/').to_string()
            } else {
                String::new()
            }
        } else {
            path.trim_start_matches('/').to_string()
        };
        (best, suffix)
    }
}
