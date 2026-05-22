use super::types::{Route, ServerBlock};
use super::Config;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub fn parse(text: &str) -> Result<Config, String> {
    let mut servers = Vec::new();
    let mut bindings: HashMap<String, Vec<usize>> = HashMap::new();
    let mut errors = Vec::new();
    let mut seen_ports: HashSet<String> = HashSet::new();

    let mut i = 0;
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

    while i < lines.len() {
        if lines[i] != "server {" {
            i += 1;
            continue;
        }
        i += 1;
        match parse_server_block(&lines, &mut i) {
            Ok(block) => {
                let key = format!("{}:{}", block.listen_host, block.listen_port);
                if !seen_ports.insert(key.clone()) {
                    errors.push(format!("duplicate listen {key} — earlier block kept as default"));
                }
                let idx = servers.len();
                servers.push(block);
                bindings.entry(key).or_default().push(idx);
            }
            Err(e) => {
                errors.push(e);
                while i < lines.len() && lines[i] != "}" {
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }
            }
        }
    }

    Ok(Config {
        servers,
        bindings,
        errors,
    })
}

fn parse_server_block(lines: &[&str], i: &mut usize) -> Result<ServerBlock, String> {
    let mut listen_host = "0.0.0.0".to_string();
    let mut listen_port = 8080u16;
    let mut names = Vec::new();
    let mut root = PathBuf::from("www");
    let mut index = "index.html".to_string();
    let mut client_max_body_size = 1_048_576;
    let mut error_pages = HashMap::new();
    let mut routes = Vec::new();
    let mut upload_dir = None;
    let mut autoindex = false;

    while *i < lines.len() {
        let line = lines[*i];
        if line == "}" {
            *i += 1;
            break;
        }
        if line == "route {" {
            *i += 1;
            routes.push(parse_route(lines, i)?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("listen ") {
            let rest = rest.trim_end_matches(';');
            if let Some((h, p)) = rest.rsplit_once(':') {
                listen_host = h.to_string();
                listen_port = p.parse().map_err(|_| format!("bad port in listen {rest}"))?;
            } else {
                listen_port = rest.parse().map_err(|_| format!("bad listen {rest}"))?;
            }
        } else if let Some(rest) = line.strip_prefix("server_name ") {
            names.push(rest.trim_end_matches(';').to_string());
        } else if let Some(rest) = line.strip_prefix("root ") {
            root = PathBuf::from(rest.trim_end_matches(';'));
        } else if let Some(rest) = line.strip_prefix("index ") {
            index = rest.trim_end_matches(';').to_string();
        } else if let Some(rest) = line.strip_prefix("client_max_body_size ") {
            client_max_body_size = parse_size(rest.trim_end_matches(';'))?;
        } else if let Some(rest) = line.strip_prefix("error_page ") {
            let parts: Vec<_> = rest.trim_end_matches(';').split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(code) = parts[0].parse::<u16>() {
                    error_pages.insert(code, PathBuf::from(parts[1]));
                }
            }
        } else if let Some(rest) = line.strip_prefix("upload_dir ") {
            upload_dir = Some(PathBuf::from(rest.trim_end_matches(';')));
        } else if line.starts_with("autoindex on") {
            autoindex = true;
        }
        *i += 1;
    }

    if routes.is_empty() {
        routes.push(Route {
            path: "/".to_string(),
            methods: vec!["GET".into(), "HEAD".into()],
            root: None,
            index: None,
            redirect: None,
            upload_dir: None,
            autoindex: None,
            cgi: HashMap::new(),
        });
    }

    Ok(ServerBlock {
        listen_host,
        listen_port,
        names,
        root,
        index,
        client_max_body_size,
        error_pages,
        routes,
        upload_dir,
        autoindex,
    })
}

fn parse_route(lines: &[&str], i: &mut usize) -> Result<Route, String> {
    let mut path = "/".to_string();
    let mut methods = Vec::new();
    let mut root = None;
    let mut index = None;
    let mut redirect = None;
    let mut upload_dir = None;
    let mut autoindex = None;
    let mut cgi = HashMap::new();

    while *i < lines.len() {
        let line = lines[*i];
        if line == "}" {
            *i += 1;
            break;
        }
        if let Some(rest) = line.strip_prefix("path ") {
            path = rest.trim_end_matches(';').to_string();
        } else if let Some(rest) = line.strip_prefix("methods ") {
            methods = rest
                .trim_end_matches(';')
                .split_whitespace()
                .map(|s| s.to_uppercase())
                .collect();
        } else if let Some(rest) = line.strip_prefix("root ") {
            root = Some(PathBuf::from(rest.trim_end_matches(';')));
        } else if let Some(rest) = line.strip_prefix("index ") {
            index = Some(rest.trim_end_matches(';').to_string());
        } else if let Some(rest) = line.strip_prefix("redirect ") {
            redirect = Some(rest.trim_end_matches(';').to_string());
        } else if let Some(rest) = line.strip_prefix("upload_dir ") {
            upload_dir = Some(PathBuf::from(rest.trim_end_matches(';')));
        } else if line == "autoindex on;" || line == "autoindex on" {
            autoindex = Some(true);
        } else if line == "autoindex off;" || line == "autoindex off" {
            autoindex = Some(false);
        } else if let Some(rest) = line.strip_prefix("cgi ") {
            let parts: Vec<_> = rest.trim_end_matches(';').splitn(2, ' ').collect();
            if parts.len() == 2 {
                cgi.insert(parts[0].to_string(), PathBuf::from(parts[1]));
            }
        }
        *i += 1;
    }

    if methods.is_empty() {
        methods = vec!["GET".into()];
    }

    Ok(Route {
        path,
        methods,
        root,
        index,
        redirect,
        upload_dir,
        autoindex,
        cgi,
    })
}

fn parse_size(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('M').or_else(|| s.strip_suffix('m')) {
        return Ok(num.parse::<usize>().map_err(|_| s.to_string())? * 1_048_576);
    }
    if let Some(num) = s.strip_suffix('K').or_else(|| s.strip_suffix('k')) {
        return Ok(num.parse::<usize>().map_err(|_| s.to_string())? * 1024);
    }
    s.parse().map_err(|_| format!("bad size {s}"))
}
